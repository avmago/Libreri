//! Markdown: YAML-style front matter, or the first `# Heading`.
//!
//! Only the simple front-matter forms people write by hand are understood:
//! `key: value`, `key: [a, b]` and `key:` followed by `- item` lines.

use crate::{find_year, split_keywords, split_people, Extracted};
use libreri_core::isbn::find_isbn;
use std::io::Read;
use std::path::Path;

/// Front matter and headings live at the top; never read a whole large file.
const HEAD_BYTES: u64 = 64 * 1024;

fn unquote(s: &str) -> String {
    let s = s.trim();
    let q = s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')));
    if q {
        s[1..s.len() - 1].to_owned()
    } else {
        s.to_owned()
    }
}

/// `(key, values)` pairs in the order written.
type Fields = Vec<(String, Vec<String>)>;

fn parse_front_matter(text: &str) -> Option<(Fields, &str)> {
    let rest = text
        .strip_prefix("\u{feff}")
        .unwrap_or(text)
        .strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let end = rest.find("\n---").or_else(|| rest.find("\n..."))?;
    let (block, after) = rest.split_at(end);
    let body = after[4..].trim_start_matches(['-', '.']);

    let mut fields: Fields = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some((_, values)) = fields.last_mut() {
                values.push(unquote(item));
            }
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim();
            let values =
                if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    inner
                        .split(',')
                        .map(unquote)
                        .filter(|v| !v.is_empty())
                        .collect()
                } else if value.is_empty() {
                    Vec::new()
                } else {
                    vec![unquote(value)]
                };
            fields.push((key, values));
        }
    }
    Some((fields, body))
}

pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&head);
    let m = &mut out.metadata;

    let body = match parse_front_matter(&text) {
        Some((fields, body)) => {
            for (key, values) in fields {
                let one = values.first().cloned().unwrap_or_default();
                match key.as_str() {
                    "title" => m.title = one,
                    "subtitle" => m.subtitle = Some(one),
                    "author" | "authors" | "creator" => {
                        m.authors = values.iter().flat_map(|v| split_people(v)).collect()
                    }
                    "tags" | "keywords" => {
                        m.tags = values.iter().flat_map(|v| split_keywords(v)).collect()
                    }
                    "categories" | "category" => m.categories = values,
                    "description" | "summary" | "abstract" => m.about = Some(one),
                    "date" | "year" => m.year = find_year(&one),
                    "publisher" => m.publisher = Some(one),
                    "lang" | "language" => m.language = Some(one),
                    "isbn" => {
                        if let Some((i13, i10)) = find_isbn(&one) {
                            m.isbn13 = Some(i13);
                            m.isbn10 = i10;
                        }
                    }
                    "doi" => m.doi = Some(one),
                    "url" => m.url = Some(one),
                    _ => {}
                }
            }
            body.to_owned()
        }
        None => text.into_owned(),
    };

    if m.title.trim().is_empty() {
        if let Some(h) = body
            .lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix("# "))
        {
            m.title = h.trim().trim_end_matches('#').trim().to_owned();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::extract;
    use libreri_core::FileType;

    fn md(text: &str) -> libreri_core::BookMetadata {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("file-name.md");
        std::fs::write(&p, text).unwrap();
        extract(&p, FileType::Md).metadata
    }

    #[test]
    fn reads_front_matter() {
        let m = md("---\ntitle: \"Lecture 3: Fourier\"\nauthors:\n  - John Smith\n  - Jane Smith\ntags: [maths, signals]\ndate: 2021-03-04\n---\n# Ignored\n$$x^2$$\n");
        assert_eq!(m.title, "Lecture 3: Fourier");
        assert_eq!(m.authors, vec!["John Smith", "Jane Smith"]);
        assert_eq!(m.tags, vec!["maths", "signals"]);
        assert_eq!(m.year, Some(2021));
    }

    #[test]
    fn falls_back_to_heading_then_file_name() {
        assert_eq!(md("intro\n\n# Linear Maps ##\n").title, "Linear Maps");
        assert_eq!(md("no heading").title, "file-name");
    }
}
