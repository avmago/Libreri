//! EPUB 2 and 3: the OPF package document and the cover image.

use crate::xml::{self, Element};
use crate::{find_doi, find_year, is_image_name, zip_read, Extracted};
use libreri_core::isbn::find_isbn;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

/// MARC relator codes for people who are not authors.
fn role_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "edt" => "editor",
        "trl" => "translator",
        "ill" => "illustrator",
        "nrt" => "narrator",
        "aui" => "foreword",
        "aft" => "afterword",
        "ctb" => "contributor",
        "pbl" => return None,
        _ => "contributor",
    })
}

fn decode_href(href: &str) -> String {
    let bytes = href.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(v) = href
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Joins an href onto the OPF's folder, resolving `..`.
fn join(base_dir: &str, href: &str) -> String {
    let href = decode_href(href.split('#').next().unwrap_or(href));
    let mut parts: Vec<&str> = base_dir.split('/').filter(|p| !p.is_empty()).collect();
    for p in href.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(p),
        }
    }
    parts.join("/")
}

pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("not a valid EPUB ({e})"))?;
    let container =
        zip_read(&mut zip, "META-INF/container.xml").ok_or("META-INF/container.xml is missing")?;
    let container = xml::parse(&String::from_utf8_lossy(&container));
    let opf_path = container
        .all("rootfile")
        .into_iter()
        .find(|r| {
            r.attr("media-type")
                .is_none_or(|t| t == "application/oebps-package+xml")
        })
        .and_then(|r| r.attr("full-path"))
        .ok_or("no package document listed")?
        .to_owned();
    let opf = zip_read(&mut zip, &opf_path).ok_or("package document is missing")?;
    let opf = xml::parse(&String::from_utf8_lossy(&opf));
    let base_dir = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let metadata = opf.find("metadata").ok_or("package has no metadata")?;
    apply_metadata(metadata, out);

    if let Some(href) = cover_href(&opf) {
        let full = join(base_dir, &href);
        out.cover = zip_read(&mut zip, &full);
    }
    Ok(())
}

fn apply_metadata(md: &Element, out: &mut Extracted) {
    let m = &mut out.metadata;
    let text = |name: &str| {
        md.find(name)
            .map(Element::text_deep)
            .filter(|t| !t.is_empty())
    };

    // EPUB 3 puts roles, title types and series details in <meta refines="#id">.
    let mut refines: HashMap<(String, String), String> = HashMap::new();
    for meta in md.all("meta") {
        if let (Some(target), Some(prop)) = (meta.attr("refines"), meta.attr("property")) {
            refines.insert(
                (target.trim_start_matches('#').to_owned(), prop.to_owned()),
                meta.text_deep(),
            );
        }
    }
    let refined = |el: &Element, prop: &str| -> Option<String> {
        let id = el.attr("id")?;
        refines.get(&(id.to_owned(), prop.to_owned())).cloned()
    };

    // Main title first, then an optional subtitle.
    let titles = md.all("title");
    let main = titles
        .iter()
        .find(|t| refined(t, "title-type").as_deref() == Some("main"))
        .or(titles.first());
    if let Some(t) = main {
        m.title = t.text_deep();
    }
    m.subtitle = titles
        .iter()
        .find(|t| refined(t, "title-type").as_deref() == Some("subtitle"))
        .map(|t| t.text_deep());

    for c in md.all("creator").into_iter().chain(md.all("contributor")) {
        let name = c.text_deep();
        if name.is_empty() {
            continue;
        }
        let role = c
            .attr("role")
            .map(str::to_owned)
            .or_else(|| refined(c, "role"))
            .unwrap_or_else(|| {
                if c.name == "creator" {
                    "aut".into()
                } else {
                    "ctb".into()
                }
            });
        if role == "aut" {
            m.authors.push(name);
        } else if let Some(r) = role_name(&role) {
            m.contributors.push(format!("{name} ({r})"));
        }
    }

    if let Some(d) = text("description") {
        let about = xml::strip_html(&d);
        if !about.is_empty() {
            m.about = Some(about);
        }
    }
    m.publisher = text("publisher");
    m.language = text("language");
    let dates = md.all("date");
    let date = dates
        .iter()
        .find(|d| d.attr("event") == Some("publication"))
        .or(dates.first());
    m.year = date.and_then(|d| find_year(&d.text_deep()));
    m.tags = md
        .all("subject")
        .into_iter()
        .flat_map(|s| crate::split_keywords(&s.text_deep()))
        .collect();

    for id in md.all("identifier") {
        let value = id.text_deep();
        let scheme = id.attr("scheme").unwrap_or("").to_ascii_lowercase();
        if m.isbn13.is_none()
            && (scheme == "isbn" || value.to_lowercase().contains("isbn") || value.len() <= 17)
        {
            if let Some((i13, i10)) = find_isbn(&value) {
                m.isbn13 = Some(i13);
                m.isbn10 = i10;
                continue;
            }
        }
        if m.doi.is_none() && (scheme == "doi" || value.to_lowercase().contains("doi")) {
            m.doi = find_doi(&value);
        }
    }

    // Series: Calibre's meta tags, or EPUB 3 collections.
    for meta in md.all("meta") {
        match meta.attr("name") {
            Some("calibre:series") => m.series = meta.attr("content").map(str::to_owned),
            Some("calibre:series_index") => {
                m.series_number = meta.attr("content").and_then(|v| v.parse().ok())
            }
            _ => {}
        }
        if meta.attr("property") == Some("belongs-to-collection") && m.series.is_none() {
            m.series = Some(meta.text_deep());
            m.series_number = refined(meta, "group-position").and_then(|v| v.parse().ok());
        }
    }
}

fn cover_href(opf: &Element) -> Option<String> {
    let manifest = opf.find("manifest")?;
    let items = manifest.all("item");
    let image = |i: &Element| {
        i.attr("media-type")
            .is_some_and(|t| t.starts_with("image/"))
            || i.attr("href").is_some_and(is_image_name)
    };
    // EPUB 3
    if let Some(i) = items.iter().find(|i| {
        i.attr("properties")
            .is_some_and(|p| p.split_whitespace().any(|p| p == "cover-image"))
    }) {
        return i.attr("href").map(str::to_owned);
    }
    // EPUB 2: <meta name="cover" content="item-id"/>
    if let Some(id) = opf
        .all("meta")
        .into_iter()
        .find(|m| m.attr("name") == Some("cover"))
        .and_then(|m| m.attr("content"))
    {
        if let Some(i) = items
            .iter()
            .filter(|i| image(i))
            .find(|i| i.attr("id") == Some(id))
        {
            return i.attr("href").map(str::to_owned);
        }
    }
    // Last resort: an image whose id or file name says "cover".
    items
        .iter()
        .filter(|i| image(i))
        .find(|i| {
            i.attr("id")
                .is_some_and(|v| v.to_lowercase().contains("cover"))
                || i.attr("href")
                    .is_some_and(|v| v.to_lowercase().contains("cover"))
        })
        .and_then(|i| i.attr("href"))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use crate::extract;
    use crate::testutil::zip_file;
    use libreri_core::FileType;

    pub const CONTAINER: &[u8] = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

    #[test]
    fn reads_epub3_metadata_and_cover() {
        let opf = br##"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title id="t1">A Brief History</dc:title>
  <meta refines="#t1" property="title-type">main</meta>
  <dc:title id="t2">of Almost Everything</dc:title>
  <meta refines="#t2" property="title-type">subtitle</meta>
  <dc:creator id="c1">John Smith</dc:creator>
  <meta refines="#c1" property="role" scheme="marc:relators">aut</meta>
  <dc:creator id="c2">Jane Smith</dc:creator>
  <meta refines="#c2" property="role" scheme="marc:relators">trl</meta>
  <dc:identifier id="uid">urn:isbn:978-0-306-40615-7</dc:identifier>
  <dc:language>en</dc:language>
  <dc:publisher>Example Press</dc:publisher>
  <dc:date>2004-05-01</dc:date>
  <dc:subject>History; Science</dc:subject>
  <dc:description>&lt;p&gt;A &lt;b&gt;short&lt;/b&gt; book.&lt;/p&gt;</dc:description>
  <meta property="belongs-to-collection" id="s">Big Ideas</meta>
  <meta refines="#s" property="group-position">2</meta>
 </metadata>
 <manifest>
  <item id="img" href="images/c%20over.jpg" media-type="image/jpeg" properties="cover-image"/>
 </manifest>
</package>"##;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.epub");
        zip_file(
            &p,
            &[
                ("mimetype", b"application/epub+zip"),
                ("META-INF/container.xml", CONTAINER),
                ("OEBPS/content.opf", opf),
                ("OEBPS/images/c over.jpg", b"JPEGDATA"),
            ],
        );
        let e = extract(&p, FileType::Epub);
        let m = &e.metadata;
        assert_eq!(m.title, "A Brief History");
        assert_eq!(m.subtitle.as_deref(), Some("of Almost Everything"));
        assert_eq!(m.authors, vec!["John Smith"]);
        assert_eq!(m.contributors, vec!["Jane Smith (translator)"]);
        assert_eq!(m.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(m.isbn10.as_deref(), Some("0306406152"));
        assert_eq!(m.year, Some(2004));
        assert_eq!(m.tags, vec!["History", "Science"]);
        assert_eq!(m.about.as_deref(), Some("A short book."));
        assert_eq!(m.series.as_deref(), Some("Big Ideas"));
        assert_eq!(m.series_number, Some(2.0));
        assert_eq!(e.cover.as_deref(), Some(&b"JPEGDATA"[..]));
    }

    #[test]
    fn reads_epub2_cover_meta() {
        let opf = br#"<package><metadata><dc:title>Old</dc:title>
<dc:creator opf:role="aut">Leo</dc:creator><meta name="cover" content="cov"/>
<meta name="calibre:series" content="Classics"/><meta name="calibre:series_index" content="3"/>
</metadata><manifest><item id="cov" href="../cover.png" media-type="image/png"/></manifest></package>"#;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.epub");
        zip_file(
            &p,
            &[
                ("META-INF/container.xml", CONTAINER),
                ("OEBPS/content.opf", opf),
                ("cover.png", b"PNG"),
            ],
        );
        let e = extract(&p, FileType::Epub);
        assert_eq!(e.metadata.title, "Old");
        assert_eq!(e.metadata.series_number, Some(3.0));
        assert_eq!(e.cover.as_deref(), Some(&b"PNG"[..]));
    }
}
