//! The comparison report (Phase 6c): a PDF to send to someone. A summary
//! with every change as text, then each page pair that changed, side by
//! side, with the changes marked in colour.

use crate::compare::{Change, ChangeKind, PagePair};
use crate::draw::{f, save};
use crate::edit::{cp1252, pdf_string, wrap};
use crate::{std_fonts, Error, Result};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use std::fmt::Write as _;
use std::path::Path;

/// A rendered page for the report (JPEG).
pub struct ReportPicture {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// What the report is about.
pub struct ReportInfo<'a> {
    pub title: &'a str,
    pub a_label: &'a str,
    pub b_label: &'a str,
    pub made: &'a str,
}

const W: f64 = 841.89;
const H: f64 = 595.28;
const M: f64 = 36.0;

fn colour(kind: ChangeKind) -> [f64; 3] {
    match kind {
        ChangeKind::Removed | ChangeKind::PageRemoved => [0.86, 0.15, 0.15],
        ChangeKind::Added | ChangeKind::PageAdded => [0.09, 0.64, 0.29],
        ChangeKind::Changed => [0.85, 0.47, 0.02],
        ChangeKind::Look => [0.15, 0.39, 0.92],
    }
}

fn label(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Removed => "Removed",
        ChangeKind::Added => "Added",
        ChangeKind::Changed => "Changed",
        ChangeKind::Look => "Looks different",
        ChangeKind::PageRemoved => "Page removed",
        ChangeKind::PageAdded => "Page added",
    }
}

/// Text in the report's font: letters it lacks become "?".
fn latin(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\n' || cp1252(c).is_some() {
                c
            } else {
                '?'
            }
        })
        .collect()
}

struct Pages {
    doc: Document,
    root: ObjectId,
    font: ObjectId,
    bold: ObjectId,
    kids: Vec<Object>,
}

impl Pages {
    fn add(&mut self, body: String, xobjects: Dictionary) {
        let mut stream = Stream::new(Dictionary::new(), body.into_bytes());
        let _ = stream.compress();
        let content = self.doc.add_object(stream);
        let page = self.doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => self.root,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(W as f32), Object::Real(H as f32)],
            "Contents" => content,
            "Resources" => dictionary! {
                "Font" => dictionary! { "F" => self.font, "B" => self.bold },
                "XObject" => xobjects,
            },
        });
        self.kids.push(Object::Reference(page));
    }
}

fn text(body: &mut String, font: &str, size: f64, x: f64, y: f64, bytes: &[u8], rgb: [f64; 3]) {
    let _ = writeln!(
        body,
        "BT /{font} {} Tf {} {} {} rg {} {} Td {} Tj ET",
        f(size),
        f(rgb[0]),
        f(rgb[1]),
        f(rgb[2]),
        f(x),
        f(y),
        pdf_string(bytes)
    );
}

fn line(s: &str) -> Vec<u8> {
    s.chars().filter_map(cp1252).collect()
}

/// Writes the report. `picture(side, page)` renders a page of the first
/// (`side` 0) or second document.
pub fn compare_report(
    info: &ReportInfo,
    pairs: &[PagePair],
    changes: &[Change],
    mut picture: impl FnMut(u8, u32) -> Option<ReportPicture>,
    dest: &Path,
) -> Result<()> {
    let mut doc = Document::with_version("1.5");
    let root = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    });
    let bold = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
    });
    let mut pages = Pages {
        doc,
        root,
        font,
        bold,
        kids: Vec::new(),
    };
    let helv = Some(std_fonts::HELVETICA);
    let grey = [0.4, 0.4, 0.4];
    let black = [0.0, 0.0, 0.0];

    // Summary.
    let mut body = String::new();
    let mut y = H - M - 10.0;
    text(&mut body, "B", 18.0, M, y, &line(&latin(info.title)), black);
    y -= 22.0;
    text(
        &mut body,
        "F",
        10.0,
        M,
        y,
        &line(&latin(&format!("Compared {}", info.made))),
        grey,
    );
    y -= 18.0;
    for (tag, name) in [("First", info.a_label), ("Second", info.b_label)] {
        text(&mut body, "B", 10.0, M, y, &line(&format!("{tag}:")), black);
        text(
            &mut body,
            "F",
            10.0,
            M + 50.0,
            y,
            &line(&latin(name)),
            black,
        );
        y -= 14.0;
    }
    y -= 6.0;
    let count = |k: &[ChangeKind]| changes.iter().filter(|c| k.contains(&c.kind)).count();
    let summary = format!(
        "{} changes: {} removed, {} added, {} changed, {} look different, {} pages removed, {} pages added.",
        changes.len(),
        count(&[ChangeKind::Removed]),
        count(&[ChangeKind::Added]),
        count(&[ChangeKind::Changed]),
        count(&[ChangeKind::Look]),
        count(&[ChangeKind::PageRemoved]),
        count(&[ChangeKind::PageAdded]),
    );
    text(&mut body, "F", 10.0, M, y, &line(&summary), black);
    y -= 22.0;
    if changes.is_empty() {
        text(
            &mut body,
            "F",
            12.0,
            M,
            y,
            &line("The two documents have the same text and look the same."),
            black,
        );
    }
    let where_ = |c: &Change| {
        let p = pairs
            .get(c.pair as usize)
            .copied()
            .unwrap_or(PagePair { a: None, b: None });
        match (p.a, p.b) {
            (Some(a), Some(b)) if a == b => format!("p. {a}"),
            (Some(a), Some(b)) => format!("p. {a} / {b}"),
            (Some(a), None) => format!("p. {a} / -"),
            (None, Some(b)) => format!("- / p. {b}"),
            (None, None) => String::new(),
        }
    };
    for c in changes {
        let what = match c.kind {
            ChangeKind::Changed => format!(
                "\u{201c}{}\u{201d} \u{2192} \u{201c}{}\u{201d}",
                c.a_text, c.b_text
            ),
            ChangeKind::Removed | ChangeKind::PageRemoved => c.a_text.clone(),
            ChangeKind::Added | ChangeKind::PageAdded => c.b_text.clone(),
            ChangeKind::Look => String::new(),
        };
        let what: String = latin(&what).chars().take(600).collect();
        let lines = wrap(&what, 9.5, W - 2.0 * M - 190.0, helv);
        let need = 13.0 * lines.len().max(1) as f64 + 3.0;
        if y - need < M {
            pages.add(std::mem::take(&mut body), Dictionary::new());
            y = H - M - 10.0;
        }
        text(&mut body, "F", 9.5, M, y, &line(&where_(c)), grey);
        text(
            &mut body,
            "B",
            9.5,
            M + 70.0,
            y,
            &line(label(c.kind)),
            colour(c.kind),
        );
        for (i, l) in lines.iter().enumerate() {
            text(
                &mut body,
                "F",
                9.5,
                M + 190.0,
                y - 13.0 * i as f64,
                l,
                black,
            );
        }
        y -= need;
    }
    pages.add(body, Dictionary::new());

    // Each page pair with changes, side by side.
    let half = (W - 3.0 * M) / 2.0;
    let top = H - M - 28.0;
    for (i, pair) in pairs.iter().enumerate() {
        let here: Vec<&Change> = changes.iter().filter(|c| c.pair as usize == i).collect();
        if here.is_empty() {
            continue;
        }
        let mut body = String::new();
        let mut xobjects = Dictionary::new();
        let title = format!(
            "{}  |  {}  -  {} {}",
            pair.a
                .map_or("(no page)".to_owned(), |p| format!("First, page {p}")),
            pair.b
                .map_or("(no page)".to_owned(), |p| format!("Second, page {p}")),
            here.len(),
            if here.len() == 1 { "change" } else { "changes" }
        );
        text(&mut body, "B", 11.0, M, H - M - 8.0, &line(&title), black);
        for side in 0..2u8 {
            let page = if side == 0 { pair.a } else { pair.b };
            let x0 = M + f64::from(side) * (half + M);
            let Some(page) = page else {
                text(
                    &mut body,
                    "F",
                    11.0,
                    x0 + 10.0,
                    top - 20.0,
                    &line("This page is not in this document."),
                    grey,
                );
                continue;
            };
            let Some(pic) = picture(side, page) else {
                text(
                    &mut body,
                    "F",
                    11.0,
                    x0 + 10.0,
                    top - 20.0,
                    &line("This page could not be drawn."),
                    grey,
                );
                continue;
            };
            // Fit the page into its half.
            let aspect = f64::from(pic.height) / f64::from(pic.width.max(1));
            let (mut w, mut h) = (half, half * aspect);
            if h > top - M {
                h = top - M;
                w = h / aspect;
            }
            let (x, y) = (x0 + (half - w) / 2.0, top - h);
            let img = pages.doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image",
                    "Width" => i64::from(pic.width), "Height" => i64::from(pic.height),
                    "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
                },
                pic.jpeg,
            ));
            let name = format!("P{side}");
            xobjects.set(name.as_bytes(), img);
            let _ = writeln!(
                body,
                "q {} 0 0 {} {} {} cm /{name} Do Q",
                f(w),
                f(h),
                f(x),
                f(y)
            );
            let _ = writeln!(
                body,
                "q 0.75 G 0.5 w {} {} {} {} re S Q",
                f(x),
                f(y),
                f(w),
                f(h)
            );
            // The changes on this side.
            let _ = writeln!(body, "q /Hl gs");
            for c in &here {
                let rects = if side == 0 { &c.a_rects } else { &c.b_rects };
                let [r, g, b] = colour(c.kind);
                for [rx, ry, rw, rh] in rects {
                    let (px, py) = (x + rx * w, y + (1.0 - ry - rh) * h);
                    let _ = writeln!(
                        body,
                        "{} {} {} rg {} {} {} RG 0.8 w {} {} {} {} re B",
                        f(r),
                        f(g),
                        f(b),
                        f(r),
                        f(g),
                        f(b),
                        f(px - 1.0),
                        f(py - 1.0),
                        f(rw * w + 2.0),
                        f(rh * h + 2.0)
                    );
                }
            }
            let _ = writeln!(body, "Q");
        }
        pages.add(body, xobjects);
    }

    // See-through fills for the marks.
    let gs = pages
        .doc
        .add_object(dictionary! { "Type" => "ExtGState", "ca" => 0.28, "CA" => 0.9 });
    let ids: Vec<ObjectId> = pages
        .kids
        .iter()
        .filter_map(|k| k.as_reference().ok())
        .collect();
    for id in ids {
        if let Ok(Object::Dictionary(res)) = pages
            .doc
            .get_dictionary_mut(id)
            .map_err(|e| Error::Read(e.to_string()))?
            .get_mut(b"Resources")
        {
            res.set("ExtGState", dictionary! { "Hl" => gs });
        }
    }
    let n = pages.kids.len() as i64;
    let Pages {
        mut doc,
        root,
        kids,
        ..
    } = pages;
    doc.objects.insert(
        root,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => n }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    doc.trailer.set("Root", catalog);
    save(&mut doc, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_report() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("report.pdf");
        let pairs = [
            PagePair {
                a: Some(1),
                b: Some(1),
            },
            PagePair {
                a: Some(2),
                b: None,
            },
        ];
        let changes = vec![
            Change {
                kind: ChangeKind::Changed,
                pair: 0,
                a_rects: vec![[0.1, 0.1, 0.3, 0.03]],
                b_rects: vec![[0.1, 0.1, 0.2, 0.03]],
                a_text: "Samuel Harte".into(),
                b_text: "Ada Lovelace – 灯台".into(),
            },
            Change {
                kind: ChangeKind::PageRemoved,
                pair: 1,
                a_rects: vec![],
                b_rects: vec![],
                a_text: "gone".into(),
                b_text: String::new(),
            },
        ];
        let jpeg = {
            let img = image::RgbImage::from_pixel(60, 80, image::Rgb([250, 250, 250]));
            let mut out = std::io::Cursor::new(Vec::new());
            img.write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
            out.into_inner()
        };
        let info = ReportInfo {
            title: "Lighthouse Report",
            a_label: "Version of 20 Sep",
            b_label: "Current",
            made: "28 Sep 2026",
        };
        compare_report(
            &info,
            &pairs,
            &changes,
            |_, _| {
                Some(ReportPicture {
                    jpeg: jpeg.clone(),
                    width: 60,
                    height: 80,
                })
            },
            &dest,
        )
        .unwrap();
        let texts = libreri_formats::pdftext::page_texts(&dest).unwrap();
        assert_eq!(texts.len(), 3);
        assert!(
            texts[0].contains("Samuel Harte") && texts[0].contains("Ada Lovelace"),
            "{}",
            texts[0]
        );
        assert!(texts[0].contains("Page removed"));
        assert!(texts[1].contains("First, page 1"));
    }
}
