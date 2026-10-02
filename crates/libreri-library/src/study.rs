//! Study (the reading calendar and the timer): each profile's reading
//! sessions, goals and timer settings, kept as one JSON document in
//! `.library-data/profiles/<profile>.study.json` (so it is in backups and
//! survives a rebuild). Its shape belongs to the app; the library only
//! checks it is a JSON object of a sensible size.

use crate::paths::write_atomic;
use crate::{Error, Library, Result};
use std::fs;
use std::path::PathBuf;

/// Larger than years of sessions; a guard against a runaway write.
const MAX_BYTES: usize = 8 * 1024 * 1024;

pub(crate) const SUFFIX: &str = ".study.json";
/// Daily review (spaced repetition): the cards' settings and schedule.
pub(crate) const REVIEW_SUFFIX: &str = ".review.json";

impl Library {
    fn study_path(&self) -> Result<PathBuf> {
        self.profile_doc_path(SUFFIX)
    }

    fn profile_doc_path(&self, suffix: &str) -> Result<PathBuf> {
        let session = self.session_info().ok_or(Error::SignedOut)?;
        if !session.kind.keeps_data() {
            return Err(Error::NotAllowed(
                "guests keep no reading calendar or review cards; sign in to your own profile"
                    .into(),
            ));
        }
        Ok(self.profiles_dir().join(format!("{}{suffix}", session.id)))
    }

    /// The signed-in profile's study document, or None before the first.
    pub fn read_study(&self) -> Result<Option<String>> {
        self.read_doc(self.study_path()?)
    }

    /// The signed-in profile's review cards and their schedule.
    pub fn read_review(&self) -> Result<Option<String>> {
        self.read_doc(self.profile_doc_path(REVIEW_SUFFIX)?)
    }

    pub fn write_review(&self, json: &str) -> Result<()> {
        self.write_doc(self.profile_doc_path(REVIEW_SUFFIX)?, json)
    }

    fn read_doc(&self, path: PathBuf) -> Result<Option<String>> {
        match fs::read_to_string(path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Saves the study document (written aside first).
    pub fn write_study(&self, json: &str) -> Result<()> {
        self.write_doc(self.study_path()?, json)
    }

    fn write_doc(&self, path: PathBuf, json: &str) -> Result<()> {
        if json.len() > MAX_BYTES {
            return Err(Error::InvalidInput(
                "the reading calendar is too large".into(),
            ));
        }
        let v: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| Error::InvalidInput(format!("not a reading calendar: {e}")))?;
        if !v.is_object() {
            return Err(Error::InvalidInput("not a reading calendar".into()));
        }
        fs::create_dir_all(self.profiles_dir())?;
        write_atomic(&path, json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;

    #[test]
    fn keeps_the_calendar_per_profile_and_survives_a_rebuild() {
        let (_d, lib) = library();
        assert_eq!(lib.read_study().unwrap(), None);
        lib.write_study(r#"{"version":1,"sessions":[]}"#).unwrap();
        assert_eq!(
            lib.read_study().unwrap().as_deref(),
            Some(r#"{"version":1,"sessions":[]}"#)
        );
        assert!(lib.write_study("[1,2]").is_err());
        assert!(lib.write_study("not json").is_err());
        lib.rebuild_index(&crate::NoProgress).unwrap();
        assert!(lib.read_study().unwrap().is_some());
        assert_eq!(lib.read_review().unwrap(), None);
        lib.write_review(r#"{"version":1}"#).unwrap();
        assert_eq!(
            lib.read_review().unwrap().as_deref(),
            Some(r#"{"version":1}"#)
        );
        lib.rebuild_index(&crate::NoProgress).unwrap();
        assert!(lib.read_review().unwrap().is_some());
    }
}
