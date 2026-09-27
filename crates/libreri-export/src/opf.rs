//! Calibre's `metadata.opf` (OPF 2.0), written next to each book so
//! Calibre's "Add books from folders" picks up the details.

use crate::names::{split_name, PersonName};
use libreri_core::Book;

fn esc(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A stable UUID made from the book's content hash.
pub fn book_uuid(book: &Book) -> String {
    let h = book.id.as_str();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// The OPF document. `with_rating` adds the reader's rating (Calibre uses
/// 0–10); `cover` names the cover file next to it, if any.
pub fn opf(book: &Book, with_rating: bool, cover: Option<&str>) -> String {
    let m = &book.metadata;
    let mut x = String::from(
        "<?xml version='1.0' encoding='utf-8'?>\n\
         <package xmlns=\"http://www.idpf.org/2007/opf\" unique-identifier=\"uuid_id\" version=\"2.0\">\n\
         \x20 <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:opf=\"http://www.idpf.org/2007/opf\">\n",
    );
    let mut line = |s: String| {
        x.push_str("    ");
        x.push_str(&s);
        x.push('\n');
    };
    line(format!(
        "<dc:identifier opf:scheme=\"uuid\" id=\"uuid_id\">{}</dc:identifier>",
        book_uuid(book)
    ));
    line(format!(
        "<dc:title>{}</dc:title>",
        esc(&crate::full_title(book))
    ));
    for a in &m.authors {
        let file_as = match split_name(a) {
            p @ PersonName::Person { .. } => p.inverted(),
            PersonName::Literal(s) => s,
        };
        line(format!(
            "<dc:creator opf:file-as=\"{}\" opf:role=\"aut\">{}</dc:creator>",
            esc(&file_as),
            esc(a)
        ));
    }
    for c in &m.contributors {
        line(format!("<dc:contributor>{}</dc:contributor>", esc(c)));
    }
    if let Some(y) = m.year {
        line(format!("<dc:date>{y:04}-01-01T00:00:00+00:00</dc:date>"));
    }
    let opt = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(esc)
    };
    if let Some(p) = opt(&m.publisher) {
        line(format!("<dc:publisher>{p}</dc:publisher>"));
    }
    if let Some(a) = opt(&m.about) {
        line(format!("<dc:description>{a}</dc:description>"));
    }
    if let Some(l) = opt(&m.language) {
        line(format!("<dc:language>{l}</dc:language>"));
    }
    if let Some(i) = opt(&m.isbn13).or_else(|| opt(&m.isbn10)) {
        line(format!(
            "<dc:identifier opf:scheme=\"ISBN\">{i}</dc:identifier>"
        ));
    }
    if let Some(d) = opt(&m.doi) {
        line(format!(
            "<dc:identifier opf:scheme=\"DOI\">{d}</dc:identifier>"
        ));
    }
    if let Some(a) = opt(&m.arxiv_id) {
        line(format!(
            "<dc:identifier opf:scheme=\"ARXIV\">{a}</dc:identifier>"
        ));
    }
    for t in &m.tags {
        line(format!("<dc:subject>{}</dc:subject>", esc(t)));
    }
    if let Some(s) = opt(&m.series) {
        line(format!("<meta name=\"calibre:series\" content=\"{s}\"/>"));
        let n = m.series_number.unwrap_or(1.0);
        line(format!(
            "<meta name=\"calibre:series_index\" content=\"{n}\"/>"
        ));
    }
    if with_rating && book.user.rating > 0 {
        line(format!(
            "<meta name=\"calibre:rating\" content=\"{}\"/>",
            u32::from(book.user.rating.min(5)) * 2
        ));
    }
    line(format!(
        "<meta name=\"calibre:timestamp\" content=\"{}\"/>",
        esc(&book.added_at)
    ));
    x.push_str("  </metadata>\n");
    if let Some(cover) = cover {
        x.push_str(&format!(
            "  <guide>\n    <reference type=\"cover\" title=\"Cover\" href=\"{}\"/>\n  </guide>\n",
            esc(cover)
        ));
    }
    x.push_str("</package>\n");
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::book;

    #[test]
    fn writes_calibre_metadata() {
        let mut b = book("Optics <2nd>");
        b.metadata.series = Some("Physics".into());
        b.metadata.series_number = Some(3.0);
        let x = opf(&b, true, Some("cover.jpg"));
        assert!(x.contains("<dc:title>Optics &lt;2nd&gt;</dc:title>"));
        assert!(x.contains(
            "<dc:creator opf:file-as=\"van Beethoven, Ludwig\" opf:role=\"aut\">Ludwig van Beethoven</dc:creator>"
        ));
        assert!(x.contains("<dc:identifier opf:scheme=\"ISBN\">9780131103627</dc:identifier>"));
        assert!(x.contains("<dc:subject>Light &amp; waves</dc:subject>"));
        assert!(x.contains("calibre:series_index\" content=\"3\""));
        assert!(x.contains("calibre:rating\" content=\"8\""));
        assert!(x.contains("href=\"cover.jpg\""));
        assert!(x.contains("<dc:date>2019-01-01T00:00:00+00:00</dc:date>"));
        assert_eq!(book_uuid(&b).len(), 36);
        assert!(!opf(&b, false, None).contains("rating"));
    }
}
