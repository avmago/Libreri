//! Comic archives. CBZ (ZIP) is read here; CBR, CB7, CBT and CBA get their
//! readers in Phase 5.

use crate::xml::{self, Element};
use crate::{is_image_name, natural_key, split_keywords, split_people, zip_read, Extracted};
use std::fs::File;
use std::path::Path;

pub fn read_cbz(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("not a valid CBZ ({e})"))?;
    let mut pages: Vec<String> = zip
        .file_names()
        .filter(|n| is_image_name(n) && !n.starts_with("__MACOSX") && !n.contains("/."))
        .map(str::to_owned)
        .collect();
    pages.sort_by_key(|n| natural_key(n));
    let m = &mut out.metadata;
    if !pages.is_empty() {
        m.pages = Some(pages.len() as u32);
    }

    let info_name = zip
        .file_names()
        .find(|n| n.eq_ignore_ascii_case("ComicInfo.xml"))
        .map(str::to_owned);
    if let Some(bytes) = info_name.and_then(|n| zip_read(&mut zip, &n)) {
        let root = xml::parse(&String::from_utf8_lossy(&bytes));
        if let Some(info) = root.find("comicinfo") {
            apply_comic_info(info, out);
        }
    }

    if let Some(first) = pages.first() {
        out.cover = zip_read(&mut zip, first);
    }
    Ok(())
}

fn apply_comic_info(info: &Element, out: &mut Extracted) {
    let get = |n: &str| {
        info.find(n)
            .map(Element::text_deep)
            .filter(|t| !t.is_empty())
    };
    let m = &mut out.metadata;
    if let Some(t) = get("title") {
        m.title = t;
    }
    m.series = get("series");
    m.series_number = get("number").and_then(|n| n.parse().ok());
    if m.title.is_empty() {
        if let Some(series) = &m.series {
            m.title = match get("number") {
                Some(n) => format!("{series} #{n}"),
                None => series.clone(),
            };
        }
    }
    if let Some(w) = get("writer") {
        m.authors = w.split(',').flat_map(split_people).collect();
    }
    for (field, role) in [
        ("penciller", "artist"),
        ("colorist", "colourist"),
        ("editor", "editor"),
    ] {
        if let Some(v) = get(field) {
            m.contributors
                .extend(v.split(',').map(|p| format!("{} ({role})", p.trim())));
        }
    }
    m.about = get("summary");
    m.publisher = get("publisher");
    m.year = get("year").and_then(|y| y.parse().ok());
    m.language = get("languageiso");
    if let Some(g) = get("genre") {
        m.tags = split_keywords(&g);
    }
    if let Some(p) = get("pagecount").and_then(|p| p.parse().ok()) {
        m.pages = Some(p);
    }
}

#[cfg(test)]
mod tests {
    use crate::extract;
    use crate::testutil::zip_file;
    use libreri_core::FileType;

    #[test]
    fn reads_pages_cover_and_comic_info() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.cbz");
        zip_file(
            &p,
            &[
                ("p10.jpg", b"TEN"),
                ("p2.jpg", b"TWO"),
                ("p1.jpg", b"ONE"),
                ("__MACOSX/._p1.jpg", b"junk"),
                (
                    "ComicInfo.xml",
                    b"<ComicInfo><Series>Night Owl</Series><Number>3</Number>\
                      <Writer>John Smith, Jane Smith</Writer><Year>2020</Year></ComicInfo>",
                ),
            ],
        );
        let e = extract(&p, FileType::Cbz);
        assert_eq!(e.metadata.title, "Night Owl #3");
        assert_eq!(e.metadata.authors, vec!["John Smith", "Jane Smith"]);
        assert_eq!(e.metadata.pages, Some(3));
        assert_eq!(e.metadata.year, Some(2020));
        assert_eq!(e.cover.as_deref(), Some(&b"ONE"[..]));
    }
}
