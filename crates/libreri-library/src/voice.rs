//! Voice notes (Phase 7b): recordings kept in the profile's notes folder,
//! `Notes/<profile>/Voice notes/`, so they are ordinary files the person
//! owns, travel with the notes and are included in archives. A voice note
//! in a book is an annotation (kind "voice") whose locator names its
//! recording; a notebook links to its recording like any other file.

use crate::{Error, Library, Result};
use std::fs;
use std::path::PathBuf;

/// The folder inside the profile's notes folder.
pub const VOICE_DIR: &str = "Voice notes";

/// File types a recording may be.
const AUDIO: &[&str] = &["flac", "wav", "ogg", "opus", "m4a", "mp3", "webm"];

impl Library {
    /// Saves a recording (already encoded, e.g. FLAC) as a new file in the
    /// profile's voice notes. Returns its path in the library, for example
    /// `Notes/Me/Voice notes/2026-09-28 14-03-11.flac`.
    pub fn save_voice(&self, bytes: &[u8], ext: &str) -> Result<String> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        if !AUDIO.contains(&ext.as_str()) {
            return Err(Error::InvalidInput("recordings must be audio".into()));
        }
        if bytes.is_empty() {
            return Err(Error::InvalidInput("the recording is empty".into()));
        }
        let dir = self.notes_folder()?.join(VOICE_DIR);
        fs::create_dir_all(&dir)?;
        let name = chrono::Local::now().format("%Y-%m-%d %H-%M-%S").to_string();
        let file = crate::paths::unique_path(&dir, &format!("{name}.{ext}"));
        crate::paths::write_atomic(&file, bytes)?;
        crate::paths::rel_of(self.layout(), &file).ok_or(Error::BookNotFound)
    }

    /// The file of one of the signed-in profile's recordings; anything
    /// else is refused.
    pub fn voice_file(&self, rel: &str) -> Result<PathBuf> {
        let dir = self.own_notes_dir()?;
        self.layout()
            .resolve_relative(rel)
            .filter(|p| {
                p.starts_with(&dir)
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| AUDIO.contains(&e.to_ascii_lowercase().as_str()))
            })
            .ok_or_else(|| Error::NotAllowed("that recording belongs to someone else".into()))
    }

    /// Moves a recording to the system trash (it can be taken back from
    /// there). A recording already gone is fine.
    pub fn delete_voice(&self, rel: &str) -> Result<()> {
        let path = self.voice_file(rel)?;
        if !path.exists() {
            return Ok(());
        }
        trash::delete(&path)
            .or_else(|_| fs::remove_file(&path))
            .map_err(|e| Error::Trash(e.to_string()))
    }

    /// Whether `rel` is in the signed-in profile's own notes folder (for
    /// the file protocol: recordings play from there).
    pub(crate) fn is_own_note(&self, rel: &str) -> bool {
        let Ok(dir) = self.own_notes_dir() else {
            return false;
        };
        if rel.split('/').any(|p| p == "..") {
            return false;
        }
        self.layout()
            .resolve_relative(rel)
            .is_some_and(|p| p.starts_with(&dir))
    }
}

/// The recording named in a voice annotation's locator.
pub fn voice_of(locator: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(locator).ok()?;
    v.get("audio")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::library;

    #[test]
    fn keeps_recordings_in_the_notes_folder() {
        let (_dir, lib) = library();
        let rel = lib.save_voice(b"fLaC....", "flac").unwrap();
        assert!(
            rel.starts_with("Notes/") && rel.contains("/Voice notes/") && rel.ends_with(".flac")
        );
        let path = lib.voice_file(&rel).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"fLaC....");
        // A second one the same second gets its own name.
        let rel2 = lib.save_voice(b"fLaC2", "flac").unwrap();
        assert_ne!(rel, rel2);
        assert!(lib.may_open(&rel));
        assert!(!lib.may_open("Notes/Someone else/Voice notes/x.flac"));
        assert!(lib.voice_file("Books/a.flac").is_err());
        assert!(lib.save_voice(b"x", "exe").is_err());
        assert_eq!(
            voice_of(r#"{"type":"pdf","page":1,"audio":"Notes/Me/Voice notes/a.flac"}"#).as_deref(),
            Some("Notes/Me/Voice notes/a.flac")
        );
    }
}
