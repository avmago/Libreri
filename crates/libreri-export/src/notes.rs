//! Markdown notes for Obsidian (or any Markdown app): one note per book
//! with its details as properties and the reader's highlights, each with a
//! `libreri://` link back to its place and a readable label such as "p. 4".

use crate::{content_type_label, full_title, status_label, Entry};
use libreri_core::annotation::book_link;
use libreri_core::{AnnotationKind, ReadingStatus};

/// A YAML string in double quotes.
fn yaml(s: &str) -> String {
    let escaped: String = s
        .chars()
        .map(|c| match c {
            '\\' => "\\\\".to_owned(),
            '"' => "\\\"".to_owned(),
            '\n' => "\\n".to_owned(),
            c if c.is_control() => " ".to_owned(),
            c => c.to_string(),
        })
        .collect();
    format!("\"{escaped}\"")
}

/// An Obsidian tag: letters, digits, `_`, `-` and `/` only, not all digits.
pub fn obsidian_tag(s: &str) -> Option<String> {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_alphanumeric() || matches!(c, '_' | '-' | '/') {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches(['-', '/']).to_owned();
    (!out.is_empty() && !out.chars().all(|c| c.is_ascii_digit())).then_some(out)
}

/// Quotes each line of `text` as a Markdown block quote.
fn block_quote(text: &str) -> String {
    text.lines()
        .map(|l| format!("> {}", l.trim_end()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The note for one book. `notebook` is the name of the copied notebook
/// note to link to (without `.md`).
pub fn book_note(e: &Entry, notebook: Option<&str>) -> String {
    let b = &e.book;
    let m = &b.metadata;
    let mut s = String::from("---\n");
    s += &format!("title: {}\n", yaml(&full_title(b)));
    if !m.authors.is_empty() {
        s += "authors:\n";
        for a in &m.authors {
            s += &format!("  - {}\n", yaml(a));
        }
    }
    if let Some(y) = m.year {
        s += &format!("year: {y}\n");
    }
    for (key, v) in [
        ("publisher", &m.publisher),
        ("isbn", &m.isbn13),
        ("doi", &m.doi),
        ("arxiv", &m.arxiv_id),
        ("series", &m.series),
        ("journal", &m.journal),
        ("language", &m.language),
    ] {
        if let Some(v) = v.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            s += &format!("{key}: {}\n", yaml(v));
        }
    }
    s += &format!("type: {}\n", yaml(content_type_label(m.content_type)));
    let tags: Vec<String> = m.tags.iter().filter_map(|t| obsidian_tag(t)).collect();
    if !tags.is_empty() {
        s += "tags:\n";
        for t in tags {
            s += &format!("  - {t}\n");
        }
    }
    if !m.categories.is_empty() {
        s += "categories:\n";
        for c in &m.categories {
            s += &format!("  - {}\n", yaml(c));
        }
    }
    if b.user.status != ReadingStatus::None {
        s += &format!("status: {}\n", yaml(status_label(b.user.status)));
    }
    if b.user.rating > 0 {
        s += &format!("rating: {}\n", b.user.rating);
    }
    s += &format!("libreri: {}\n", yaml(&book_link(&b.id, None)));
    s += "---\n\n";

    s += &format!("# {}\n\n", full_title(b));
    if !m.authors.is_empty() {
        s += &m.authors.join(", ");
        if let Some(y) = m.year {
            s += &format!(" · {y}");
        }
        s += "\n\n";
    }
    s += &format!("[Open in Libreri]({})\n", book_link(&b.id, None));
    if let Some(nb) = notebook {
        s += &format!("\nNotebook: [[{nb}]]\n");
    }

    let mut notes: Vec<_> = e.annotations.iter().collect();
    notes.sort_by(|a, b| a.position.total_cmp(&b.position));
    let highlights: Vec<_> = notes
        .iter()
        .filter(|a| a.kind == AnnotationKind::Highlight)
        .collect();
    if !highlights.is_empty() {
        s += "\n## Highlights\n";
        for a in highlights {
            let quote = a.quote.as_ref().map(|q| q.exact.trim()).unwrap_or("");
            s += &format!("\n{}\n", block_quote(quote));
            let label = a.label.as_deref().unwrap_or("Open");
            let colour = a
                .color
                .map(|c| format!(" · {}", c.as_str()))
                .unwrap_or_default();
            s += &format!("> — [{label}]({}){colour}\n", a.link());
            if let Some(note) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                s += &format!("\n{}\n", note.trim());
            }
        }
    }
    let bookmarks: Vec<_> = notes
        .iter()
        .filter(|a| a.kind == AnnotationKind::Bookmark)
        .collect();
    if !bookmarks.is_empty() {
        s += "\n## Bookmarks\n\n";
        for a in bookmarks {
            let name = a
                .note
                .as_deref()
                .or(a.label.as_deref())
                .unwrap_or("Bookmark");
            let place = a
                .label
                .as_deref()
                .filter(|l| Some(*l) != a.note.as_deref())
                .map(|l| format!(" ({l})"))
                .unwrap_or_default();
            s += &format!("- [{name}]({}){place}\n", a.link());
        }
    }
    s
}

/// An index note linking every book note: (title, note name).
pub fn index_note(library: &str, books: &[(String, String)]) -> String {
    let mut s = format!("# {library}\n\n");
    for (title, name) in books {
        if title == name {
            s += &format!("- [[{name}]]\n");
        } else {
            s += &format!("- [[{name}|{}]]\n", title.replace(['[', ']', '|'], " "));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{book, highlight};
    use libreri_core::AnnotationKind;

    #[test]
    fn book_notes_link_back_to_every_place() {
        let b = book("Optics \"2\"");
        let mut e = Entry::new(b.clone());
        e.annotations
            .push(highlight(&b, "light bends\nat edges", Some("See ch. 3")));
        let mut bm = highlight(&b, "", None);
        bm.id = "11111111-1111-4111-8111-111111111111".into();
        bm.kind = AnnotationKind::Bookmark;
        bm.quote = None;
        bm.note = Some("Start here".into());
        e.annotations.push(bm);
        let note = book_note(&e, Some("Optics notebook"));
        assert!(note.starts_with("---\ntitle: \"Optics \\\"2\\\"\"\n"));
        assert!(note.contains("tags:\n  - Optics\n  - Light-waves\n"));
        assert!(note.contains("status: \"Finished\"\nrating: 4\n"));
        assert!(note.contains("> light bends\n> at edges\n> — [p. 4](libreri://book/"));
        assert!(note
            .contains("#annotation=8f14e45f-ceea-4e7a-9c3f-0b6d9d2a1c11) · yellow\n\nSee ch. 3\n"));
        assert!(note.contains("- [Start here](libreri://book/"));
        assert!(note.contains("Notebook: [[Optics notebook]]"));
    }

    #[test]
    fn tags_and_index() {
        assert_eq!(
            obsidian_tag("Machine learning"),
            Some("Machine-learning".into())
        );
        assert_eq!(
            obsidian_tag("Science/Physics"),
            Some("Science/Physics".into())
        );
        assert_eq!(obsidian_tag("1984"), None);
        let idx = index_note(
            "Home",
            &[("A: B".into(), "A B".into()), ("C".into(), "C".into())],
        );
        assert_eq!(idx, "# Home\n\n- [[A B|A: B]]\n- [[C]]\n");
    }
}
