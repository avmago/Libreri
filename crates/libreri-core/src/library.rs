//! Information stored in `.library-data/library.json`.

use crate::ids::LibraryId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Version of the on-disk library format. Bump when the layout changes in a
/// way older versions of Libreri cannot read.
pub const LIBRARY_FORMAT_VERSION: u32 = 1;

/// Identity and descriptive details of one library folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub id: LibraryId,
    pub name: String,
    pub format_version: u32,
    pub created_at: DateTime<Utc>,
    /// Version of Libreri that created the library, for diagnostics.
    pub created_by: String,
}

impl LibraryInfo {
    pub fn new(name: impl Into<String>, app_version: impl Into<String>) -> Self {
        Self {
            id: LibraryId::new(),
            name: name.into(),
            format_version: LIBRARY_FORMAT_VERSION,
            created_at: Utc::now(),
            created_by: app_version.into(),
        }
    }
}
