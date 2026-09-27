//! Tidying what sources send: subject lists become a few useful tags,
//! subject paths become categories, dates become years, three-letter
//! language codes become two-letter ones.

use libreri_core::{BookMetadata, ContentType};
use libreri_formats::xml::{collapse, strip_html};

/// Most tags kept from one source.
pub const MAX_TAGS: usize = 12;

/// Subjects that say how a library holds the book, not what it is about.
const NOT_SUBJECTS: &[&str] = &[
    "accessible book",
    "protected daisy",
    "in library",
    "lending library",
    "large type books",
    "large print books",
    "internet archive wishlist",
    "overdrive",
    "open library staff picks",
    "long now manual for civilization",
    "reading level",
    "general",
    "textbooks",
    "electronic books",
    "ebooks",
    "juvenile literature",
    "popular works",
    "history and criticism",
    "fiction, general",
];

/// Papers, as opposed to books, for choosing sources.
pub fn is_paper(t: ContentType) -> bool {
    matches!(
        t,
        ContentType::ResearchPaper
            | ContentType::ConferencePaper
            | ContentType::Preprint
            | ContentType::Thesis
            | ContentType::TechnicalReport
            | ContentType::WhitePaper
            | ContentType::Standard
            | ContentType::Article
    )
}

/// Turns a source's subject headings into tags: "Physics -- Textbooks"
/// gives "Physics"; headings about the edition or the lending library are
/// dropped; duplicates are merged; at most [`MAX_TAGS`].
pub fn tags(subjects: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in subjects {
        for part in s.split("--").flat_map(|p| p.split(" / ")) {
            let t = collapse(part.trim().trim_end_matches(['.', ',']));
            let lower = t.to_lowercase();
            let useless = t.is_empty()
                || t.chars().count() > 40
                || t.contains(':')
                || t.contains('=')
                || t.chars()
                    .all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
                || NOT_SUBJECTS
                    .iter()
                    .any(|n| lower == *n || lower.starts_with(&format!("{n},")))
                || lower.starts_with("nyt")
                || lower.starts_with("accessible book");
            if useless || out.iter().any(|o| o.to_lowercase() == lower) {
                continue;
            }
            out.push(t);
            if out.len() == MAX_TAGS {
                return out;
            }
        }
    }
    out
}

/// "Science / Physics / General" becomes "Science/Physics".
pub fn category_path(heading: &str) -> Option<String> {
    let parts: Vec<String> = heading
        .split('/')
        .map(|p| collapse(p.trim()))
        .filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case("general"))
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The first four-digit year in a date ("2004-05-01", "May 2004", "c1999").
pub fn year(date: &str) -> Option<i32> {
    let b = date.as_bytes();
    (0..b.len().saturating_sub(3)).find_map(|i| {
        let w = &b[i..i + 4];
        let boundary = (i == 0 || !b[i - 1].is_ascii_digit())
            && b.get(i + 4).is_none_or(|c| !c.is_ascii_digit());
        (boundary && w.iter().all(u8::is_ascii_digit))
            .then(|| std::str::from_utf8(w).ok()?.parse().ok())
            .flatten()
            .filter(|y: &i32| (1000..=2100).contains(y))
    })
}

/// A language as a two-letter code where one exists ("eng" → "en").
pub fn language(code: &str) -> Option<String> {
    let c = code.trim().trim_start_matches("/languages/").to_lowercase();
    // "en_US" and "en-GB" are kept as the language alone.
    let c = c.split(['_', '-']).next().unwrap_or_default().to_owned();
    let two = match c.as_str() {
        "" | "und" | "mul" | "zxx" => return None,
        "eng" => "en",
        "fre" | "fra" => "fr",
        "ger" | "deu" => "de",
        "spa" => "es",
        "ita" => "it",
        "por" => "pt",
        "dut" | "nld" => "nl",
        "rus" => "ru",
        "jpn" => "ja",
        "chi" | "zho" => "zh",
        "kor" => "ko",
        "ara" => "ar",
        "hin" => "hi",
        "ben" => "bn",
        "urd" => "ur",
        "pan" => "pa",
        "tam" => "ta",
        "tel" => "te",
        "mar" => "mr",
        "guj" => "gu",
        "san" => "sa",
        "per" | "fas" => "fa",
        "tur" => "tr",
        "gre" | "ell" => "el",
        "heb" => "he",
        "pol" => "pl",
        "swe" => "sv",
        "nor" | "nob" => "no",
        "dan" => "da",
        "fin" => "fi",
        "cze" | "ces" => "cs",
        "hun" => "hu",
        "rum" | "ron" => "ro",
        "ukr" => "uk",
        "lat" => "la",
        "cat" => "ca",
        "vie" => "vi",
        "tha" => "th",
        "ind" => "id",
        "may" | "msa" => "ms",
        other => other,
    };
    Some(two.to_owned())
}

/// Plain text from a description that may hold HTML (or JATS, in Crossref
/// abstracts). Very short descriptions are dropped.
pub fn about(text: &str) -> Option<String> {
    let text = text.replace("<jats:", "<").replace("</jats:", "</");
    let plain = strip_html(&text);
    let plain = plain.trim();
    // "Abstract" as a first line adds nothing.
    let plain = plain
        .strip_prefix("Abstract")
        .map(str::trim_start)
        .unwrap_or(plain);
    (plain.chars().count() >= 20).then(|| plain.to_owned())
}

/// Final tidying of any candidate: whitespace, ISBNs, empty strings.
pub fn tidy(m: &mut BookMetadata) {
    let clean = |s: &mut Option<String>| {
        if let Some(v) = s {
            let c = collapse(v);
            *s = (!c.is_empty()).then_some(c);
        }
    };
    m.title = collapse(&m.title);
    clean(&mut m.subtitle);
    clean(&mut m.publisher);
    clean(&mut m.journal);
    clean(&mut m.series);
    clean(&mut m.edition);
    clean(&mut m.volume);
    clean(&mut m.issue);
    clean(&mut m.language);
    if let Some(a) = &m.about {
        m.about = Some(a.trim().to_owned()).filter(|a| !a.is_empty());
    }
    for list in [&mut m.authors, &mut m.contributors] {
        let mut seen: Vec<String> = Vec::new();
        list.retain_mut(|a| {
            *a = collapse(a);
            let dup = a.is_empty() || seen.iter().any(|s| s.eq_ignore_ascii_case(a));
            seen.push(a.clone());
            !dup
        });
    }
    // A subtitle repeated in the title ("Title: Subtitle").
    if let Some(sub) = &m.subtitle {
        let tail = format!(": {sub}");
        if m.title.ends_with(&tail) {
            let cut = m.title.len() - tail.len();
            m.title.truncate(cut);
        }
    }
    m.isbn13 = m
        .isbn13
        .as_deref()
        .and_then(|i| libreri_core::isbn::normalize_isbn13(i).ok());
    m.isbn10 = m
        .isbn10
        .as_deref()
        .and_then(|i| libreri_core::isbn::normalize_isbn10(i).ok());
    if m.isbn13.is_none() {
        m.isbn13 = m
            .isbn10
            .as_deref()
            .and_then(libreri_core::isbn::isbn10_to_13);
    }
    if m.isbn10.is_none() {
        m.isbn10 = m
            .isbn13
            .as_deref()
            .and_then(libreri_core::isbn::isbn13_to_10);
    }
    if m.pages == Some(0) {
        m.pages = None;
    }
}

/// "Smith, John" becomes "John Smith".
pub fn person(name: &str) -> String {
    match name.split_once(", ") {
        Some((last, first)) if !first.contains(',') && !first.is_empty() => {
            format!("{first} {last}")
        }
        _ => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subjects_become_tags() {
        let t = tags(
            [
                "Accessible book",
                "Protected DAISY",
                "Physics -- Textbooks",
                "physics",
                "Quantum theory.",
                "nyt:hardcover-nonfiction=2019-01-01",
                "1939-1945",
                "In library",
                "Fiction, general",
            ]
            .map(String::from),
        );
        assert_eq!(t, ["Physics", "Quantum theory"]);
        let many = tags((0..30).map(|i| format!("Topic {i}")));
        assert_eq!(many.len(), MAX_TAGS);
    }

    #[test]
    fn headings_become_categories() {
        assert_eq!(
            category_path("Science / Physics / General").as_deref(),
            Some("Science/Physics")
        );
        assert_eq!(category_path("General"), None);
        assert_eq!(category_path("Computers").as_deref(), Some("Computers"));
    }

    #[test]
    fn reads_years_languages_and_names() {
        assert_eq!(year("2004-05-01"), Some(2004));
        assert_eq!(year("May 12, 1999"), Some(1999));
        assert_eq!(year("c1987."), Some(1987));
        assert_eq!(year("12345"), None);
        assert_eq!(year(""), None);
        assert_eq!(language("eng").as_deref(), Some("en"));
        assert_eq!(language("/languages/ger").as_deref(), Some("de"));
        assert_eq!(language("en").as_deref(), Some("en"));
        assert_eq!(language("und"), None);
        assert_eq!(language("en_US").as_deref(), Some("en"));
        assert_eq!(person("Smith, John"), "John Smith");
        assert_eq!(person("John Smith"), "John Smith");
    }

    #[test]
    fn tidies_candidates() {
        let mut m = BookMetadata {
            title: "Linear  Algebra: Done Right".into(),
            subtitle: Some("Done Right".into()),
            authors: vec!["John Smith".into(), "john smith".into(), " ".into()],
            isbn10: Some("0-306-40615-2".into()),
            publisher: Some("  ".into()),
            pages: Some(0),
            ..Default::default()
        };
        tidy(&mut m);
        assert_eq!(m.title, "Linear Algebra");
        assert_eq!(m.authors, ["John Smith"]);
        assert_eq!(m.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(m.isbn10.as_deref(), Some("0306406152"));
        assert_eq!(m.publisher, None);
        assert_eq!(m.pages, None);
        assert_eq!(
            about(
                "<jats:title>Abstract</jats:title><jats:p>We study the thing in detail.</jats:p>"
            )
            .as_deref(),
            Some("We study the thing in detail.")
        );
    }
}
