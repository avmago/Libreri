//! Saving Libreri's markup into a PDF as standard annotations, so other
//! PDF readers show it and can edit it. Every annotation carries its own
//! appearance (drawn from the same drawing list as the marked-up copy), so
//! it looks the same everywhere.

use crate::draw::{draw_body, save, DrawOp};
use crate::geom::{apply, Geometry};
use crate::{Error, Result};
use lopdf::{dictionary, Dictionary, Document, Object, Stream, StringFormat};
use serde::Deserialize;
use std::path::Path;

/// The kind of PDF annotation a mark becomes.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PdfAnnotKind {
    Ink,
    Square,
    Circle,
    Line,
    FreeText,
    Note,
    Stamp,
    Highlight,
    Underline,
    StrikeOut,
}

impl PdfAnnotKind {
    fn subtype(self) -> &'static str {
        match self {
            Self::Ink => "Ink",
            Self::Square => "Square",
            Self::Circle => "Circle",
            Self::Line => "Line",
            Self::FreeText => "FreeText",
            Self::Note => "Text",
            Self::Stamp => "Stamp",
            Self::Highlight => "Highlight",
            Self::Underline => "Underline",
            Self::StrikeOut => "StrikeOut",
        }
    }
}

/// One mark as an annotation. Positions are fractions of the page as
/// shown (top-left origin); `width` is a fraction of the page width.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfAnnot {
    pub page: u32,
    pub kind: PdfAnnotKind,
    /// Libreri's id for the mark (kept as /NM).
    pub id: String,
    /// x, y, w, h around everything the mark draws.
    pub rect: [f64; 4],
    pub color: String,
    #[serde(default)]
    pub width: f64,
    /// The note or text of the mark.
    #[serde(default)]
    pub contents: Option<String>,
    /// Ink strokes; the two ends of a line.
    #[serde(default)]
    pub points: Vec<Vec<[f64; 2]>>,
    /// Boxes of a highlight, underline or strike-out.
    #[serde(default)]
    pub rects: Vec<[f64; 4]>,
    /// How it looks.
    pub ops: Vec<DrawOp>,
}

fn real(v: f64) -> Object {
    Object::Real(v as f32)
}

fn text(s: &str) -> Object {
    // UTF-16BE with a byte-order mark: any language survives.
    let mut bytes = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        bytes.extend(u.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}

/// Adds `annots` to the PDF at `src` and writes it to `dest`.
pub fn annotate(src: &Path, annots: &[PdfAnnot], dest: &Path) -> Result<()> {
    let mut doc = Document::load(src).map_err(|e| Error::Read(e.to_string()))?;
    if doc.is_encrypted() {
        return Err(Error::Encrypted);
    }
    let pages = doc.get_pages();
    for a in annots {
        let page = *pages.get(&a.page).ok_or(Error::NoPage(a.page))?;
        let geo = Geometry::of(&doc, page);
        let (w, h) = geo.shown();
        let m = geo.matrix();
        let user = |x: f64, y: f64| apply(&m, x * w, y * h);
        let pad = a.width * w / 2.0 + 1.0;
        let [x, y, rw, rh] = a.rect;
        let corners = [
            user(x, y),
            user(x + rw, y),
            user(x, y + rh),
            user(x + rw, y + rh),
        ];
        let xs = corners.iter().map(|p| p.0);
        let ys = corners.iter().map(|p| p.1);
        let r = [
            xs.clone().fold(f64::MAX, f64::min) - pad,
            ys.clone().fold(f64::MAX, f64::min) - pad,
            xs.fold(f64::MIN, f64::max) + pad,
            ys.fold(f64::MIN, f64::max) + pad,
        ];
        let rect: Vec<Object> = r.iter().map(|v| real(*v)).collect();

        // The appearance draws in page space, clipped to the box.
        let mut res = Dictionary::new();
        let body = draw_body(&mut doc, &mut res, &a.ops, (w, h), m)?;
        let mut ap = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => rect.clone(), "Resources" => res,
            },
            body.into_bytes(),
        );
        let _ = ap.compress();
        let ap = doc.add_object(ap);

        let [cr, cg, cb] = crate::draw::color(&a.color);
        let mut d = dictionary! {
            "Type" => "Annot",
            "Subtype" => a.kind.subtype(),
            "Rect" => rect,
            "F" => 4,
            "NM" => text(&a.id),
            "T" => text("Libreri"),
            "C" => vec![real(cr), real(cg), real(cb)],
            "AP" => dictionary! { "N" => ap },
        };
        if let Some(c) = a.contents.as_deref().filter(|c| !c.is_empty()) {
            d.set("Contents", text(c));
        }
        if a.width > 0.0 {
            d.set("BS", dictionary! { "W" => real(a.width * w) });
        }
        let flat = |pts: &[[f64; 2]]| -> Vec<Object> {
            pts.iter()
                .flat_map(|p| {
                    let (ux, uy) = user(p[0], p[1]);
                    [real(ux), real(uy)]
                })
                .collect()
        };
        match a.kind {
            PdfAnnotKind::Ink => {
                let list: Vec<Object> = a.points.iter().map(|s| Object::Array(flat(s))).collect();
                d.set("InkList", list);
            }
            PdfAnnotKind::Line => {
                let ends: Vec<[f64; 2]> = a.points.iter().flatten().copied().collect();
                if ends.len() >= 2 {
                    d.set("L", flat(&[ends[0], ends[ends.len() - 1]]));
                }
            }
            PdfAnnotKind::FreeText => {
                d.set(
                    "DA",
                    Object::string_literal(format!("/Helv 12 Tf {cr} {cg} {cb} rg")),
                );
            }
            PdfAnnotKind::Note => {
                d.set("Name", "Comment");
            }
            PdfAnnotKind::Stamp => {
                d.set("Name", "Draft");
            }
            PdfAnnotKind::Highlight | PdfAnnotKind::Underline | PdfAnnotKind::StrikeOut => {
                // Upper left, upper right, lower left, lower right.
                let quads: Vec<Object> = a
                    .rects
                    .iter()
                    .flat_map(|[x, y, rw, rh]| {
                        flat(&[[*x, *y], [x + rw, *y], [*x, y + rh], [x + rw, y + rh]])
                    })
                    .collect();
                d.set("QuadPoints", quads);
            }
            PdfAnnotKind::Square | PdfAnnotKind::Circle => {}
        }
        d.set("P", page);
        let id = doc.add_object(d);
        let annots_obj = doc
            .get_dictionary(page)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .cloned();
        let mut list = match annots_obj {
            Some(Object::Array(v)) => v,
            Some(Object::Reference(r)) => doc
                .get_object(r)
                .ok()
                .and_then(|o| o.as_array().ok().cloned())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        list.push(Object::Reference(id));
        doc.get_dictionary_mut(page)
            .map_err(|e| Error::Read(e.to_string()))?
            .set("Annots", list);
    }
    save(&mut doc, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_become_annotations_with_appearances() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        libreri_formats::test_text_pdf(&src, &["The lighthouse keeper"]);
        let dest = dir.path().join("b.pdf");
        let annots: Vec<PdfAnnot> = serde_json::from_str(
            r##"[
            {"page":1,"kind":"ink","id":"m1","rect":[0.1,0.3,0.2,0.1],"color":"#dc2626","width":0.004,
             "points":[[[0.1,0.3],[0.2,0.35],[0.3,0.4]]],
             "ops":[{"type":"stroke","d":"M 0.1 0.3 L 0.2 0.35 L 0.3 0.4","color":"#dc2626","opacity":1,"width":0.004}]},
            {"page":1,"kind":"highlight","id":"m2","rect":[0.1,0.1,0.3,0.02],"color":"#facc15",
             "rects":[[0.1,0.1,0.3,0.02]],"contents":"Ein Leuchtturm – 灯台",
             "ops":[{"type":"fill","d":"M 0.1 0.1 L 0.4 0.1 L 0.4 0.12 L 0.1 0.12 Z","color":"#facc15","opacity":0.4,"multiply":true}]}
            ]"##,
        )
        .unwrap();
        annotate(&src, &annots, &dest).unwrap();
        let doc = Document::load(&dest).unwrap();
        let page = *doc.get_pages().get(&1).unwrap();
        let list = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(list.len(), 2);
        let ink = doc.get_dictionary(list[0].as_reference().unwrap()).unwrap();
        assert_eq!(ink.get(b"Subtype").unwrap().as_name().unwrap(), b"Ink");
        let r: Vec<f32> = ink
            .get(b"Rect")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_float().unwrap())
            .collect();
        // 0.1 of 612 = 61.2, minus the pad; the top of the ink is 792 * 0.7.
        assert!(
            (r[0] - 61.2 + 2.2).abs() < 0.1 && (r[3] - 554.4 - 2.2).abs() < 0.1,
            "{r:?}"
        );
        let hl = doc.get_dictionary(list[1].as_reference().unwrap()).unwrap();
        assert_eq!(hl.get(b"QuadPoints").unwrap().as_array().unwrap().len(), 8);
        let Object::String(c, _) = hl.get(b"Contents").unwrap() else {
            panic!()
        };
        let units: Vec<u16> = c[2..]
            .chunks(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(String::from_utf16(&units).unwrap(), "Ein Leuchtturm – 灯台");
        // Other readers draw them: the ink's red shows up.
        let pdf = crate::inspect::open(std::fs::read(&dest).unwrap()).unwrap();
        let img = image::load_from_memory(&crate::inspect::page_jpeg(&pdf, 0, 72.0).unwrap())
            .unwrap()
            .to_rgb8();
        let p = img.get_pixel(
            (0.2 * f64::from(img.width())) as u32,
            (0.35 * f64::from(img.height())) as u32,
        );
        assert!(p.0[0] > 150 && p.0[1] < 120, "{p:?}");
    }
}
