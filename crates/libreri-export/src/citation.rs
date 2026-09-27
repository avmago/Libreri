//! Citations: BibTeX, RIS and CSL-JSON for reference managers, and ready
//! formatted references (APA, MLA, Chicago, Harvard, IEEE) to paste.
//!
//! The formatted styles follow each guide's reference-list form for the
//! common cases (book, journal article, conference paper, thesis, report,
//! preprint). Titles are used as typed: Libreri does not change their case.

use crate::full_title;
use crate::names::{split_name, PersonName};
use libreri_core::{Book, ContentType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

/// Reference styles offered by "Copy citation".
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CitationStyle {
    #[default]
    Apa,
    Mla,
    Chicago,
    Harvard,
    Ieee,
    /// A BibTeX entry, for pasting into a `.bib` file.
    Bibtex,
}

/// A formatted reference as plain text and as HTML (with italics).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    pub text: String,
    pub html: String,
}

/// What kind of work a book is, for citing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Book,
    Article,
    Conference,
    Thesis,
    Report,
    Preprint,
    Other,
}

fn has(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn kind(book: &Book) -> Kind {
    let m = &book.metadata;
    match m.content_type {
        ContentType::Book
        | ContentType::Textbook
        | ContentType::Manual
        | ContentType::Reference
        | ContentType::Comic
        | ContentType::Audiobook => Kind::Book,
        ContentType::ResearchPaper | ContentType::Article | ContentType::Magazine => {
            if has(&m.journal).is_some() || has(&m.doi).is_some() {
                Kind::Article
            } else if has(&m.arxiv_id).is_some() {
                Kind::Preprint
            } else {
                Kind::Other
            }
        }
        ContentType::ConferencePaper => Kind::Conference,
        ContentType::Preprint => {
            if has(&m.journal).is_some() {
                Kind::Article
            } else {
                Kind::Preprint
            }
        }
        ContentType::Thesis => Kind::Thesis,
        ContentType::TechnicalReport | ContentType::WhitePaper | ContentType::Standard => {
            Kind::Report
        }
        _ => {
            if has(&m.isbn13).is_some() || has(&m.isbn10).is_some() {
                Kind::Book
            } else {
                Kind::Other
            }
        }
    }
}

fn authors(book: &Book) -> Vec<PersonName> {
    book.metadata
        .authors
        .iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .map(split_name)
        .collect()
}

/// "2nd" for "2", "2nd edition" for itself; `None` for a first edition.
fn edition(book: &Book) -> Option<String> {
    let e = has(&book.metadata.edition)?;
    let digits: String = e.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() == e.len() {
        let n: u32 = digits.parse().ok()?;
        if n <= 1 {
            return None;
        }
        let suffix = match (n % 10, n % 100) {
            (1, x) if x != 11 => "st",
            (2, x) if x != 12 => "nd",
            (3, x) if x != 13 => "rd",
            _ => "th",
        };
        return Some(format!("{n}{suffix}"));
    }
    let lower = e.to_lowercase();
    let e = lower
        .trim_end_matches("edition")
        .trim_end_matches("ed.")
        .trim()
        .to_owned();
    (!e.is_empty() && e != "1st" && e != "first").then_some(e)
}

/// The best link: the DOI, then the book's URL, then its arXiv page.
fn link(book: &Book) -> Option<String> {
    let m = &book.metadata;
    if let Some(doi) = has(&m.doi) {
        return Some(format!("https://doi.org/{doi}"));
    }
    if let Some(url) = has(&m.url) {
        return Some(url.to_owned());
    }
    has(&m.arxiv_id).map(|id| format!("https://arxiv.org/abs/{id}"))
}

fn year(book: &Book) -> Option<String> {
    book.metadata.year.map(|y| y.to_string())
}

/// Text and HTML built side by side.
#[derive(Default)]
struct Out {
    text: String,
    html: String,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Out {
    fn push(&mut self, s: &str) -> &mut Self {
        self.text.push_str(s);
        self.html.push_str(&escape_html(s));
        self
    }

    fn italic(&mut self, s: &str) -> &mut Self {
        self.text.push_str(s);
        self.html.push_str("<i>");
        self.html.push_str(&escape_html(s));
        self.html.push_str("</i>");
        self
    }

    /// Adds a full stop unless the text already ends with one (or ? !).
    fn stop(&mut self) -> &mut Self {
        if !self.text.trim_end().ends_with(['.', '?', '!']) {
            self.push(".");
        }
        self
    }

    fn done(self) -> Citation {
        Citation {
            text: self.text.trim().to_owned(),
            html: self.html.trim().to_owned(),
        }
    }
}

/// `"Title."` with the stop inside the quotes (unless the title ends in ? or !).
fn quoted(title: &str, punct: &str) -> String {
    if title.ends_with(['?', '!']) {
        format!("\u{201c}{title}\u{201d}")
    } else {
        format!("\u{201c}{title}{punct}\u{201d}")
    }
}

fn join_list(items: &[String], sep: &str, last: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] if !last.starts_with(',') => format!("{a}{last}{b}"),
        [a, b] => format!("{a}{}{b}", last.trim_start_matches(',')),
        _ => {
            let (end, rest) = items.split_last().expect("not empty");
            format!("{}{last}{end}", rest.join(sep))
        }
    }
}

// ---------- formatted styles ----------

fn apa_name(p: &PersonName) -> String {
    match p {
        PersonName::Person { family, .. } => format!("{family}, {}", p.initials()),
        PersonName::Literal(s) => s.clone(),
    }
}

fn apa_authors(names: &[PersonName]) -> String {
    let list: Vec<String> = names.iter().map(apa_name).collect();
    if list.len() > 20 {
        let mut head = list[..19].join(", ");
        head.push_str(", . . . ");
        head.push_str(list.last().expect("more than 20"));
        return head;
    }
    // APA keeps the comma before "&" even with two authors.
    match list.as_slice() {
        [a, b] => format!("{a}, & {b}"),
        _ => join_list(&list, ", ", ", & "),
    }
}

fn apa(book: &Book) -> Citation {
    let m = &book.metadata;
    let names = authors(book);
    let title = full_title(book);
    let date = format!("({}).", year(book).unwrap_or_else(|| "n.d.".into()));
    let mut o = Out::default();
    if names.is_empty() {
        o.italic(&title).stop().push(" ").push(&date);
    } else {
        o.push(&apa_authors(&names))
            .stop()
            .push(" ")
            .push(&date)
            .push(" ");
    }
    let italic_title = |o: &mut Out| {
        if !names.is_empty() {
            o.italic(&title);
        }
    };
    match kind(book) {
        Kind::Article => {
            if !names.is_empty() {
                o.push(&title).stop();
            }
            if let Some(j) = has(&m.journal) {
                o.push(" ").italic(j);
                if let Some(v) = has(&m.volume) {
                    o.push(", ").italic(v);
                }
                if let Some(i) = has(&m.issue) {
                    o.push(&format!("({i})"));
                }
                o.stop();
            }
        }
        Kind::Conference => {
            if !names.is_empty() {
                o.push(&title).stop();
            }
            if let Some(j) = has(&m.journal) {
                o.push(" In ").italic(j);
                if let Some(v) = has(&m.volume) {
                    o.push(&format!(" (Vol. {v})"));
                }
                o.stop();
            }
            if let Some(p) = has(&m.publisher) {
                o.push(" ").push(p).stop();
            }
        }
        Kind::Preprint => {
            italic_title(&mut o);
            if let Some(id) = has(&m.arxiv_id) {
                o.push(&format!(" (arXiv:{id})"));
            }
            o.stop().push(" arXiv.");
        }
        Kind::Thesis => {
            italic_title(&mut o);
            match has(&m.publisher) {
                Some(p) => o.push(&format!(" [Thesis, {p}]")),
                None => o.push(" [Thesis]"),
            };
            o.stop();
        }
        Kind::Book | Kind::Report | Kind::Other => {
            italic_title(&mut o);
            if let Some(e) = edition(book) {
                o.push(&format!(" ({e} ed.)"));
            }
            o.stop();
            if let Some(p) = has(&m.publisher) {
                o.push(" ").push(p).stop();
            }
        }
    }
    if let Some(l) = link(book) {
        o.push(" ").push(&l);
    }
    o.done()
}

fn mla_authors(names: &[PersonName]) -> String {
    match names {
        [] => String::new(),
        [one] => one.inverted(),
        [a, b] => format!("{}, and {}", a.inverted(), b.natural()),
        [a, ..] => format!("{}, et al", a.inverted()),
    }
}

fn mla(book: &Book) -> Citation {
    let m = &book.metadata;
    let names = authors(book);
    let title = full_title(book);
    let mut o = Out::default();
    if !names.is_empty() {
        o.push(&mla_authors(&names)).stop().push(" ");
    }
    let tail = |o: &mut Out, parts: Vec<String>| {
        let parts: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
        if !parts.is_empty() {
            o.push(" ").push(&parts.join(", ")).stop();
        }
    };
    match kind(book) {
        Kind::Article | Kind::Conference | Kind::Preprint => {
            o.push(&quoted(&title, "."));
            let container = has(&m.journal).or_else(|| has(&m.arxiv_id).map(|_| "arXiv"));
            let mut parts = Vec::new();
            if let Some(j) = container {
                o.push(" ").italic(j);
                parts.push(String::new());
            }
            if let Some(v) = has(&m.volume) {
                parts.push(format!("vol. {v}"));
            }
            if let Some(i) = has(&m.issue) {
                parts.push(format!("no. {i}"));
            }
            parts.extend(year(book));
            parts.extend(link(book));
            let parts: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
            if !parts.is_empty() {
                o.push(if container.is_some() { ", " } else { " " })
                    .push(&parts.join(", "));
            }
            o.stop();
        }
        Kind::Thesis => {
            o.italic(&title).stop();
            let mut parts = year(book).into_iter().collect::<Vec<_>>();
            parts.push(match has(&m.publisher) {
                Some(p) => format!("{p}, Thesis"),
                None => "Thesis".into(),
            });
            tail(&mut o, parts);
        }
        Kind::Book | Kind::Report | Kind::Other => {
            o.italic(&title).stop();
            let mut parts = Vec::new();
            parts.extend(edition(book).map(|e| format!("{e} ed.")));
            parts.extend(has(&m.publisher).map(str::to_owned));
            parts.extend(year(book));
            tail(&mut o, parts);
        }
    }
    o.done()
}

fn chicago_authors(names: &[PersonName]) -> String {
    let shown: Vec<String> = if names.len() > 10 {
        names[..7]
            .iter()
            .enumerate()
            .map(|(i, n)| if i == 0 { n.inverted() } else { n.natural() })
            .collect()
    } else {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| if i == 0 { n.inverted() } else { n.natural() })
            .collect()
    };
    if names.len() > 10 {
        return format!("{}, et al", shown.join(", "));
    }
    // The first name is inverted, so a comma always follows it.
    match shown.as_slice() {
        [a, b] => format!("{a}, and {b}"),
        _ => join_list(&shown, ", ", ", and "),
    }
}

fn chicago(book: &Book) -> Citation {
    let m = &book.metadata;
    let names = authors(book);
    let title = full_title(book);
    let mut o = Out::default();
    if !names.is_empty() {
        o.push(&chicago_authors(&names)).stop().push(" ");
    }
    match kind(book) {
        Kind::Article => {
            o.push(&quoted(&title, "."));
            if let Some(j) = has(&m.journal) {
                o.push(" ").italic(j);
            }
            if let Some(v) = has(&m.volume) {
                o.push(" ").push(v);
            }
            if let Some(i) = has(&m.issue) {
                o.push(&format!(", no. {i}"));
            }
            if let Some(y) = year(book) {
                o.push(&format!(" ({y})"));
            }
            o.stop();
        }
        Kind::Conference => {
            o.push(&quoted(&title, "."));
            if let Some(j) = has(&m.journal) {
                o.push(" In ").italic(j);
            }
            let parts: Vec<String> = has(&m.publisher)
                .map(str::to_owned)
                .into_iter()
                .chain(year(book))
                .collect();
            if !parts.is_empty() {
                o.push(", ").push(&parts.join(", "));
            }
            o.stop();
        }
        Kind::Preprint => {
            o.push(&quoted(&title, "."));
            o.push(" arXiv");
            if let Some(y) = year(book) {
                o.push(&format!(", {y}"));
            }
            o.stop();
        }
        Kind::Thesis => {
            o.push(&quoted(&title, "."));
            let mut parts = vec!["Thesis".to_owned()];
            parts.extend(has(&m.publisher).map(str::to_owned));
            parts.extend(year(book));
            o.push(" ").push(&parts.join(", ")).stop();
        }
        Kind::Book | Kind::Report | Kind::Other => {
            o.italic(&title).stop();
            if let Some(e) = edition(book) {
                o.push(&format!(" {e} ed."));
            }
            let parts: Vec<String> = has(&m.publisher)
                .map(str::to_owned)
                .into_iter()
                .chain(year(book))
                .collect();
            if !parts.is_empty() {
                o.push(" ").push(&parts.join(", ")).stop();
            }
        }
    }
    if let Some(l) = link(book) {
        o.push(" ").push(&l).stop();
    }
    o.done()
}

fn harvard_authors(names: &[PersonName]) -> String {
    let list: Vec<String> = names
        .iter()
        .map(|p| match p {
            PersonName::Person { family, .. } => format!("{family}, {}", p.initials()),
            PersonName::Literal(s) => s.clone(),
        })
        .collect();
    if list.len() >= 4 {
        return format!("{} et al.", list[0]);
    }
    join_list(&list, ", ", " and ")
}

fn harvard(book: &Book) -> Citation {
    let m = &book.metadata;
    let names = authors(book);
    let title = full_title(book);
    let date = format!("({})", year(book).unwrap_or_else(|| "no date".into()));
    let mut o = Out::default();
    if !names.is_empty() {
        o.push(&harvard_authors(&names)).push(" ");
    }
    o.push(&date).push(" ");
    match kind(book) {
        Kind::Article | Kind::Conference | Kind::Preprint => {
            o.push(&format!("\u{2018}{title}\u{2019}"));
            let container = has(&m.journal);
            match (kind(book), container) {
                (Kind::Conference, Some(j)) => {
                    o.push(", in ").italic(j);
                }
                (_, Some(j)) => {
                    o.push(", ").italic(j);
                    if let Some(v) = has(&m.volume) {
                        o.push(&format!(", {v}"));
                        if let Some(i) = has(&m.issue) {
                            o.push(&format!("({i})"));
                        }
                    }
                }
                (_, None) if has(&m.arxiv_id).is_some() => {
                    o.push(". arXiv");
                }
                _ => {}
            }
            o.stop();
        }
        Kind::Thesis => {
            o.italic(&title).push(". Thesis");
            if let Some(p) = has(&m.publisher) {
                o.push(". ").push(p);
            }
            o.stop();
        }
        Kind::Book | Kind::Report | Kind::Other => {
            o.italic(&title).stop();
            if let Some(e) = edition(book) {
                o.push(&format!(" {e} edn."));
            }
            if let Some(p) = has(&m.publisher) {
                o.push(" ").push(p).stop();
            }
        }
    }
    if let Some(l) = link(book) {
        o.push(&format!(" Available at: {l}")).stop();
    }
    o.done()
}

fn ieee_authors(names: &[PersonName]) -> String {
    let list: Vec<String> = names
        .iter()
        .map(|p| match p {
            PersonName::Person { family, .. } => format!("{} {family}", p.initials()),
            PersonName::Literal(s) => s.clone(),
        })
        .collect();
    if list.len() > 6 {
        return format!("{} et al.", list[0]);
    }
    join_list(&list, ", ", ", and ")
}

fn ieee(book: &Book) -> Citation {
    let m = &book.metadata;
    let names = authors(book);
    let title = full_title(book);
    let mut o = Out::default();
    if !names.is_empty() {
        o.push(&ieee_authors(&names)).push(", ");
    }
    let doi = has(&m.doi);
    match kind(book) {
        Kind::Article | Kind::Conference => {
            o.push(&quoted(&title, ","));
            let mut parts: Vec<String> = Vec::new();
            if let Some(j) = has(&m.journal) {
                o.push(if kind(book) == Kind::Conference {
                    " in "
                } else {
                    " "
                })
                .italic(j);
                parts.push(String::new());
            }
            parts.extend(has(&m.volume).map(|v| format!("vol. {v}")));
            parts.extend(has(&m.issue).map(|i| format!("no. {i}")));
            parts.extend(year(book));
            parts.extend(doi.map(|d| format!("doi: {d}")));
            let parts: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
            if !parts.is_empty() {
                o.push(if has(&m.journal).is_some() { ", " } else { " " })
                    .push(&parts.join(", "));
            }
            o.stop();
        }
        Kind::Preprint => {
            o.push(&quoted(&title, ","));
            let mut parts: Vec<String> = year(book).into_iter().collect();
            parts.extend(has(&m.arxiv_id).map(|id| format!("arXiv:{id}")));
            o.push(" ").push(&parts.join(", ")).stop();
        }
        Kind::Thesis | Kind::Report => {
            o.push(&quoted(&title, ","));
            let mut parts = Vec::new();
            parts.extend(has(&m.publisher).map(str::to_owned));
            parts.push(if kind(book) == Kind::Thesis {
                "Thesis".to_owned()
            } else {
                "Tech. Rep.".to_owned()
            });
            parts.extend(year(book));
            o.push(" ").push(&parts.join(", ")).stop();
        }
        Kind::Book | Kind::Other => {
            o.italic(&title);
            if let Some(e) = edition(book) {
                o.push(&format!(", {e} ed"));
            }
            o.stop();
            let parts: Vec<String> = has(&m.publisher)
                .map(str::to_owned)
                .into_iter()
                .chain(year(book))
                .collect();
            if !parts.is_empty() {
                o.push(" ").push(&parts.join(", ")).stop();
            }
        }
    }
    o.done()
}

/// One formatted reference.
pub fn format(book: &Book, style: CitationStyle) -> Citation {
    match style {
        CitationStyle::Apa => apa(book),
        CitationStyle::Mla => mla(book),
        CitationStyle::Chicago => chicago(book),
        CitationStyle::Harvard => harvard(book),
        CitationStyle::Ieee => ieee(book),
        CitationStyle::Bibtex => {
            let text = bibtex(std::slice::from_ref(book));
            Citation {
                html: format!("<pre>{}</pre>", escape_html(&text)),
                text: text.trim_end().to_owned(),
            }
        }
    }
}

/// A reference list: sorted by author and title (IEEE keeps the given order
/// and numbers the entries).
pub fn format_list(books: &[Book], style: CitationStyle) -> Citation {
    if style == CitationStyle::Bibtex {
        let text = bibtex(books);
        return Citation {
            html: format!("<pre>{}</pre>", escape_html(&text)),
            text: text.trim_end().to_owned(),
        };
    }
    let mut refs: Vec<(String, Citation)> = books
        .iter()
        .map(|b| (format(b, style).text.to_lowercase(), format(b, style)))
        .collect();
    if style != CitationStyle::Ieee {
        refs.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let mut text = Vec::new();
    let mut html = Vec::new();
    for (i, (_, c)) in refs.into_iter().enumerate() {
        if style == CitationStyle::Ieee {
            text.push(format!("[{}] {}", i + 1, c.text));
            html.push(format!("<p>[{}] {}</p>", i + 1, c.html));
        } else {
            text.push(c.text);
            html.push(format!("<p>{}</p>", c.html));
        }
    }
    Citation {
        text: text.join("\n"),
        html: html.join("\n"),
    }
}

// ---------- BibTeX ----------

fn bib_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' | '}' | '&' | '%' | '$' | '#' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// "smith2019optics": first author's family name, year, first title word.
fn bib_key(book: &Book) -> String {
    let ascii = |s: &str| -> String {
        s.chars()
            .filter_map(|c| {
                let c = match c {
                    'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
                    'è' | 'é' | 'ê' | 'ë' => 'e',
                    'ì' | 'í' | 'î' | 'ï' => 'i',
                    'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
                    'ù' | 'ú' | 'û' | 'ü' => 'u',
                    'ç' => 'c',
                    'ñ' => 'n',
                    'ß' => 's',
                    c => c,
                };
                c.is_ascii_alphanumeric().then(|| c.to_ascii_lowercase())
            })
            .collect()
    };
    let author = authors(book)
        .first()
        .map(|p| ascii(p.family().rsplit(' ').next().unwrap_or("")))
        .unwrap_or_default();
    const SKIP: &[&str] = &["a", "an", "the", "on", "of", "and", "in", "to", "for"];
    let word = book
        .metadata
        .title
        .split_whitespace()
        .map(ascii)
        .find(|w| !w.is_empty() && !SKIP.contains(&w.as_str()))
        .unwrap_or_default();
    let year = book
        .metadata
        .year
        .map(|y| y.to_string())
        .unwrap_or_default();
    let key = format!("{author}{year}{word}");
    if key.is_empty() {
        format!("libreri{}", &book.id.as_str()[..8])
    } else {
        key
    }
}

fn bib_type(book: &Book) -> &'static str {
    match kind(book) {
        Kind::Book => "book",
        Kind::Article => "article",
        Kind::Conference => "inproceedings",
        Kind::Thesis => "phdthesis",
        Kind::Report => "techreport",
        Kind::Preprint | Kind::Other => "misc",
    }
}

/// BibTeX entries for `books`, with unique keys (smith2019optics,
/// smith2019opticsb…).
pub fn bibtex(books: &[Book]) -> String {
    let mut taken = HashSet::new();
    let mut out = String::new();
    for book in books {
        let base = bib_key(book);
        let mut key = base.clone();
        let mut n = 0u8;
        while !taken.insert(key.clone()) {
            n += 1;
            key = format!("{base}{}", (b'a' + n) as char);
        }
        let m = &book.metadata;
        let k = kind(book);
        let mut fields: Vec<(&str, String)> = Vec::new();
        let names: Vec<String> = authors(book)
            .iter()
            .map(|p| match p {
                PersonName::Person { .. } => p.inverted(),
                PersonName::Literal(s) => format!("{{{s}}}"),
            })
            .collect();
        if !names.is_empty() {
            fields.push(("author", names.join(" and ")));
        }
        fields.push(("title", full_title(book)));
        match k {
            Kind::Article => fields.extend(has(&m.journal).map(|j| ("journal", j.to_owned()))),
            Kind::Conference => fields.extend(has(&m.journal).map(|j| ("booktitle", j.to_owned()))),
            _ => {}
        }
        fields.extend(m.year.map(|y| ("year", y.to_string())));
        fields.extend(has(&m.volume).map(|v| ("volume", v.to_owned())));
        fields.extend(has(&m.issue).map(|i| ("number", i.to_owned())));
        match k {
            Kind::Thesis => fields.extend(has(&m.publisher).map(|p| ("school", p.to_owned()))),
            Kind::Report => fields.extend(has(&m.publisher).map(|p| ("institution", p.to_owned()))),
            _ => fields.extend(has(&m.publisher).map(|p| ("publisher", p.to_owned()))),
        }
        fields.extend(has(&m.edition).map(|e| ("edition", e.to_owned())));
        if let Some(s) = has(&m.series) {
            fields.push(("series", s.to_owned()));
            if k == Kind::Book && has(&m.volume).is_none() {
                fields.extend(m.series_number.map(|n| ("volume", trim_number(n))));
            }
        }
        fields.extend(
            has(&m.isbn13)
                .or(has(&m.isbn10))
                .map(|i| ("isbn", i.to_owned())),
        );
        fields.extend(has(&m.doi).map(|d| ("doi", d.to_owned())));
        if let Some(id) = has(&m.arxiv_id) {
            fields.push(("eprint", id.to_owned()));
            fields.push(("archiveprefix", "arXiv".to_owned()));
        }
        fields.extend(has(&m.url).map(|u| ("url", u.to_owned())));
        fields.extend(has(&m.language).map(|l| ("language", l.to_owned())));
        if !m.tags.is_empty() {
            fields.push(("keywords", m.tags.join(", ")));
        }
        fields.extend(has(&m.about).map(|a| ("abstract", a.to_owned())));

        out.push_str(&format!("@{}{{{key},\n", bib_type(book)));
        for (name, value) in &fields {
            let value = match *name {
                // Already braced for organisations; keep as given.
                "author" => value.replace('&', "\\&"),
                "url" | "doi" => value.replace(['{', '}'], ""),
                _ => bib_escape(value),
            };
            out.push_str(&format!("  {name} = {{{value}}},\n"));
        }
        out.push_str("}\n\n");
    }
    out
}

fn trim_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

// ---------- RIS ----------

/// RIS records for `books` (Zotero, Mendeley, EndNote).
pub fn ris(books: &[Book]) -> String {
    let mut out = String::new();
    for book in books {
        let m = &book.metadata;
        let k = kind(book);
        let ty = match k {
            Kind::Book => "BOOK",
            Kind::Article => "JOUR",
            Kind::Conference => "CPAPER",
            Kind::Thesis => "THES",
            Kind::Report => "RPRT",
            Kind::Preprint => "ELEC",
            Kind::Other => "GEN",
        };
        let mut line = |tag: &str, value: &str| {
            let value = value.replace(['\r', '\n'], " ");
            if !value.trim().is_empty() {
                out.push_str(&format!("{tag}  - {}\r\n", value.trim()));
            }
        };
        line("TY", ty);
        for p in authors(book) {
            line("AU", &p.inverted());
        }
        line("TI", &full_title(book));
        if let Some(j) = has(&m.journal) {
            line("T2", j);
        }
        if let Some(y) = m.year {
            line("PY", &y.to_string());
        }
        for (tag, v) in [
            ("VL", &m.volume),
            ("IS", &m.issue),
            ("PB", &m.publisher),
            ("ET", &m.edition),
            ("DO", &m.doi),
            ("UR", &m.url),
            ("LA", &m.language),
            ("T3", &m.series),
        ] {
            if let Some(v) = has(v) {
                line(tag, v);
            }
        }
        if let Some(isbn) = has(&m.isbn13).or(has(&m.isbn10)) {
            line("SN", isbn);
        }
        if let Some(id) = has(&m.arxiv_id) {
            line("M1", &format!("arXiv:{id}"));
        }
        for t in &m.tags {
            line("KW", t);
        }
        if let Some(a) = has(&m.about) {
            line("AB", a);
        }
        out.push_str("ER  - \r\n\r\n");
    }
    out
}

// ---------- CSL-JSON ----------

/// One CSL-JSON item (the format Zotero, Pandoc and citeproc read).
pub fn csl_item(book: &Book) -> Value {
    let m = &book.metadata;
    let ty = match kind(book) {
        Kind::Book => "book",
        Kind::Article => "article-journal",
        Kind::Conference => "paper-conference",
        Kind::Thesis => "thesis",
        Kind::Report => "report",
        Kind::Preprint => "article",
        Kind::Other => "document",
    };
    let mut o = Map::new();
    o.insert("id".into(), json!(bib_key(book)));
    o.insert("type".into(), json!(ty));
    o.insert("title".into(), json!(full_title(book)));
    let names: Vec<Value> = authors(book)
        .into_iter()
        .map(|p| match p {
            PersonName::Person { family, given } => json!({ "family": family, "given": given }),
            PersonName::Literal(s) => json!({ "literal": s }),
        })
        .collect();
    if !names.is_empty() {
        o.insert("author".into(), Value::Array(names));
    }
    if let Some(y) = m.year {
        o.insert("issued".into(), json!({ "date-parts": [[y]] }));
    }
    let mut put = |key: &str, v: Option<&str>| {
        if let Some(v) = v {
            o.insert(key.into(), json!(v));
        }
    };
    put("container-title", has(&m.journal));
    put("publisher", has(&m.publisher));
    put("volume", has(&m.volume));
    put("issue", has(&m.issue));
    put("edition", has(&m.edition));
    put("DOI", has(&m.doi));
    put("URL", has(&m.url));
    put("ISBN", has(&m.isbn13).or(has(&m.isbn10)));
    put("language", has(&m.language));
    put("collection-title", has(&m.series));
    put("abstract", has(&m.about));
    if let Some(id) = has(&m.arxiv_id) {
        o.insert("number".into(), json!(format!("arXiv:{id}")));
    }
    if let Some(n) = m.series_number {
        o.insert("collection-number".into(), json!(trim_number(n)));
    }
    if let Some(p) = m.pages {
        o.insert("number-of-pages".into(), json!(p.to_string()));
    }
    if !m.tags.is_empty() {
        o.insert("keyword".into(), json!(m.tags.join(", ")));
    }
    Value::Object(o)
}

/// A CSL-JSON array with unique ids.
pub fn csl_json(books: &[Book]) -> String {
    let mut taken = HashSet::new();
    let items: Vec<Value> = books
        .iter()
        .map(|b| {
            let mut item = csl_item(b);
            let base = item["id"].as_str().unwrap_or("item").to_owned();
            let mut key = base.clone();
            let mut n = 0u8;
            while !taken.insert(key.clone()) {
                n += 1;
                key = format!("{base}{}", (b'a' + n) as char);
            }
            item["id"] = json!(key);
            item
        })
        .collect();
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{book, paper};

    #[test]
    fn formats_a_book_in_every_style() {
        let b = book("Optics");
        assert_eq!(
            format(&b, CitationStyle::Apa).text,
            "Smith, J. Q., & van Beethoven, L. (2019). Optics (2nd ed.). Acme Press."
        );
        assert_eq!(
            format(&b, CitationStyle::Apa).html,
            "Smith, J. Q., &amp; van Beethoven, L. (2019). <i>Optics</i> (2nd ed.). Acme Press."
        );
        assert_eq!(
            format(&b, CitationStyle::Mla).text,
            "Smith, Jane Q., and Ludwig van Beethoven. Optics. 2nd ed., Acme Press, 2019."
        );
        assert_eq!(
            format(&b, CitationStyle::Chicago).text,
            "Smith, Jane Q., and Ludwig van Beethoven. Optics. 2nd ed. Acme Press, 2019."
        );
        assert_eq!(
            format(&b, CitationStyle::Harvard).text,
            "Smith, J. Q. and van Beethoven, L. (2019) Optics. 2nd edn. Acme Press."
        );
        assert_eq!(
            format(&b, CitationStyle::Ieee).text,
            "J. Q. Smith and L. van Beethoven, Optics, 2nd ed. Acme Press, 2019."
        );
    }

    #[test]
    fn formats_a_conference_paper() {
        let p = paper();
        assert_eq!(
            format(&p, CitationStyle::Apa).text,
            "Vaswani, A., Shazeer, N., & Parmar, N. (2017). Attention Is All You Need. In Advances in Neural Information Processing Systems (Vol. 30). https://doi.org/10.48550/arXiv.1706.03762"
        );
        assert_eq!(
            format(&p, CitationStyle::Mla).text,
            "Vaswani, Ashish, et al. \u{201c}Attention Is All You Need.\u{201d} Advances in Neural Information Processing Systems, vol. 30, 2017, https://doi.org/10.48550/arXiv.1706.03762."
        );
        assert_eq!(
            format(&p, CitationStyle::Ieee).text,
            "A. Vaswani, N. Shazeer, and N. Parmar, \u{201c}Attention Is All You Need,\u{201d} in Advances in Neural Information Processing Systems, vol. 30, 2017, doi: 10.48550/arXiv.1706.03762."
        );
        let list = format_list(&[p.clone(), book("Optics")], CitationStyle::Ieee);
        assert!(list.text.starts_with("[1] A. Vaswani"));
        let list = format_list(&[p, book("Optics")], CitationStyle::Apa);
        assert!(list.text.starts_with("Smith"));
    }

    #[test]
    fn writes_bibtex_ris_and_csl() {
        let mut b = book("Optics & Lasers");
        b.metadata.about = Some("50% off {sale}".into());
        let bib = bibtex(&[b.clone(), b.clone()]);
        assert!(bib.contains("@book{smith2019optics,"));
        assert!(bib.contains("@book{smith2019opticsb,"));
        assert!(bib.contains("author = {Smith, Jane Q. and van Beethoven, Ludwig}"));
        assert!(bib.contains("title = {Optics \\& Lasers}"));
        assert!(bib.contains("abstract = {50\\% off \\{sale\\}}"));
        assert!(bib.contains("isbn = {9780131103627}"));

        let bib = bibtex(&[paper()]);
        assert!(bib.contains("@inproceedings{vaswani2017attention,"));
        assert!(bib.contains("booktitle = {Advances in Neural"));
        assert!(bib.contains("eprint = {1706.03762}"));

        let r = ris(&[b.clone()]);
        assert!(r.starts_with("TY  - BOOK\r\nAU  - Smith, Jane Q.\r\n"));
        assert!(r.contains("SN  - 9780131103627\r\n"));
        assert!(r.contains("KW  - Light & waves\r\n"));
        assert!(r.ends_with("ER  - \r\n\r\n"));

        let csl: Value = serde_json::from_str(&csl_json(&[b, paper()])).unwrap();
        assert_eq!(csl[0]["type"], "book");
        assert_eq!(csl[0]["author"][1]["family"], "van Beethoven");
        assert_eq!(csl[0]["issued"]["date-parts"][0][0], 2019);
        assert_eq!(csl[1]["type"], "paper-conference");
        assert_eq!(
            csl[1]["container-title"],
            "Advances in Neural Information Processing Systems"
        );
    }

    #[test]
    fn handles_missing_details() {
        let mut b = book("Notes?");
        b.metadata.authors.clear();
        b.metadata.year = None;
        b.metadata.publisher = None;
        b.metadata.edition = None;
        assert_eq!(format(&b, CitationStyle::Apa).text, "Notes? (n.d.).");
        assert_eq!(format(&b, CitationStyle::Mla).text, "Notes?");
        assert_eq!(format(&b, CitationStyle::Harvard).text, "(no date) Notes?");
    }
}
