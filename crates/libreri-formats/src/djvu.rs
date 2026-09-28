//! DjVu, through DjVuLibre's programs (`djvused`, `ddjvu`, `djvutxt`),
//! found or installed by `libreri-helpers`.
//!
//! Pages are rendered to PNM (the caller turns them into JPEG), text comes
//! with word boxes so the reader can select, highlight and find, and the
//! outline becomes the table of contents.

use crate::Extracted;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Raised when DjVuLibre is not installed; the interface offers to install it.
pub const NOT_INSTALLED: &str =
    "DjVu books need DjVuLibre. Install it in Settings › Helpers (Libreri can do it for you).";

fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<Vec<u8>, String> {
    let mut cmd = libreri_helpers::command(program).ok_or(NOT_INSTALLED)?;
    let out = cmd.args(args).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with("***"))
            .or_else(|| err.lines().next())
            .unwrap_or("failed");
        return Err(format!("{program}: {}", line.trim()));
    }
    Ok(out.stdout)
}

// ---------- S-expressions (DjVuLibre's text and outline format) ----------

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Sexp {
    Atom(String),
    Str(String),
    List(Vec<Sexp>),
}

impl Sexp {
    fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::List(l) => Some(l),
            _ => None,
        }
    }

    fn head(&self) -> Option<&str> {
        match self.as_list()?.first()? {
            Sexp::Atom(a) => Some(a),
            _ => None,
        }
    }

    fn num(&self) -> Option<f64> {
        match self {
            Sexp::Atom(a) => a.parse().ok(),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Sexp::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Parses every top-level form. Strings may hold `\"`, `\\`, `\n` and octal
/// escapes (DjVuLibre writes non-ASCII bytes as `\303\237`).
pub(crate) fn parse_sexps(input: &[u8]) -> Vec<Sexp> {
    let mut stack: Vec<Vec<Sexp>> = vec![Vec::new()];
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        match c {
            b'(' => stack.push(Vec::new()),
            b')' => {
                if stack.len() > 1 {
                    let list = stack.pop().unwrap_or_default();
                    if let Some(top) = stack.last_mut() {
                        top.push(Sexp::List(list));
                    }
                }
            }
            b'"' => {
                let mut bytes = Vec::new();
                i += 1;
                while i < input.len() && input[i] != b'"' {
                    if input[i] == b'\\' && i + 1 < input.len() {
                        i += 1;
                        match input[i] {
                            b'n' => bytes.push(b'\n'),
                            b't' => bytes.push(b'\t'),
                            d @ b'0'..=b'7' => {
                                let mut v = u32::from(d - b'0');
                                let mut n = 1;
                                while n < 3
                                    && i + 1 < input.len()
                                    && (b'0'..=b'7').contains(&input[i + 1])
                                {
                                    i += 1;
                                    v = v * 8 + u32::from(input[i] - b'0');
                                    n += 1;
                                }
                                bytes.push(v as u8);
                            }
                            other => bytes.push(other),
                        }
                    } else {
                        bytes.push(input[i]);
                    }
                    i += 1;
                }
                if let Some(top) = stack.last_mut() {
                    top.push(Sexp::Str(String::from_utf8_lossy(&bytes).into_owned()));
                }
            }
            c if c.is_ascii_whitespace() => {}
            _ => {
                let start = i;
                while i < input.len()
                    && !input[i].is_ascii_whitespace()
                    && !matches!(input[i], b'(' | b')' | b'"')
                {
                    i += 1;
                }
                if let Some(top) = stack.last_mut() {
                    top.push(Sexp::Atom(
                        String::from_utf8_lossy(&input[start..i]).into_owned(),
                    ));
                }
                continue;
            }
        }
        i += 1;
    }
    while stack.len() > 1 {
        let list = stack.pop().unwrap_or_default();
        if let Some(top) = stack.last_mut() {
            top.push(Sexp::List(list));
        }
    }
    stack.pop().unwrap_or_default()
}

// ---------- Book information ----------

/// An entry of the table of contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItem {
    pub title: String,
    /// 1-based page, when the link points to one.
    pub page: Option<u32>,
    pub children: Vec<OutlineItem>,
}

/// What the reader needs to lay out a DjVu book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DjvuInfo {
    /// Width and height of every page, in pixels at the page's resolution.
    pub sizes: Vec<(u32, u32)>,
    pub outline: Vec<OutlineItem>,
    pub title: Option<String>,
    pub author: Option<String>,
}

fn outline_items(list: &[Sexp]) -> Vec<OutlineItem> {
    list.iter()
        .filter_map(|e| {
            let l = e.as_list()?;
            let title = l.first()?.text()?.trim().to_owned();
            let target = l.get(1).and_then(Sexp::text).unwrap_or("");
            let page = target.strip_prefix('#').and_then(|p| p.parse().ok());
            Some(OutlineItem {
                title,
                page,
                children: outline_items(l.get(2..).unwrap_or(&[])),
            })
        })
        .collect()
}

/// Page sizes, outline and title/author of a DjVu file.
pub fn info(path: &Path) -> Result<DjvuInfo, String> {
    let p = path.as_os_str();
    let count: u32 = String::from_utf8_lossy(&run("djvused", &[p, "-e".as_ref(), "n".as_ref()])?)
        .trim()
        .parse()
        .map_err(|_| "this DjVu file could not be read".to_owned())?;
    let mut script = String::from("print-outline; print-meta; ");
    for n in 1..=count {
        script.push_str(&format!("select {n}; size; "));
    }
    let out = run(
        "djvused",
        &[p, "-u".as_ref(), "-e".as_ref(), script.as_ref()],
    )?;
    let text = String::from_utf8_lossy(&out);
    let mut sizes = Vec::new();
    let mut title = None;
    let mut author = None;
    let mut outline_src = String::new();
    let mut depth = 0i32;
    for line in text.lines() {
        if depth > 0 || line.trim_start().starts_with("(bookmarks") {
            depth += line.matches('(').count() as i32 - line.matches(')').count() as i32;
            outline_src.push_str(line);
            outline_src.push('\n');
            continue;
        }
        if let Some(rest) = line.strip_prefix("width=") {
            let (w, h) = rest.split_once(" height=").unwrap_or((rest, "0"));
            let h = h.split_whitespace().next().unwrap_or("0");
            sizes.push((w.trim().parse().unwrap_or(0), h.parse().unwrap_or(0)));
        } else if let Some((key, value)) = line.split_once('\t') {
            let value = value.trim().trim_matches('"').replace("\\\"", "\"");
            match key.trim() {
                "title" if !value.is_empty() => title = Some(value),
                "author" if !value.is_empty() => author = Some(value),
                _ => {}
            }
        }
    }
    let outline = parse_sexps(outline_src.as_bytes())
        .first()
        .and_then(Sexp::as_list)
        .map(|l| outline_items(l.get(1..).unwrap_or(&[])))
        .unwrap_or_default();
    Ok(DjvuInfo {
        sizes,
        outline,
        title,
        author,
    })
}

/// Details and cover for the library.
pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let i = info(path)?;
    let m = &mut out.metadata;
    if !i.sizes.is_empty() {
        m.pages = Some(i.sizes.len() as u32);
    }
    if let Some(t) = i.title {
        m.title = t;
    }
    if let Some(a) = i.author {
        m.authors = crate::split_people(&a);
    }
    out.cover = render_page(path, 1, 600).ok();
    Ok(())
}

/// Renders a page (1-based) to fit `width` pixels. Returns PNM bytes.
pub fn render_page(path: &Path, page: u32, width: u32) -> Result<Vec<u8>, String> {
    let tmp = std::env::temp_dir().join(format!(
        "libreri-djvu-{}-{}-{page}.pnm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let width = width.clamp(64, 6000);
    let result = run(
        "ddjvu",
        &[
            "-format=pnm".as_ref(),
            format!("-page={page}").as_ref(),
            format!("-size={width}x{}", width * 8).as_ref(),
            path.as_os_str(),
            tmp.as_os_str(),
        ],
    )
    .and_then(|_| std::fs::read(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    result
}

/// A word on a page and where it is, as fractions of the page from the top
/// left (the same rectangles PDF highlights use).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub text: String,
    pub rect: [f64; 4],
}

/// The words of one page, in reading order.
pub fn page_words(path: &Path, page: u32) -> Result<Vec<Word>, String> {
    let out = run(
        "djvutxt",
        &[
            format!("--page={page}").as_ref(),
            "--detail=word".as_ref(),
            path.as_os_str(),
        ],
    )?;
    Ok(words_from(&parse_sexps(&out)))
}

/// The words of every page (`pages` of them), in one run of djvutxt.
pub fn all_page_words(path: &Path, pages: usize) -> Result<Vec<Vec<Word>>, String> {
    let out = run("djvutxt", &["--detail=word".as_ref(), path.as_os_str()])?;
    let forms = parse_sexps(&out);
    let per_page: Vec<Vec<Word>> = forms
        .iter()
        .filter(|f| f.head() == Some("page"))
        .map(|f| words_from(std::slice::from_ref(f)))
        .collect();
    if per_page.len() == pages {
        return Ok(per_page);
    }
    // Some files leave out pages without text: ask page by page.
    (1..=pages as u32).map(|p| page_words(path, p)).collect()
}

fn words_from(forms: &[Sexp]) -> Vec<Word> {
    let mut words = Vec::new();
    for form in forms {
        let Some(l) = form.as_list() else { continue };
        if form.head() != Some("page") || l.len() < 5 {
            continue;
        }
        let w = l[3].num().unwrap_or(0.0).max(1.0);
        let h = l[4].num().unwrap_or(0.0).max(1.0);
        collect_words(form, w, h, &mut words);
    }
    words
}

fn collect_words(e: &Sexp, pw: f64, ph: f64, out: &mut Vec<Word>) {
    let Some(l) = e.as_list() else { return };
    if e.head() == Some("word") && l.len() >= 6 {
        let n = |i: usize| l[i].num().unwrap_or(0.0);
        let (x0, y0, x1, y1) = (n(1), n(2), n(3), n(4));
        if let Some(t) = l[5].text() {
            if !t.trim().is_empty() {
                out.push(Word {
                    text: t.to_owned(),
                    rect: [
                        (x0 / pw).clamp(0.0, 1.0),
                        (1.0 - y1 / ph).clamp(0.0, 1.0),
                        ((x1 - x0) / pw).clamp(0.0, 1.0),
                        ((y1 - y0) / ph).clamp(0.0, 1.0),
                    ],
                });
            }
        }
        return;
    }
    for child in l.iter().skip(5) {
        collect_words(child, pw, ph, out);
    }
}

/// The plain text of every page (empty for pages without a text layer).
pub fn page_texts(path: &Path, pages: usize) -> Result<Vec<String>, String> {
    let out = run("djvutxt", &[path.as_os_str()])?;
    let text = String::from_utf8_lossy(&out);
    let mut list: Vec<String> = text.split('\u{c}').map(|s| s.trim().to_owned()).collect();
    // A form feed ends every page, so there is one piece too many.
    list.resize(pages, String::new());
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libreri_core::FileType;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.djvu")
    }

    fn have_djvulibre() -> bool {
        let ok = libreri_helpers::status(libreri_helpers::Helper::DjVuLibre).installed;
        if !ok {
            eprintln!("DjVuLibre is not installed; skipping");
        }
        ok
    }

    #[test]
    fn parses_sexps_with_escapes() {
        let s = parse_sexps(
            br#"(page 0 0 400 600 (word 40 520 90 545 "Stra\303\237e") (word 1 2 3 4 "a\"b"))"#,
        );
        let words = words_from(&s);
        assert_eq!(words[0].text, "Straße");
        assert_eq!(words[1].text, "a\"b");
        let r = words[0].rect;
        assert!((r[0] - 0.1).abs() < 1e-9);
        assert!((r[1] - (1.0 - 545.0 / 600.0)).abs() < 1e-9);
    }

    #[test]
    fn reads_a_djvu_book() {
        if !have_djvulibre() {
            return;
        }
        let p = fixture();
        let i = info(&p).unwrap();
        assert_eq!(i.sizes, [(400, 600), (400, 600)]);
        assert_eq!(i.title.as_deref(), Some("Optics of DjVu"));
        assert_eq!(i.outline.len(), 2);
        assert_eq!(i.outline[1].page, Some(2));
        assert_eq!(i.outline[1].children[0].title, "Section");

        let e = crate::extract(&p, FileType::Djvu);
        assert_eq!(e.metadata.title, "Optics of DjVu");
        assert_eq!(e.metadata.authors, ["Jane Smith"]);
        assert_eq!(e.metadata.pages, Some(2));
        assert!(e.cover.as_deref().is_some_and(|c| c.starts_with(b"P6")));

        let pnm = render_page(&p, 2, 200).unwrap();
        assert!(pnm.starts_with(b"P6\n200 300"));
        let words = page_words(&p, 2).unwrap();
        assert_eq!(
            words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
            ["Page", "2", "Straße"]
        );
        let texts = page_texts(&p, 2).unwrap();
        assert_eq!(texts.len(), 2);
        assert!(texts[0].contains("light bends"));
    }
}
