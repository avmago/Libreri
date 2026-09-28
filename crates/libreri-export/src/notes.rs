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
    let links: Vec<_> = notes
        .iter()
        .filter(|a| a.kind == AnnotationKind::Link)
        .filter_map(|a| link_of(&a.locator).map(|l| (a, l)))
        .collect();
    if !links.is_empty() {
        s += "\n## Links\n\n";
        for (a, (title, href)) in links {
            let target = if href.contains([' ', '(', ')', '<', '>']) {
                format!("<{href}>")
            } else {
                href.clone()
            };
            let about = a
                .quote
                .as_ref()
                .map(|q| q.exact.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|q| !q.is_empty())
                .map(|q| format!(" about “{q}”"))
                .unwrap_or_default();
            let label = a.label.as_deref().unwrap_or("Open");
            s += &format!("- [{title}]({target}){about} — [{label}]({})\n", a.link());
            if let Some(note) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                s += &format!("  {}\n", note.trim());
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

/// A link annotation's title and where it goes (a video at its start
/// time; a file by its path).
fn link_of(locator: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(locator).ok()?;
    let l = v.get("link")?;
    let s = |k: &str| l.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty());
    let mut href = s("url").or_else(|| s("file"))?.to_owned();
    if let Some(start) = l.get("start").and_then(|x| x.as_f64()).filter(|t| *t > 0.0) {
        let t = start.round() as u64;
        let youtube = l.pointer("/video/provider").and_then(|p| p.as_str()) == Some("youtube");
        if youtube {
            let sep = if href.contains('?') { '&' } else { '?' };
            href = format!("{href}{sep}t={t}s");
        } else if s("url").is_some() {
            href = format!("{}#t={t}s", href.split('#').next().unwrap_or(&href));
        }
    }
    let title = s("title").unwrap_or(&href).replace(['[', ']'], " ");
    Some((title, href))
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

        let mut link = highlight(&b, "light bends", Some("A good demo"));
        link.id = "22222222-2222-4222-8222-222222222222".into();
        link.kind = AnnotationKind::Link;
        link.locator = r#"{"type":"pdf","page":4,"link":{"url":"https://www.youtube.com/watch?v=abc123def45","kind":"video","title":"Refraction [demo]","start":90,"video":{"provider":"youtube","id":"abc123def45"}}}"#.into();
        e.annotations.push(link);
        let note = book_note(&e, None);
        assert!(
            note.contains("## Links\n\n- [Refraction  demo ](https://www.youtube.com/watch?v=abc123def45&t=90s) about “light bends” — [p. 4](libreri://book/"),
            "{note}"
        );
        assert!(note.contains("  A good demo\n"));
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
