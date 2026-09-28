//! The person's own dictionary (Phase 7c): words spell check should
//! accept, one per line in `Notes/<profile>/Dictionary.txt`. It is an
//! ordinary file in their notes folder, so it can be edited by hand and
//! travels with their notes in backups and archives.

use crate::{Library, Result};
use std::fs;

pub const OWN_DICTIONARY: &str = "Dictionary.txt";

const HEADER: &str = "# Words Libreri's spell check accepts, one per line.\n";

/// Most note text read for learning words, in bytes.
const NOTES_LIMIT: u64 = 20 * 1024 * 1024;

fn clean(word: &str) -> Option<String> {
    let w = word.trim().replace('’', "'");
    (!w.is_empty()
        && !w.starts_with('#')
        && !w.contains(char::is_whitespace)
        && w.chars().count() <= 64)
        .then_some(w)
}

impl Library {
    /// The signed-in profile's own words (none for guests).
    pub fn own_words(&self) -> Result<Vec<String>> {
        let Ok(dir) = self.own_notes_dir() else {
            return Ok(Vec::new());
        };
        let text = fs::read_to_string(dir.join(OWN_DICTIONARY)).unwrap_or_default();
        let mut words: Vec<String> = text.lines().filter_map(clean).collect();
        words.sort_by_key(|w| w.to_lowercase());
        words.dedup();
        Ok(words)
    }

    fn write_own_words(&self, words: &[String]) -> Result<()> {
        let dir = self.notes_folder()?;
        let mut text = HEADER.to_owned();
        for w in words {
            text.push_str(w);
            text.push('\n');
        }
        crate::paths::write_atomic(&dir.join(OWN_DICTIONARY), text.as_bytes())?;
        Ok(())
    }

    /// Adds a word to the profile's own dictionary.
    pub fn add_own_word(&self, word: &str) -> Result<Vec<String>> {
        let w =
            clean(word).ok_or_else(|| crate::Error::InvalidInput("that is not a word".into()))?;
        let mut words = self.own_words()?;
        if !words.iter().any(|x| x == &w) {
            words.push(w);
            words.sort_by_key(|w| w.to_lowercase());
        }
        self.write_own_words(&words)?;
        Ok(words)
    }

    /// Removes a word from the profile's own dictionary.
    pub fn remove_own_word(&self, word: &str) -> Result<Vec<String>> {
        let w = word.trim().replace('’', "'");
        let mut words = self.own_words()?;
        words.retain(|x| x != &w);
        self.write_own_words(&words)?;
        Ok(words)
    }

    /// The text of the profile's Markdown notes, for learning the words
    /// they use (newest first, up to 20 MB).
    pub fn note_texts(&self) -> Result<Vec<String>> {
        let Ok(dir) = self.own_notes_dir() else {
            return Ok(Vec::new());
        };
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf, u64)> =
            walkdir::WalkDir::new(&dir)
                .into_iter()
                .flatten()
                .filter(|e| {
                    e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "md")
                })
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    Some((m.modified().ok()?, e.into_path(), m.len()))
                })
                .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let mut used = 0;
        let mut out = Vec::new();
        for (_, path, len) in files {
            if used + len > NOTES_LIMIT {
                break;
            }
            if let Ok(t) = fs::read_to_string(&path) {
                used += len;
                out.push(t);
            }
        }
        Ok(out)
    }

    /// The words of a book with text, as one text (for learning its names
    /// and terms).
    pub fn book_words(&self, id: &libreri_core::BookId) -> Result<String> {
        let words = self.sync_words(id)?;
        let mut out = String::with_capacity(words.len() * 7);
        for w in words {
            out.push_str(&w.word);
            out.push(' ');
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::library;

    #[test]
    fn keeps_own_words_in_the_notes_folder() {
        let (_d, lib) = library();
        assert!(lib.own_words().unwrap().is_empty());
        lib.add_own_word("Libreri").unwrap();
        lib.add_own_word(" Harte’s ").unwrap();
        let words = lib.add_own_word("Libreri").unwrap();
        assert_eq!(words, ["Harte's", "Libreri"]);
        assert!(lib.add_own_word("two words").is_err());
        let file = lib.notes_folder().unwrap().join(super::OWN_DICTIONARY);
        assert!(std::fs::read_to_string(&file)
            .unwrap()
            .contains("Libreri\n"));
        assert_eq!(lib.remove_own_word("Libreri").unwrap(), ["Harte's"]);
        std::fs::write(lib.notes_folder().unwrap().join("Trip.md"), "Kerrera ferry").unwrap();
        assert_eq!(lib.note_texts().unwrap(), ["Kerrera ferry"]);
    }
}
