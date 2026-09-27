//! Reader profiles: who is using the library right now.
//!
//! Several people can share one library. Each profile keeps its own reading
//! status, positions, highlights and notebooks. A profile can be protected by
//! a 6-digit PIN. PINs keep people who share a computer out of each other's
//! notes; they are not encryption (see docs/adr/0010-profiles-and-pins.md).
//! PIN rules and hashing live in `libreri-profiles`.

use crate::ProfileId;
use serde::{Deserialize, Serialize};

/// What a profile may do.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    /// Created the library. Manages profiles and everything else.
    Owner,
    /// A grown-up: reads, notes, and organises the shared library.
    #[default]
    Standard,
    /// Reads and makes notes, only in the folders the owner allows.
    /// Cannot import, edit details, move or delete books.
    Kids,
    /// Reads without an account. Nothing is kept after the guest leaves.
    Guest,
}

impl ProfileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Standard => "standard",
            Self::Kids => "kids",
            Self::Guest => "guest",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "owner" => Self::Owner,
            "standard" => Self::Standard,
            "kids" => Self::Kids,
            "guest" => Self::Guest,
            _ => return None,
        })
    }

    /// May import, edit details, organise, move and delete books.
    pub fn can_edit_library(self) -> bool {
        matches!(self, Self::Owner | Self::Standard)
    }

    /// May add, change and remove other profiles.
    pub fn can_manage_profiles(self) -> bool {
        self == Self::Owner
    }

    /// Personal data (positions, highlights, notebooks) is kept.
    pub fn keeps_data(self) -> bool {
        self != Self::Guest
    }
}

/// Colours offered for profile avatars.
pub const PROFILE_COLOURS: &[&str] = &[
    "graphite", "red", "orange", "amber", "green", "teal", "blue", "indigo", "violet", "pink",
];

/// A profile as stored in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub colour: String,
    pub kind: ProfileKind,
    /// Argon2 hash (PHC string) of the PIN; `None` means no PIN.
    pub pin_hash: Option<String>,
    /// Argon2 hash of the owner's recovery code, shown once when the PIN is set.
    pub recovery_hash: Option<String>,
    /// For Kids profiles: folders (relative to `Books/`) they may open.
    /// Empty means none, not all.
    pub allowed_folders: Vec<String>,
    /// Interface preferences (shortcuts, reader defaults, auto-lock…), JSON
    /// owned by the interface.
    pub prefs: String,
    pub created_at: String,
    pub last_used: Option<String>,
}

impl Profile {
    pub fn new(
        name: impl Into<String>,
        colour: impl Into<String>,
        kind: ProfileKind,
        now: &str,
    ) -> Self {
        Self {
            id: ProfileId::new(),
            name: name.into(),
            colour: colour.into(),
            kind,
            pin_hash: None,
            recovery_hash: None,
            allowed_folders: Vec::new(),
            prefs: "{}".into(),
            created_at: now.into(),
            last_used: None,
        }
    }

    pub fn has_pin(&self) -> bool {
        self.pin_hash.is_some()
    }
}

/// Checks a profile name: 1–40 characters, not only spaces.
pub fn validate_profile_name(name: &str) -> Result<String, &'static str> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("a profile needs a name");
    }
    if name.chars().count() > 40 {
        return Err("a profile name can be at most 40 characters");
    }
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err("a profile name cannot contain / \\ : * ? \" < > |");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_rules() {
        for k in [
            ProfileKind::Owner,
            ProfileKind::Standard,
            ProfileKind::Kids,
            ProfileKind::Guest,
        ] {
            assert_eq!(ProfileKind::parse(k.as_str()), Some(k));
        }
        assert!(ProfileKind::Owner.can_manage_profiles());
        assert!(!ProfileKind::Standard.can_manage_profiles());
        assert!(ProfileKind::Standard.can_edit_library());
        assert!(!ProfileKind::Kids.can_edit_library());
        assert!(!ProfileKind::Guest.keeps_data());
    }

    #[test]
    fn names_and_pins() {
        assert_eq!(
            validate_profile_name("  Jane   Smith ").unwrap(),
            "Jane Smith"
        );
        assert!(validate_profile_name("   ").is_err());
        assert!(validate_profile_name("a/b").is_err());
        assert!(validate_profile_name(&"x".repeat(41)).is_err());
    }
}
