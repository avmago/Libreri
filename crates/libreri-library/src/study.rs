//! Study (the reading calendar and the timer): each profile's reading
//! sessions, goals and timer settings, kept as one JSON document in
//! `.library-data/profiles/<profile>.study.json`, and the daily review in
//! `<profile>.review.json`. Both survive a rebuild and travel in backups and
//! whole-library exports. Their shape belongs to the app; the library only
//! checks they are JSON objects of a sensible size, and merges them field by
//! field when an archive brings one in.

use crate::paths::write_atomic;
use crate::{Error, Library, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Larger than years of sessions; a guard against a runaway write.
const MAX_BYTES: usize = 8 * 1024 * 1024;

pub(crate) const SUFFIX: &str = ".study.json";
/// Daily review (spaced repetition): the cards' settings and schedule.
pub(crate) const REVIEW_SUFFIX: &str = ".review.json";
/// The per-profile documents archives carry.
pub(crate) const DOC_SUFFIXES: [&str; 2] = [SUFFIX, REVIEW_SUFFIX];

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

    /// Brings in a profile's document from an archive: kept as it is when
    /// the profile has none; otherwise merged (see [`merge_docs`]). Book ids
    /// are moved to where the archive's books landed.
    pub(crate) fn merge_profile_doc(
        &self,
        target: &libreri_core::ProfileId,
        suffix: &str,
        bytes: &[u8],
        books: &HashMap<String, String>,
    ) -> Result<()> {
        let Ok(mut incoming) = serde_json::from_slice::<Value>(bytes) else {
            return Ok(());
        };
        if !incoming.is_object() {
            return Ok(());
        }
        remap_books(&mut incoming, books);
        let path = self.profiles_dir().join(format!("{target}{suffix}"));
        let merged = match self.read_doc(path.clone())? {
            Some(s) => match serde_json::from_str::<Value>(&s) {
                Ok(mut current) if current.is_object() => {
                    merge_docs(&mut current, incoming);
                    current
                }
                _ => incoming,
            },
            None => incoming,
        };
        let json = serde_json::to_string(&merged).map_err(std::io::Error::other)?;
        self.write_doc(path, &json)
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

/// Merges an archive's document into this one without losing anything here:
/// - lists are joined (items with an `id` once, other items once each);
/// - maps (review cards and schedules) gain the entries they lack;
/// - single values (settings, version) stay as they are here.
pub(crate) fn merge_docs(current: &mut Value, incoming: Value) {
    let (Value::Object(cur), Value::Object(inc)) = (current, incoming) else {
        return;
    };
    for (k, v) in inc {
        match (cur.get_mut(&k), v) {
            (None, v) => {
                cur.insert(k, v);
            }
            (Some(Value::Array(a)), Value::Array(b)) => {
                for item in b {
                    let id = item.get("id").filter(|i| !i.is_null());
                    let seen = match id {
                        Some(id) => a.iter().any(|x| x.get("id") == Some(id)),
                        None => a.contains(&item),
                    };
                    if !seen {
                        a.push(item);
                    }
                }
            }
            (Some(Value::Object(a)), Value::Object(b)) if k != "settings" => {
                for (bk, bv) in b {
                    a.entry(bk).or_insert(bv);
                }
            }
            _ => {}
        }
    }
}

/// Every `bookId` that names an archive book now known by another id.
fn remap_books(v: &mut Value, books: &HashMap<String, String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "bookId" {
                    if let Some(to) = x.as_str().and_then(|s| books.get(s)) {
                        *x = Value::String(to.clone());
                    }
                } else {
                    remap_books(x, books);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| remap_books(x, books)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use serde_json::json;

    #[test]
    fn merges_documents_without_losing_anything() {
        let mut here = json!({
            "version": 1,
            "settings": {"newPerDay": 5},
            "sessions": [{"id": "a", "minutes": 10}],
            "booksOff": ["x"],
            "sched": {"h1:passage": {"due": "here"}},
            "log": [{"t": "1", "key": "k"}]
        });
        let mut there = json!({
            "version": 1,
            "settings": {"newPerDay": 50, "maxReviews": 9},
            "sessions": [{"id": "a", "minutes": 99}, {"id": "b", "minutes": 20, "bookId": "old"}],
            "booksOff": ["x", "y"],
            "sched": {"h1:passage": {"due": "there"}, "h2:qa": {"due": "t"}},
            "log": [{"t": "1", "key": "k"}, {"t": "2", "key": "k"}],
            "goals": [{"id": "g"}]
        });
        let books = [("old".to_owned(), "new".to_owned())].into_iter().collect();
        super::remap_books(&mut there, &books);
        super::merge_docs(&mut here, there);
        assert_eq!(
            here,
            json!({
                "version": 1,
                "settings": {"newPerDay": 5},
                "sessions": [{"id": "a", "minutes": 10}, {"id": "b", "minutes": 20, "bookId": "new"}],
                "booksOff": ["x", "y"],
                "sched": {"h1:passage": {"due": "here"}, "h2:qa": {"due": "t"}},
                "log": [{"t": "1", "key": "k"}, {"t": "2", "key": "k"}],
                "goals": [{"id": "g"}]
            })
        );
    }

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
