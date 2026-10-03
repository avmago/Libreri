//! Spell checking and word suggestions.
//!
//! Hunspell dictionaries are read with spellbook (pure Rust), so checking
//! is the same on every system. A word is fine when a chosen dictionary
//! knows it, when it is in the person's own dictionary, or when the open
//! book uses it (names and terms).

pub mod catalog;
pub mod text;
pub mod vocab;

pub use catalog::{dictionaries, DictionaryInfo};
pub use vocab::{complete, Sources, Vocab};

use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

/// A dictionary ready to use.
pub struct Loaded {
    pub code: String,
    pub dict: spellbook::Dictionary,
    /// Its words, for completing.
    pub stems: Vocab,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loaded").field("code", &self.code).finish()
    }
}

/// Reads a dictionary (built in, or downloaded into `dir`).
pub fn load(dir: &Path, code: &str) -> Result<Loaded, String> {
    let (aff, dic) = catalog::read(dir, code)?;
    let dict = spellbook::Dictionary::new(&aff, &dic)
        .map_err(|e| format!("the {code} dictionary could not be read: {e}"))?;
    Ok(Loaded {
        code: code.to_owned(),
        dict,
        stems: Vocab::from_list(catalog::stems(&dic)),
    })
}

/// A word that looks misspelt (UTF-16 offsets, end exclusive).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Miss {
    pub start: u32,
    pub end: u32,
    pub word: String,
}

/// How often the open book must use a word for it to count as a term.
const BOOK_TERM: u32 = 2;

/// Checks words against the chosen dictionaries, the person's own words
/// and the open book.
pub struct Checker<'a> {
    pub dicts: &'a [Arc<Loaded>],
    /// The person's own words, lower case.
    pub own: &'a HashSet<String>,
    pub book: Option<&'a Vocab>,
}

impl Checker<'_> {
    pub fn is_known(&self, word: &str) -> bool {
        if self.dicts.is_empty() {
            return true;
        }
        let w = word.replace('’', "'");
        // "Harte's" is fine when "Harte" is.
        let base = w.strip_suffix("'s").unwrap_or(&w);
        for form in [w.as_str(), base] {
            if self.own.contains(&form.to_lowercase())
                || self.book.is_some_and(|b| b.count(form) >= BOOK_TERM)
            {
                return true;
            }
        }
        self.dicts.iter().any(|d| d.dict.check(&w))
    }

    /// The words of `text` that look misspelt.
    pub fn misses(&self, text: &str) -> Vec<Miss> {
        text::words(text)
            .into_iter()
            .filter(|w| !self.is_known(w.text))
            .map(|w| Miss {
                start: w.start as u32,
                end: w.end as u32,
                word: w.text.to_owned(),
            })
            .collect()
    }

    /// Up to `limit` corrections: the book's own words first (names and
    /// terms), then the dictionaries'.
    pub fn suggest(&self, word: &str, limit: usize) -> Vec<String> {
        let w = word.replace('’', "'");
        let mut out: Vec<String> = Vec::new();
        let push = |s: String, out: &mut Vec<String>| {
            if !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) && s != w {
                out.push(s);
            }
        };
        if let Some(book) = self.book {
            let first: String = w.chars().take(1).flat_map(char::to_lowercase).collect();
            let mut near: Vec<(usize, u32, &str)> = book
                .starting(&first)
                .filter(|(c, n)| {
                    *n >= BOOK_TERM && c.chars().count().abs_diff(w.chars().count()) <= 2
                })
                .filter_map(|(c, n)| {
                    let d = distance(&c.to_lowercase(), &w.to_lowercase());
                    (d <= 2).then_some((d, n, c))
                })
                .collect();
            near.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
            for (_, _, c) in near.into_iter().take(3) {
                push(c.to_owned(), &mut out);
            }
        }
        for d in self.dicts {
            let mut found = Vec::new();
            d.dict.suggest(&w, &mut found);
            for s in found {
                push(s, &mut out);
            }
        }
        out.truncate(limit);
        out
    }
}

/// Edit distance (insertions, deletions, changes, swaps of neighbours).
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english() -> Vec<Arc<Loaded>> {
        let dir = tempfile::tempdir().unwrap();
        vec![Arc::new(load(dir.path(), "en-US").unwrap())]
    }

    #[test]
    fn marks_misspelt_words() {
        let dicts = english();
        let own: HashSet<String> = ["libreri".to_owned()].into();
        let book = Vocab::learn(["Samuel Harte kept the light. Harte wrote in Kerrera."]);
        let c = Checker {
            dicts: &dicts,
            own: &own,
            book: Some(&book),
        };
        let text =
            "The keeper wrote teh log at Kerrera; Harte used Libreri’s lamp, didn’t he? Recieve.";
        let words: Vec<String> = c.misses(text).into_iter().map(|m| m.word).collect();
        // "Kerrera" is in the book only once, so it still looks wrong.
        assert_eq!(words, ["teh", "Kerrera", "Recieve"]);
        let m = &c.misses("a teh")[0];
        assert_eq!((m.start, m.end), (2, 5));
        assert!(c.suggest("teh", 5).contains(&"the".to_owned()));
        assert_eq!(c.suggest("Hartte", 3)[0], "Harte");
        let none = Checker {
            dicts: &[],
            own: &own,
            book: None,
        };
        assert!(none.misses("teh").is_empty());
    }

    #[test]
    fn measures_distance() {
        assert_eq!(distance("teh", "the"), 1);
        assert_eq!(distance("kitten", "sitting"), 3);
    }
}
