//! Looking up book details online.
//!
//! Each source (Open Library, Google Books, Crossref, OpenAlex, Semantic
//! Scholar, arXiv, ComicVine, ISBNdb) turns a [`Query`] into
//! [`Candidate`]s: the details it knows, where it found them and how well
//! they match. Nothing is saved here; the library shows the candidates and
//! the reader picks, field by field, what to keep (see
//! docs/adr/0013-online-details.md).
//!
//! Requests carry no personal data: only the identifier or the title and
//! author being looked up, and a User-Agent naming the app.

mod covers;
mod http;
pub mod lookup;
pub mod normalise;
mod providers;

pub use covers::{cover_allowed, fetch_cover, Cover};
pub use http::{FixtureHttp, Http, Response, UreqHttp};

use libreri_core::{BookMetadata, ContentType};
use serde::{Deserialize, Serialize};

/// A place book details come from.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    OpenLibrary,
    GoogleBooks,
    Crossref,
    OpenAlex,
    SemanticScholar,
    Arxiv,
    ComicVine,
    Isbndb,
}

impl Source {
    pub const ALL: [Source; 8] = [
        Source::OpenLibrary,
        Source::GoogleBooks,
        Source::Crossref,
        Source::OpenAlex,
        Source::SemanticScholar,
        Source::Arxiv,
        Source::ComicVine,
        Source::Isbndb,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Source::OpenLibrary => "Open Library",
            Source::GoogleBooks => "Google Books",
            Source::Crossref => "Crossref",
            Source::OpenAlex => "OpenAlex",
            Source::SemanticScholar => "Semantic Scholar",
            Source::Arxiv => "arXiv",
            Source::ComicVine => "ComicVine",
            Source::Isbndb => "ISBNdb",
        }
    }

    /// Sources that only work with the reader's own API key.
    pub fn needs_key(self) -> bool {
        matches!(self, Source::ComicVine | Source::Isbndb)
    }

    /// Sources used unless the reader turns them off. Keyed sources start
    /// off; they are turned on by entering a key.
    pub fn on_by_default(self) -> bool {
        !self.needs_key()
    }
}

/// What to look up. Identifiers are tried first; the title and author are
/// used when there are none (or they find nothing).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    pub isbn: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    /// What kind of item it is, if known, so a comic goes to ComicVine and
    /// a paper to Crossref.
    pub content_type: Option<ContentType>,
}

impl Query {
    /// A query for a book's current details.
    pub fn for_book(m: &BookMetadata) -> Self {
        Self {
            isbn: m.isbn13.clone().or_else(|| m.isbn10.clone()),
            doi: m.doi.clone(),
            arxiv_id: m.arxiv_id.clone(),
            title: Some(m.title.clone()).filter(|t| !t.trim().is_empty()),
            author: m.authors.first().cloned(),
            content_type: Some(m.content_type),
        }
    }

    /// The ISBN-13, if the query holds a valid ISBN.
    pub fn isbn13(&self) -> Option<String> {
        let raw = self.isbn.as_deref()?;
        libreri_core::isbn::normalize_isbn13(raw)
            .ok()
            .or_else(|| libreri_core::isbn::isbn10_to_13(raw))
    }

    pub fn doi(&self) -> Option<String> {
        self.doi.as_deref().and_then(libreri_formats::find_doi)
    }

    pub fn arxiv(&self) -> Option<String> {
        let raw = self.arxiv_id.as_deref()?;
        libreri_formats::find_arxiv(&format!("arXiv:{}", raw.trim_start_matches("arXiv:")))
    }

    fn title(&self) -> Option<&str> {
        self.title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
    }

    pub fn is_empty(&self) -> bool {
        self.isbn13().is_none()
            && self.doi().is_none()
            && self.arxiv().is_none()
            && self.title().is_none()
    }
}

/// Which sources to use and the reader's keys. Saved per computer, never
/// exported (docs/data-portability.md).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub enabled: Vec<Source>,
    pub comicvine_key: Option<String>,
    pub isbndb_key: Option<String>,
    /// Podcast Index (podcast search, trending, categories): the reader's
    /// own free key and secret. Not a book-details source.
    pub podcastindex_key: Option<String>,
    pub podcastindex_secret: Option<String>,
    /// Fill in missing details of newly imported books (off unless the
    /// reader turns it on: it sends titles to the sources).
    pub fill_on_import: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: Source::ALL
                .into_iter()
                .filter(|s| s.on_by_default())
                .collect(),
            comicvine_key: None,
            isbndb_key: None,
            podcastindex_key: None,
            podcastindex_secret: None,
            fill_on_import: false,
        }
    }
}

impl Settings {
    fn key(&self, s: Source) -> Option<&str> {
        let k = match s {
            Source::ComicVine => self.comicvine_key.as_deref(),
            Source::Isbndb => self.isbndb_key.as_deref(),
            _ => return Some(""),
        };
        k.map(str::trim).filter(|k| !k.is_empty())
    }

    /// True if the source is turned on and, when it needs one, has a key.
    pub fn usable(&self, s: Source) -> bool {
        self.enabled.contains(&s) && self.key(s).is_some()
    }
}

/// One source's idea of what the book is.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub source: Source,
    /// The source's own id ("OL7353617M", "10.1038/nature14539").
    pub source_id: String,
    /// The page about it on the source's website.
    pub link: Option<String>,
    pub metadata: BookMetadata,
    pub cover_url: Option<String>,
    /// How well it matches the query, 0–1. An identifier match is 1.
    pub score: f32,
}

/// A source that could not be reached or answered with an error.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceError {
    pub source: Source,
    pub message: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    /// Best first.
    pub candidates: Vec<Candidate>,
    pub errors: Vec<SourceError>,
}

impl Lookup {
    /// The best candidate if it is a sure match (an identifier, or a close
    /// title and author), for filling details without asking.
    pub fn sure_match(&self) -> Option<&Candidate> {
        self.candidates.first().filter(|c| c.score >= SURE)
    }
}

/// Score above which a candidate is taken without asking.
pub const SURE: f32 = 0.85;

/// Most candidates kept from one source.
const PER_SOURCE: usize = 5;

type Job<'a> = (
    Source,
    Box<dyn FnOnce() -> Result<Vec<Candidate>, String> + Send + 'a>,
);

/// Asks every usable source about `q` at once and gathers what they say.
pub fn lookup(q: &Query, settings: &Settings, http: &dyn Http) -> Lookup {
    let by_id = run(plan_by_id(q, settings, http));
    let mut out = by_id;
    // Nothing found by identifier (or none given): search by title.
    if out.candidates.is_empty() && q.title().is_some() {
        let more = run(plan_by_title(q, settings, http));
        out.candidates = more.candidates;
        out.errors.extend(more.errors);
    }
    for c in &mut out.candidates {
        if c.score < 1.0 {
            c.score = match_score(q, &c.metadata);
        }
    }
    out.candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| richness(&b.metadata).cmp(&richness(&a.metadata)))
    });
    out
}

fn plan_by_id<'a>(q: &'a Query, s: &'a Settings, http: &'a dyn Http) -> Vec<Job<'a>> {
    use providers::{arxiv, crossref, google, isbndb, openalex, openlibrary, semantic};
    let mut jobs: Vec<Job<'a>> = Vec::new();
    let mut add =
        |src: Source, f: Box<dyn FnOnce() -> Result<Vec<Candidate>, String> + Send + 'a>| {
            if s.usable(src) {
                jobs.push((src, f));
            }
        };
    if let Some(isbn) = q.isbn13() {
        let i = isbn.clone();
        add(
            Source::OpenLibrary,
            Box::new(move || openlibrary::by_isbn(http, &i)),
        );
        let i = isbn.clone();
        add(
            Source::GoogleBooks,
            Box::new(move || google::by_isbn(http, &i)),
        );
        let key = s.key(Source::Isbndb).unwrap_or_default();
        add(
            Source::Isbndb,
            Box::new(move || isbndb::by_isbn(http, key, &isbn)),
        );
    }
    if let Some(doi) = q.doi() {
        let d = doi.clone();
        add(
            Source::Crossref,
            Box::new(move || crossref::by_doi(http, &d)),
        );
        let d = doi.clone();
        add(
            Source::OpenAlex,
            Box::new(move || openalex::by_doi(http, &d)),
        );
        add(
            Source::SemanticScholar,
            Box::new(move || semantic::by_id(http, &format!("DOI:{doi}"))),
        );
    }
    if let Some(id) = q.arxiv() {
        let a = id.clone();
        add(Source::Arxiv, Box::new(move || arxiv::by_id(http, &a)));
        if q.doi().is_none() {
            add(
                Source::SemanticScholar,
                Box::new(move || semantic::by_id(http, &format!("ARXIV:{id}"))),
            );
        }
    }
    jobs
}

fn plan_by_title<'a>(q: &'a Query, s: &'a Settings, http: &'a dyn Http) -> Vec<Job<'a>> {
    use providers::{arxiv, comicvine, crossref, google, isbndb, openalex, openlibrary};
    let Some(title) = q.title() else {
        return Vec::new();
    };
    let author = q.author.as_deref().map(str::trim).filter(|a| !a.is_empty());
    let kind = q.content_type;
    let paper = kind.is_some_and(normalise::is_paper);
    let comic = kind == Some(ContentType::Comic);
    let mut jobs: Vec<Job<'a>> = Vec::new();
    let mut add =
        |src: Source, f: Box<dyn FnOnce() -> Result<Vec<Candidate>, String> + Send + 'a>| {
            if s.usable(src) {
                jobs.push((src, f));
            }
        };
    if comic {
        let key = s.key(Source::ComicVine).unwrap_or_default();
        add(
            Source::ComicVine,
            Box::new(move || comicvine::search(http, key, title)),
        );
    }
    if !paper {
        add(
            Source::OpenLibrary,
            Box::new(move || openlibrary::search(http, title, author)),
        );
        add(
            Source::GoogleBooks,
            Box::new(move || google::search(http, title, author)),
        );
        let key = s.key(Source::Isbndb).unwrap_or_default();
        add(
            Source::Isbndb,
            Box::new(move || isbndb::search(http, key, title)),
        );
    }
    if paper || kind.is_none() {
        add(
            Source::Crossref,
            Box::new(move || crossref::search(http, title, author)),
        );
        add(
            Source::OpenAlex,
            Box::new(move || openalex::search(http, title)),
        );
    }
    if matches!(kind, Some(ContentType::Preprint)) {
        add(
            Source::Arxiv,
            Box::new(move || arxiv::search(http, title, author)),
        );
    }
    jobs
}

fn run(jobs: Vec<Job<'_>>) -> Lookup {
    let results: Vec<(Source, Result<Vec<Candidate>, String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .into_iter()
            .map(|(src, job)| (src, scope.spawn(job)))
            .collect();
        handles
            .into_iter()
            .map(|(src, h)| {
                (
                    src,
                    h.join()
                        .unwrap_or_else(|_| Err("the answer could not be read".into())),
                )
            })
            .collect()
    });
    let mut out = Lookup::default();
    for (source, r) in results {
        match r {
            Ok(mut list) => {
                list.truncate(PER_SOURCE);
                for c in &mut list {
                    normalise::tidy(&mut c.metadata);
                }
                list.retain(|c| !c.metadata.title.is_empty());
                out.candidates.extend(list);
            }
            Err(message) => out.errors.push(SourceError { source, message }),
        }
    }
    out
}

/// How many fields a candidate fills; ties are broken by this.
fn richness(m: &BookMetadata) -> usize {
    [
        !m.authors.is_empty(),
        m.subtitle.is_some(),
        m.about.is_some(),
        m.year.is_some(),
        m.publisher.is_some(),
        m.pages.is_some(),
        m.isbn13.is_some(),
        m.language.is_some(),
        !m.tags.is_empty(),
        !m.categories.is_empty(),
        m.series.is_some(),
        m.doi.is_some(),
        m.journal.is_some(),
    ]
    .iter()
    .filter(|b| **b)
    .count()
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| !matches!(w.as_str(), "the" | "a" | "an" | "of" | "and" | "to" | "in"))
        .collect()
}

/// Share of the query's title words found in the candidate's title, and
/// the other way round, averaged.
fn title_similarity(a: &str, b: &str) -> f32 {
    let (wa, wb) = (words(a), words(b));
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    let hit_a = wa.iter().filter(|w| wb.contains(w)).count() as f32 / wa.len() as f32;
    let hit_b = wb.iter().filter(|w| wa.contains(w)).count() as f32 / wb.len() as f32;
    (hit_a + hit_b) / 2.0
}

/// How well a candidate found by title matches the query, 0–1 (below 1:
/// only an identifier match is certain).
pub fn match_score(q: &Query, m: &BookMetadata) -> f32 {
    let Some(title) = q.title() else {
        return 0.5;
    };
    // A candidate's subtitle often sits in the query's title.
    let full = match &m.subtitle {
        Some(s) => format!("{} {s}", m.title),
        None => m.title.clone(),
    };
    let t = title_similarity(title, &m.title).max(title_similarity(title, &full));
    let a = match q.author.as_deref().map(words) {
        None => 0.6,
        Some(q_words) if q_words.is_empty() => 0.6,
        Some(q_words) => {
            let names: Vec<String> = m.authors.iter().flat_map(|a| words(a)).collect();
            if names.is_empty() {
                0.3
            } else if q_words.iter().any(|w| w.len() > 1 && names.contains(w)) {
                1.0
            } else {
                0.0
            }
        }
    };
    ((t * 0.7 + a * 0.3) * 0.99).min(0.99)
}

/// Fills the empty fields of `m` from `from`. Returns the names of the
/// fields changed. Used for "fill in missing details" in bulk and on import;
/// nothing the reader typed is replaced.
pub fn fill_empty(m: &mut BookMetadata, from: &BookMetadata) -> Vec<&'static str> {
    let mut changed = Vec::new();
    macro_rules! opt {
        ($($f:ident),*) => {$(
            if m.$f.is_none() && from.$f.is_some() {
                m.$f = from.$f.clone();
                changed.push(stringify!($f));
            }
        )*};
    }
    macro_rules! list {
        ($($f:ident),*) => {$(
            if m.$f.is_empty() && !from.$f.is_empty() {
                m.$f = from.$f.clone();
                changed.push(stringify!($f));
            }
        )*};
    }
    opt!(
        subtitle,
        about,
        year,
        publisher,
        pages,
        isbn13,
        isbn10,
        edition,
        language,
        series,
        series_number,
        doi,
        arxiv_id,
        journal,
        volume,
        issue,
        url
    );
    list!(authors, contributors, tags, categories);
    if m.content_type == ContentType::Book && from.content_type != ContentType::Book {
        m.content_type = from.content_type;
        changed.push("content_type");
    }
    changed
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Answers requests from fixture files; records the URLs asked for.
    #[derive(Default)]
    pub struct FakeHttp {
        pub answers: HashMap<String, (u16, Vec<u8>)>,
        pub asked: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        /// Answers any URL containing `part` with `body`.
        pub fn on(mut self, part: &str, body: &str) -> Self {
            self.answers
                .insert(part.to_owned(), (200, body.as_bytes().to_vec()));
            self
        }
        pub fn status(mut self, part: &str, status: u16) -> Self {
            self.answers.insert(part.to_owned(), (status, Vec::new()));
            self
        }
    }

    impl Http for FakeHttp {
        fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, String> {
            self.asked.lock().unwrap().push(url.to_owned());
            let mut hits: Vec<_> = self
                .answers
                .iter()
                .filter(|(k, _)| url.contains(k.as_str()))
                .collect();
            // The most specific pattern wins.
            hits.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
            match hits.first() {
                Some((_, (status, body))) => Ok(Response {
                    status: *status,
                    body: body.clone(),
                    content_type: None,
                }),
                None => Err("offline".into()),
            }
        }
    }

    pub fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn isbn_lookups_ask_book_sources_only() {
        let http = FakeHttp::default()
            .on(
                "openlibrary.org/api/books",
                &fixture("openlibrary_isbn.json"),
            )
            .on("googleapis.com/books", &fixture("google_isbn.json"));
        let q = Query {
            isbn: Some("0-306-40615-2".into()),
            ..Default::default()
        };
        let r = lookup(&q, &Settings::default(), &http);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.candidates.len(), 2);
        assert!(r.candidates.iter().all(|c| c.score == 1.0));
        let asked = http.asked.lock().unwrap().clone();
        assert_eq!(asked.len(), 2, "{asked:?}");
        assert!(asked.iter().all(|u| u.contains("9780306406157")));
        assert!(r.sure_match().is_some());
    }

    #[test]
    fn falls_back_to_title_search_and_ranks() {
        let http = FakeHttp::default()
            .on(
                "openlibrary.org/search.json",
                &fixture("openlibrary_search.json"),
            )
            .on("googleapis.com/books", &fixture("google_search.json"))
            .status("api.crossref.org", 503)
            .on("api.openalex.org", r#"{"results":[]}"#);
        let q = Query {
            title: Some("Linear Algebra Done Right".into()),
            author: Some("Axler".into()),
            ..Default::default()
        };
        let r = lookup(&q, &Settings::default(), &http);
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.errors[0].source, Source::Crossref);
        let best = &r.candidates[0];
        assert_eq!(best.metadata.title, "Linear Algebra Done Right");
        assert!(best.score > SURE, "{}", best.score);
        let worst = r.candidates.last().unwrap();
        assert!(worst.score < best.score);
    }

    #[test]
    fn keyed_sources_need_a_key() {
        let mut s = Settings::default();
        assert!(!s.usable(Source::Isbndb));
        s.enabled.push(Source::Isbndb);
        assert!(!s.usable(Source::Isbndb), "no key yet");
        s.isbndb_key = Some("  ".into());
        assert!(!s.usable(Source::Isbndb));
        s.isbndb_key = Some("k".into());
        assert!(s.usable(Source::Isbndb));
        assert!(s.usable(Source::OpenLibrary));
    }

    #[test]
    fn scores_titles_and_authors() {
        let q = Query {
            title: Some("The Art of Computer Programming".into()),
            author: Some("Donald Knuth".into()),
            ..Default::default()
        };
        let m = |t: &str, a: &str| BookMetadata {
            title: t.into(),
            authors: vec![a.into()],
            ..Default::default()
        };
        let good = match_score(&q, &m("Art of Computer Programming", "Donald E. Knuth"));
        let wrong_author = match_score(&q, &m("The Art of Computer Programming", "John Smith"));
        let wrong_title = match_score(&q, &m("Concrete Mathematics", "Donald Knuth"));
        assert!(good > SURE, "{good}");
        assert!(wrong_author < SURE && wrong_author > wrong_title);
        assert!(wrong_title < 0.5);
    }

    #[test]
    fn fills_only_empty_fields() {
        let mut m = BookMetadata {
            title: "Mine".into(),
            publisher: Some("Kept".into()),
            ..Default::default()
        };
        let from = BookMetadata {
            title: "Theirs".into(),
            publisher: Some("Other".into()),
            year: Some(2015),
            tags: vec!["algebra".into()],
            content_type: ContentType::Textbook,
            ..Default::default()
        };
        let changed = fill_empty(&mut m, &from);
        assert_eq!(changed, ["year", "tags", "content_type"]);
        assert_eq!(m.title, "Mine");
        assert_eq!(m.publisher.as_deref(), Some("Kept"));
    }

    #[test]
    fn reads_queries() {
        let q = Query {
            isbn: Some("x".into()),
            doi: Some("https://doi.org/10.1038/nature14539".into()),
            arxiv_id: Some("arXiv:1706.03762v5".into()),
            ..Default::default()
        };
        assert_eq!(q.isbn13(), None);
        assert_eq!(q.doi().as_deref(), Some("10.1038/nature14539"));
        assert_eq!(q.arxiv().as_deref(), Some("1706.03762"));
        assert!(Query::default().is_empty());
    }
}
