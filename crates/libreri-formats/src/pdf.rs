//! PDF: the Info dictionary and the page count.
//!
//! Covers for PDFs are rendered from the first page by the interface
//! (PDF.js), which already has a full renderer; see `save_cover`.

use crate::{find_doi, split_keywords, split_people, Extracted};
use std::path::Path;

pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let meta = lopdf::Document::load_metadata(path).map_err(|e| e.to_string())?;
    let m = &mut out.metadata;
    if meta.page_count > 0 {
        m.pages = Some(meta.page_count);
    }
    if meta.encrypted && meta.title.is_none() {
        out.warnings
            .push("the PDF is password-protected; only the file name was used".into());
    }
    if let Some(t) = meta.title {
        m.title = t;
    }
    if let Some(a) = meta.author {
        m.authors = split_people(&a);
    }
    // Some producers fill Subject with a placeholder.
    let placeholder = |s: &str| {
        matches!(
            s.trim().to_lowercase().as_str(),
            "" | "unspecified" | "untitled" | "none" | "subject" | "n/a"
        )
    };
    if let Some(s) = meta.subject.filter(|s| !placeholder(s)) {
        m.doi = find_doi(&s);
        if m.doi.as_deref() != Some(s.trim()) {
            m.about = Some(s.trim().to_owned());
        }
    }
    if let Some(k) = meta.keywords {
        m.tags = split_keywords(&k);
        if m.doi.is_none() {
            m.doi = find_doi(&k);
        }
    }
    Ok(())
}

/// Largest PDF whose text is searched for identifiers (the whole file is
/// parsed to reach its pages).
const MAX_SCAN_BYTES: u64 = 120 * 1024 * 1024;

pub fn identifiers(path: &Path) -> Option<crate::Identifiers> {
    if std::fs::metadata(path).ok()?.len() > MAX_SCAN_BYTES {
        return None;
    }
    let doc = lopdf::Document::load(path).ok()?;
    let count = doc.get_pages().len() as u32;
    if count == 0 {
        return None;
    }
    let mut pages: Vec<u32> = (1..=count.min(4)).collect();
    for p in count.saturating_sub(1)..=count {
        if p >= 1 && !pages.contains(&p) {
            pages.push(p);
        }
    }
    let mut found = crate::Identifiers::default();
    for p in pages {
        let Ok(text) = doc.extract_text(&[p]) else {
            continue;
        };
        if found.isbn13.is_none() {
            found.isbn13 = libreri_core::isbn::find_isbn(&text).map(|(i13, _)| i13);
        }
        if found.doi.is_none() {
            found.doi = find_doi(&text);
        }
        if found.arxiv_id.is_none() {
            found.arxiv_id = crate::find_arxiv(&text);
        }
    }
    Some(found)
}

/// The visible area of a PDF page in PDF points (crop box within the
/// media box), and its rotation in degrees clockwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub rotate: i64,
}

fn number(doc: &lopdf::Document, o: &lopdf::Object) -> Option<f64> {
    match o {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(f64::from(*f)),
        lopdf::Object::Reference(r) => number(doc, doc.get_object(*r).ok()?),
        _ => None,
    }
}

/// A page attribute, looking up the page tree for inherited ones.
fn inherited<'a>(
    doc: &'a lopdf::Document,
    mut dict: &'a lopdf::Dictionary,
    key: &[u8],
) -> Option<&'a lopdf::Object> {
    for _ in 0..32 {
        if let Ok(v) = dict.get(key) {
            return Some(match v {
                lopdf::Object::Reference(r) => doc.get_object(*r).ok()?,
                v => v,
            });
        }
        let parent = dict.get(b"Parent").ok()?.as_reference().ok()?;
        dict = doc.get_object(parent).ok()?.as_dict().ok()?;
    }
    None
}

fn rect(doc: &lopdf::Document, o: &lopdf::Object) -> Option<[f64; 4]> {
    let a = o.as_array().ok()?;
    if a.len() != 4 {
        return None;
    }
    let v: Vec<f64> = a.iter().filter_map(|x| number(doc, x)).collect();
    (v.len() == 4).then(|| {
        [
            v[0].min(v[2]),
            v[1].min(v[3]),
            v[0].max(v[2]),
            v[1].max(v[3]),
        ]
    })
}

/// Every page's visible box, in page order. `None` if the file cannot be
/// read.
pub fn page_boxes(path: &Path) -> Option<Vec<PageBox>> {
    let doc = lopdf::Document::load(path).ok()?;
    let mut out = Vec::new();
    for (_, id) in doc.get_pages() {
        let dict = doc.get_object(id).ok()?.as_dict().ok()?;
        let media = inherited(&doc, dict, b"MediaBox")
            .and_then(|o| rect(&doc, o))
            .unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let crop = inherited(&doc, dict, b"CropBox")
            .and_then(|o| rect(&doc, o))
            .unwrap_or(media);
        // PDF.js shows the crop box clipped to the media box.
        let b = [
            crop[0].max(media[0]),
            crop[1].max(media[1]),
            crop[2].min(media[2]),
            crop[3].min(media[3]),
        ];
        let rotate = inherited(&doc, dict, b"Rotate")
            .and_then(|o| number(&doc, o))
            .map_or(0, |r| (r as i64).rem_euclid(360));
        out.push(PageBox {
            x0: b[0],
            y0: b[1],
            x1: b[2],
            y1: b[3],
            rotate,
        });
    }
    Some(out)
}

/// A page's own scale for measuring, from its viewport measure
/// dictionary (/VP … /Measure, common in CAD and map exports).
#[derive(Debug, Clone, PartialEq)]
pub struct PageScale {
    pub page: u32,
    /// "mm", "cm", "m", "in" or "ft".
    pub unit: String,
    /// Real length per PDF point.
    pub per_point: f64,
}

fn text_of(doc: &lopdf::Document, o: &lopdf::Object) -> Option<String> {
    match o {
        lopdf::Object::String(b, _) => Some(String::from_utf8_lossy(b).trim().to_owned()),
        lopdf::Object::Reference(r) => text_of(doc, doc.get_object(*r).ok()?),
        _ => None,
    }
}

fn dict<'a>(doc: &'a lopdf::Document, o: &'a lopdf::Object) -> Option<&'a lopdf::Dictionary> {
    match o {
        lopdf::Object::Dictionary(d) => Some(d),
        lopdf::Object::Reference(r) => dict(doc, doc.get_object(*r).ok()?),
        _ => None,
    }
}

fn array<'a>(doc: &'a lopdf::Document, o: &'a lopdf::Object) -> Option<&'a Vec<lopdf::Object>> {
    match o {
        lopdf::Object::Array(a) => Some(a),
        lopdf::Object::Reference(r) => array(doc, doc.get_object(*r).ok()?),
        _ => None,
    }
}

/// Pages that say how to measure them.
pub fn measure_scales(path: &Path) -> Vec<PageScale> {
    let Ok(doc) = lopdf::Document::load(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (n, id) in doc.get_pages() {
        let Ok(page) = doc.get_dictionary(id) else {
            continue;
        };
        let Some(vps) = page.get(b"VP").ok().and_then(|o| array(&doc, o)) else {
            continue;
        };
        let found = vps.iter().find_map(|vp| {
            let m = dict(&doc, dict(&doc, vp)?.get(b"Measure").ok()?)?;
            let x = array(&doc, m.get(b"X").ok()?)?;
            let first = dict(&doc, x.first()?)?;
            let unit = text_of(&doc, first.get(b"U").ok()?)?.to_lowercase();
            let factor = number(&doc, first.get(b"C").ok()?)?;
            let unit = match unit.as_str() {
                "mm" | "cm" | "m" | "ft" => unit,
                "in" | "inch" | "inches" | "\"" => "in".to_owned(),
                _ => return None,
            };
            (factor > 0.0).then_some(PageScale {
                page: n,
                unit,
                per_point: factor,
            })
        });
        out.extend(found);
    }
    out
}

/// Writes a blank PDF of `pages` US Letter pages, for other crates' tests.
#[doc(hidden)]
pub fn test_pdf(path: &Path, pages: u32) {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for _ in 0..pages {
        let content = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content,
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => pages as i64,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc.save(path).expect("the test PDF is written");
}

/// Writes a PDF with one page per entry of `pages`, each showing its text
/// in Helvetica (lines split on `\n`), for tests. An empty entry makes a
/// page without text, like a scan.
#[doc(hidden)]
pub fn test_text_pdf(path: &Path, pages: &[&str]) {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let mut kids = Vec::new();
    for text in pages {
        let mut body = String::new();
        for (i, line) in text.lines().enumerate() {
            let line = line
                .replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)");
            body.push_str(&format!(
                "BT /F1 12 Tf 72 {} Td ({line}) Tj ET\n",
                700 - 16 * i as i64
            ));
        }
        let content = doc.add_object(Stream::new(dictionary! {}, body.into_bytes()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => kids.len() as i64, "Kids" => kids,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc.save(path).expect("the test PDF is written");
}

#[cfg(test)]
mod tests {
    use crate::extract;
    use libreri_core::FileType;
    use lopdf::{dictionary, Document, Object, Stream};

    /// Writes a small PDF with `pages` blank pages and an Info dictionary.
    pub fn make_pdf(path: &std::path::Path, pages: u32, info: lopdf::Dictionary) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for _ in 0..pages {
            let content = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content,
            });
            kids.push(Object::Reference(page));
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => pages as i64,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        let info = doc.add_object(info);
        doc.trailer.set("Root", catalog);
        doc.trailer.set("Info", info);
        doc.save(path).unwrap();
    }

    #[test]
    fn page_boxes_are_read_with_inheritance() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.pdf");
        make_pdf(&p, 2, dictionary! {});
        let boxes = super::page_boxes(&p).unwrap();
        assert_eq!(boxes.len(), 2);
        assert_eq!(
            (boxes[0].x1, boxes[0].y1, boxes[0].rotate),
            (612.0, 792.0, 0)
        );
    }

    fn text_pdf(path: &std::path::Path, text: &str) {
        crate::test_text_pdf(path, &[text]);
    }

    #[test]
    fn reads_viewport_measure_scales() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("plan.pdf");
        crate::test_pdf(&p, 1);
        let mut doc = Document::load(&p).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        let measure = dictionary! {
            "Type" => "Measure", "Subtype" => "RL", "R" => Object::string_literal("1 in = 10 ft"),
            "X" => vec![Object::Dictionary(dictionary! { "U" => Object::string_literal("ft"), "C" => 0.1389 })],
        };
        let vp = dictionary! { "Type" => "Viewport", "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Measure" => measure };
        doc.get_dictionary_mut(page_id)
            .unwrap()
            .set("VP", vec![Object::Dictionary(vp)]);
        doc.save(&p).unwrap();
        let scales = crate::measure_scales(&p);
        assert_eq!(scales.len(), 1);
        assert_eq!(scales[0].unit, "ft");
        assert!((scales[0].per_point - 0.1389).abs() < 1e-4);
    }

    #[test]
    fn finds_identifiers_in_page_text() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.pdf");
        text_pdf(
            &p,
            "ISBN 978-0-306-40615-7 arXiv:1706.03762v5 doi:10.1000/xyz123",
        );
        let ids = crate::find_identifiers(&p, FileType::Pdf);
        assert_eq!(ids.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(ids.arxiv_id.as_deref(), Some("1706.03762"));
        assert_eq!(ids.doi.as_deref(), Some("10.1000/xyz123"));
        assert_eq!(
            crate::find_identifiers(&p, FileType::Epub),
            Default::default()
        );
    }

    #[test]
    fn reads_info_and_pages() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.pdf");
        make_pdf(
            &p,
            3,
            dictionary! {
                "Title" => Object::string_literal("Linear Algebra Done Right"),
                "Author" => Object::string_literal("John Smith; Jane Smith"),
                "Keywords" => Object::string_literal("algebra, vectors"),
                "Subject" => Object::string_literal("doi:10.1007/978-3-319-11080-6"),
            },
        );
        let e = extract(&p, FileType::Pdf);
        assert_eq!(e.metadata.title, "Linear Algebra Done Right");
        assert_eq!(e.metadata.authors, vec!["John Smith", "Jane Smith"]);
        assert_eq!(e.metadata.pages, Some(3));
        assert_eq!(e.metadata.tags, vec!["algebra", "vectors"]);
        assert_eq!(e.metadata.doi.as_deref(), Some("10.1007/978-3-319-11080-6"));
        assert!(e.warnings.is_empty(), "{:?}", e.warnings);
    }

    #[test]
    fn junk_titles_fall_back_to_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Real Title.pdf");
        make_pdf(
            &p,
            1,
            dictionary! { "Title" => Object::string_literal("Microsoft Word - draft.docx") },
        );
        assert_eq!(extract(&p, FileType::Pdf).metadata.title, "Real Title");
    }

    #[test]
    fn damaged_files_still_get_a_title() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("broken.pdf");
        std::fs::write(&p, b"%PDF-1.4 garbage").unwrap();
        let e = extract(&p, FileType::Pdf);
        assert_eq!(e.metadata.title, "broken");
        assert_eq!(e.warnings.len(), 1);
    }
}
