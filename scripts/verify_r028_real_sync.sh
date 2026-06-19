#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TABLES=(workspaces devices fields tags notes note_revisions note_tags note_links sync_changes)

read_config() {
  python3 - "$ROOT_DIR" <<'PY'
import os
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
config = {}
for path in [root / "config/default.toml", Path.home() / ".zembra.env"]:
    if path.exists():
        with path.open("rb") as handle:
            data = tomllib.load(handle)
        for section, values in data.items():
            if isinstance(values, dict):
                config.setdefault(section, {}).update(values)

database_path = os.environ.get("ZEMBRA_DATABASE_PATH") or str(config.get("database", {}).get("path", "data/zembra.db"))
supabase_url = os.environ.get("ZEMBRA_SUPABASE_URL") or str(config.get("sync", {}).get("supabase_url", ""))
secret_key = os.environ.get("ZEMBRA_SUPABASE_SECRET_KEY") or str(config.get("sync", {}).get("secret_key", ""))
print(database_path)
print(supabase_url.rstrip("/"))
print(secret_key)
PY
}

CONFIG_OUTPUT="$(read_config)"
DATABASE_PATH="$(printf "%s\n" "$CONFIG_OUTPUT" | sed -n '1p')"
SUPABASE_URL="$(printf "%s\n" "$CONFIG_OUTPUT" | sed -n '2p')"
SUPABASE_SECRET_KEY="$(printf "%s\n" "$CONFIG_OUTPUT" | sed -n '3p')"

if [[ "$DATABASE_PATH" != /* ]]; then
  DATABASE_PATH="$ROOT_DIR/$DATABASE_PATH"
fi

if [[ ! -f "$DATABASE_PATH" ]]; then
  echo "local database not found: $DATABASE_PATH" >&2
  exit 1
fi

if [[ -z "$SUPABASE_URL" || -z "$SUPABASE_SECRET_KEY" ]]; then
  echo "Supabase config missing. Set ~/.zembra.env or ZEMBRA_SUPABASE_URL and ZEMBRA_SUPABASE_SECRET_KEY." >&2
  exit 1
fi

local_count() {
  local table="$1"
  sqlite3 "$DATABASE_PATH" "SELECT COUNT(*) FROM $table;"
}

remote_count() {
  local table="$1"
  curl -sS \
    -H "apikey: $SUPABASE_SECRET_KEY" \
    -H "Authorization: Bearer $SUPABASE_SECRET_KEY" \
    -H "Prefer: count=exact" \
    "$SUPABASE_URL/rest/v1/$table?select=*" \
    -o /tmp/zembra-r028-count.json \
    -D /tmp/zembra-r028-count.headers
  awk 'BEGIN{IGNORECASE=1} /^content-range:/ {print $2}' /tmp/zembra-r028-count.headers | awk -F/ '{print $2}' | tr -d '\r'
}

echo "r028 real sync verification"
echo "database=$DATABASE_PATH"
echo "supabase=$SUPABASE_URL"
echo
echo "before sync"
for table in "${TABLES[@]}"; do
  printf "%-16s local=%s remote=%s\n" "$table" "$(local_count "$table")" "$(remote_count "$table")"
done

echo
echo "running real sync through backend CLI path"
cargo run --quiet --bin zembra-backend-rust >/tmp/zembra-r028-server.log 2>&1 &
SERVER_PID=$!
trap 'kill "$SERVER_PID" >/dev/null 2>&1 || true' EXIT

sleep 2
curl -sS -X POST "http://127.0.0.1:3000/sync/run" -H "content-type: application/json" -o /tmp/zembra-r028-sync-run.json
cat /tmp/zembra-r028-sync-run.json
echo

echo "after sync"
FAILED=0
for table in "${TABLES[@]}"; do
  local_value="$(local_count "$table")"
  remote_value="$(remote_count "$table")"
  printf "%-16s local=%s remote=%s\n" "$table" "$local_value" "$remote_value"
  if [[ "$local_value" != "$remote_value" ]]; then
    FAILED=1
  fi
done

if [[ "$FAILED" -ne 0 ]]; then
  echo "r028 existing-data sync verification failed: counts differ" >&2
  exit 1
fi

echo "r028 existing-data sync verification passed by table counts"
