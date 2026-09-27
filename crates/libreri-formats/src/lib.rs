//! Reading what a book file says about itself.
//!
//! [`extract`] never fails: a damaged or unusual file still gets a title
//! (from its file name) and whatever else could be read, plus warnings.
//! Nothing here writes to disk.

mod comic;
mod epub;
mod fb2;
mod markdown;
mod pdf;
#[doc(hidden)]
pub use pdf::test_pdf;
pub use pdf::{page_boxes, PageBox};
pub mod xml;

use libreri_core::{BookMetadata, FileType};
use std::io::Read;
use std::path::Path;

/// What was found in a file.
#[derive(Debug, Clone, Default)]
pub struct Extracted {
    pub metadata: BookMetadata,
    /// Raw bytes of an embedded cover image (JPEG, PNG, …), if any.
    pub cover: Option<Vec<u8>>,
    /// Problems worth showing in the import report.
    pub warnings: Vec<String>,
}

/// Largest single entry we read out of a ZIP-based format (covers, OPF).
pub(crate) const MAX_ENTRY: u64 = 32 * 1024 * 1024;

/// Reads metadata and the cover from `path`. `file_type` decides the reader.
pub fn extract(path: &Path, file_type: FileType) -> Extracted {
    let mut out = Extracted::default();
    out.metadata.content_type = file_type.default_content_type();
    let result = match file_type {
        FileType::Pdf => pdf::read(path, &mut out),
        FileType::Epub => epub::read(path, &mut out),
        FileType::Md => markdown::read(path, &mut out),
        FileType::Fb2 => fb2::read(path, &mut out),
        FileType::Cbz => comic::read_cbz(path, &mut out),
        _ => Ok(()),
    };
    if let Err(e) = result {
        out.warnings.push(format!("could not read details: {e}"));
    }
    finish(path, &mut out.metadata);
    out
}

/// Fills gaps from the file name and cleans up what readers found.
fn finish(path: &Path, m: &mut BookMetadata) {
    m.title = xml::collapse(&m.title);
    if !plausible_title(&m.title) {
        m.title.clear();
    }
    if m.title.is_empty() {
        let name = BookMetadata::title_from_file_name(path);
        match split_author_title(&name) {
            Some((authors, title)) if m.authors.is_empty() => {
                m.authors = authors;
                m.title = title;
            }
            _ => m.title = name,
        }
    }
    m.authors.retain(|a| plausible_author(a));
}

/// Rejects titles that producers leave behind ("Microsoft Word - doc1.docx",
/// "untitled", "Slide 1").
fn plausible_title(t: &str) -> bool {
    let lower = t.to_lowercase();
    let junk_ext = [
        ".doc", ".docx", ".pdf", ".tex", ".dvi", ".indd", ".qxd", ".rtf", ".odt", ".pages", ".ps",
        ".txt",
    ];
    t.chars().filter(|c| c.is_alphabetic()).count() >= 2
        && !lower.starts_with("microsoft word")
        && !lower.starts_with("microsoft powerpoint")
        && !matches!(
            lower.as_str(),
            "untitled" | "title" | "no title" | "unknown"
        )
        && !junk_ext.iter().any(|e| lower.ends_with(e))
}

fn plausible_author(a: &str) -> bool {
    let lower = a.to_lowercase();
    a.chars().any(char::is_alphabetic)
        && !matches!(
            lower.as_str(),
            "unknown" | "author" | "administrator" | "admin" | "user"
        )
}

/// "John Smith - Quantum Mechanics" → (["John Smith"], "Quantum Mechanics").
fn split_author_title(name: &str) -> Option<(Vec<String>, String)> {
    let (left, right) = name.split_once(" - ")?;
    if right.contains(" - ") || left.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let authors = split_people(left);
    let looks_like_names = !authors.is_empty()
        && authors.iter().all(|a| {
            let words: Vec<&str> = a.split_whitespace().collect();
            (1..=4).contains(&words.len())
                && words
                    .iter()
                    .all(|w| w.chars().next().is_some_and(char::is_uppercase))
        });
    (looks_like_names && !right.trim().is_empty()).then(|| (authors, right.trim().to_owned()))
}

/// Splits "John Smith; Jane Smith", "John Smith and Jane Smith" or
/// "John Smith & Jane Smith" into names. Commas are kept ("Smith, John").
pub(crate) fn split_people(s: &str) -> Vec<String> {
    s.split([';', '&', '\n'])
        .flat_map(|p| p.split(" and "))
        .map(xml::collapse)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Splits keyword lists ("physics, quantum; waves").
pub(crate) fn split_keywords(s: &str) -> Vec<String> {
    s.split([',', ';', '\n'])
        .map(xml::collapse)
        .filter(|k| !k.is_empty() && k.chars().count() <= 60)
        .collect()
}

/// The first four-digit year between 1000 and 2999 in `s`.
pub(crate) fn find_year(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    (0..bytes.len().saturating_sub(3)).find_map(|i| {
        let w = &bytes[i..i + 4];
        let before_ok = i == 0 || !bytes[i - 1].is_ascii_digit();
        let after_ok = bytes.get(i + 4).is_none_or(|b| !b.is_ascii_digit());
        (before_ok && after_ok && w.iter().all(u8::is_ascii_digit) && matches!(w[0], b'1' | b'2'))
            .then(|| std::str::from_utf8(w).ok()?.parse().ok())
            .flatten()
    })
}

/// A DOI such as "10.1000/xyz123" inside free text.
pub fn find_doi(s: &str) -> Option<String> {
    let start = s.find("10.")?;
    let rest = &s[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>'))
        .unwrap_or(rest.len());
    let doi = rest[..end].trim_end_matches(['.', ',', ';', ')']);
    let (prefix, suffix) = doi.split_once('/')?;
    (prefix.len() >= 7
        && prefix[3..].chars().all(|c| c.is_ascii_digit() || c == '.')
        && !suffix.is_empty())
    .then(|| doi.to_owned())
}

/// A new-style arXiv identifier such as "arXiv:1706.03762v5" inside free
/// text. Returns it without the "arXiv:" prefix or version.
pub fn find_arxiv(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("arxiv") {
        let at = from + i + 5;
        let rest = s[at..].trim_start_matches([':', ' ', '.', '/']);
        let rest = rest.strip_prefix("org/").unwrap_or(rest);
        let rest = ["abs/", "pdf/"]
            .iter()
            .find_map(|p| rest.strip_prefix(p))
            .unwrap_or(rest);
        let id: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let ok = id.len() >= 9
            && id.as_bytes().get(4) == Some(&b'.')
            && id[..4].chars().all(|c| c.is_ascii_digit())
            && id[5..].chars().all(|c| c.is_ascii_digit());
        if ok {
            return Some(id);
        }
        from = at;
    }
    None
}

/// Identifiers found inside a book's pages, for looking its details up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identifiers {
    pub isbn13: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
}

/// Looks for an ISBN, DOI or arXiv id in the first and last pages of a PDF
/// (copyright pages, title pages, headers of papers). Other formats carry
/// identifiers in their metadata, which `extract` already reads.
pub fn find_identifiers(path: &Path, file_type: FileType) -> Identifiers {
    if file_type != FileType::Pdf {
        return Identifiers::default();
    }
    pdf::identifiers(path).unwrap_or_default()
}

/// Reads one ZIP entry, refusing entries larger than [`MAX_ENTRY`].
pub(crate) fn zip_read<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_ENTRY {
        return None;
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_ENTRY).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Natural sort key so "page2.jpg" comes before "page10.jpg".
pub(crate) fn natural_key(s: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut num = String::new();
    let mut text = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_digit() {
            if !text.is_empty() {
                out.push((0, std::mem::take(&mut text)));
            }
            num.push(c);
        } else {
            if !num.is_empty() {
                out.push((num.parse().unwrap_or(u64::MAX), String::new()));
                num.clear();
            }
            text.push(c);
        }
    }
    if !num.is_empty() {
        out.push((num.parse().unwrap_or(u64::MAX), String::new()));
    }
    if !text.is_empty() {
        out.push((0, text));
    }
    out
}

pub(crate) fn is_image_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp"]
        .iter()
        .any(|e| lower.ends_with(e))
}

#[cfg(test)]
pub(crate) mod testutil {
    use std::io::Write;

    /// Builds a ZIP file in memory from `(name, bytes)` pairs.
    pub fn zip_file(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_from_file_names() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir
            .path()
            .join("John Smith & Jane Smith - Quantum Mechanics.txt");
        std::fs::write(&p, "x").unwrap();
        let e = extract(&p, FileType::Txt);
        assert_eq!(e.metadata.title, "Quantum Mechanics");
        assert_eq!(e.metadata.authors, vec!["John Smith", "Jane Smith"]);

        let p = dir.path().join("chapter 1 - intro.txt");
        std::fs::write(&p, "x").unwrap();
        assert_eq!(
            extract(&p, FileType::Txt).metadata.title,
            "chapter 1 - intro"
        );
    }

    #[test]
    fn helpers() {
        assert!(!plausible_title("Microsoft Word - thesis.docx"));
        assert!(!plausible_title("untitled"));
        assert!(plausible_title("Linear Algebra"));
        assert_eq!(find_year("D:20190412"), None);
        assert_eq!(find_year("2019-04-12"), Some(2019));
        assert_eq!(find_year("c. 1998"), Some(1998));
        assert_eq!(
            find_doi("see doi:10.1103/PhysRev.47.777.").as_deref(),
            Some("10.1103/PhysRev.47.777")
        );
        assert_eq!(find_doi("version 10.2"), None);
        let mut names = vec!["p10.jpg", "p2.jpg", "p1.jpg"];
        names.sort_by_key(|n| natural_key(n));
        assert_eq!(names, vec!["p1.jpg", "p2.jpg", "p10.jpg"]);
        assert_eq!(
            split_people("John Smith and Jane Smith; Smith, J."),
            vec!["John Smith", "Jane Smith", "Smith, J."]
        );
    }
}

#[cfg(test)]
mod arxiv_tests {
    use super::find_arxiv;

    #[test]
    fn finds_arxiv_ids() {
        assert_eq!(
            find_arxiv("arXiv:2301.00001v2 [cs.LG]").as_deref(),
            Some("2301.00001")
        );
        assert_eq!(
            find_arxiv("https://arxiv.org/abs/1706.03762").as_deref(),
            Some("1706.03762")
        );
        assert_eq!(
            find_arxiv("arXiv preprint arXiv:1412.6980").as_deref(),
            Some("1412.6980")
        );
        assert_eq!(find_arxiv("the arxiv 12.3"), None);
    }
}
