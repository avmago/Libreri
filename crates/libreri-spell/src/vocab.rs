//! Words someone is likely to type: the words of the open book and of
//! their notes (by how often they appear), their own dictionary, and the
//! dictionaries' words. Used for completing words and for knowing a
//! book's names and terms.

use std::collections::HashMap;

/// Words with how often each appears, sorted for finding by beginning.
#[derive(Debug, Default, Clone)]
pub struct Vocab {
    /// (lower case, as usually written, count), sorted by lower case.
    words: Vec<(String, String, u32)>,
}

impl Vocab {
    /// Counts the words of a text (or many).
    pub fn learn<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        let mut counts: HashMap<String, HashMap<String, u32>> = HashMap::new();
        for t in texts {
            for w in crate::text::words(t) {
                let norm = w.text.replace('’', "'");
                *counts
                    .entry(norm.to_lowercase())
                    .or_default()
                    .entry(norm)
                    .or_default() += 1;
            }
        }
        Self::from_counts(counts)
    }

    /// Words with no count (a dictionary's stems).
    pub fn from_list(words: impl IntoIterator<Item = String>) -> Self {
        let mut counts: HashMap<String, HashMap<String, u32>> = HashMap::new();
        for w in words {
            counts
                .entry(w.to_lowercase())
                .or_default()
                .entry(w)
                .or_default();
        }
        Self::from_counts(counts)
    }

    fn from_counts(counts: HashMap<String, HashMap<String, u32>>) -> Self {
        let mut words: Vec<(String, String, u32)> = counts
            .into_iter()
            .map(|(lower, forms)| {
                let total = forms.values().sum();
                // The form written most; lower case wins a tie, so a word
                // that starts sentences is not taken for a name.
                let best = forms
                    .into_iter()
                    .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                    .map(|f| f.0)
                    .unwrap_or_else(|| lower.clone());
                (lower, best, total)
            })
            .collect();
        words.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        Self { words }
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// How often a word appears (any case).
    pub fn count(&self, word: &str) -> u32 {
        let lower = word.replace('’', "'").to_lowercase();
        self.words
            .binary_search_by(|w| w.0.as_str().cmp(&lower))
            .map_or(0, |i| self.words[i].2)
    }

    /// Words beginning with `prefix` (any case): (as written, count).
    pub fn starting(&self, prefix: &str) -> impl Iterator<Item = (&str, u32)> {
        let lower = prefix.replace('’', "'").to_lowercase();
        let from = self
            .words
            .partition_point(|w| w.0.as_str() < lower.as_str());
        self.words[from..]
            .iter()
            .take_while(move |w| w.0.starts_with(&lower))
            .map(|w| (w.1.as_str(), w.2))
    }

    /// Words that appear at least `n` times.
    pub fn frequent(&self, n: u32) -> impl Iterator<Item = &str> {
        self.words
            .iter()
            .filter(move |w| w.2 >= n)
            .map(|w| w.1.as_str())
    }
}

/// Where the words for completing come from, most trusted first.
pub struct Sources<'a> {
    pub own: &'a [String],
    pub book: Option<&'a Vocab>,
    pub notes: Option<&'a Vocab>,
    pub dictionaries: &'a [&'a Vocab],
}

/// Up to `limit` ways to finish `prefix`, best first, in the prefix's case.
pub fn complete(prefix: &str, sources: &Sources<'_>, limit: usize) -> Vec<String> {
    let plen = prefix.chars().count();
    if plen < 2 {
        return Vec::new();
    }
    let mut score: HashMap<String, (f64, String)> = HashMap::new();
    let mut add = |word: &str, s: f64| {
        if word.chars().count() <= plen + 1 {
            return;
        }
        let e = score
            .entry(word.to_lowercase())
            .or_insert((0.0, word.to_owned()));
        e.0 += s;
    };
    let lower = prefix.to_lowercase();
    for w in sources.own {
        if w.to_lowercase().starts_with(&lower) {
            add(w, 40.0);
        }
    }
    if let Some(b) = sources.book {
        for (w, n) in b.starting(prefix) {
            add(w, 6.0 + 4.0 * f64::from(n).ln_1p());
        }
    }
    if let Some(v) = sources.notes {
        for (w, n) in v.starting(prefix) {
            add(w, 4.0 + 3.0 * f64::from(n).ln_1p());
        }
    }
    for d in sources.dictionaries {
        for (w, _) in d.starting(prefix) {
            add(w, 1.0);
        }
    }
    let mut list: Vec<(f64, String)> = score.into_values().collect();
    // Higher score first, then shorter words (more likely to be meant).
    list.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| a.1.chars().count().cmp(&b.1.chars().count()))
            .then_with(|| a.1.cmp(&b.1))
    });
    let all_caps = plen > 1 && prefix.chars().all(|c| !c.is_lowercase());
    list.into_iter()
        .take(limit)
        .map(|(_, w)| {
            // Keep what was typed and add the rest as usually written.
            let rest: String = w.chars().skip(plen).collect();
            let rest = if all_caps { rest.to_uppercase() } else { rest };
            format!("{prefix}{rest}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_finds_by_beginning() {
        let v = Vocab::learn([
            "The keeper kept the Keeper's log. Keepers keep lamps.",
            "keeper",
        ]);
        assert_eq!(v.count("keeper"), 2);
        assert_eq!(v.count("KEEPER"), 2);
        let found: Vec<&str> = v.starting("kee").map(|w| w.0).collect();
        assert_eq!(found, ["keep", "keeper", "Keeper's", "Keepers"]);
    }

    #[test]
    fn completes_from_the_book_first() {
        let book =
            Vocab::learn(["Samuel Harte kept the lighthouse. Harte wrote at dusk. Harte slept."]);
        let dict = Vocab::from_list([
            "hart".into(),
            "hartebeest".into(),
            "harvest".into(),
            "light".into(),
        ]);
        let own = vec!["Hartford".to_owned()];
        let s = Sources {
            own: &own,
            book: Some(&book),
            notes: None,
            dictionaries: &[&dict],
        };
        assert_eq!(complete("Ha", &s, 3), ["Hartford", "Harte", "Hart"]);
        assert_eq!(complete("lig", &s, 2), ["lighthouse", "light"]);
        assert!(complete("l", &s, 3).is_empty());
    }
}
