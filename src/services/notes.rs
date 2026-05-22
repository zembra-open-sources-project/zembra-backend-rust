use std::collections::HashMap;

use chrono::{Duration, Local, NaiveDate, TimeZone};
use sqlx::SqlitePool;

use crate::dto::notes::{
    BatchCreateNotesResponse, CreateNoteRequest, DailyNoteCount, DailyNoteCountsResponse,
    FieldNotesGroup, FieldNotesResponse, NoteMetadata, NoteResponse, NotesByDateQuery,
    NotesByDateResponse, RandomFieldsQuery, RandomNotesQuery, RandomTagsQuery, RecentNotesRequest,
    TaggedNotesGroup, TaggedNotesResponse, UpdateNoteRequest,
};
use crate::error::ApiError;
use crate::models::note::NoteRecord;
use crate::models::revision::NoteRevisionRecord;
use crate::models::tag::TagRecord;
use crate::repositories::notes::{
    CreateNoteInput, CreatedNote, NoteLinkInput, NotesRepository, UpdateNoteInput,
};

const DEFAULT_UPDATE_FIELD: &str = "inbox";
const DAILY_NOTE_COUNT_DAYS: i64 = 30;

/// Service for note business workflows.
#[derive(Debug, Clone)]
pub struct NotesService {
    /// Repository used by note workflows.
    repository: NotesRepository,
}

impl NotesService {
    /// Creates a notes service backed by a SQLite pool.
    ///
    /// # Arguments
    ///
    /// * `pool` - Shared SQLite pool.
    ///
    /// # Returns
    ///
    /// Returns a notes service.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: NotesRepository::new(pool),
        }
    }

    /// Creates a single note.
    ///
    /// # Arguments
    ///
    /// * `request` - Validated create-note request.
    ///
    /// # Returns
    ///
    /// Returns the created note response.
    pub async fn create_note(&self, request: CreateNoteRequest) -> Result<NoteResponse, ApiError> {
        let input = normalize_create_request(request)?;
        let created = self.repository.create_note(input).await?;

        self.created_note_to_response(created).await
    }

    /// Creates notes in a single transaction.
    ///
    /// # Arguments
    ///
    /// * `items` - Validated create-note requests.
    ///
    /// # Returns
    ///
    /// Returns all created notes.
    pub async fn create_notes_batch(
        &self,
        items: Vec<CreateNoteRequest>,
    ) -> Result<BatchCreateNotesResponse, ApiError> {
        let inputs = items
            .into_iter()
            .map(normalize_create_request)
            .collect::<Result<Vec<_>, _>>()?;
        let notes = self
            .repository
            .create_notes_batch(inputs)
            .await?
            .into_iter();
        let mut responses = Vec::new();
        for created in notes {
            responses.push(self.created_note_to_response(created).await?);
        }

        Ok(BatchCreateNotesResponse { notes: responses })
    }

    /// Lists active notes.
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional maximum record count.
    ///
    /// # Returns
    ///
    /// Returns active note records.
    pub async fn list_notes(&self, limit: Option<i64>) -> Result<Vec<NoteRecord>, ApiError> {
        self.repository.list_notes(limit).await
    }

    /// Lists recent notes for Web presentation.
    ///
    /// # Arguments
    ///
    /// * `request` - Validated recent-notes request.
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records ordered by update time.
    pub async fn recent_notes(
        &self,
        request: RecentNotesRequest,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        let limit = request.limit.unwrap_or(50);
        self.repository
            .list_recent_notes(limit, request.note_uuid.as_deref())
            .await
    }

    /// Lists random visible notes.
    ///
    /// # Arguments
    ///
    /// * `query` - Validated random notes query.
    ///
    /// # Returns
    ///
    /// Returns random non-deleted and non-archived note records.
    pub async fn random_notes(&self, query: RandomNotesQuery) -> Result<Vec<NoteRecord>, ApiError> {
        self.repository.list_random_notes(query.n).await
    }

    /// Counts visible notes created per server-local day for the past 30 days.
    ///
    /// # Returns
    ///
    /// Returns a fixed 30-day series ordered by date ascending.
    pub async fn daily_note_counts(&self) -> Result<DailyNoteCountsResponse, ApiError> {
        self.daily_note_counts_for_today(Local::now().date_naive())
            .await
    }

    /// Lists visible notes created on one server-local date.
    ///
    /// # Arguments
    ///
    /// * `query` - Validated notes-by-date query.
    ///
    /// # Returns
    ///
    /// Returns visible notes created on the requested local date.
    pub async fn notes_by_date(
        &self,
        query: NotesByDateQuery,
    ) -> Result<NotesByDateResponse, ApiError> {
        let date = query.date.ok_or(ApiError::Validation)?;
        let parsed_date =
            NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| ApiError::Validation)?;
        let start_timestamp = local_start_of_day_timestamp(parsed_date);
        let end_timestamp = local_start_of_day_timestamp(parsed_date + Duration::days(1));
        let notes = self
            .repository
            .list_visible_notes_created_between(start_timestamp, end_timestamp)
            .await?;

        Ok(NotesByDateResponse { date, notes })
    }

    /// Counts visible notes created per local day ending at a supplied date.
    ///
    /// # Arguments
    ///
    /// * `today` - Server-local date to use as the final bucket.
    ///
    /// # Returns
    ///
    /// Returns a fixed 30-day series ordered by date ascending.
    async fn daily_note_counts_for_today(
        &self,
        today: NaiveDate,
    ) -> Result<DailyNoteCountsResponse, ApiError> {
        let start_date = today - Duration::days(DAILY_NOTE_COUNT_DAYS - 1);
        let start_timestamp = local_start_of_day_timestamp(start_date);
        let counts_by_date: HashMap<_, _> = self
            .repository
            .daily_note_counts_since(start_timestamp)
            .await?
            .into_iter()
            .map(|row| (row.date, row.count))
            .collect();
        let days = (0..DAILY_NOTE_COUNT_DAYS)
            .map(|offset| {
                let date = start_date + Duration::days(offset);
                let date = date.format("%Y-%m-%d").to_string();
                let count = counts_by_date.get(&date).copied().unwrap_or(0);

                DailyNoteCount { date, count }
            })
            .collect();

        Ok(DailyNoteCountsResponse { days })
    }

    /// Lists notes grouped by randomly selected tags.
    ///
    /// # Arguments
    ///
    /// * `query` - Validated random tags query.
    ///
    /// # Returns
    ///
    /// Returns tagged note groups under the `tagged_notes` response field.
    pub async fn random_tagged_notes(
        &self,
        query: RandomTagsQuery,
    ) -> Result<TaggedNotesResponse, ApiError> {
        let tag_limit = query.n.unwrap_or(3);
        let mut remaining_notes = query.count.unwrap_or(20);
        let tags = self.repository.random_tags(tag_limit).await?;
        let mut tagged_notes = Vec::with_capacity(tags.len());

        for tag in tags {
            let notes = if remaining_notes > 0 {
                let notes = self
                    .repository
                    .list_visible_notes_by_tag(&tag.id, remaining_notes)
                    .await?;
                remaining_notes -= notes.len() as i64;
                notes
            } else {
                Vec::new()
            };
            tagged_notes.push(TaggedNotesGroup { tag, notes });
        }

        Ok(TaggedNotesResponse { tagged_notes })
    }

    /// Lists notes grouped by randomly selected fields.
    ///
    /// # Arguments
    ///
    /// * `query` - Validated random fields query.
    ///
    /// # Returns
    ///
    /// Returns field note groups under the `field_notes` response field.
    pub async fn random_field_notes(
        &self,
        query: RandomFieldsQuery,
    ) -> Result<FieldNotesResponse, ApiError> {
        let field_limit = query.n.unwrap_or(3);
        let mut remaining_notes = query.count.unwrap_or(20);
        let fields = self.repository.random_fields(field_limit).await?;
        let mut field_notes = Vec::with_capacity(fields.len());

        for field in fields {
            let notes = if remaining_notes > 0 {
                let notes = self
                    .repository
                    .list_visible_notes_by_field(&field.id, remaining_notes)
                    .await?;
                remaining_notes -= notes.len() as i64;
                notes
            } else {
                Vec::new()
            };
            field_notes.push(FieldNotesGroup { field, notes });
        }

        Ok(FieldNotesResponse { field_notes })
    }

    /// Reads a note by reference.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns the matching note.
    pub async fn get_note(&self, note_ref: &str) -> Result<NoteResponse, ApiError> {
        let note = self.repository.get_note_by_ref(note_ref).await?;
        self.note_to_response(note).await
    }

    /// Updates note content.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `request` - Update-note request.
    ///
    /// # Returns
    ///
    /// Returns the updated note.
    pub async fn update_note(
        &self,
        note_ref: &str,
        request: UpdateNoteRequest,
    ) -> Result<NoteResponse, ApiError> {
        let input = normalize_update_request(request)?;
        let note = self.repository.update_note(note_ref, input).await?;
        self.note_to_response(note).await
    }

    /// Archives a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns the archived note.
    pub async fn archive_note(&self, note_ref: &str) -> Result<NoteRecord, ApiError> {
        self.repository.archive_note(note_ref).await
    }

    /// Soft deletes a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after deletion.
    pub async fn delete_note(&self, note_ref: &str) -> Result<(), ApiError> {
        self.repository.delete_note(note_ref).await
    }

    /// Lists note revisions.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns note revisions.
    pub async fn list_note_revisions(
        &self,
        note_ref: &str,
    ) -> Result<Vec<NoteRevisionRecord>, ApiError> {
        self.repository.list_note_revisions(note_ref).await
    }

    /// Lists note tags.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns note tags.
    pub async fn list_note_tags(&self, note_ref: &str) -> Result<Vec<TagRecord>, ApiError> {
        self.repository.list_note_tags(note_ref).await
    }

    /// Adds a tag to a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Tag name to add.
    ///
    /// # Returns
    ///
    /// Returns the associated tag.
    pub async fn add_tag_to_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<TagRecord, ApiError> {
        let tag_name = normalize_required_text(tag_name)?;
        self.repository.add_tag_to_note(note_ref, &tag_name).await
    }

    /// Removes a tag from a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Tag name to remove.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the association is removed.
    pub async fn remove_tag_from_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<(), ApiError> {
        let tag_name = normalize_required_text(tag_name)?;
        self.repository
            .remove_tag_from_note(note_ref, &tag_name)
            .await
    }

    /// Converts a repository creation result into an API response.
    ///
    /// # Arguments
    ///
    /// * `created` - Repository creation result.
    ///
    /// # Returns
    ///
    /// Returns API note response with link metadata.
    async fn created_note_to_response(
        &self,
        created: CreatedNote,
    ) -> Result<NoteResponse, ApiError> {
        let backlinks = self
            .repository
            .list_visible_backlinks(&created.note.id)
            .await?;

        Ok(NoteResponse {
            metadata: NoteMetadata {
                field: created.field,
                tags: created.tags,
                role: created.note.role.clone(),
                outgoing_links: created.links,
                backlinks,
            },
            note: created.note,
        })
    }

    /// Converts a note record into an API response.
    ///
    /// # Arguments
    ///
    /// * `note` - Persisted note record.
    ///
    /// # Returns
    ///
    /// Returns API note response with resolved metadata.
    async fn note_to_response(&self, note: NoteRecord) -> Result<NoteResponse, ApiError> {
        let field = self
            .repository
            .field_name_by_id(note.field_id.as_deref())
            .await?;
        let tags = self
            .repository
            .list_note_tags_by_id(&note.id)
            .await?
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        let outgoing_links = self
            .repository
            .list_visible_outgoing_links(&note.id)
            .await?;
        let backlinks = self.repository.list_visible_backlinks(&note.id).await?;

        Ok(NoteResponse {
            metadata: NoteMetadata {
                field,
                tags,
                role: note.role.clone(),
                outgoing_links,
                backlinks,
            },
            note,
        })
    }
}

/// Normalizes a create-note request into repository input.
///
/// # Arguments
///
/// * `request` - Create-note request.
///
/// # Returns
///
/// Returns normalized repository input.
fn normalize_create_request(request: CreateNoteRequest) -> Result<CreateNoteInput, ApiError> {
    let content = normalize_required_text(&request.content)?;
    let role = normalize_role(&request.role)?;
    let field = request
        .field
        .as_deref()
        .map(normalize_required_text)
        .transpose()?;
    let tags = normalize_tags(request.tags);
    let links = normalize_links(request.links)?;

    Ok(CreateNoteInput {
        content,
        field,
        tags,
        role,
        device_id: request.device_id,
        links,
    })
}

/// Normalizes an update-note request into repository input.
///
/// # Arguments
///
/// * `request` - Update-note request.
///
/// # Returns
///
/// Returns normalized repository input.
fn normalize_update_request(request: UpdateNoteRequest) -> Result<UpdateNoteInput, ApiError> {
    let content = normalize_required_text(&request.content)?;
    let field = request
        .field
        .map(|field| match field {
            Some(field) => normalize_required_text(&field),
            None => Ok(DEFAULT_UPDATE_FIELD.to_string()),
        })
        .transpose()?;
    let tags = request.tags.map(normalize_tags);
    let links = request.links.map(normalize_links).transpose()?;

    Ok(UpdateNoteInput {
        content,
        device_id: request.device_id,
        field,
        tags,
        links,
    })
}

/// Normalizes a required text field.
///
/// # Arguments
///
/// * `value` - Raw text value.
///
/// # Returns
///
/// Returns trimmed text or a validation error.
fn normalize_required_text(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation);
    }

    Ok(trimmed.to_string())
}

/// Normalizes a note role.
///
/// # Arguments
///
/// * `role` - Raw role value.
///
/// # Returns
///
/// Returns a valid role string.
fn normalize_role(role: &str) -> Result<String, ApiError> {
    match role {
        "Human" | "Agent" => Ok(role.to_string()),
        _ => Err(ApiError::Validation),
    }
}

/// Normalizes tag names by trimming, removing blanks, and deduplicating.
///
/// # Arguments
///
/// * `tags` - Raw tag names.
///
/// # Returns
///
/// Returns normalized tag names preserving first-seen order.
fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if !trimmed.is_empty() && !normalized.iter().any(|item: &String| item == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }

    normalized
}

/// Normalizes client-parsed note links.
///
/// # Arguments
///
/// * `links` - Raw link request values.
///
/// # Returns
///
/// Returns normalized link inputs.
fn normalize_links(
    links: Vec<crate::dto::notes::NoteLinkRequest>,
) -> Result<Vec<NoteLinkInput>, ApiError> {
    links
        .into_iter()
        .map(|link| {
            let target_note_ref = normalize_required_text(&link.target_note_ref)?;
            let anchor_text = link
                .anchor_text
                .map(|anchor_text| anchor_text.trim().to_string())
                .filter(|anchor_text| !anchor_text.is_empty());

            Ok(NoteLinkInput {
                target_note_ref,
                anchor_text,
                position: link.position,
            })
        })
        .collect()
}

/// Converts a local date into a Unix timestamp at local start of day.
///
/// # Arguments
///
/// * `date` - Server-local calendar date.
///
/// # Returns
///
/// Returns the Unix timestamp for local midnight.
fn local_start_of_day_timestamp(date: NaiveDate) -> i64 {
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .expect("valid local midnight components");

    Local
        .from_local_datetime(&midnight)
        .single()
        .or_else(|| Local.from_local_datetime(&midnight).earliest())
        .or_else(|| Local.from_local_datetime(&midnight).latest())
        .expect("local date has a representable midnight")
        .timestamp()
}
