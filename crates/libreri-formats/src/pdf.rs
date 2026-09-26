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
