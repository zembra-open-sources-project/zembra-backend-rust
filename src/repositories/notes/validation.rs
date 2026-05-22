use crate::error::ApiError;

/// Validates a note reference before SQL lookup.
///
/// # Arguments
///
/// * `note_ref` - Full note ID or prefix.
///
/// # Returns
///
/// Returns `Ok(())` when the reference can be queried safely.
pub(super) fn validate_note_ref(note_ref: &str) -> Result<(), ApiError> {
    if note_ref.len() < 4 {
        return Err(ApiError::NoteReferenceTooShort);
    }

    if !note_ref
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ApiError::InvalidNoteReference);
    }

    Ok(())
}

/// Validates a full note UUID before cursor lookup.
///
/// # Arguments
///
/// * `note_uuid` - Full note ID.
///
/// # Returns
///
/// Returns `Ok(())` when the UUID is a 32-character hexadecimal string.
pub(super) fn validate_full_note_uuid(note_uuid: &str) -> Result<(), ApiError> {
    if note_uuid.len() != 32 {
        return Err(ApiError::Validation);
    }

    if !note_uuid
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ApiError::Validation);
    }

    Ok(())
}
