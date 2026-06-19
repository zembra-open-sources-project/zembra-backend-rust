#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT_DIR" <<'PY'
import json
import os
import sqlite3
import subprocess
import sys
import time
import tempfile
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT_DIR = Path(sys.argv[1])
TABLES = [
    "workspaces",
    "devices",
    "fields",
    "tags",
    "notes",
    "note_revisions",
    "note_tags",
    "note_links",
    "sync_changes",
]
TABLE_KEYS = {
    "workspaces": ["id"],
    "devices": ["workspace_id", "id"],
    "fields": ["workspace_id", "id"],
    "tags": ["workspace_id", "id"],
    "notes": ["workspace_id", "id"],
    "note_revisions": ["workspace_id", "id"],
    "note_tags": ["workspace_id", "note_id", "tag_id"],
    "note_links": ["workspace_id", "id"],
    "sync_changes": ["workspace_id", "device_id", "id"],
}
BOOLEAN_COLUMNS = {
    "devices": {"sync_enabled"},
}
JSON_COLUMNS = {
    "sync_changes": {"payload"},
}


def load_config():
    """Load local database and Supabase settings from repo defaults, user config, and environment."""
    config = {}
    for path in [ROOT_DIR / "config/default.toml", Path.home() / ".zembra.env"]:
        if path.exists():
            with path.open("rb") as handle:
                data = tomllib.load(handle)
            for section, values in data.items():
                if isinstance(values, dict):
                    config.setdefault(section, {}).update(values)

    database_path = os.environ.get("ZEMBRA_DATABASE_PATH") or str(
        config.get("database", {}).get("path", "data/zembra.db")
    )
    supabase_url = os.environ.get("ZEMBRA_SUPABASE_URL") or str(
        config.get("sync", {}).get("supabase_url", "")
    )
    secret_key = os.environ.get("ZEMBRA_SUPABASE_SECRET_KEY") or str(
        config.get("sync", {}).get("secret_key", "")
    )
    sync_enabled = os.environ.get("ZEMBRA_SYNC_ENABLED")
    if sync_enabled is None:
        sync_enabled = bool(config.get("sync", {}).get("enabled", False))
    else:
        sync_enabled = sync_enabled.strip().lower() in {"1", "true", "yes", "on"}
    migrate_remote_schema = os.environ.get("ZEMBRA_SYNC_MIGRATE_REMOTE_SCHEMA")
    if migrate_remote_schema is None:
        migrate_remote_schema = bool(config.get("sync", {}).get("migrate_remote_schema", False))
    else:
        migrate_remote_schema = migrate_remote_schema.strip().lower() in {"1", "true", "yes", "on"}
    remote_database_url = os.environ.get("ZEMBRA_SYNC_REMOTE_DATABASE_URL") or str(
        config.get("sync", {}).get("remote_database_url", "")
    )

    if not database_path.startswith("/"):
        database_path = str(ROOT_DIR / database_path)
    return (
        Path(database_path),
        supabase_url.rstrip("/"),
        secret_key,
        sync_enabled,
        migrate_remote_schema,
        remote_database_url,
    )


def toml_string(value):
    """Serialize a string as a TOML-compatible quoted value."""
    return json.dumps(str(value))


def write_runtime_config(home, database_path, supabase_url, secret_key, sync_enabled, migrate_remote_schema, remote_database_url):
    """Write a temporary backend config without modifying the user's real config file."""
    config_path = Path(home) / ".zembra.env"
    config_path.write_text(
        "\n".join(
            [
                "[server]",
                'host = "127.0.0.1"',
                "port = 3000",
                "cors_allowed_origins = []",
                "",
                "[database]",
                f"path = {toml_string(database_path)}",
                "",
                "[logging]",
                'level = "INFO"',
                'path = "logs"',
                "",
                "[sync]",
                f"enabled = {str(sync_enabled).lower()}",
                "interval_seconds = 60",
                f"supabase_url = {toml_string(supabase_url)}",
                f"secret_key = {toml_string(secret_key)}",
                f"migrate_remote_schema = {str(migrate_remote_schema).lower()}",
                f"remote_database_url = {toml_string(remote_database_url)}",
                "",
            ]
        ),
        encoding="utf-8",
    )


def request_json(url, secret_key, method="GET"):
    """Call Supabase REST or backend HTTP and return parsed JSON."""
    command = [
        "curl",
        "-sS",
        "-X",
        method,
        "-H",
        f"apikey: {secret_key}",
        "-H",
        f"Authorization: Bearer {secret_key}",
        "-H",
        "content-type: application/json",
        "-w",
        "\n%{http_code}",
        url,
    ]
    result = subprocess.run(command, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"{url} curl failed: {result.stderr.strip()}")
    body, status = result.stdout.rsplit("\n", 1)
    if not status.startswith("2"):
        raise RuntimeError(f"{url} returned {status}: {body}")
    return json.loads(body) if body else None


def backend_post_json(url):
    """Call a local backend route and return parsed JSON."""
    request = urllib.request.Request(url, headers={"content-type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            body = response.read().decode("utf-8")
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8")
        raise RuntimeError(f"{url} returned {error.code}: {body}") from error
    return json.loads(body) if body else None


def local_columns(connection, table):
    """Read local table columns from SQLite metadata."""
    rows = connection.execute(f"PRAGMA table_info({table})").fetchall()
    return [row[1] for row in rows]


def normalize_row(table, row):
    """Normalize SQLite and Supabase row values before equality comparison."""
    normalized = {}
    for column, value in row.items():
        if column in BOOLEAN_COLUMNS.get(table, set()) and value is not None:
            normalized[column] = bool(value)
        elif column in JSON_COLUMNS.get(table, set()) and isinstance(value, str):
            normalized[column] = json.loads(value)
        else:
            normalized[column] = value
    return normalized


def local_rows(connection, table):
    """Read all local rows for one synchronized table in stable key order."""
    columns = local_columns(connection, table)
    select_columns = ", ".join(columns)
    order_columns = ", ".join(TABLE_KEYS[table])
    cursor = connection.execute(f"SELECT {select_columns} FROM {table} ORDER BY {order_columns}")
    rows = [dict(zip(columns, row)) for row in cursor.fetchall()]
    return [normalize_row(table, row) for row in rows]


def remote_rows(supabase_url, secret_key, table):
    """Read all remote rows for one synchronized table in stable key order."""
    order = ",".join(f"{column}.asc" for column in TABLE_KEYS[table])
    query = urllib.parse.urlencode({"select": "*", "order": order})
    rows = request_json(f"{supabase_url}/rest/v1/{table}?{query}", secret_key)
    if not isinstance(rows, list):
        raise RuntimeError(f"{table} returned non-list Supabase response: {rows}")
    normalized = [normalize_row(table, row) for row in rows]
    return sorted(normalized, key=lambda row: tuple(str(row.get(column)) for column in TABLE_KEYS[table]))


def schema_versions(connection, supabase_url, secret_key):
    """Read local and remote schema contract versions."""
    local = connection.execute(
        "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1"
    ).fetchone()
    try:
        remote = request_json(
            f"{supabase_url}/rest/v1/schema_migrations?select=version&order=version.desc&limit=1",
            secret_key,
        )
    except RuntimeError as error:
        if "PGRST205" in str(error) and "schema_migrations" in str(error):
            remote = []
        else:
            raise
    local_version = local[0] if local else "missing"
    remote_version = remote[0]["version"] if isinstance(remote, list) and remote else "missing"
    return local_version, remote_version


def first_difference(local, remote):
    """Return a compact description of the first row difference."""
    local_keys = [json.dumps(row, sort_keys=True, separators=(",", ":")) for row in local]
    remote_keys = [json.dumps(row, sort_keys=True, separators=(",", ":")) for row in remote]
    for index, (local_row, remote_row) in enumerate(zip(local_keys, remote_keys)):
        if local_row != remote_row:
            return f"row {index} differs"
    if len(local_keys) != len(remote_keys):
        return f"row count differs local={len(local_keys)} remote={len(remote_keys)}"
    return "unknown difference"


def compare_all(connection, supabase_url, secret_key, label):
    """Compare all synchronized tables and return True when every table matches."""
    print(label)
    failed = False
    for table in TABLES:
        local = local_rows(connection, table)
        remote = remote_rows(supabase_url, secret_key, table)
        status = "ok" if local == remote else "diff"
        print(f"{table:<16} local={len(local)} remote={len(remote)} {status}")
        if local != remote:
            print(f"{table:<16} {first_difference(local, remote)}")
            failed = True
    return not failed


def wait_for_backend():
    """Wait until the local backend is ready to accept sync requests."""
    deadline = time.time() + 90
    while time.time() < deadline:
        try:
            with urllib.request.urlopen("http://127.0.0.1:3000/health", timeout=2):
                return
        except Exception:
            time.sleep(0.5)
    raise RuntimeError("backend did not become ready on http://127.0.0.1:3000")


def main():
    """Run the r028 existing-data real synchronization verification."""
    (
        database_path,
        supabase_url,
        secret_key,
        sync_enabled,
        migrate_remote_schema,
        remote_database_url,
    ) = load_config()
    if not database_path.exists():
        raise RuntimeError(f"local database not found: {database_path}")
    if not supabase_url or not secret_key:
        raise RuntimeError(
            "Supabase config missing. Set ~/.zembra.env or ZEMBRA_SUPABASE_URL and ZEMBRA_SUPABASE_SECRET_KEY."
        )

    connection = sqlite3.connect(database_path)
    connection.row_factory = sqlite3.Row
    local_version, remote_version = schema_versions(connection, supabase_url, secret_key)
    print("r028 real sync verification")
    print(f"database={database_path}")
    print(f"supabase={supabase_url}")
    print(f"local_schema_contract={local_version}")
    print(f"remote_schema_contract={remote_version}")
    print(f"migration_enabled={str(migrate_remote_schema).lower()}")
    print()

    if local_version == remote_version:
        compare_all(connection, supabase_url, secret_key, "before sync")
    else:
        print("before sync")
        print("table comparison skipped until schema contracts match")
    print()
    print("running real sync through backend /sync/run")
    runtime_home = tempfile.TemporaryDirectory(prefix="zembra-r028-home-")
    write_runtime_config(
        runtime_home.name,
        database_path,
        supabase_url,
        secret_key,
        sync_enabled,
        migrate_remote_schema,
        remote_database_url,
    )
    server_env = {**os.environ, "HOME": runtime_home.name}
    server_log_path = Path(runtime_home.name) / "zembra-r028-server.log"
    server_log = server_log_path.open("w", encoding="utf-8")
    server = subprocess.Popen(
        ["cargo", "run", "--quiet", "--bin", "zembra-backend-rust"],
        cwd=ROOT_DIR,
        stdout=server_log,
        stderr=server_log,
        env=server_env,
    )
    try:
        try:
            wait_for_backend()
        except Exception as error:
            server_log.flush()
            log_tail = server_log_path.read_text(encoding="utf-8", errors="replace").splitlines()[-40:]
            if log_tail:
                print("backend log tail")
                print("\n".join(log_tail))
            raise error
        result = backend_post_json("http://127.0.0.1:3000/sync/run")
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    finally:
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=10)
        server_log.close()
        runtime_home.cleanup()

    print()
    local_version, remote_version = schema_versions(connection, supabase_url, secret_key)
    print(f"after_local_schema_contract={local_version}")
    print(f"after_remote_schema_contract={remote_version}")
    if local_version != remote_version:
        raise RuntimeError("r028 existing-data sync verification failed: schema contracts differ")

    if not compare_all(connection, supabase_url, secret_key, "after sync"):
        raise RuntimeError("r028 existing-data sync verification failed: table rows differ")
    print("r028 existing-data sync verification passed by full table row comparison")


if __name__ == "__main__":
    main()
PY
