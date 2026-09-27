//! Organising: bulk edits and tidying tags and categories.
//!
//! These are pure functions on metadata; `libreri-library` applies them to
//! books and writes the sidecars.

use crate::{BookMetadata, ContentType};
use serde::{Deserialize, Serialize};

/// Changes applied to many books at once. `None` leaves a field alone;
/// `Some("")` clears a text field.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BulkEdit {
    /// Replaces the authors (an empty list removes them).
    pub authors: Option<Vec<String>>,
    pub publisher: Option<String>,
    /// A year, or "" to clear.
    pub year: Option<String>,
    pub language: Option<String>,
    pub content_type: Option<ContentType>,
    pub series: Option<String>,
    /// Number the books 1, 2, 3… in the order given.
    pub number_series: bool,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub add_categories: Vec<String>,
    pub remove_categories: Vec<String>,
}

fn text(v: &str) -> Option<String> {
    let v = v.split_whitespace().collect::<Vec<_>>().join(" ");
    (!v.is_empty()).then_some(v)
}

impl BulkEdit {
    /// True if the edit changes nothing.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Applies the edit to one book's metadata. `index` is the book's place
    /// in the selection (for series numbers).
    pub fn apply(&self, m: &mut BookMetadata, index: usize) -> Result<(), String> {
        if let Some(a) = &self.authors {
            m.authors = a.clone();
        }
        if let Some(v) = &self.publisher {
            m.publisher = text(v);
        }
        if let Some(v) = &self.year {
            m.year = match text(v) {
                None => None,
                Some(y) => Some(y.parse().map_err(|_| format!("“{y}” is not a year"))?),
            };
        }
        if let Some(v) = &self.language {
            m.language = text(v);
        }
        if let Some(t) = self.content_type {
            m.content_type = t;
        }
        if let Some(v) = &self.series {
            m.series = text(v);
            if m.series.is_none() {
                m.series_number = None;
            }
        }
        if self.number_series {
            m.series_number = Some(index as f64 + 1.0);
        }
        m.tags
            .retain(|t| !self.remove_tags.iter().any(|r| r.eq_ignore_ascii_case(t)));
        m.tags.extend(self.add_tags.iter().cloned());
        m.categories.retain(|c| {
            !self
                .remove_categories
                .iter()
                .any(|r| c.eq_ignore_ascii_case(r) || is_below(c, r))
        });
        m.categories.extend(self.add_categories.iter().cloned());
        Ok(())
    }
}

/// True if category `c` is inside `parent` ("Science/Physics" in "Science").
pub fn is_below(c: &str, parent: &str) -> bool {
    c.len() > parent.len()
        && c.as_bytes()[parent.len()] == b'/'
        && c[..parent.len()].eq_ignore_ascii_case(parent)
}

/// Renames a tag in a list; if the new name is already there, the two merge.
pub fn rename_in(list: &mut Vec<String>, from: &str, to: &str) -> bool {
    let Some(i) = list.iter().position(|t| t.eq_ignore_ascii_case(from)) else {
        return false;
    };
    if list
        .iter()
        .enumerate()
        .any(|(j, t)| j != i && t.eq_ignore_ascii_case(to))
    {
        list.remove(i);
    } else {
        list[i] = to.to_owned();
    }
    true
}

/// Moves category `from` (and everything below it) to `to`.
/// "Science/Physics" renamed to "Physics" turns "Science/Physics/Optics"
/// into "Physics/Optics".
pub fn rename_category_in(list: &mut Vec<String>, from: &str, to: &str) -> bool {
    let mut changed = false;
    for c in list.iter_mut() {
        if c.eq_ignore_ascii_case(from) {
            *c = to.to_owned();
            changed = true;
        } else if is_below(c, from) {
            *c = format!("{to}{}", &c[from.len()..]);
            changed = true;
        }
    }
    if changed {
        let mut seen: Vec<String> = Vec::new();
        list.retain(|c| {
            let dup = seen.iter().any(|s| s.eq_ignore_ascii_case(c));
            seen.push(c.clone());
            !dup
        });
    }
    changed
}

/// A comparison key: lower case, letters and digits only, a plural "s"
/// dropped. "Machine-Learning", "machine learning" and "machinelearnings"
/// share one key.
fn tag_key(tag: &str) -> String {
    let mut k: String = tag
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect();
    if k.len() > 3 && k.ends_with('s') && !k.ends_with("ss") {
        k.pop();
    }
    k
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Groups tags that are probably the same: the same key, or keys of five
/// or more letters one typo apart ("Pysics" / "Physics"). Each group is
/// ordered by how many books use the tag, most first. `tags` holds
/// (name, book count).
pub fn similar_tags(tags: &[(String, u32)]) -> Vec<Vec<(String, u32)>> {
    let keys: Vec<String> = tags.iter().map(|(t, _)| tag_key(t)).collect();
    let n = tags.len();
    // Union-find over tags.
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        p[i] = r;
        r
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&keys[i], &keys[j]);
            let close = a == b
                || (a.chars().count() >= 5
                    && b.chars().count() >= 5
                    && a.chars().count().abs_diff(b.chars().count()) <= 1
                    && edit_distance(a, b) <= 1);
            if close {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<(String, u32)>> = Default::default();
    for (i, tag) in tags.iter().enumerate() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(tag.clone());
    }
    let mut out: Vec<Vec<(String, u32)>> = groups
        .into_values()
        .filter(|g| g.len() > 1)
        .map(|mut g| {
            g.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            g
        })
        .collect();
    out.sort_by(|a, b| a[0].0.to_lowercase().cmp(&b[0].0.to_lowercase()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> BookMetadata {
        BookMetadata {
            title: "T".into(),
            tags: vec!["physics".into(), "Old".into()],
            categories: vec!["Science/Physics".into(), "Fiction".into()],
            ..Default::default()
        }
    }

    #[test]
    fn bulk_edit_sets_clears_and_numbers() {
        let edit = BulkEdit {
            publisher: Some("  Acme   Press ".into()),
            year: Some("".into()),
            series: Some("Foundations".into()),
            number_series: true,
            add_tags: vec!["new".into()],
            remove_tags: vec!["OLD".into()],
            add_categories: vec!["Maths".into()],
            remove_categories: vec!["science".into()],
            ..Default::default()
        };
        let mut m = meta();
        m.year = Some(1999);
        edit.apply(&mut m, 2).unwrap();
        assert_eq!(m.publisher.as_deref(), Some("Acme Press"));
        assert_eq!(m.year, None);
        assert_eq!(m.series.as_deref(), Some("Foundations"));
        assert_eq!(m.series_number, Some(3.0));
        assert_eq!(m.tags, ["physics", "new"]);
        assert_eq!(m.categories, ["Fiction", "Maths"]);
        assert!(BulkEdit {
            year: Some("soon".into()),
            ..Default::default()
        }
        .apply(&mut meta(), 0)
        .is_err());
        assert!(BulkEdit::default().is_empty());
    }

    #[test]
    fn renaming_tags_merges_duplicates() {
        let mut l = vec!["ML".to_owned(), "Machine learning".to_owned()];
        assert!(rename_in(&mut l, "ml", "Machine Learning"));
        assert_eq!(l, ["Machine learning"]);
        let mut l = vec!["a".to_owned()];
        assert!(rename_in(&mut l, "A", "b"));
        assert_eq!(l, ["b"]);
        assert!(!rename_in(&mut l, "zzz", "b"));
    }

    #[test]
    fn renaming_categories_moves_children() {
        let mut l = vec![
            "Science/Physics".to_owned(),
            "Science/Physics/Optics".to_owned(),
            "Physics".to_owned(),
            "Sciences".to_owned(),
        ];
        assert!(rename_category_in(&mut l, "science/physics", "Physics"));
        assert_eq!(l, ["Physics", "Physics/Optics", "Sciences"]);
    }

    #[test]
    fn finds_similar_tags() {
        let tags = vec![
            ("Machine learning".to_owned(), 10),
            ("machine-learning".to_owned(), 2),
            ("Physics".to_owned(), 5),
            ("Pysics".to_owned(), 1),
            ("Novel".to_owned(), 3),
            ("Novels".to_owned(), 4),
            ("Art".to_owned(), 2),
            ("Arts".to_owned(), 1),
            ("Maths".to_owned(), 1),
            ("Paths".to_owned(), 1),
        ];
        let groups = similar_tags(&tags);
        let names: Vec<Vec<&str>> = groups
            .iter()
            .map(|g| g.iter().map(|(t, _)| t.as_str()).collect())
            .collect();
        assert_eq!(
            names,
            vec![
                vec!["Art", "Arts"],
                vec!["Machine learning", "machine-learning"],
                vec!["Novels", "Novel"],
                vec!["Physics", "Pysics"],
            ]
        );
    }
}
