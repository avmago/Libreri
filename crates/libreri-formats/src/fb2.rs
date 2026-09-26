//! FictionBook 2: the `<description>` header and the cover picture.

use crate::xml::{self, Element};
use crate::{find_year, Extracted};
use base64::Engine;
use libreri_core::isbn::find_isbn;
use std::io::Read;
use std::path::Path;

/// The header (and usually the cover) sit before the book body; the cover
/// `<binary>` is normally near the end, so large files are read in full up to
/// this limit.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

fn person(p: &Element) -> String {
    let part = |n: &str| p.find(n).map(Element::text_deep).unwrap_or_default();
    let name = [part("first-name"), part("middle-name"), part("last-name")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        part("nickname")
    } else {
        name
    }
}

pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let root = xml::parse(&String::from_utf8_lossy(&bytes));
    let info = root.find("title-info").ok_or("no FB2 title-info")?;
    let m = &mut out.metadata;

    m.title = info
        .find("book-title")
        .map(Element::text_deep)
        .unwrap_or_default();
    m.authors = info
        .kids("author")
        .map(person)
        .filter(|a| !a.is_empty())
        .collect();
    m.contributors = info
        .kids("translator")
        .map(person)
        .filter(|a| !a.is_empty())
        .map(|a| format!("{a} (translator)"))
        .collect();
    m.tags = info.kids("genre").map(Element::text_deep).collect();
    m.about = info
        .find("annotation")
        .map(|a| {
            a.children
                .iter()
                .map(Element::text_deep)
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .filter(|t| !t.is_empty());
    m.language = info.find("lang").map(Element::text_deep);
    if let Some(seq) = info.find("sequence") {
        m.series = seq.attr("name").map(str::to_owned);
        m.series_number = seq.attr("number").and_then(|n| n.parse().ok());
    }
    m.year = info.find("date").and_then(|d| {
        d.attr("value")
            .and_then(find_year)
            .or_else(|| find_year(&d.text_deep()))
    });

    if let Some(publish) = root.find("publish-info") {
        m.publisher = publish.find("publisher").map(Element::text_deep);
        if let Some(y) = publish.find("year").and_then(|y| find_year(&y.text_deep())) {
            m.year = Some(y);
        }
        if let Some((i13, i10)) = publish.find("isbn").and_then(|i| find_isbn(&i.text_deep())) {
            m.isbn13 = Some(i13);
            m.isbn10 = i10;
        }
    }

    // <coverpage><image l:href="#cover.jpg"/></coverpage> → <binary id="cover.jpg">
    let cover_id = info
        .find("coverpage")
        .and_then(|c| c.find("image"))
        .and_then(|i| i.attr("href"))
        .map(|h| h.trim_start_matches('#').to_owned());
    if let Some(id) = cover_id {
        if let Some(bin) = root
            .all("binary")
            .into_iter()
            .find(|b| b.attr("id") == Some(id.as_str()))
        {
            let data: String = bin.text.chars().filter(|c| !c.is_whitespace()).collect();
            out.cover = base64::engine::general_purpose::STANDARD.decode(data).ok();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::extract;
    use libreri_core::FileType;

    #[test]
    fn reads_fb2_header_and_cover() {
        let fb2 = r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<description><title-info>
 <genre>sf</genre>
 <author><first-name>John</first-name><last-name>Smith</last-name></author>
 <book-title>Stars</book-title>
 <annotation><p>First.</p><p>Second.</p></annotation>
 <date value="1999-01-01">1999</date>
 <coverpage><image l:href="#c.jpg"/></coverpage>
 <lang>en</lang>
 <sequence name="Sky" number="4"/>
</title-info>
<publish-info><publisher>Pub</publisher><year>2001</year><isbn>0-306-40615-2</isbn></publish-info>
</description><body><p>Text</p></body>
<binary id="c.jpg" content-type="image/jpeg">SlBH</binary>
</FictionBook>"##;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.fb2");
        std::fs::write(&p, fb2).unwrap();
        let e = extract(&p, FileType::Fb2);
        let m = &e.metadata;
        assert_eq!(m.title, "Stars");
        assert_eq!(m.authors, vec!["John Smith"]);
        assert_eq!(m.about.as_deref(), Some("First.\n\nSecond."));
        assert_eq!(m.year, Some(2001));
        assert_eq!(m.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(m.series.as_deref(), Some("Sky"));
        assert_eq!(e.cover.as_deref(), Some(&b"JPG"[..]));
    }
}
