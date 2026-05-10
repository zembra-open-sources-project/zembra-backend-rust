use utoipa::OpenApi;

/// Runtime OpenAPI document for the Zembra backend.
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::health::health,
        crate::handlers::notes::list_notes,
        crate::handlers::notes::recent_notes,
        crate::handlers::notes::create_note,
        crate::handlers::notes::create_notes_batch,
        crate::handlers::notes::get_note,
        crate::handlers::notes::update_note,
        crate::handlers::notes::archive_note,
        crate::handlers::notes::delete_note,
        crate::handlers::notes::list_note_revisions,
        crate::handlers::notes::list_note_tags,
        crate::handlers::notes::add_tag_to_note,
        crate::handlers::notes::remove_tag_from_note,
        crate::handlers::taxonomy::list_fields,
        crate::handlers::taxonomy::list_tags,
        crate::handlers::sync::status,
        crate::handlers::sync::config,
        crate::handlers::sync::update_config,
        crate::handlers::sync::test_config,
        crate::handlers::sync::run,
        crate::handlers::sync::push,
        crate::handlers::sync::pull
    ),
    components(
        schemas(
            crate::dto::error::ErrorBody,
            crate::dto::error::ErrorResponse,
            crate::dto::notes::BatchCreateNotesRequest,
            crate::dto::notes::BatchCreateNotesResponse,
            crate::dto::notes::CreateNoteRequest,
            crate::dto::notes::ListNoteRevisionsResponse,
            crate::dto::notes::ListNoteTagsResponse,
            crate::dto::notes::ListNotesResponse,
            crate::dto::notes::NoteMetadata,
            crate::dto::notes::NoteResponse,
            crate::dto::notes::RecentNotesRequest,
            crate::dto::notes::UpdateNoteRequest,
            crate::dto::sync::SyncDirectionResponse,
            crate::dto::sync::SyncConfigResponse,
            crate::dto::sync::SyncConfigTestResponse,
            crate::dto::sync::SyncRunResponse,
            crate::dto::sync::SyncStateResponse,
            crate::dto::sync::SyncStatusResponse,
            crate::dto::sync::TestSyncConfigRequest,
            crate::dto::sync::UpdateSyncConfigRequest,
            crate::dto::taxonomy::ListFieldsResponse,
            crate::dto::taxonomy::ListTagsResponse,
            crate::handlers::health::HealthResponse,
            crate::models::field::FieldRecord,
            crate::models::note::NoteRecord,
            crate::models::revision::NoteRevisionRecord,
            crate::models::tag::TagRecord
        )
    ),
    tags(
        (name = "health", description = "Service health and runtime readiness"),
        (name = "notes", description = "Note CRUD and note relations"),
        (name = "sync", description = "Supabase synchronization status and triggers"),
        (name = "taxonomy", description = "Fields and tags lookup")
    )
)]
pub struct ApiDoc;
