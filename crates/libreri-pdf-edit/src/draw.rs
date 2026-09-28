//! Drawing markup onto PDF pages.

use crate::path::{self, Seg};
use crate::{Error, Result};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

/// One thing to draw. Paths use SVG path data (absolute M, L, Q, C, Z) in
/// fractions of the page as shown (top-left origin); widths and dashes are
/// fractions of the page width.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DrawOp {
    #[serde(rename_all = "camelCase")]
    Fill {
        d: String,
        color: String,
        opacity: f32,
        /// Mixes like a highlighter (darkens, never covers).
        #[serde(default)]
        multiply: bool,
    },
    #[serde(rename_all = "camelCase")]
    Stroke {
        d: String,
        color: String,
        opacity: f32,
        width: f64,
        #[serde(default)]
        dash: Vec<f64>,
    },
    /// A picture (PNG or JPEG data URL): text boxes, stamps, notes and
    /// signatures are sent as pictures so every font and script survives.
    #[serde(rename_all = "camelCase")]
    Image { rect: [f64; 4], src: String },
}

/// What to draw on one page (1-based).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawPage {
    pub page: u32,
    pub ops: Vec<DrawOp>,
}

/// A page picture for [`pages_to_pdf`] (DjVu and comics).
pub struct PageImage {
    pub bytes: Vec<u8>,
}

/// The visible page: crop box within the media box, and rotation.
struct Geometry {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    rotate: i64,
}

impl Geometry {
    /// Width and height as shown.
    fn shown(&self) -> (f64, f64) {
        let (w, h) = (self.x1 - self.x0, self.y1 - self.y0);
        if self.rotate % 180 == 0 {
            (w, h)
        } else {
            (h, w)
        }
    }

    /// Maps shown points (y down, from the top left) to PDF user space.
    fn matrix(&self) -> [f64; 6] {
        let w = self.x1 - self.x0;
        match self.rotate {
            90 => [0.0, 1.0, 1.0, 0.0, self.x0, self.y0],
            180 => [-1.0, 0.0, 0.0, 1.0, self.x0 + w, self.y0],
            270 => [0.0, -1.0, -1.0, 0.0, self.x0 + w, self.y1],
            _ => [1.0, 0.0, 0.0, -1.0, self.x0, self.y1],
        }
    }
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference(r) => doc.get_object(*r).unwrap_or(o),
        _ => o,
    }
}

fn num(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

/// A page attribute, looking up the page tree for inherited ones.
fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page;
    for _ in 0..32 {
        let d = doc.get_dictionary(id).ok()?;
        if let Ok(v) = d.get(key) {
            return Some(resolve(doc, v).clone());
        }
        id = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn rect(doc: &Document, o: &Object) -> Option<[f64; 4]> {
    let a = resolve(doc, o).as_array().ok()?;
    let v: Vec<f64> = a.iter().filter_map(|x| num(resolve(doc, x))).collect();
    (v.len() == 4).then(|| {
        [
            v[0].min(v[2]),
            v[1].min(v[3]),
            v[0].max(v[2]),
            v[1].max(v[3]),
        ]
    })
}

fn geometry(doc: &Document, page: ObjectId) -> Geometry {
    let media = inherited(doc, page, b"MediaBox")
        .and_then(|o| rect(doc, &o))
        .unwrap_or([0.0, 0.0, 612.0, 792.0]);
    let crop = inherited(doc, page, b"CropBox")
        .and_then(|o| rect(doc, &o))
        .map(|c| {
            [
                c[0].max(media[0]),
                c[1].max(media[1]),
                c[2].min(media[2]),
                c[3].min(media[3]),
            ]
        })
        .filter(|c| c[2] > c[0] && c[3] > c[1])
        .unwrap_or(media);
    let rotate = inherited(doc, page, b"Rotate")
        .and_then(|o| num(&o))
        .map_or(0, |r| (r as i64).rem_euclid(360) / 90 * 90);
    Geometry {
        x0: crop[0],
        y0: crop[1],
        x1: crop[2],
        y1: crop[3],
        rotate,
    }
}

fn color(hex: &str) -> [f64; 3] {
    let h = hex.trim_start_matches('#');
    let c = |i: usize| {
        h.get(i..i + 2)
            .and_then(|v| u8::from_str_radix(v, 16).ok())
            .map_or(0.0, |v| f64::from(v) / 255.0)
    };
    if h.len() == 3 {
        let c1 = |i: usize| {
            h.get(i..i + 1)
                .and_then(|v| u8::from_str_radix(&v.repeat(2), 16).ok())
                .map_or(0.0, |v| f64::from(v) / 255.0)
        };
        return [c1(0), c1(1), c1(2)];
    }
    [c(0), c(2), c(4)]
}

fn f(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_owned()
    }
}

/// Writes path operators, in shown points.
fn path_ops(out: &mut String, d: &str, w: f64, h: f64) -> Result<()> {
    let mut cur = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    for seg in path::parse(d)? {
        match seg {
            Seg::Move(x, y) => {
                cur = (x * w, y * h);
                start = cur;
                let _ = writeln!(out, "{} {} m", f(cur.0), f(cur.1));
            }
            Seg::Line(x, y) => {
                cur = (x * w, y * h);
                let _ = writeln!(out, "{} {} l", f(cur.0), f(cur.1));
            }
            Seg::Quad(cx, cy, x, y) => {
                let (qx, qy) = (cx * w, cy * h);
                let (ex, ey) = (x * w, y * h);
                let c1 = (
                    cur.0 + 2.0 / 3.0 * (qx - cur.0),
                    cur.1 + 2.0 / 3.0 * (qy - cur.1),
                );
                let c2 = (ex + 2.0 / 3.0 * (qx - ex), ey + 2.0 / 3.0 * (qy - ey));
                let _ = writeln!(
                    out,
                    "{} {} {} {} {} {} c",
                    f(c1.0),
                    f(c1.1),
                    f(c2.0),
                    f(c2.1),
                    f(ex),
                    f(ey)
                );
                cur = (ex, ey);
            }
            Seg::Cubic(a, b, c, d2, x, y) => {
                let _ = writeln!(
                    out,
                    "{} {} {} {} {} {} c",
                    f(a * w),
                    f(b * h),
                    f(c * w),
                    f(d2 * h),
                    f(x * w),
                    f(y * h)
                );
                cur = (x * w, y * h);
            }
            Seg::Close => {
                out.push_str("h\n");
                cur = start;
            }
        }
    }
    Ok(())
}

fn decode_data_url(src: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let (_, b64) = src
        .split_once(";base64,")
        .ok_or_else(|| Error::Picture("not a data URL".into()))?;
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| Error::Picture(e.to_string()))
}

/// An image XObject (with a soft mask when it has transparency).
fn image_xobject(doc: &mut Document, bytes: &[u8]) -> Result<ObjectId> {
    let img = image::load_from_memory(bytes).map_err(|e| Error::Picture(e.to_string()))?;
    let (w, h) = (img.width(), img.height());
    let rgba = img.to_rgba8();
    let opaque = rgba.pixels().all(|p| p.0[3] == 255);
    let rgb: Vec<u8> = rgba
        .pixels()
        .flat_map(|p| [p.0[0], p.0[1], p.0[2]])
        .collect();
    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => i64::from(w),
        "Height" => i64::from(h),
        "ColorSpace" => "DeviceRGB",
        "BitsPerComponent" => 8,
    };
    if !opaque {
        let alpha: Vec<u8> = rgba.pixels().map(|p| p.0[3]).collect();
        let mut mask = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image",
                "Width" => i64::from(w), "Height" => i64::from(h),
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
            },
            alpha,
        );
        let _ = mask.compress();
        dict.set("SMask", doc.add_object(mask));
    }
    let mut stream = Stream::new(dict, rgb);
    let _ = stream.compress();
    Ok(doc.add_object(stream))
}

/// A page's resources as a direct dictionary of its own (inherited or
/// shared ones are copied, so other pages are not affected).
fn own_resources(doc: &Document, page: ObjectId) -> Dictionary {
    let mut res = match inherited(doc, page, b"Resources") {
        Some(Object::Dictionary(d)) => d,
        _ => Dictionary::new(),
    };
    for key in [b"ExtGState".as_slice(), b"XObject".as_slice()] {
        if let Ok(o) = res.get(key) {
            if let Object::Dictionary(d) = resolve(doc, o).clone() {
                res.set(key, Object::Dictionary(d));
            }
        }
    }
    res
}

fn sub<'a>(res: &'a mut Dictionary, key: &[u8]) -> &'a mut Dictionary {
    if !matches!(res.get(key), Ok(Object::Dictionary(_))) {
        res.set(key, Dictionary::new());
    }
    match res.get_mut(key) {
        Ok(Object::Dictionary(d)) => d,
        _ => unreachable!("just set"),
    }
}

fn unique_name(dict: &Dictionary, prefix: &str, n: &mut usize) -> String {
    loop {
        *n += 1;
        let name = format!("{prefix}{n}");
        if !dict.has(name.as_bytes()) {
            return name;
        }
    }
}

/// An ExtGState for this opacity (and blend), shared by marks that use it.
fn graphics_state(
    doc: &mut Document,
    res: &mut Dictionary,
    states: &mut HashMap<(i32, bool), String>,
    counter: &mut usize,
    alpha: f32,
    multiply: bool,
) -> String {
    let key = ((alpha.clamp(0.0, 1.0) * 1000.0) as i32, multiply);
    if let Some(n) = states.get(&key) {
        return n.clone();
    }
    let name = unique_name(sub(res, b"ExtGState"), "LbGs", counter);
    let a = f64::from(key.0) / 1000.0;
    let mut g = dictionary! { "Type" => "ExtGState", "CA" => a, "ca" => a };
    if multiply {
        g.set("BM", "Multiply");
    }
    let id = doc.add_object(g);
    sub(res, b"ExtGState").set(name.as_bytes(), id);
    states.insert(key, name.clone());
    name
}

fn draw_on_page(doc: &mut Document, page: ObjectId, ops: &[DrawOp]) -> Result<()> {
    if ops.is_empty() {
        return Ok(());
    }
    let geo = geometry(doc, page);
    let (w, h) = geo.shown();
    let m = geo.matrix();
    let mut res = own_resources(doc, page);
    let mut states: HashMap<(i32, bool), String> = HashMap::new();
    let mut counter = 0usize;
    let mut body = String::new();
    let _ = writeln!(
        body,
        "q {} {} {} {} {} {} cm",
        f(m[0]),
        f(m[1]),
        f(m[2]),
        f(m[3]),
        f(m[4]),
        f(m[5])
    );
    for op in ops {
        match op {
            DrawOp::Fill {
                d,
                color: c,
                opacity,
                multiply,
            } => {
                let name = graphics_state(
                    doc,
                    &mut res,
                    &mut states,
                    &mut counter,
                    *opacity,
                    *multiply,
                );
                let [r, g, b] = color(c);
                let _ = writeln!(body, "q /{name} gs {} {} {} rg", f(r), f(g), f(b));
                path_ops(&mut body, d, w, h)?;
                body.push_str("f Q\n");
            }
            DrawOp::Stroke {
                d,
                color: c,
                opacity,
                width,
                dash,
            } => {
                let name =
                    graphics_state(doc, &mut res, &mut states, &mut counter, *opacity, false);
                let [r, g, b] = color(c);
                let dashes: Vec<String> = dash.iter().map(|v| f(v * w)).collect();
                let _ = writeln!(
                    body,
                    "q /{name} gs {} {} {} RG {} w 1 J 1 j [{}] 0 d",
                    f(r),
                    f(g),
                    f(b),
                    f(width * w),
                    dashes.join(" ")
                );
                path_ops(&mut body, d, w, h)?;
                body.push_str("S Q\n");
            }
            DrawOp::Image { rect, src } => {
                let bytes = decode_data_url(src)?;
                let id = image_xobject(doc, &bytes)?;
                let xo = sub(&mut res, b"XObject");
                let name = unique_name(xo, "LbIm", &mut counter);
                xo.set(name.as_bytes(), id);
                let [x, y, iw, ih] = *rect;
                // The picture's unit square is drawn upward; the shown space
                // runs downward, so flip it.
                let _ = writeln!(
                    body,
                    "q {} 0 0 {} {} {} cm /{name} Do Q",
                    f(iw * w),
                    f(-ih * h),
                    f(x * w),
                    f((y + ih) * h)
                );
            }
        }
    }
    body.push_str("Q\n");

    let before = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let mut ours = Stream::new(Dictionary::new(), body.into_bytes());
    let _ = ours.compress();
    let after = doc.add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));
    let ours = doc.add_object(ours);
    let existing: Vec<Object> = match doc
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
    let mut contents = vec![Object::Reference(before)];
    contents.extend(existing);
    contents.push(Object::Reference(after));
    contents.push(Object::Reference(ours));
    let dict = doc
        .get_dictionary_mut(page)
        .map_err(|e| Error::Read(e.to_string()))?;
    dict.set("Contents", contents);
    dict.set("Resources", res);
    Ok(())
}

fn save(doc: &mut Document, dest: &Path) -> Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = dest.with_extension("pdf.part");
    doc.compress();
    doc.save(&tmp)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// Copies the PDF at `src` to `dest` with the markup drawn on its pages.
pub fn mark_up(src: &Path, pages: &[DrawPage], dest: &Path) -> Result<()> {
    let mut doc = Document::load(src).map_err(|e| Error::Read(e.to_string()))?;
    if doc.is_encrypted() {
        return Err(Error::Encrypted);
    }
    let ids = doc.get_pages();
    for p in pages {
        let id = *ids.get(&p.page).ok_or(Error::NoPage(p.page))?;
        draw_on_page(&mut doc, id, &p.ops)?;
    }
    save(&mut doc, dest)
}

/// Makes a PDF with one page per picture (JPEG kept as it is, others
/// converted), each `width` points wide, then draws the markup on it.
pub fn pages_to_pdf(
    images: Vec<PageImage>,
    width: f64,
    pages: &[DrawPage],
    dest: &Path,
) -> Result<()> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for img in images {
        let decoded =
            image::load_from_memory(&img.bytes).map_err(|e| Error::Picture(e.to_string()))?;
        let (pw, ph) = (decoded.width(), decoded.height());
        let is_jpeg = img.bytes.starts_with(&[0xFF, 0xD8]);
        let xobj = if is_jpeg {
            let gray = decoded.color().channel_count() == 1;
            doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image",
                    "Width" => i64::from(pw), "Height" => i64::from(ph),
                    "ColorSpace" => if gray { "DeviceGray" } else { "DeviceRGB" },
                    "BitsPerComponent" => 8, "Filter" => "DCTDecode",
                },
                img.bytes,
            ))
        } else {
            let mut jpg = std::io::Cursor::new(Vec::new());
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg, 88)
                .encode_image(&decoded.to_rgb8())
                .map_err(|e| Error::Picture(e.to_string()))?;
            doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image",
                    "Width" => i64::from(pw), "Height" => i64::from(ph),
                    "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
                },
                jpg.into_inner(),
            ))
        };
        let height = width * f64::from(ph) / f64::from(pw.max(1));
        let content = doc.add_object(Stream::new(
            Dictionary::new(),
            format!("q {} 0 0 {} 0 0 cm /Page Do Q\n", f(width), f(height)).into_bytes(),
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(width as f32), Object::Real(height as f32)],
            "Contents" => content,
            "Resources" => dictionary! { "XObject" => dictionary! { "Page" => xobj } },
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let ids = doc.get_pages();
    for p in pages {
        let id = *ids.get(&p.page).ok_or(Error::NoPage(p.page))?;
        draw_on_page(&mut doc, id, &p.ops)?;
    }
    save(&mut doc, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_data_url() -> String {
        use base64::Engine;
        let img = image::RgbaImage::from_fn(20, 10, |x, _| {
            image::Rgba([200, 0, 0, if x < 10 { 255 } else { 0 }])
        });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(out.into_inner())
        )
    }

    pub(super) fn ops_for_look() -> Vec<DrawOp> {
        ops()
    }

    fn ops() -> Vec<DrawOp> {
        vec![
            DrawOp::Fill {
                d: "M 0.1 0.1 Q 0.2 0.05 0.3 0.1 L 0.3 0.12 Z".into(),
                color: "#dc2626".into(),
                opacity: 1.0,
                multiply: false,
            },
            DrawOp::Fill {
                d: "M 0.1 0.2 L 0.5 0.2 L 0.5 0.22 L 0.1 0.22 Z".into(),
                color: "#fde047".into(),
                opacity: 0.4,
                multiply: true,
            },
            DrawOp::Stroke {
                d: "M 0.1 0.5 L 0.9 0.5".into(),
                color: "#2563eb".into(),
                opacity: 1.0,
                width: 0.003,
                dash: vec![0.01, 0.005],
            },
            DrawOp::Image {
                rect: [0.6, 0.7, 0.2, 0.05],
                src: png_data_url(),
            },
        ]
    }

    #[test]
    fn draws_on_a_copy_and_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        libreri_formats::test_text_pdf(&src, &["The lighthouse keeper", "Second page"]);
        let before = std::fs::read(&src).unwrap();
        let dest = dir.path().join("out/a marked.pdf");
        mark_up(
            &src,
            &[DrawPage {
                page: 2,
                ops: ops(),
            }],
            &dest,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(&src).unwrap(),
            before,
            "the original is untouched"
        );

        let doc = Document::load(&dest).unwrap();
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2);
        let content = doc.get_page_content(*pages.get(&2).unwrap());
        let text = String::from_utf8_lossy(&content);
        assert!(text.contains("1 0 0 -1 0 792 cm"), "{text}");
        assert!(text.contains(" RG ") && text.contains(" Do Q"), "{text}");
        // The page's own text is still there, and still readable.
        let texts = libreri_formats::pdftext::page_texts(&dest).unwrap();
        assert_eq!(texts[1], "Second page");
        // Page 1 was not touched.
        let first = doc.get_page_content(*pages.get(&1).unwrap());
        assert!(!String::from_utf8_lossy(&first).contains("cm"));
        assert!(mark_up(
            &src,
            &[DrawPage {
                page: 9,
                ops: ops()
            }],
            &dest
        )
        .is_err());
    }

    #[test]
    fn rotated_pages_map_to_what_is_shown() {
        let g = Geometry {
            x0: 0.0,
            y0: 0.0,
            x1: 612.0,
            y1: 792.0,
            rotate: 90,
        };
        assert_eq!(g.shown(), (792.0, 612.0));
        // The top-left corner as shown is (0, 0) in user space for 90°.
        let [a, b, c, d, e, f2] = g.matrix();
        let at = |x: f64, y: f64| (a * x + c * y + e, b * x + d * y + f2);
        assert_eq!(at(0.0, 0.0), (0.0, 0.0));
        assert_eq!(at(792.0, 0.0), (0.0, 792.0));
        assert_eq!(at(0.0, 612.0), (612.0, 0.0));
    }

    #[test]
    fn pictures_become_a_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let mut jpg = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(60, 90, image::Rgb([240, 240, 230]))
            .write_to(&mut jpg, image::ImageFormat::Jpeg)
            .unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(60, 90, image::Rgb([10, 10, 10]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let dest = dir.path().join("comic.pdf");
        pages_to_pdf(
            vec![
                PageImage {
                    bytes: jpg.into_inner(),
                },
                PageImage {
                    bytes: png.into_inner(),
                },
            ],
            600.0,
            &[DrawPage {
                page: 1,
                ops: ops(),
            }],
            &dest,
        )
        .unwrap();
        let doc = Document::load(&dest).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
    }
}

#[cfg(test)]
mod look {
    /// Writes a marked-up sample for eyeballing: `LIBRERI_LOOK=dir cargo test look`.
    #[test]
    fn look() {
        let Some(dir) = std::env::var_os("LIBRERI_LOOK") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        if let (Some(src), Some(ops)) = (
            std::env::var_os("LIBRERI_LOOK_SRC"),
            std::env::var_os("LIBRERI_LOOK_OPS"),
        ) {
            let pages: Vec<super::DrawPage> =
                serde_json::from_str(&std::fs::read_to_string(ops).unwrap()).unwrap();
            super::mark_up(std::path::Path::new(&src), &pages, &dir.join("ui.pdf")).unwrap();
        }
        let src = dir.join("src.pdf");
        libreri_formats::test_text_pdf(&src, &["The lighthouse keeper wrote every night"]);
        super::mark_up(
            &src,
            &[super::DrawPage {
                page: 1,
                ops: super::tests::ops_for_look(),
            }],
            &dir.join("out.pdf"),
        )
        .unwrap();
    }
}
