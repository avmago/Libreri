//! The plain text of a book, in pieces, for the search index.
//!
//! Page-based books (PDF, DjVu) give one piece per page and say which pages
//! have no text (scans that need OCR). Flowing books (EPUB, Kindle, FB2,
//! Markdown, plain text) give one piece per chapter or file, split further
//! when long; `section` is the EPUB spine position so a search result can
//! open the right chapter.

use crate::{epub, mobi, xml, zip_read};
use libreri_core::FileType;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

/// A page or part of a chapter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextChunk {
    /// 1-based page, for page-based books.
    pub page: Option<u32>,
    /// 0-based chapter (EPUB spine item), for flowing books.
    pub section: Option<u32>,
    /// Chapter title, when known.
    pub label: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct BookText {
    pub chunks: Vec<TextChunk>,
    /// Page count for page-based books, 0 otherwise.
    pub pages: u32,
    /// Pages with (almost) no text: candidates for OCR.
    pub empty_pages: Vec<u32>,
}

/// Longest piece stored as one search entry.
const PIECE: usize = 6_000;
/// Most text kept for one book.
const MAX_TEXT: usize = 60_000_000;
/// A page with fewer letters than this counts as having no text.
const MIN_PAGE_LETTERS: usize = 16;

/// Formats whose words can be searched (comics and audio have none).
pub fn has_text(ft: FileType) -> bool {
    matches!(
        ft,
        FileType::Pdf
            | FileType::Epub
            | FileType::Mobi
            | FileType::Azw3
            | FileType::Fb2
            | FileType::Txt
            | FileType::Md
            | FileType::Rtf
            | FileType::Djvu
    )
}

/// Formats whose pages can be read with OCR.
pub fn can_ocr(ft: FileType) -> bool {
    matches!(ft, FileType::Pdf | FileType::Djvu)
}

/// Reads the text of a book.
pub fn book_text(path: &Path, ft: FileType) -> Result<BookText, String> {
    let mut out = match ft {
        FileType::Pdf => paged(crate::pdftext::page_texts(path)?),
        FileType::Djvu => {
            let pages = crate::djvu::info(path)?.sizes.len();
            paged(crate::djvu::page_texts(path, pages)?)
        }
        FileType::Epub => epub_text(path)?,
        FileType::Mobi | FileType::Azw3 => mobi_text(path)?,
        FileType::Fb2 => fb2_text(&read_text_file(path)?),
        FileType::Txt | FileType::Md => {
            let mut b = BookText::default();
            split_into(&mut b, None, None, &read_text_file(path)?);
            b
        }
        FileType::Rtf => {
            let mut b = BookText::default();
            split_into(&mut b, None, None, &crate::rtf::read_markdown(path)?);
            b
        }
        _ => BookText::default(),
    };
    // Keep within bounds on enormous files.
    let mut total = 0;
    out.chunks.retain(|c| {
        total += c.text.len();
        total <= MAX_TEXT
    });
    Ok(out)
}

fn letters(s: &str) -> usize {
    s.chars().filter(|c| c.is_alphanumeric()).count()
}

/// One chunk per page with text.
pub fn paged(pages: Vec<String>) -> BookText {
    let mut out = BookText {
        pages: pages.len() as u32,
        ..Default::default()
    };
    for (i, text) in pages.into_iter().enumerate() {
        let page = i as u32 + 1;
        if letters(&text) < MIN_PAGE_LETTERS {
            out.empty_pages.push(page);
        }
        let text = tidy(&text);
        if !text.is_empty() {
            out.chunks.push(TextChunk {
                page: Some(page),
                text,
                ..Default::default()
            });
        }
    }
    out
}

fn read_text_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(decode_text(&bytes))
}

/// UTF-8 (with or without BOM) or UTF-16 with a BOM.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |be: bool| {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| {
                if be {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Collapses spaces inside lines and keeps at most one blank line.
pub fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        blank = 0;
        out.push_str(&line);
    }
    out
}

/// Adds `text` as one or more chunks of at most about [`PIECE`] bytes,
/// split at paragraph or line ends.
fn split_into(out: &mut BookText, section: Option<u32>, label: Option<String>, text: &str) {
    let text = tidy(text);
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let cut = if rest.len() <= PIECE {
            rest.len()
        } else {
            let mut limit = PIECE;
            while !rest.is_char_boundary(limit) {
                limit -= 1;
            }
            let window = &rest[..limit];
            window
                .rfind("\n\n")
                .or_else(|| window.rfind('\n'))
                .or_else(|| window.rfind(' '))
                .filter(|&i| i > PIECE / 4)
                .unwrap_or(limit)
        };
        let piece = rest[..cut].trim();
        if !piece.is_empty() {
            out.chunks.push(TextChunk {
                page: None,
                section,
                label: label.clone(),
                text: piece.to_owned(),
            });
        }
        rest = &rest[cut..];
    }
}

// ---------- HTML ----------

fn entity(name: &str) -> Option<String> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => num.parse().ok(),
        };
        return code.and_then(char::from_u32).map(String::from);
    }
    Some(
        match name {
            "amp" => "&",
            "lt" => "<",
            "gt" => ">",
            "quot" => "\"",
            "apos" => "'",
            "nbsp" => " ",
            "shy" => "",
            "mdash" => "—",
            "ndash" => "–",
            "hellip" => "…",
            "rsquo" => "’",
            "lsquo" => "‘",
            "rdquo" => "”",
            "ldquo" => "“",
            "laquo" => "«",
            "raquo" => "»",
            "copy" => "©",
            "reg" => "®",
            "trade" => "™",
            "deg" => "°",
            "middot" => "·",
            "bull" => "•",
            "times" => "×",
            "eacute" => "é",
            "egrave" => "è",
            "agrave" => "à",
            "aacute" => "á",
            "ccedil" => "ç",
            "uuml" => "ü",
            "ouml" => "ö",
            "auml" => "ä",
            "szlig" => "ß",
            "ntilde" => "ñ",
            "oacute" => "ó",
            "iacute" => "í",
            "uacute" => "ú",
            _ => return None,
        }
        .to_owned(),
    )
}

const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "tr",
    "section",
    "article",
    "blockquote",
    "pre",
    "dt",
    "dd",
    "table",
    "ul",
    "ol",
    "hr",
    "figure",
    "figcaption",
    "title",
    "subtitle",
    "v",
    "stanza",
    "poem",
    "epigraph",
    "text-author",
    "empty-line",
    "td",
    "th",
    "aside",
    "header",
    "footer",
    "nav",
];

const SKIP_TAGS: &[&str] = &["script", "style", "head", "binary", "svg", "math-alt"];

/// The text of an HTML, XHTML or FB2 fragment: tags removed, entities
/// decoded, block elements on their own lines.
pub fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut skip: Option<String> = None;
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < html.len() {
        let rest = &html[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |e| e + 3);
            continue;
        }
        if let Some(cdata) = rest.strip_prefix("<![CDATA[") {
            let end = cdata.find("]]>").unwrap_or(cdata.len());
            if skip.is_none() {
                out.push_str(&cdata[..end]);
            }
            i += 9 + end + 3.min(cdata.len() - end);
            continue;
        }
        if bytes[i] == b'<' {
            let end = rest.find('>').map_or(rest.len(), |e| e + 1);
            let tag = &rest[1..end.saturating_sub(1).max(1)];
            let closing = tag.starts_with('/');
            let name: String = tag
                .trim_start_matches(['/', '!', '?'])
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
                .collect::<String>()
                .to_ascii_lowercase();
            let name = name.rsplit(':').next().unwrap_or("").to_owned();
            let self_closing = tag.ends_with('/');
            match &skip {
                Some(s) if closing && *s == name => skip = None,
                Some(_) => {}
                None if !closing && !self_closing && SKIP_TAGS.contains(&name.as_str()) => {
                    skip = Some(name)
                }
                None if name == "td" || name == "th" => out.push(' '),
                None if BLOCK_TAGS.contains(&name.as_str()) && !out.ends_with('\n') => {
                    out.push('\n')
                }
                None => {}
            }
            i += end;
            continue;
        }
        if skip.is_some() {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        }
        if bytes[i] == b'&' {
            if let Some(semi) = rest[..rest.len().min(12)].find(';') {
                if let Some(s) = entity(&rest[1..semi]) {
                    out.push_str(&s);
                    i += semi + 1;
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap_or(' ');
        out.push(if ch == '\u{a0}' { ' ' } else { ch });
        i += ch.len_utf8();
    }
    tidy(&out)
}

// ---------- EPUB ----------

fn epub_text(path: &Path) -> Result<BookText, String> {
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("not a valid EPUB ({e})"))?;
    let container =
        zip_read(&mut zip, "META-INF/container.xml").ok_or("META-INF/container.xml is missing")?;
    let container = xml::parse(&String::from_utf8_lossy(&container));
    let opf_path = container
        .all("rootfile")
        .into_iter()
        .find_map(|r| r.attr("full-path"))
        .ok_or("no package document listed")?
        .to_owned();
    let opf = zip_read(&mut zip, &opf_path).ok_or("package document is missing")?;
    let opf = xml::parse(&String::from_utf8_lossy(&opf));
    let base = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let manifest: HashMap<String, (String, Option<String>)> = opf
        .find("manifest")
        .map(|m| {
            m.all("item")
                .into_iter()
                .filter_map(|i| {
                    Some((
                        i.attr("id")?.to_owned(),
                        (
                            epub::join(base, i.attr("href")?),
                            i.attr("properties").map(str::to_owned),
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let spine = opf.find("spine").ok_or("the book has no reading order")?;
    let labels = epub_labels(&mut zip, &manifest, spine.attr("toc"));

    let mut out = BookText::default();
    for (index, item) in spine.all("itemref").into_iter().enumerate() {
        let Some((href, _)) = item.attr("idref").and_then(|id| manifest.get(id)) else {
            continue;
        };
        let Some(bytes) = zip_read(&mut zip, href) else {
            continue;
        };
        let text = html_text(&String::from_utf8_lossy(&bytes));
        split_into(
            &mut out,
            Some(index as u32),
            labels.get(href).cloned(),
            &text,
        );
    }
    Ok(out)
}

/// Chapter titles by file, from the EPUB 3 navigation document or the
/// EPUB 2 NCX.
fn epub_labels(
    zip: &mut zip::ZipArchive<File>,
    manifest: &HashMap<String, (String, Option<String>)>,
    ncx_id: Option<&str>,
) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    fn add(labels: &mut HashMap<String, String>, file: String, label: String) {
        let label = xml::collapse(&label);
        if !label.is_empty() {
            labels.entry(file).or_insert(label);
        }
    }
    let nav = manifest.values().find(|(_, props)| {
        props
            .as_deref()
            .is_some_and(|p| p.split_whitespace().any(|p| p == "nav"))
    });
    if let Some((href, _)) = nav {
        if let Some(bytes) = zip_read(zip, href) {
            let doc = xml::parse(&String::from_utf8_lossy(&bytes));
            let dir = href.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let navs = doc.all("nav");
            let toc = navs
                .iter()
                .find(|n| n.attr("type") == Some("toc"))
                .or(navs.first());
            if let Some(toc) = toc {
                for a in toc.all("a") {
                    if let Some(h) = a.attr("href") {
                        add(&mut labels, epub::join(dir, h), a.text_deep());
                    }
                }
            }
        }
    }
    if labels.is_empty() {
        if let Some((href, _)) = ncx_id.and_then(|id| manifest.get(id)) {
            if let Some(bytes) = zip_read(zip, href) {
                let doc = xml::parse(&String::from_utf8_lossy(&bytes));
                let dir = href.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                for point in doc.all("navpoint") {
                    let label = point.find("navlabel").map(|l| l.text_deep());
                    let src = point.find("content").and_then(|c| c.attr("src"));
                    if let (Some(label), Some(src)) = (label, src) {
                        add(&mut labels, epub::join(dir, src), label);
                    }
                }
            }
        }
    }
    labels
}

// ---------- Kindle ----------

fn mobi_text(path: &Path) -> Result<BookText, String> {
    let markup = mobi::Mobi::open(path)?.markup()?;
    let mut out = BookText::default();
    for part in markup.split("<mbp:pagebreak") {
        let part = part.split_once('>').map_or(
            part,
            |(attrs, rest)| {
                if attrs.len() < 40 {
                    rest
                } else {
                    part
                }
            },
        );
        split_into(&mut out, None, None, &html_text(part));
    }
    Ok(out)
}

// ---------- FB2 ----------

fn fb2_text(src: &str) -> BookText {
    let mut out = BookText::default();
    // Every <body> (the main text, then notes); <description> and <binary>
    // (metadata and pictures) are left out.
    let mut rest = src;
    while let Some(start) = rest.find("<body") {
        let after = &rest[start..];
        let end = after.find("</body>").unwrap_or(after.len());
        let body = &after[..end];
        for part in body.split("<section") {
            let label = part
                .find("<title")
                .and_then(|t| {
                    let tail = &part[t..];
                    tail.find("</title>").map(|e| html_text(&tail[..e]))
                })
                .map(|l| xml::collapse(&l))
                .filter(|l| !l.is_empty());
            let part = part.split_once('>').map_or(part, |(_, r)| r);
            split_into(&mut out, None, label, &html_text(part));
        }
        rest = &after[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_text() {
        let html = "<html><head><title>x</title><style>p{}</style></head><body>\
            <h1>Chapter&nbsp;One</h1><p>The <i>keeper</i> wrote &amp; slept&#8230;</p>\
            <!-- note --><p>Next<br/>line</p><script>var a = '<p>';</script></body></html>";
        assert_eq!(
            html_text(html),
            "Chapter One\nThe keeper wrote & slept…\nNext\nline"
        );
    }

    #[test]
    fn long_text_is_split_at_paragraphs() {
        let para = "word ".repeat(300);
        let text = format!("{para}\n\n{para}\n\n{para}\n\n{para}\n\n{para}");
        let mut b = BookText::default();
        split_into(&mut b, Some(2), Some("Two".into()), &text);
        assert!(b.chunks.len() >= 2);
        assert!(b.chunks.iter().all(|c| c.text.len() <= PIECE));
        assert!(b.chunks.iter().all(|c| c.section == Some(2)));
        let joined: usize = b
            .chunks
            .iter()
            .map(|c| c.text.matches("word").count())
            .sum();
        assert_eq!(joined, 1500);
    }

    #[test]
    fn epub_chapters_with_titles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.epub");
        crate::testutil::zip_file(
            &path,
            &[
                ("mimetype", b"application/epub+zip"),
                (
                    "META-INF/container.xml",
                    br#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
                ),
                (
                    "OEBPS/content.opf",
                    br#"<package><metadata><dc:title>T</dc:title></metadata><manifest>
                    <item id="nav" href="nav.xhtml" properties="nav"/>
                    <item id="c1" href="text/one.xhtml"/><item id="c2" href="text/two.xhtml"/>
                    </manifest><spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#,
                ),
                (
                    "OEBPS/nav.xhtml",
                    br#"<html><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">Arrival</a></li>
                    <li><a href="text/two.xhtml#s">The Storm</a></li></ol></nav></body></html>"#,
                ),
                ("OEBPS/text/one.xhtml", b"<html><body><p>The ship came in.</p></body></html>"),
                ("OEBPS/text/two.xhtml", b"<html><body><p>Waves rose high.</p></body></html>"),
            ],
        );
        let t = book_text(&path, FileType::Epub).unwrap();
        assert_eq!(t.chunks.len(), 2);
        assert_eq!(t.chunks[1].section, Some(1));
        assert_eq!(t.chunks[1].label.as_deref(), Some("The Storm"));
        assert_eq!(t.chunks[1].text, "Waves rose high.");
    }

    #[test]
    fn pdf_pages_and_empty_pages() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.pdf");
        crate::test_text_pdf(&path, &["The lighthouse keeper wrote every night", "", "x"]);
        let t = book_text(&path, FileType::Pdf).unwrap();
        assert_eq!(t.pages, 3);
        assert_eq!(t.empty_pages, vec![2, 3]);
        assert_eq!(t.chunks[0].page, Some(1));
        assert_eq!(t.chunks.len(), 2);
    }

    #[test]
    fn fb2_sections() {
        let src = r#"<FictionBook><description><title-info><book-title>B</book-title></title-info></description>
            <body><section><title><p>First</p></title><p>Alpha <emphasis>beta</emphasis> gamma</p></section>
            <section><title><p>Second</p></title><p>Delta</p></section></body>
            <binary id="c">AAAA</binary></FictionBook>"#;
        let t = fb2_text(src);
        let texts: Vec<_> = t.chunks.iter().map(|c| c.text.as_str()).collect();
        assert!(
            texts.iter().any(|t| t.contains("Alpha beta gamma")),
            "{texts:?}"
        );
        assert_eq!(
            t.chunks
                .iter()
                .find(|c| c.text.contains("Delta"))
                .unwrap()
                .label
                .as_deref(),
            Some("Second")
        );
        assert!(!texts.iter().any(|t| t.contains("AAAA")));
    }

    #[test]
    fn text_files_in_utf16() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "héllo".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_text(&bytes), "héllo");
    }
}
