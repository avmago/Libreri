//! Editing a PDF's pages (Phase 6b): order, rotation, cropping, pages from
//! other files, blank and photographed pages, redaction, small text
//! corrections, an OCR text layer and compression. Always writes a new
//! file; the caller keeps the old one as a version.

use crate::draw::{
    color, decode_data_url, draw_on_page, f, image_xobject, own_resources, save, DrawOp,
};
use crate::geom::{inherited, num, FBox, Geometry};
use crate::{inspect, redact, std_fonts, Error, Result};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

/// One page of the result, in order.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutPage {
    /// A page of the book (1-based), turned by `rotate` degrees clockwise
    /// and cut to `crop` (x, y, w, h as fractions of the page as shown
    /// after turning).
    #[serde(rename_all = "camelCase")]
    Page {
        page: u32,
        #[serde(default)]
        rotate: i32,
        #[serde(default)]
        crop: Option<[f64; 4]>,
    },
    /// A page of another PDF (`files[file]`).
    #[serde(rename_all = "camelCase")]
    File {
        file: u32,
        page: u32,
        #[serde(default)]
        rotate: i32,
        #[serde(default)]
        crop: Option<[f64; 4]>,
    },
    /// An empty page, in points.
    Blank { width: f64, height: f64 },
    /// A picture as a page (camera, phone, scanner or a picture file).
    Picture { src: String },
}

/// Boxes to black out on a page of the book (fractions of the shown page).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Redaction {
    pub page: u32,
    pub boxes: Vec<[f64; 4]>,
    #[serde(default = "black")]
    pub color: String,
}

fn black() -> String {
    "#000000".into()
}

/// A small text correction: the old words under `rect` are removed and
/// covered, and `text` is written in their place.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Correction {
    pub page: u32,
    pub rect: [f64; 4],
    pub text: String,
    /// Font size as a fraction of the page width.
    pub size: f64,
    pub color: String,
    /// The paper colour around the old words.
    pub background: String,
    /// "sans", "serif" or "mono".
    pub font: String,
    /// The text drawn as a picture, used when it has characters the
    /// standard PDF fonts cannot show.
    #[serde(default)]
    pub picture: Option<String>,
}

/// How much to shrink pictures.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    High,
    Medium,
    Small,
}

impl Quality {
    fn limits(self) -> (u32, u8) {
        match self {
            Quality::High => (3000, 85),
            Quality::Medium => (2000, 72),
            Quality::Small => (1400, 55),
        }
    }
}

/// Everything to do to a PDF.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditPlan {
    pub pages: Vec<OutPage>,
    /// Other PDFs that pages come from (absolute paths, set by the app).
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub redactions: Vec<Redaction>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
    #[serde(default)]
    pub compress: Option<Quality>,
    /// Write saved OCR text into the PDF as an invisible text layer.
    #[serde(default)]
    pub embed_ocr: bool,
}

/// OCR words of one page of the book, for the text layer.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrWords {
    pub page: u32,
    /// Text and box (x, y, w, h as fractions of the page).
    pub words: Vec<(String, [f64; 4])>,
}

/// Where a page of the old file went.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMove {
    /// Page of the book before (None: a new page).
    pub from: Option<u32>,
    /// Page after.
    pub to: u32,
    pub rotate: i32,
    pub crop: Option<[f64; 4]>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditReport {
    pub pages: u32,
    pub page_map: Vec<PageMove>,
    /// Pages (of the book) turned into pictures so nothing was left under
    /// a redaction.
    pub flattened: Vec<u32>,
    pub warnings: Vec<String>,
}

fn rect_path(b: &[f64; 4]) -> String {
    let [x, y, w, h] = *b;
    format!(
        "M {x} {y} L {} {y} L {} {} L {x} {} Z",
        x + w,
        x + w,
        y + h,
        y + h
    )
}

// ---------- text ----------

/// Windows-1252 byte for a character, as the standard fonts are used.
pub(crate) fn cp1252(c: char) -> Option<u8> {
    let u = c as u32;
    if (0x20..0x7F).contains(&u) || (0xA0..=0xFF).contains(&u) {
        return Some(u as u8);
    }
    const HIGH: [(char, u8); 27] = [
        ('€', 0x80),
        ('‚', 0x82),
        ('ƒ', 0x83),
        ('„', 0x84),
        ('…', 0x85),
        ('†', 0x86),
        ('‡', 0x87),
        ('ˆ', 0x88),
        ('‰', 0x89),
        ('Š', 0x8A),
        ('‹', 0x8B),
        ('Œ', 0x8C),
        ('Ž', 0x8E),
        ('‘', 0x91),
        ('’', 0x92),
        ('“', 0x93),
        ('”', 0x94),
        ('•', 0x95),
        ('–', 0x96),
        ('—', 0x97),
        ('˜', 0x98),
        ('™', 0x99),
        ('š', 0x9A),
        ('›', 0x9B),
        ('œ', 0x9C),
        ('ž', 0x9E),
        ('Ÿ', 0x9F),
    ];
    HIGH.iter().find(|(ch, _)| *ch == c).map(|(_, b)| *b)
}

fn std_font(font: &str) -> (&'static str, Option<&'static [(&'static str, u16)]>) {
    match font {
        "serif" => ("Times-Roman", Some(std_fonts::TIMES_ROMAN)),
        "mono" => ("Courier", None),
        _ => ("Helvetica", Some(std_fonts::HELVETICA)),
    }
}

pub(crate) fn width_of(bytes: &[u8], table: Option<&[(&str, u16)]>) -> f64 {
    bytes
        .iter()
        .map(|&b| match table {
            None => 600.0,
            Some(t) => {
                let name = std_fonts::WIN_ANSI[b as usize];
                t.binary_search_by(|(k, _)| k.cmp(&name))
                    .map_or(500.0, |i| f64::from(t[i].1))
            }
        })
        .sum::<f64>()
        / 1000.0
}

/// Splits text into lines that fit `width` points.
pub(crate) fn wrap(
    text: &str,
    size: f64,
    width: f64,
    table: Option<&[(&str, u16)]>,
) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();
    for para in text.lines() {
        let mut line: Vec<u8> = Vec::new();
        for word in para.split(' ') {
            let w: Vec<u8> = word.chars().filter_map(cp1252).collect();
            let mut candidate = line.clone();
            if !candidate.is_empty() {
                candidate.push(b' ');
            }
            candidate.extend(&w);
            if !line.is_empty() && width_of(&candidate, table) * size > width {
                lines.push(std::mem::take(&mut line));
                line = w;
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

pub(crate) fn pdf_string(bytes: &[u8]) -> String {
    let mut s = String::from("(");
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(b as char);
            }
            0x20..=0x7E => s.push(b as char),
            _ => {
                let _ = write!(s, "\\{b:03o}");
            }
        }
    }
    s.push(')');
    s
}

/// Adds a content stream (wrapped in q … Q) and fonts to a page.
fn append(
    doc: &mut Document,
    page: ObjectId,
    body: String,
    fonts: Vec<(String, Object)>,
) -> Result<()> {
    let mut stream = Stream::new(Dictionary::new(), body.into_bytes());
    let _ = stream.compress();
    let id = doc.add_object(stream);
    let mut res = match doc
        .get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"Resources").ok())
    {
        Some(Object::Dictionary(d)) => d.clone(),
        _ => own_resources(doc, page),
    };
    if !fonts.is_empty() {
        let mut fd = match res.get(b"Font") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(r)) => doc.get_dictionary(*r).cloned().unwrap_or_default(),
            _ => Dictionary::new(),
        };
        for (name, font) in fonts {
            fd.set(name.into_bytes(), font);
        }
        res.set("Font", fd);
    }
    let contents: Vec<Object> = match doc
        .get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"Contents").ok())
    {
        Some(Object::Array(a)) => a.clone(),
        Some(o @ Object::Reference(r)) => match doc.get_object(*r) {
            Ok(Object::Array(a)) => a.clone(),
            _ => vec![o.clone()],
        },
        _ => Vec::new(),
    };
    // Earlier drawing must not leak its state into ours.
    let q = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let end = doc.add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));
    let mut all = vec![Object::Reference(q)];
    all.extend(contents);
    all.push(Object::Reference(end));
    all.push(Object::Reference(id));
    let d = doc
        .get_dictionary_mut(page)
        .map_err(|e| Error::Read(e.to_string()))?;
    d.set("Contents", all);
    d.set("Resources", res);
    Ok(())
}

/// Writes a correction's text; false when it has characters the standard
/// fonts cannot show.
fn write_text(doc: &mut Document, page: ObjectId, c: &Correction, n: usize) -> Result<bool> {
    if c.text.chars().any(|ch| ch != '\n' && cp1252(ch).is_none()) {
        return Ok(false);
    }
    let geo = Geometry::of(doc, page);
    let (w, _) = geo.shown();
    let (base, table) = std_font(&c.font);
    let size = (c.size * w).max(2.0);
    let [x, y, bw, _] = c.rect;
    let [r, g, b] = color(&c.color);
    let m = geo.matrix();
    let name = format!("LbFix{n}");
    let mut body = format!(
        "q {} {} {} {} {} {} cm BT /{name} {} Tf {} {} {} rg\n",
        f(m[0]),
        f(m[1]),
        f(m[2]),
        f(m[3]),
        f(m[4]),
        f(m[5]),
        f(size),
        f(r),
        f(g),
        f(b)
    );
    let (_, h) = geo.shown();
    let left = x * w + size * 0.1;
    let mut baseline = y * h + size * 0.95;
    for line in wrap(&c.text, size, bw * w - size * 0.2, table) {
        let _ = writeln!(
            body,
            "1 0 0 -1 {} {} Tm {} Tj",
            f(left),
            f(baseline),
            pdf_string(&line)
        );
        baseline += size * 1.2;
    }
    body.push_str("ET Q\n");
    let font = dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => base, "Encoding" => "WinAnsiEncoding",
    };
    let id = doc.add_object(font);
    append(doc, page, body, vec![(name, Object::Reference(id))])?;
    Ok(true)
}

// ---------- OCR text layer ----------

/// A font for invisible text: two-byte codes that are Unicode code units,
/// every character half an em wide, no glyphs. Text shown with it can be
/// found and selected but draws nothing.
fn glyphless_font(doc: &mut Document) -> ObjectId {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Libreri-Identity-UCS def /CMapType 2 def\n\
         1 begincodespacerange <0000> <FFFF> endcodespacerange\n",
    );
    for chunk in (0u32..256).collect::<Vec<_>>().chunks(100) {
        let _ = writeln!(cmap, "{} beginbfrange", chunk.len());
        for &hi in chunk {
            let _ = writeln!(cmap, "<{hi:02X}00> <{hi:02X}FF> <{hi:02X}00>");
        }
        cmap.push_str("endbfrange\n");
    }
    cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
    let mut to_unicode = Stream::new(Dictionary::new(), cmap.into_bytes());
    let _ = to_unicode.compress();
    let to_unicode = doc.add_object(to_unicode);
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "GlyphLessFont", "Flags" => 5,
        "FontBBox" => vec![0.into(), 0.into(), 500.into(), 1000.into()],
        "ItalicAngle" => 0, "Ascent" => 1000, "Descent" => 0, "CapHeight" => 1000, "StemV" => 80,
    });
    let cid = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "GlyphLessFont",
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => descriptor, "DW" => 500, "CIDToGIDMap" => "Identity",
    });
    doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "GlyphLessFont",
        "Encoding" => "Identity-H", "DescendantFonts" => vec![Object::Reference(cid)],
        "ToUnicode" => to_unicode,
    })
}

fn write_ocr_layer(
    doc: &mut Document,
    page: ObjectId,
    font: ObjectId,
    words: &[(String, [f64; 4])],
) -> Result<()> {
    let geo = Geometry::of(doc, page);
    let (w, h) = geo.shown();
    let m = geo.matrix();
    let mut body = format!(
        "q {} {} {} {} {} {} cm BT 3 Tr\n",
        f(m[0]),
        f(m[1]),
        f(m[2]),
        f(m[3]),
        f(m[4]),
        f(m[5])
    );
    for (text, [x, y, bw, bh]) in words {
        let units: Vec<u16> = text.encode_utf16().collect();
        if units.is_empty() || *bh <= 0.0 || *bw <= 0.0 {
            continue;
        }
        let size = (bh * h).max(1.0);
        let natural = units.len() as f64 * 0.5 * size;
        let tz = (bw * w / natural * 100.0).clamp(1.0, 1000.0);
        let hex: String = units.iter().map(|u| format!("{u:04X}")).collect();
        let _ = writeln!(
            body,
            "/LbOcr {} Tf {} Tz 1 0 0 -1 {} {} Tm <{hex}> Tj",
            f(size),
            f(tz),
            f(x * w),
            f((y + bh) * h - size * 0.1)
        );
    }
    body.push_str("ET Q\n");
    append(
        doc,
        page,
        body,
        vec![("LbOcr".into(), Object::Reference(font))],
    )
}

// ---------- pages ----------

/// Copies inherited attributes onto the page, so it can move anywhere.
fn materialize(doc: &mut Document, page: ObjectId) {
    for key in [b"MediaBox".as_slice(), b"CropBox", b"Rotate", b"Resources"] {
        let has = doc.get_dictionary(page).is_ok_and(|d| d.has(key));
        if has {
            continue;
        }
        if let Some(v) = inherited(doc, page, key) {
            if let Ok(d) = doc.get_dictionary_mut(page) {
                d.set(key, v);
            }
        }
    }
}

fn picture_page(doc: &mut Document, pages_root: ObjectId, src: &str) -> Result<ObjectId> {
    let bytes = decode_data_url(src)?;
    picture_page_bytes(doc, pages_root, bytes)
}

fn picture_page_bytes(
    doc: &mut Document,
    pages_root: ObjectId,
    bytes: Vec<u8>,
) -> Result<ObjectId> {
    let (pw, ph, xobj) = if bytes.starts_with(&[0xFF, 0xD8]) {
        let img = image::load_from_memory(&bytes).map_err(|e| Error::Picture(e.to_string()))?;
        let gray = img.color().channel_count() == 1;
        let (w, h) = (img.width(), img.height());
        let id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => i64::from(w), "Height" => i64::from(h),
                "ColorSpace" => if gray { "DeviceGray" } else { "DeviceRGB" },
                "BitsPerComponent" => 8, "Filter" => "DCTDecode",
            },
            bytes,
        ));
        (w, h, id)
    } else {
        let img = image::load_from_memory(&bytes).map_err(|e| Error::Picture(e.to_string()))?;
        let (w, h) = (img.width(), img.height());
        (w, h, image_xobject(doc, &bytes)?)
    };
    let width = 595.28;
    let height = width * f64::from(ph) / f64::from(pw.max(1));
    let content = doc.add_object(Stream::new(
        Dictionary::new(),
        format!("q {} 0 0 {} 0 0 cm /Picture Do Q\n", f(width), f(height)).into_bytes(),
    ));
    Ok(doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_root,
        "MediaBox" => vec![0.into(), 0.into(), Object::Real(width as f32), Object::Real(height as f32)],
        "Contents" => content,
        "Resources" => dictionary! { "XObject" => dictionary! { "Picture" => xobj } },
    }))
}

fn turn_and_crop(
    doc: &mut Document,
    page: ObjectId,
    rotate: i32,
    crop: Option<[f64; 4]>,
) -> Result<()> {
    let current = doc
        .get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"Rotate").ok())
        .and_then(num)
        .unwrap_or(0.0) as i64;
    let turned = (current + i64::from(rotate)).rem_euclid(360) / 90 * 90;
    let d = doc
        .get_dictionary_mut(page)
        .map_err(|e| Error::Read(e.to_string()))?;
    d.set("Rotate", turned);
    if let Some([x, y, w, h]) = crop {
        let geo = Geometry::of(doc, page);
        let (ax, ay) = geo.point_at(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        let (bx, by) = geo.point_at((x + w).clamp(0.0, 1.0), (y + h).clamp(0.0, 1.0));
        let r = [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)];
        if r[2] - r[0] > 1.0 && r[3] - r[1] > 1.0 {
            let d = doc
                .get_dictionary_mut(page)
                .map_err(|e| Error::Read(e.to_string()))?;
            d.set(
                "CropBox",
                r.iter()
                    .map(|v| Object::Real(*v as f32))
                    .collect::<Vec<_>>(),
            );
        }
    }
    Ok(())
}

fn is_identity(plan: &EditPlan, count: u32) -> bool {
    plan.pages.len() == count as usize
        && plan.pages.iter().enumerate().all(|(i, p)| {
            matches!(p, OutPage::Page { page, rotate: 0, crop: None } if *page == i as u32 + 1)
        })
}

// ---------- compressing ----------

fn compress_images(doc: &mut Document, quality: Quality) -> u32 {
    let (max_side, q) = quality.limits();
    let ids: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter(|(_, o)| {
            o.as_stream().is_ok_and(|s| {
                s.dict.get(b"Subtype").ok().and_then(|n| n.as_name().ok()) == Some(b"Image")
                    && s.content.len() > 60_000
            })
        })
        .map(|(id, _)| *id)
        .collect();
    let mut changed = 0;
    for id in ids {
        let Ok(Object::Stream(s)) = doc.get_object(id) else {
            continue;
        };
        let d = &s.dict;
        if matches!(d.get(b"ImageMask"), Ok(Object::Boolean(true))) || d.has(b"Decode") {
            continue;
        }
        let filter = d
            .get(b"Filter")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(<[u8]>::to_vec);
        let img = match filter.as_deref() {
            Some(b"DCTDecode") => {
                image::load_from_memory_with_format(&s.content, image::ImageFormat::Jpeg).ok()
            }
            Some(b"FlateDecode") | None => {
                let bpc = d.get(b"BitsPerComponent").ok().and_then(num).unwrap_or(0.0) as u32;
                let cs = d
                    .get(b"ColorSpace")
                    .ok()
                    .and_then(|o| o.as_name().ok())
                    .map(<[u8]>::to_vec);
                let comps = match cs.as_deref() {
                    Some(b"DeviceGray") => 1,
                    Some(b"DeviceRGB") => 3,
                    _ => 0,
                };
                let (w, h) = (
                    d.get(b"Width").ok().and_then(num).unwrap_or(0.0) as u32,
                    d.get(b"Height").ok().and_then(num).unwrap_or(0.0) as u32,
                );
                if bpc != 8 || comps == 0 || d.has(b"DecodeParms") {
                    None
                } else {
                    let raw = s.decompressed_content().ok();
                    raw.and_then(|r| match comps {
                        1 => {
                            image::GrayImage::from_raw(w, h, r).map(image::DynamicImage::ImageLuma8)
                        }
                        _ => image::RgbImage::from_raw(w, h, r).map(image::DynamicImage::ImageRgb8),
                    })
                }
            }
            _ => None,
        };
        let Some(img) = img else { continue };
        if img.color().channel_count() == 4 {
            continue;
        }
        let img = if img.width().max(img.height()) > max_side {
            img.resize(max_side, max_side, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        let gray = img.color().channel_count() == 1;
        let mut out = std::io::Cursor::new(Vec::new());
        let encoded = if gray {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q)
                .encode_image(&img.to_luma8())
        } else {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q)
                .encode_image(&img.to_rgb8())
        };
        if encoded.is_err() {
            continue;
        }
        let bytes = out.into_inner();
        if bytes.len() as f64 > s.content.len() as f64 * 0.9 {
            continue;
        }
        let mut dict = s.dict.clone();
        dict.set("Filter", "DCTDecode");
        dict.remove(b"DecodeParms");
        dict.remove(b"Length");
        dict.set("Width", i64::from(img.width()));
        dict.set("Height", i64::from(img.height()));
        dict.set("BitsPerComponent", 8);
        if !matches!(dict.get(b"ColorSpace"), Ok(Object::Array(_))) {
            dict.set("ColorSpace", if gray { "DeviceGray" } else { "DeviceRGB" });
        }
        doc.objects
            .insert(id, Object::Stream(Stream::new(dict, bytes)));
        changed += 1;
    }
    changed
}

// ---------- flattening ----------

/// Replaces a page's drawing with a picture of it (from `pdf`, which shows
/// the page as it is now).
fn flatten(
    doc: &mut Document,
    page: ObjectId,
    pdf: &hayro::hayro_syntax::Pdf,
    index: usize,
) -> Result<()> {
    let jpeg = inspect::page_jpeg(pdf, index, 200.0)
        .ok_or_else(|| Error::Read(format!("page {} could not be drawn", index + 1)))?;
    let img = image::load_from_memory(&jpeg).map_err(|e| Error::Picture(e.to_string()))?;
    let xobj = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image",
            "Width" => i64::from(img.width()), "Height" => i64::from(img.height()),
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
        },
        jpeg,
    ));
    let geo = Geometry::of(doc, page);
    let (w, h) = geo.shown();
    let m = geo.matrix();
    let body = format!(
        "q {} {} {} {} {} {} cm {} 0 0 {} 0 {} cm /LbPage Do Q\n",
        f(m[0]),
        f(m[1]),
        f(m[2]),
        f(m[3]),
        f(m[4]),
        f(m[5]),
        f(w),
        f(-h),
        f(h)
    );
    let content = doc.add_object(Stream::new(Dictionary::new(), body.into_bytes()));
    let d = doc
        .get_dictionary_mut(page)
        .map_err(|e| Error::Read(e.to_string()))?;
    d.set("Contents", content);
    d.set(
        "Resources",
        dictionary! { "XObject" => dictionary! { "LbPage" => xobj } },
    );
    Ok(())
}

fn to_bytes(doc: &mut Document) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

/// Makes a PDF of pictures (JPEG or PNG), one page each, A4 wide, with
/// OCR words (pages counted from 1) written as an invisible text layer so
/// the PDF can be searched and its text selected.
pub fn pictures_pdf(pictures: Vec<Vec<u8>>, ocr: &[OcrWords], dest: &Path) -> Result<u32> {
    let mut doc = Document::with_version("1.5");
    let root = doc.new_object_id();
    let mut kids = Vec::new();
    for bytes in pictures {
        kids.push(picture_page_bytes(&mut doc, root, bytes)?);
    }
    if !ocr.is_empty() {
        let font = glyphless_font(&mut doc);
        for o in ocr {
            if let Some(&id) = kids.get(o.page.saturating_sub(1) as usize) {
                write_ocr_layer(&mut doc, id, font, &o.words)?;
            }
        }
    }
    let count = kids.len() as u32;
    doc.objects.insert(
        root,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => i64::from(count),
            "Kids" => kids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    doc.trailer.set("Root", catalog);
    save(&mut doc, dest)?;
    Ok(count)
}

/// Applies `plan` to the PDF at `src` and writes the result to `dest`.
pub fn apply(src: &Path, plan: &EditPlan, ocr: &[OcrWords], dest: &Path) -> Result<EditReport> {
    let mut doc = Document::load(src).map_err(|e| Error::Read(e.to_string()))?;
    if doc.is_encrypted() {
        return Err(Error::Encrypted);
    }
    let ids = doc.get_pages();
    let count = ids.len() as u32;
    let page_id = |p: u32| ids.get(&p).copied().ok_or(Error::NoPage(p));
    let mut report = EditReport::default();

    // 1. The OCR text layer, first, so redaction also removes OCR words.
    if plan.embed_ocr && !ocr.is_empty() {
        let font = glyphless_font(&mut doc);
        for o in ocr {
            let id = page_id(o.page)?;
            write_ocr_layer(&mut doc, id, font, &o.words)?;
        }
    }

    // 2. Redactions and corrections on the book's pages.
    let mut per_page: BTreeMap<u32, (Vec<&Redaction>, Vec<&Correction>)> = BTreeMap::new();
    for r in &plan.redactions {
        per_page.entry(r.page).or_default().0.push(r);
    }
    for c in &plan.corrections {
        per_page.entry(c.page).or_default().1.push(c);
    }
    let mut check: BTreeMap<u32, Vec<FBox>> = BTreeMap::new();
    let mut must_flatten: HashSet<u32> = HashSet::new();
    let mut fix_count = 0;
    for (&page, (reds, fixes)) in &per_page {
        let id = page_id(page)?;
        let boxes: Vec<FBox> = reds
            .iter()
            .flat_map(|r| r.boxes.iter().map(|b| FBox::from_xywh(*b)))
            .chain(fixes.iter().map(|c| FBox::from_xywh(c.rect)))
            .collect();
        let mut res = own_resources(&doc, id);
        let outcome = redact::remove_under(&mut doc, id, &mut res, &boxes)?;
        doc.get_dictionary_mut(id)
            .map_err(|e| Error::Read(e.to_string()))?
            .set("Resources", res);
        if outcome.needs_flatten {
            must_flatten.insert(page);
            for n in outcome.notes {
                report.warnings.push(format!("page {page}: {n}"));
            }
        }
        let mut ops: Vec<DrawOp> = Vec::new();
        for r in reds {
            for b in &r.boxes {
                ops.push(DrawOp::Fill {
                    d: rect_path(b),
                    color: r.color.clone(),
                    opacity: 1.0,
                    multiply: false,
                });
            }
        }
        for c in fixes {
            ops.push(DrawOp::Fill {
                d: rect_path(&c.rect),
                color: c.background.clone(),
                opacity: 1.0,
                multiply: false,
            });
        }
        draw_on_page(&mut doc, id, &ops)?;
        for c in fixes {
            fix_count += 1;
            if !write_text(&mut doc, id, c, fix_count)? {
                match &c.picture {
                    Some(p) => draw_on_page(
                        &mut doc,
                        id,
                        &[DrawOp::Image {
                            rect: c.rect,
                            src: p.clone(),
                        }],
                    )?,
                    None => report.warnings.push(format!(
                        "page {page}: the correction has letters the PDF fonts cannot show"
                    )),
                }
            }
        }
        check.insert(page, boxes);
    }

    // 3. Check: nothing may be left under a box; otherwise flatten the page.
    if !check.is_empty() || !must_flatten.is_empty() {
        let bytes = to_bytes(&mut doc)?;
        let pdf = inspect::open(bytes)
            .ok_or_else(|| Error::Read("the edited PDF could not be read back".into()))?;
        for &page in check.keys() {
            // Corrections put new text in their boxes; only redactions must be empty.
            let red_boxes: Vec<FBox> = plan
                .redactions
                .iter()
                .filter(|r| r.page == page)
                .flat_map(|r| r.boxes.iter().map(|b| FBox::from_xywh(*b)))
                .collect();
            let leak = !red_boxes.is_empty()
                && inspect::glyph_middles(&pdf, page as usize - 1)
                    .into_iter()
                    .any(|p| red_boxes.iter().any(|b| b.contains(p)));
            if leak {
                must_flatten.insert(page);
            }
        }
        let mut pages: Vec<u32> = must_flatten.iter().copied().collect();
        pages.sort_unstable();
        for page in pages {
            flatten(&mut doc, page_id(page)?, &pdf, page as usize - 1)?;
            report.flattened.push(page);
        }
    }

    // 4. Page order, turning, cropping and new pages.
    let root = doc
        .catalog()
        .map_err(|e| Error::Read(e.to_string()))?
        .get(b"Pages")
        .and_then(Object::as_reference)
        .map_err(|e| Error::Read(e.to_string()))?;
    if !is_identity(plan, count) {
        if plan.pages.is_empty() {
            return Err(Error::Path("a PDF needs at least one page".into()));
        }
        for id in ids.values() {
            materialize(&mut doc, *id);
        }
        let mut files: Vec<Vec<ObjectId>> = Vec::new();
        for path in &plan.files {
            let mut other =
                Document::load(path).map_err(|e| Error::Read(format!("{path}: {e}")))?;
            if other.is_encrypted() {
                return Err(Error::Encrypted);
            }
            let other_pages: Vec<ObjectId> = other.get_pages().values().copied().collect();
            for id in &other_pages {
                materialize(&mut other, *id);
            }
            let first = doc.max_id + 1;
            other.renumber_objects_with(first);
            doc.max_id = other.max_id;
            let renumbered: Vec<ObjectId> = other.get_pages().values().copied().collect();
            doc.objects.extend(other.objects);
            files.push(renumbered);
        }
        let mut used: HashSet<ObjectId> = HashSet::new();
        let mut kids = Vec::new();
        for (i, out) in plan.pages.iter().enumerate() {
            let (id, rotate, crop, from) = match out {
                OutPage::Page { page, rotate, crop } => {
                    (page_id(*page)?, *rotate, *crop, Some(*page))
                }
                OutPage::File {
                    file,
                    page,
                    rotate,
                    crop,
                } => {
                    let list = files.get(*file as usize).ok_or(Error::NoPage(*page))?;
                    let id = *list
                        .get(page.saturating_sub(1) as usize)
                        .ok_or(Error::NoPage(*page))?;
                    (id, *rotate, *crop, None)
                }
                OutPage::Blank { width, height } => {
                    let content = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));
                    let id = doc.add_object(dictionary! {
                        "Type" => "Page", "Parent" => root,
                        "MediaBox" => vec![0.into(), 0.into(), Object::Real(*width as f32), Object::Real(*height as f32)],
                        "Contents" => content, "Resources" => Dictionary::new(),
                    });
                    (id, 0, None, None)
                }
                OutPage::Picture { src } => (picture_page(&mut doc, root, src)?, 0, None, None),
            };
            // The same page twice: the second is a copy.
            let id = if used.contains(&id) {
                let copy = doc
                    .get_dictionary(id)
                    .map_err(|e| Error::Read(e.to_string()))?
                    .clone();
                doc.add_object(copy)
            } else {
                id
            };
            used.insert(id);
            turn_and_crop(&mut doc, id, rotate, crop)?;
            doc.get_dictionary_mut(id)
                .map_err(|e| Error::Read(e.to_string()))?
                .set("Parent", root);
            kids.push(Object::Reference(id));
            report.page_map.push(PageMove {
                from,
                to: i as u32 + 1,
                rotate,
                crop,
            });
        }
        // Pages left out keep no content (outlines may still point at them).
        for id in ids.values() {
            if !used.contains(id) {
                if let Ok(d) = doc.get_dictionary_mut(*id) {
                    d.set("Contents", Vec::<Object>::new());
                    d.set("Resources", Dictionary::new());
                    d.remove(b"Annots");
                }
            }
        }
        let n = kids.len() as i64;
        let d = doc
            .get_dictionary_mut(root)
            .map_err(|e| Error::Read(e.to_string()))?;
        d.set("Kids", kids);
        d.set("Count", n);
    } else {
        report.page_map = (1..=count)
            .map(|p| PageMove {
                from: Some(p),
                to: p,
                rotate: 0,
                crop: None,
            })
            .collect();
    }
    report.pages = report.page_map.len() as u32;

    // 5. Smaller pictures.
    if let Some(q) = plan.compress {
        let n = compress_images(&mut doc, q);
        if n == 0 {
            report
                .warnings
                .push("no pictures could be made smaller".into());
        }
    }

    doc.prune_objects();
    save(&mut doc, dest)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libreri_formats::pdftext::page_texts;
    use lopdf::{dictionary, Stream};

    const LINE: &str = "The lighthouse keeper wrote every night";

    fn sample(dir: &Path, name: &str, pages: &[&str]) -> std::path::PathBuf {
        let p = dir.join(name);
        libreri_formats::test_text_pdf(&p, pages);
        p
    }

    fn page(p: u32) -> OutPage {
        OutPage::Page {
            page: p,
            rotate: 0,
            crop: None,
        }
    }

    fn texts(p: &Path) -> Vec<String> {
        page_texts(p).unwrap()
    }

    #[test]
    fn redaction_removes_the_words_under_the_box() {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path(), "a.pdf", &[LINE]);
        let dest = dir.path().join("b.pdf");
        let plan = EditPlan {
            pages: vec![page(1)],
            redactions: vec![Redaction {
                page: 1,
                boxes: vec![[0.155, 0.095, 0.09, 0.03]],
                color: black(),
            }],
            ..Default::default()
        };
        let report = apply(&src, &plan, &[], &dest).unwrap();
        assert!(report.flattened.is_empty(), "{report:?}");
        let t = texts(&dest)[0].clone();
        assert!(!t.contains("lighthouse"), "{t}");
        assert!(
            t.contains("The") && t.contains("keeper wrote every night"),
            "{t}"
        );
        // What stays is where it was: "keeper" still starts at the same place.
        let pdf = inspect::open(std::fs::read(&dest).unwrap()).unwrap();
        let before = inspect::open(std::fs::read(&src).unwrap()).unwrap();
        let after = inspect::glyph_middles(&pdf, 0);
        let orig = inspect::glyph_middles(&before, 0);
        let last = |v: &[(f64, f64)]| *v.last().unwrap();
        assert!((last(&after).0 - last(&orig).0).abs() < 1e-3);
        assert_eq!(after.len(), orig.len() - "lighthouse".len());
    }

    #[test]
    fn pages_are_reordered_turned_cropped_and_added() {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path(), "a.pdf", &["one", "two", "three"]);
        let other = sample(dir.path(), "o.pdf", &["alpha", "beta"]);
        let dest = dir.path().join("b.pdf");
        let plan = EditPlan {
            pages: vec![
                OutPage::Page {
                    page: 3,
                    rotate: 90,
                    crop: None,
                },
                page(1),
                OutPage::File {
                    file: 0,
                    page: 2,
                    rotate: 0,
                    crop: None,
                },
                OutPage::Blank {
                    width: 300.0,
                    height: 400.0,
                },
                OutPage::Page {
                    page: 1,
                    rotate: 0,
                    crop: Some([0.0, 0.0, 0.5, 0.5]),
                },
            ],
            files: vec![other.to_string_lossy().into_owned()],
            ..Default::default()
        };
        let report = apply(&src, &plan, &[], &dest).unwrap();
        assert_eq!(report.pages, 5);
        assert_eq!(report.page_map[0].from, Some(3));
        assert_eq!(report.page_map[2].from, None);
        let t: Vec<String> = texts(&dest).iter().map(|s| s.trim().to_owned()).collect();
        assert_eq!(t, ["three", "one", "beta", "", "one"]);
        let doc = Document::load(&dest).unwrap();
        let ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
        let rot = |i: usize| {
            doc.get_dictionary(ids[i])
                .unwrap()
                .get(b"Rotate")
                .ok()
                .and_then(num)
        };
        assert_eq!(rot(0), Some(90.0));
        let g = Geometry::of(&doc, ids[4]);
        assert_eq!((g.x0, g.y0, g.x1, g.y1), (0.0, 396.0, 306.0, 792.0));
        assert_eq!(Geometry::of(&doc, ids[3]).shown(), (300.0, 400.0));
        // Page two was left out: its text is gone from the file.
        let raw = std::fs::read(&dest).unwrap();
        assert!(!raw.windows(5).any(|w| w == b"(two)"));
    }

    #[test]
    fn corrections_replace_the_words() {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path(), "a.pdf", &[LINE]);
        let dest = dir.path().join("b.pdf");
        let plan = EditPlan {
            pages: vec![page(1)],
            corrections: vec![Correction {
                page: 1,
                rect: [0.155, 0.095, 0.095, 0.03],
                text: "lamplighter".into(),
                size: 12.0 / 612.0,
                color: "#000000".into(),
                background: "#ffffff".into(),
                font: "sans".into(),
                picture: None,
            }],
            ..Default::default()
        };
        let report = apply(&src, &plan, &[], &dest).unwrap();
        assert!(report.warnings.is_empty(), "{report:?}");
        let t = texts(&dest)[0].clone();
        assert!(
            t.contains("lamplighter") && !t.contains("lighthouse"),
            "{t}"
        );
    }

    #[test]
    fn ocr_words_become_invisible_text() {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path(), "a.pdf", &[""]);
        let dest = dir.path().join("b.pdf");
        let plan = EditPlan {
            pages: vec![page(1)],
            embed_ocr: true,
            ..Default::default()
        };
        let ocr = [OcrWords {
            page: 1,
            words: vec![
                ("Lighthouse".into(), [0.1, 0.1, 0.3, 0.03]),
                ("ÆON".into(), [0.5, 0.1, 0.1, 0.03]),
            ],
        }];
        apply(&src, &plan, &ocr, &dest).unwrap();
        let t = texts(&dest)[0].clone();
        assert!(t.contains("Lighthouse") && t.contains("ÆON"), "{t}");
        // Nothing shows.
        let pdf = inspect::open(std::fs::read(&dest).unwrap()).unwrap();
        let jpeg = inspect::page_jpeg(&pdf, 0, 36.0).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap().to_luma8();
        assert!(img.pixels().all(|p| p.0[0] > 240));
    }

    #[test]
    fn pictures_are_made_smaller_and_become_pages() {
        use base64::Engine;
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path(), "a.pdf", &["one"]);
        let noisy = image::RgbImage::from_fn(1600, 1200, |x, y| {
            image::Rgb([
                (x * 7 % 251) as u8,
                (y * 13 % 241) as u8,
                ((x ^ y) % 256) as u8,
            ])
        });
        let mut png = std::io::Cursor::new(Vec::new());
        noisy.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let src_url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png.into_inner())
        );
        let big = dir.path().join("big.pdf");
        let plan = EditPlan {
            pages: vec![page(1), OutPage::Picture { src: src_url }],
            ..Default::default()
        };
        apply(&src, &plan, &[], &big).unwrap();
        let doc = Document::load(&big).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let small = dir.path().join("small.pdf");
        let plan = EditPlan {
            pages: vec![page(1), page(2)],
            compress: Some(Quality::Small),
            ..Default::default()
        };
        let report = apply(&big, &plan, &[], &small).unwrap();
        assert!(report.warnings.is_empty(), "{report:?}");
        let (a, b) = (
            std::fs::metadata(&big).unwrap().len(),
            std::fs::metadata(&small).unwrap().len(),
        );
        assert!(b < a / 2, "{a} -> {b}");
    }

    /// A page with kerned text, text inside a form, a picture and a
    /// rectangle, turned 90 degrees.
    fn harder(path: &Path) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Roman",
        });
        let form = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            },
            b"BT /F1 20 Tf 100 400 Td (secretform) Tj ET".to_vec(),
        ));
        let pixels: Vec<u8> = (0..100 * 100).flat_map(|_| [200u8, 30, 30]).collect();
        let mut img = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 100, "Height" => 100,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
            },
            pixels,
        );
        img.compress().unwrap();
        let img = doc.add_object(img);
        let body = b"BT /F1 20 Tf 100 600 Td [(A) 120 (gent) -300 (secret) 50 (word)] TJ ET\n\
            q 1 0 0 1 0 -100 cm /Fm Do Q\n\
            q 200 0 0 200 300 100 cm /Im Do Q\n\
            0 0 1 rg 100 200 50 50 re f\n"
            .to_vec();
        let content = doc.add_object(Stream::new(dictionary! {}, body));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Rotate" => 90,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content,
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => font },
                "XObject" => dictionary! { "Fm" => form, "Im" => img },
            },
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page)],
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    /// A box around user-space points, as fractions of the shown page.
    fn around(path: &Path, x0: f64, y0: f64, x1: f64, y1: f64) -> [f64; 4] {
        let doc = Document::load(path).unwrap();
        let id = *doc.get_pages().get(&1).unwrap();
        let g = Geometry::of(&doc, id);
        let (a, b) = (g.fraction_of(x0, y0), g.fraction_of(x1, y1));
        [
            a.0.min(b.0),
            a.1.min(b.1),
            (a.0 - b.0).abs(),
            (a.1 - b.1).abs(),
        ]
    }

    #[test]
    fn redaction_reaches_kerned_text_forms_pictures_and_shapes() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        harder(&src);
        let before = texts(&src)[0].clone();
        assert!(
            before.contains("secret") && before.contains("secretform"),
            "{before}"
        );
        // "Agent" ends near 147 (20pt Times, kerned); "secret" starts near 152.
        let boxes = vec![
            around(&src, 150.0, 595.0, 205.0, 620.0),
            around(&src, 95.0, 295.0, 400.0, 320.0),
            around(&src, 280.0, 80.0, 400.0, 200.0),
            around(&src, 90.0, 190.0, 160.0, 260.0),
        ];
        let dest = dir.path().join("b.pdf");
        let plan = EditPlan {
            pages: vec![page(1)],
            redactions: vec![Redaction {
                page: 1,
                boxes,
                color: black(),
            }],
            ..Default::default()
        };
        let report = apply(&src, &plan, &[], &dest).unwrap();
        if let Some(d) = std::env::var_os("LIBRERI_KEEP") {
            std::fs::copy(&dest, Path::new(&d).join("harder.pdf")).unwrap();
        }
        assert!(report.flattened.is_empty(), "{report:?}");
        let after: String = texts(&dest)[0].split_whitespace().collect();
        assert!(!after.contains("secret"), "{after}");
        assert!(after.contains("Agent") && after.contains("word"), "{after}");
        // The picture is cut, not dropped: the part outside the box stays red.
        let pdf = inspect::open(std::fs::read(&dest).unwrap()).unwrap();
        let shown = image::load_from_memory(&inspect::page_jpeg(&pdf, 0, 72.0).unwrap())
            .unwrap()
            .to_rgb8();
        let doc = Document::load(&dest).unwrap();
        let g = Geometry::of(&doc, *doc.get_pages().get(&1).unwrap());
        let at = |x: f64, y: f64| {
            let (u, v) = g.fraction_of(x, y);
            *shown.get_pixel(
                (u * f64::from(shown.width())) as u32,
                (v * f64::from(shown.height())) as u32,
            )
        };
        let red = at(450.0, 250.0);
        assert!(red.0[0] > 150 && red.0[1] < 90, "{red:?}");
        let covered = at(340.0, 140.0);
        assert!(covered.0.iter().all(|c| *c < 60), "{covered:?}");
    }

    #[test]
    fn pages_that_cannot_be_cleaned_become_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path(), "a.pdf", &[LINE]);
        // Add a picture Libreri cannot cut (a fax-coded one) under the text.
        let mut doc = Document::load(&src).unwrap();
        let id = *doc.get_pages().get(&1).unwrap();
        let fax = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 8, "Height" => 8,
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 1, "Filter" => "CCITTFaxDecode",
            },
            vec![0; 8],
        ));
        let extra = doc.add_object(Stream::new(
            Dictionary::new(),
            b"q 200 0 0 200 50 600 cm /Fax Do Q".to_vec(),
        ));
        let d = doc.get_dictionary_mut(id).unwrap();
        let first = d.get(b"Contents").unwrap().clone();
        d.set("Contents", vec![first, Object::Reference(extra)]);
        let mut res = d.get(b"Resources").unwrap().as_dict().unwrap().clone();
        res.set("XObject", dictionary! { "Fax" => fax });
        doc.get_dictionary_mut(id).unwrap().set("Resources", res);
        doc.save(&src).unwrap();

        let dest = dir.path().join("b.pdf");
        let plan = EditPlan {
            pages: vec![page(1)],
            redactions: vec![Redaction {
                page: 1,
                boxes: vec![[0.155, 0.095, 0.09, 0.03]],
                color: black(),
            }],
            ..Default::default()
        };
        let report = apply(&src, &plan, &[], &dest).unwrap();
        assert_eq!(report.flattened, [1]);
        assert!(texts(&dest)[0].trim().is_empty());
    }

    #[test]
    fn plans_come_from_json() {
        let plan: EditPlan = serde_json::from_str(
            r#"{"pages":[{"kind":"page","page":2,"rotate":180},{"kind":"blank","width":10,"height":20}],"embedOcr":true}"#,
        )
        .unwrap();
        assert_eq!(
            plan.pages[0],
            OutPage::Page {
                page: 2,
                rotate: 180,
                crop: None
            }
        );
        assert!(plan.embed_ocr);
    }
}

#[cfg(test)]
mod look {
    /// Applies a plan saved from the interface, for eyeballing:
    /// `LIBRERI_LOOK=dir LIBRERI_EDIT_SRC=a.pdf LIBRERI_EDIT_PLAN=plan.json cargo test edit::look`.
    #[test]
    fn look() {
        let (Some(dir), Some(src), Some(plan)) = (
            std::env::var_os("LIBRERI_LOOK"),
            std::env::var_os("LIBRERI_EDIT_SRC"),
            std::env::var_os("LIBRERI_EDIT_PLAN"),
        ) else {
            return;
        };
        let plan: super::EditPlan =
            serde_json::from_str(&std::fs::read_to_string(plan).unwrap()).unwrap();
        let out = std::path::PathBuf::from(dir).join("edited.pdf");
        let report = super::apply(std::path::Path::new(&src), &plan, &[], &out).unwrap();
        println!("{report:?}");
    }
}

#[cfg(test)]
mod pictures_tests {
    use super::*;

    #[test]
    fn makes_a_searchable_pdf_of_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let img = image::RgbImage::from_pixel(400, 560, image::Rgb([250, 250, 250]));
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        let dest = dir.path().join("notes.pdf");
        let ocr = [OcrWords {
            page: 2,
            words: vec![("Refraction".into(), [0.1, 0.1, 0.3, 0.04])],
        }];
        assert_eq!(
            pictures_pdf(vec![jpeg.clone(), jpeg], &ocr, &dest).unwrap(),
            2
        );
        let doc = Document::load(&dest).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let text = doc.extract_text(&[2]).unwrap_or_default();
        assert!(text.contains("Refraction"), "{text:?}");
    }
}
