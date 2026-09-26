//! Strongly typed identifiers.
//!
//! Books are identified by the BLAKE3 hash of their content (see
//! `docs/adr/0004-content-addressed-books.md`), so a book keeps its identity
//! when it is moved, renamed or opened on another computer. Libraries and
//! profiles use random UUIDs.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Creates a new random identifier.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    };
}

uuid_id!(LibraryId, "Identifier of a library folder.");
uuid_id!(
    ProfileId,
    "Identifier of a reader profile (John Smith, Jane Smith, …)."
);

/// Identifier of a book: the lowercase hex BLAKE3 hash of the file content.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BookId(String);

/// Error returned when a string is not a valid [`BookId`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("a book id must be 64 lowercase hex characters")]
pub struct InvalidBookId;

impl BookId {
    /// Wraps an already computed hex hash, validating its shape.
    pub fn from_hex(hex: impl Into<String>) -> Result<Self, InvalidBookId> {
        let hex = hex.into();
        let ok = hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if ok {
            Ok(Self(hex))
        } else {
            Err(InvalidBookId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_id_accepts_blake3_hex() {
        let hex = "a".repeat(64);
        assert_eq!(BookId::from_hex(hex.clone()).unwrap().as_str(), hex);
    }

    #[test]
    fn book_id_rejects_bad_input() {
        assert_eq!(BookId::from_hex("abc"), Err(InvalidBookId));
        assert_eq!(BookId::from_hex("A".repeat(64)), Err(InvalidBookId));
        assert_eq!(BookId::from_hex("g".repeat(64)), Err(InvalidBookId));
    }

    #[test]
    fn uuid_ids_round_trip_through_strings() {
        let id = LibraryId::new();
        let parsed: LibraryId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }
}
