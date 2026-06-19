use serde::Serialize;

use super::{SupabaseClient, SupabaseError};
use crate::repositories::sync::SyncChangeRecord;
use crate::repositories::sync::payload::{
    FieldPayload, NoteLinkAttachPayload, NoteLinkDetachPayload, NotePayload, NoteRevisionPayload,
    NoteTagPayload, TagPayload,
};

impl SupabaseClient {
    /// Builds Supabase business table projection requests for sync changes.
    ///
    /// # Arguments
    ///
    /// * `changes` - Local sync changes to project.
    ///
    /// # Returns
    ///
    /// Returns ordered requests that can be executed before pushing `sync_changes`.
    pub fn build_business_projection_requests(
        &self,
        changes: &[SyncChangeRecord],
    ) -> Result<Vec<reqwest::Request>, SupabaseError> {
        changes
            .iter()
            .map(|change| self.build_business_projection_request(change))
            .collect()
    }

    /// Builds a Supabase business table projection request for one sync change.
    ///
    /// # Arguments
    ///
    /// * `change` - Local sync change to project.
    ///
    /// # Returns
    ///
    /// Returns one request for a supported business projection operation.
    pub fn build_business_projection_request(
        &self,
        change: &SyncChangeRecord,
    ) -> Result<reqwest::Request, SupabaseError> {
        let payload = parse_payload(change)?;
        match (change.entity_type.as_str(), change.operation.as_str()) {
            ("field", "insert") => self.build_upsert_field_request(change, &payload),
            ("tag", "insert") => self.build_upsert_tag_request(change, &payload),
            ("note", "insert" | "update" | "delete" | "restore" | "archive") => {
                self.build_upsert_note_request(change, &payload)
            }
            ("note_revision", "insert") => {
                self.build_upsert_note_revision_request(change, &payload)
            }
            ("note_tag", "attach") => self.build_upsert_note_tag_request(change, &payload),
            ("note_tag", "detach") => self.build_delete_note_tag_request(change, &payload),
            ("note_link", "attach") => self.build_upsert_note_link_request(change, &payload),
            ("note_link", "detach") => self.build_delete_note_link_request(change, &payload),
            _ => Err(SupabaseError::Payload(format!(
                "unsupported projection {} {} for change {}",
                change.entity_type, change.operation, change.id
            ))),
        }
    }

    /// Builds a Supabase field upsert request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a request for the `fields` table.
    fn build_upsert_field_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let field = FieldPayload::try_from(payload).map_err(projection_payload_error)?;
        self.post_projection(
            "fields",
            &[SupabaseFieldRecord {
                id: field.id,
                workspace_id: change.workspace_id.as_str(),
                name: field.name,
                created_at: field.created_at,
            }],
        )
    }

    /// Builds a Supabase tag upsert request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a request for the `tags` table.
    fn build_upsert_tag_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let tag = TagPayload::try_from(payload).map_err(projection_payload_error)?;
        self.post_projection(
            "tags",
            &[SupabaseTagRecord {
                id: tag.id,
                workspace_id: change.workspace_id.as_str(),
                name: tag.name,
                parent_tag_id: tag.parent_tag_id,
                path: tag.path,
                depth: tag.depth,
                created_at: tag.created_at,
            }],
        )
    }

    /// Builds a Supabase note upsert request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a request for the `notes` table.
    fn build_upsert_note_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let note = NotePayload::try_from(payload).map_err(projection_payload_error)?;
        self.post_projection(
            "notes",
            &[SupabaseNoteRecord {
                id: note.id,
                workspace_id: change.workspace_id.as_str(),
                content: note.content,
                role: note.role,
                field_id: note.field_id,
                created_at: note.created_at,
                updated_at: note.updated_at,
                archived_at: note.archived_at,
                deleted_at: note.deleted_at,
                current_revision_id: note.current_revision_id,
                last_change_id: Some(change.id.as_str()),
            }],
        )
    }

    /// Builds a Supabase note revision upsert request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a request for the `note_revisions` table.
    fn build_upsert_note_revision_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let revision = NoteRevisionPayload::try_from(payload).map_err(projection_payload_error)?;
        self.post_projection(
            "note_revisions",
            &[SupabaseNoteRevisionRecord {
                id: revision.id,
                workspace_id: change.workspace_id.as_str(),
                note_id: revision.note_id,
                content: revision.content,
                title: revision.title,
                device_id: revision.device_id,
                created_at: revision.created_at,
                base_revision_id: revision.base_revision_id,
                change_id: Some(change.id.as_str()),
            }],
        )
    }

    /// Builds a Supabase note tag upsert request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a request for the `note_tags` table.
    fn build_upsert_note_tag_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let relation = NoteTagPayload::try_from(payload).map_err(projection_payload_error)?;
        self.post_projection(
            "note_tags",
            &[SupabaseNoteTagRecord {
                workspace_id: change.workspace_id.as_str(),
                note_id: relation.note_id,
                tag_id: relation.tag_id,
                created_at: relation.created_at.unwrap_or(change.created_at),
            }],
        )
    }

    /// Builds a Supabase note tag delete request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a delete request for the `note_tags` table.
    fn build_delete_note_tag_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let relation = NoteTagPayload::try_from(payload).map_err(projection_payload_error)?;
        self.client
            .delete(format!("{}/rest/v1/note_tags", self.base_url))
            .headers(self.headers()?)
            .query(&[
                ("workspace_id", format!("eq.{}", change.workspace_id)),
                ("note_id", format!("eq.{}", relation.note_id)),
                ("tag_id", format!("eq.{}", relation.tag_id)),
            ])
            .build()
            .map_err(Into::into)
    }

    /// Builds a Supabase note link upsert request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a request for the `note_links` table.
    fn build_upsert_note_link_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let link = NoteLinkAttachPayload::try_from(payload).map_err(projection_payload_error)?;
        self.post_projection(
            "note_links",
            &[SupabaseNoteLinkRecord {
                id: link.id,
                workspace_id: change.workspace_id.as_str(),
                source_note_id: link.source_note_id,
                target_note_id: link.target_note_id,
                anchor_text: link.anchor_text,
                position: link.position,
                created_at: link.created_at.unwrap_or(change.created_at),
            }],
        )
    }

    /// Builds a Supabase note link delete request.
    ///
    /// # Arguments
    ///
    /// * `change` - Sync change that owns the workspace.
    /// * `payload` - Parsed JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a delete request for the `note_links` table.
    fn build_delete_note_link_request(
        &self,
        change: &SyncChangeRecord,
        payload: &serde_json::Value,
    ) -> Result<reqwest::Request, SupabaseError> {
        let link = NoteLinkDetachPayload::try_from(payload).map_err(projection_payload_error)?;
        self.client
            .delete(format!("{}/rest/v1/note_links", self.base_url))
            .headers(self.headers()?)
            .query(&[
                ("workspace_id", format!("eq.{}", change.workspace_id)),
                ("id", format!("eq.{}", link.id)),
            ])
            .build()
            .map_err(Into::into)
    }

    /// Builds a Supabase upsert request for a business projection table.
    ///
    /// # Arguments
    ///
    /// * `table` - Supabase REST table name.
    /// * `records` - Records to serialize as the request body.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    fn post_projection<T: Serialize>(
        &self,
        table: &str,
        records: &[T],
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .post(format!("{}/rest/v1/{table}", self.base_url))
            .headers(self.headers()?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(records)
            .build()
            .map_err(Into::into)
    }
}

/// Parses a sync change payload into JSON.
///
/// # Arguments
///
/// * `change` - Sync change whose payload should be parsed.
///
/// # Returns
///
/// Returns the parsed JSON value.
fn parse_payload(change: &SyncChangeRecord) -> Result<serde_json::Value, SupabaseError> {
    serde_json::from_str(&change.payload).map_err(|error| {
        SupabaseError::Payload(format!("change {} has invalid JSON: {error}", change.id))
    })
}

/// Converts a parser error into a Supabase projection error.
///
/// # Arguments
///
/// * `message` - Payload parser error message.
///
/// # Returns
///
/// Returns a Supabase projection error.
fn projection_payload_error(message: String) -> SupabaseError {
    SupabaseError::Payload(message)
}

/// Field row projected to Supabase.
#[derive(Debug, Serialize)]
struct SupabaseFieldRecord<'a> {
    /// Field identifier.
    id: &'a str,
    /// Workspace identifier.
    workspace_id: &'a str,
    /// Field display name.
    name: &'a str,
    /// Unix timestamp for creation.
    created_at: i64,
}

/// Tag row projected to Supabase.
#[derive(Debug, Serialize)]
struct SupabaseTagRecord<'a> {
    /// Tag identifier.
    id: &'a str,
    /// Workspace identifier.
    workspace_id: &'a str,
    /// Tag display name.
    name: &'a str,
    /// Optional parent tag identifier.
    parent_tag_id: Option<&'a str>,
    /// Full hierarchical tag path.
    path: &'a str,
    /// Depth from the tag root.
    depth: i64,
    /// Unix timestamp for creation.
    created_at: i64,
}

/// Note row projected to Supabase.
#[derive(Debug, Serialize)]
struct SupabaseNoteRecord<'a> {
    /// Note identifier.
    id: &'a str,
    /// Workspace identifier.
    workspace_id: &'a str,
    /// Note content.
    content: &'a str,
    /// Note role.
    role: &'a str,
    /// Optional field identifier.
    field_id: Option<&'a str>,
    /// Unix timestamp for creation.
    created_at: i64,
    /// Unix timestamp for last update.
    updated_at: i64,
    /// Optional archive timestamp.
    archived_at: Option<i64>,
    /// Optional deletion timestamp.
    deleted_at: Option<i64>,
    /// Optional current revision identifier.
    current_revision_id: Option<&'a str>,
    /// Last sync change that projected this row.
    last_change_id: Option<&'a str>,
}

/// Note revision row projected to Supabase.
#[derive(Debug, Serialize)]
struct SupabaseNoteRevisionRecord<'a> {
    /// Revision identifier.
    id: &'a str,
    /// Workspace identifier.
    workspace_id: &'a str,
    /// Parent note identifier.
    note_id: &'a str,
    /// Revision content.
    content: &'a str,
    /// Optional title.
    title: Option<&'a str>,
    /// Optional device identifier.
    device_id: Option<&'a str>,
    /// Unix timestamp for creation.
    created_at: i64,
    /// Optional base revision identifier.
    base_revision_id: Option<&'a str>,
    /// Sync change that projected this revision.
    change_id: Option<&'a str>,
}

/// Note tag relation row projected to Supabase.
#[derive(Debug, Serialize)]
struct SupabaseNoteTagRecord<'a> {
    /// Workspace identifier.
    workspace_id: &'a str,
    /// Note identifier.
    note_id: &'a str,
    /// Tag identifier.
    tag_id: &'a str,
    /// Unix timestamp for creation.
    created_at: i64,
}

/// Note link relation row projected to Supabase.
#[derive(Debug, Serialize)]
struct SupabaseNoteLinkRecord<'a> {
    /// Link identifier.
    id: &'a str,
    /// Workspace identifier.
    workspace_id: &'a str,
    /// Source note identifier.
    source_note_id: &'a str,
    /// Target note identifier.
    target_note_id: &'a str,
    /// Optional anchor text.
    anchor_text: Option<&'a str>,
    /// Optional position in note content.
    position: Option<i64>,
    /// Unix timestamp for creation.
    created_at: i64,
}

#[cfg(test)]
mod tests;
