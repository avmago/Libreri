//! Finding where an audiobook is in its text: short stretches of the audio
//! are transcribed, and their words are looked up in the book's words. The
//! places found become sync points.

use crate::transcribe::Segment;
use std::collections::HashMap;

/// A word for matching: lower case, letters and digits only.
pub fn norm(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Splits text into matching words.
pub fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(norm)
        .filter(|w| !w.is_empty())
        .collect()
}

fn key(a: &str, b: &str, c: &str) -> String {
    format!("{a} {b} {c}")
}

/// The book's words, with an index of every three-word run.
pub struct TextIndex {
    pub words: Vec<String>,
    grams: HashMap<String, Vec<u32>>,
}

/// Runs of words this common say nothing about where we are.
const COMMON: usize = 40;

impl TextIndex {
    pub fn new(words: Vec<String>) -> Self {
        let mut grams: HashMap<String, Vec<u32>> = HashMap::new();
        for i in 0..words.len().saturating_sub(2) {
            grams
                .entry(key(&words[i], &words[i + 1], &words[i + 2]))
                .or_default()
                .push(i as u32);
        }
        grams.retain(|_, v| v.len() <= COMMON);
        Self { words, grams }
    }

    /// Where `heard` (the words of a stretch of audio) is in the text:
    /// (text word index, heard word index) of the match, and how many
    /// three-word runs agree.
    pub fn find(&self, heard: &[String]) -> Option<(usize, usize, usize)> {
        if heard.len() < 5 {
            return None;
        }
        // Votes for each alignment (text index minus heard index), in
        // buckets so a missed or extra word still counts.
        let mut votes: HashMap<i64, (usize, usize, usize)> = HashMap::new();
        for j in 0..heard.len() - 2 {
            let Some(list) = self
                .grams
                .get(&key(&heard[j], &heard[j + 1], &heard[j + 2]))
            else {
                continue;
            };
            for &i in list {
                let diag = (i64::from(i) - j as i64).div_euclid(4);
                let e = votes.entry(diag).or_insert((0, i as usize, j));
                e.0 += 1;
            }
        }
        let grams = heard.len() - 2;
        let (count, i, j) = votes.into_values().max_by_key(|v| v.0)?;
        (count >= 3 && count * 5 >= grams).then_some((i, j, count))
    }
}

/// A stretch of audio and what was heard in it.
pub struct Heard {
    /// Where the stretch starts in the audiobook, in seconds.
    pub start: f64,
    pub segments: Vec<Segment>,
}

/// A moment of the audio matched to a word of the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub t: f64,
    pub word: usize,
}

/// The time of heard word `j`, spread evenly over its segment.
fn time_of(h: &Heard, j: usize) -> f64 {
    let mut n = 0;
    for s in &h.segments {
        let count = words(&s.text).len();
        if j < n + count {
            let k = (j - n) as f64 / count.max(1) as f64;
            return h.start + s.start + (s.end - s.start) * k;
        }
        n += count;
    }
    h.start
}

/// Anchors for the stretches that could be placed, in order through both
/// the audio and the text (places that go backwards are dropped).
pub fn anchors(index: &TextIndex, heard: &[Heard]) -> Vec<Anchor> {
    let mut found: Vec<Anchor> = heard
        .iter()
        .filter_map(|h| {
            let w: Vec<String> = h.segments.iter().flat_map(|s| words(&s.text)).collect();
            let (i, j, _) = index.find(&w)?;
            Some(Anchor {
                t: time_of(h, j),
                word: i,
            })
        })
        .collect();
    found.sort_by(|a, b| a.t.total_cmp(&b.t));
    increasing(&found)
}

/// The longest run of anchors that goes forward in the text.
fn increasing(list: &[Anchor]) -> Vec<Anchor> {
    let n = list.len();
    let mut len = vec![1usize; n];
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        for j in 0..i {
            if list[j].word < list[i].word && len[j] + 1 > len[i] {
                len[i] = len[j] + 1;
                prev[i] = j;
            }
        }
    }
    let Some(mut i) = (0..n).max_by_key(|&i| len[i]) else {
        return Vec::new();
    };
    let mut out = vec![list[i]];
    while prev[i] != usize::MAX {
        i = prev[i];
        out.push(list[i]);
    }
    out.reverse();
    out
}

/// Where to listen: `count` stretches of `len` seconds spread over the
/// audiobook (not the very start, which is often credits).
pub fn plan(duration: f64, count: usize, len: f64) -> Vec<f64> {
    if duration <= len * 2.0 || count == 0 {
        return vec![0.0];
    }
    let usable = duration - len;
    let step = usable / count as f64;
    (0..count)
        .map(|k| (step * (k as f64 + 0.5)).max(0.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> TextIndex {
        let text = "The keeper climbed the stair every evening at dusk. He lit the great lamp and \
                    wound the clockwork that turned the lens. Storms came in from the west in \
                    autumn and the waves broke over the rocks below the tower. In the morning \
                    he wrote the log: wind, weather, ships seen, and the oil burned in the night.";
        TextIndex::new(words(text))
    }

    fn heard(start: f64, text: &str) -> Heard {
        Heard {
            start,
            segments: vec![Segment {
                start: 0.0,
                end: 10.0,
                text: text.into(),
            }],
        }
    }

    #[test]
    fn finds_what_was_heard() {
        let idx = book();
        // Whisper mishears a word or two; the place is still found.
        let h = words("storms came in from the waist in autumn and the waves broke over");
        let (i, j, n) = idx.find(&h).unwrap();
        assert_eq!(idx.words[i - j], "storms");
        assert!(n >= 3);
        assert!(idx
            .find(&words("completely different words about a garden party"))
            .is_none());
    }

    #[test]
    fn keeps_anchors_that_go_forward() {
        let idx = book();
        let got = anchors(
            &idx,
            &[
                heard(0.0, "the keeper climbed the stair every evening at dusk"),
                heard(
                    300.0,
                    "in the morning he wrote the log wind weather ships seen",
                ),
                // Out of order (a repeated passage): dropped.
                heard(450.0, "he lit the great lamp and wound the clockwork"),
                heard(600.0, "and the oil burned in the night"),
            ],
        );
        let ts: Vec<f64> = got.iter().map(|a| a.t).collect();
        assert_eq!(ts, [0.0 + 0.0, 300.0, 600.0].map(|t| t), "{got:?}");
        assert!(got.windows(2).all(|w| w[0].word < w[1].word));
    }

    #[test]
    fn plans_stretches() {
        let p = plan(3600.0, 12, 20.0);
        assert_eq!(p.len(), 12);
        assert!(p[0] > 100.0 && p[11] < 3600.0 - 20.0);
        assert_eq!(plan(30.0, 12, 20.0), [0.0]);
    }
}
