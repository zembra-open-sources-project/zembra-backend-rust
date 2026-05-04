create table if not exists public.workspaces (
    id uuid primary key,
    workspace_name text check (workspace_name is null or length(btrim(workspace_name)) > 0),
    created_at bigint not null check (created_at >= 0),
    updated_at bigint not null check (updated_at >= created_at),
    archived_at bigint check (archived_at is null or archived_at >= created_at),
    deleted_at bigint check (deleted_at is null or deleted_at >= created_at)
);

insert into public.workspaces (id, workspace_name, created_at, updated_at)
values ('00000000-0000-4000-8000-000000000300', null, extract(epoch from now())::bigint, extract(epoch from now())::bigint)
on conflict (id) do nothing;

create table if not exists public.devices (
    id text primary key,
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    name text not null check (length(btrim(name)) > 0),
    platform text not null check (length(btrim(platform)) > 0),
    created_at bigint not null check (created_at >= 0),
    last_seen_at bigint check (last_seen_at is null or last_seen_at >= 0),
    sync_enabled boolean not null default true,
    last_synced_at bigint check (last_synced_at is null or last_synced_at >= 0),
    unique (workspace_id, id)
);

create table if not exists public.fields (
    id text primary key,
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    name text not null check (length(btrim(name)) > 0),
    created_at bigint not null check (created_at >= 0),
    unique (workspace_id, id),
    unique (workspace_id, name)
);

create table if not exists public.tags (
    id text primary key,
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    name text not null check (length(btrim(name)) > 0),
    created_at bigint not null check (created_at >= 0),
    unique (workspace_id, id),
    unique (workspace_id, name)
);

create table if not exists public.notes (
    id text primary key,
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    content text not null,
    role text not null default 'Human' check (role in ('Human', 'Agent')),
    field_id text,
    created_at bigint not null check (created_at >= 0),
    updated_at bigint not null check (updated_at >= created_at),
    archived_at bigint check (archived_at is null or archived_at >= created_at),
    deleted_at bigint check (deleted_at is null or deleted_at >= created_at),
    current_revision_id text,
    last_change_id text,
    conflict_status text not null default 'none' check (conflict_status in ('none', 'auto_resolved', 'needs_review')),
    unique (workspace_id, id),
    foreign key (workspace_id, field_id) references public.fields(workspace_id, id) on update cascade on delete set null
);

create table if not exists public.note_revisions (
    id text primary key,
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    note_id text not null,
    content text not null,
    title text,
    device_id text,
    created_at bigint not null check (created_at >= 0),
    base_revision_id text,
    change_id text,
    foreign key (workspace_id, note_id) references public.notes(workspace_id, id) on update cascade on delete cascade,
    foreign key (workspace_id, device_id) references public.devices(workspace_id, id) on update cascade on delete set null
);

create table if not exists public.note_tags (
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    note_id text not null,
    tag_id text not null,
    created_at bigint not null check (created_at >= 0),
    primary key (workspace_id, note_id, tag_id),
    foreign key (workspace_id, note_id) references public.notes(workspace_id, id) on update cascade on delete cascade,
    foreign key (workspace_id, tag_id) references public.tags(workspace_id, id) on update cascade on delete cascade
);

create table if not exists public.sync_changes (
    id text primary key,
    workspace_id uuid not null references public.workspaces(id) on update cascade on delete restrict,
    device_id text not null,
    entity_type text not null check (entity_type in ('workspace', 'device', 'field', 'tag', 'note', 'note_revision', 'note_tag', 'note_link', 'attachment')),
    entity_id text not null check (length(btrim(entity_id)) > 0),
    operation text not null check (operation in ('insert', 'update', 'delete', 'restore', 'attach', 'detach')),
    base_revision_id text,
    new_revision_id text,
    payload jsonb not null,
    created_at bigint not null check (created_at >= 0),
    applied_at bigint check (applied_at is null or applied_at >= 0),
    supabase_committed_at bigint check (supabase_committed_at is null or supabase_committed_at >= 0),
    unique (workspace_id, device_id, id),
    foreign key (workspace_id, device_id) references public.devices(workspace_id, id) on update cascade on delete restrict
);

create index if not exists idx_workspaces_updated_at on public.workspaces(updated_at);
create index if not exists idx_workspaces_deleted_at on public.workspaces(deleted_at);
create index if not exists idx_fields_workspace_name on public.fields(workspace_id, name);
create index if not exists idx_tags_workspace_name on public.tags(workspace_id, name);
create index if not exists idx_notes_workspace_field_id on public.notes(workspace_id, field_id);
create index if not exists idx_notes_workspace_created_at on public.notes(workspace_id, created_at);
create index if not exists idx_notes_workspace_updated_at on public.notes(workspace_id, updated_at);
create index if not exists idx_notes_workspace_archived_at on public.notes(workspace_id, archived_at);
create index if not exists idx_notes_workspace_deleted_at on public.notes(workspace_id, deleted_at);
create index if not exists idx_note_revisions_workspace_note_id_created_at on public.note_revisions(workspace_id, note_id, created_at);
create index if not exists idx_note_revisions_workspace_device_id on public.note_revisions(workspace_id, device_id);
create index if not exists idx_note_tags_workspace_tag_id on public.note_tags(workspace_id, tag_id);
create index if not exists idx_sync_changes_workspace_created on public.sync_changes(workspace_id, created_at, id);
create index if not exists idx_sync_changes_workspace_device_created on public.sync_changes(workspace_id, device_id, created_at, id);
create index if not exists idx_sync_changes_entity on public.sync_changes(workspace_id, entity_type, entity_id);
create index if not exists idx_sync_changes_revision on public.sync_changes(workspace_id, entity_type, new_revision_id);
