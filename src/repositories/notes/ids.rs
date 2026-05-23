/// Identifier for a note inside private repository helpers.
#[derive(Debug, Clone, Copy)]
pub(super) struct NoteId<'a>(&'a str);

impl<'a> NoteId<'a> {
    /// Creates a note ID wrapper.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw note ID.
    ///
    /// # Returns
    ///
    /// Returns a note ID wrapper for private helper calls.
    pub(super) fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Returns the raw note ID.
    ///
    /// # Returns
    ///
    /// Returns the wrapped note ID string.
    pub(super) fn as_str(self) -> &'a str {
        self.0
    }
}

/// Identifier for a note revision inside private repository helpers.
#[derive(Debug, Clone, Copy)]
pub(super) struct RevisionId<'a>(&'a str);

impl<'a> RevisionId<'a> {
    /// Creates a revision ID wrapper.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw revision ID.
    ///
    /// # Returns
    ///
    /// Returns a revision ID wrapper for private helper calls.
    pub(super) fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Returns the raw revision ID.
    ///
    /// # Returns
    ///
    /// Returns the wrapped revision ID string.
    pub(super) fn as_str(self) -> &'a str {
        self.0
    }
}
