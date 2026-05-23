/// Identifier for a synchronized entity inside repository helpers.
#[derive(Debug, Clone, Copy)]
pub(super) struct SyncEntityId<'a>(&'a str);

impl<'a> SyncEntityId<'a> {
    /// Creates a sync entity identifier wrapper.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw synchronized entity ID.
    ///
    /// # Returns
    ///
    /// Returns a wrapper for passing entity IDs across private helpers.
    pub(super) fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Returns the raw identifier string.
    ///
    /// # Returns
    ///
    /// Returns the wrapped sync entity ID.
    pub(super) fn as_str(self) -> &'a str {
        self.0
    }
}
