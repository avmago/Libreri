//! Removing what lies under boxes on a page: the characters of text, the
//! pixels of pictures, and small drawings (vector outlines of letters,
//! underlines, marks). The rest of the page is left as it was, so it stays
//! sharp and selectable (user, 2026-09-28).
//!
//! The page's drawing instructions are walked with the text, graphics and
//! transformation state tracked, so the place of every character is known.
//! Characters under a box are taken out of their text-showing operation and
//! replaced by the same amount of spacing, so the rest of the line does not
//! move. Pictures are decoded, painted over and stored as a new copy (other
//! pages that share the picture keep theirs). Forms (reusable drawings) are
//! walked the same way and copied when they change. Anything that cannot
//! be edited safely (a picture in a format Libreri cannot decode, an inline
//! picture it cannot parse) is reported, and the caller flattens the page.

use crate::fonts::{font_info, FontInfo};
use crate::geom::{apply, invert, mul, num, resolve, FBox, Geometry, Mat, IDENTITY};
use crate::Result;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use std::collections::HashMap;
use std::rc::Rc;

/// What happened on one page.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Outcome {
    pub changed: bool,
    /// Something under a box could not be removed; flatten the page.
    pub needs_flatten: bool,
    pub notes: Vec<String>,
}

#[derive(Clone)]
struct Gs {
    ctm: Mat,
    tc: f64,
    tw: f64,
    th: f64,
    tl: f64,
    tfs: f64,
    rise: f64,
    font: Option<Rc<FontInfo>>,
}

impl Gs {
    fn new(ctm: Mat) -> Self {
        Self {
            ctm,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            tfs: 0.0,
            rise: 0.0,
            font: None,
        }
    }
}

enum Elem {
    Str(Vec<u8>),
    Num(f64),
}

struct Walker<'a> {
    doc: &'a Document,
    /// User space → shown page points.
    to_shown: Mat,
    shown: (f64, f64),
    boxes: &'a [FBox],
    flatten: bool,
    notes: Vec<String>,
    counter: usize,
}

/// Content to add to a resources dictionary: `(category, name, stream)`.
type Additions = Vec<(&'static [u8], Vec<u8>, Stream)>;

fn f(v: f64) -> Object {
    Object::Real(v as f32)
}

fn operand(ops: &[Object], i: usize) -> f64 {
    ops.get(i).and_then(num).unwrap_or(0.0)
}

fn matrix_of(ops: &[Object]) -> Mat {
    [
        operand(ops, 0),
        operand(ops, 1),
        operand(ops, 2),
        operand(ops, 3),
        operand(ops, 4),
        operand(ops, 5),
    ]
}

impl Walker<'_> {
    fn frac(&self, m: &Mat, x: f64, y: f64) -> (f64, f64) {
        let (ux, uy) = apply(m, x, y);
        let (sx, sy) = apply(&self.to_shown, ux, uy);
        (sx / self.shown.0, sy / self.shown.1)
    }

    fn fbox(&self, m: &Mat, x0: f64, y0: f64, x1: f64, y1: f64) -> FBox {
        FBox::around(&[
            self.frac(m, x0, y0),
            self.frac(m, x1, y0),
            self.frac(m, x1, y1),
            self.frac(m, x0, y1),
        ])
    }

    /// A character counts as covered when its middle is in a box, or most
    /// of it is.
    fn covered(&self, b: &FBox) -> bool {
        let area = b.area().max(1e-12);
        self.boxes
            .iter()
            .any(|x| x.contains(b.center()) || b.overlap(x) / area >= 0.5)
    }

    fn touches(&self, b: &FBox) -> bool {
        self.boxes.iter().any(|x| b.intersects(x))
    }

    fn new_name(&mut self, prefix: &str) -> Vec<u8> {
        self.counter += 1;
        format!("{prefix}{}", self.counter).into_bytes()
    }

    /// Shows text; returns the new TJ array when characters were removed.
    fn show(&self, g: &Gs, tm: &mut Mat, elems: Vec<Elem>) -> Option<Vec<Object>> {
        let font = g
            .font
            .clone()
            .unwrap_or_else(|| Rc::new(FontInfo::unknown()));
        let mut out: Vec<Object> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut removed = false;
        let push_num = |out: &mut Vec<Object>, n: f64| match out.last_mut() {
            Some(Object::Real(prev)) => *prev += n as f32,
            _ => out.push(f(n)),
        };
        for e in elems {
            match e {
                Elem::Num(n) => {
                    if !buf.is_empty() {
                        out.push(Object::String(
                            std::mem::take(&mut buf),
                            StringFormat::Hexadecimal,
                        ));
                    }
                    push_num(&mut out, n);
                    let tx = -n / 1000.0 * g.tfs * g.th;
                    *tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], tm);
                }
                Elem::Str(bytes) => {
                    for code in font.codes(&bytes) {
                        let w0 = font.width(code) / 1000.0;
                        let word = if !font.two_byte && code == 32 {
                            g.tw
                        } else {
                            0.0
                        };
                        let trm = mul(
                            &[g.tfs * g.th, 0.0, 0.0, g.tfs, 0.0, g.rise],
                            &mul(tm, &g.ctm),
                        );
                        let b = self.fbox(&trm, 0.0, -0.2, w0.max(0.05), 0.8);
                        if self.covered(&b) && g.tfs != 0.0 {
                            removed = true;
                            if !buf.is_empty() {
                                out.push(Object::String(
                                    std::mem::take(&mut buf),
                                    StringFormat::Hexadecimal,
                                ));
                            }
                            push_num(&mut out, -(w0 * 1000.0 + (g.tc + word) * 1000.0 / g.tfs));
                        } else {
                            buf.extend(font.bytes(code));
                        }
                        let tx = (w0 * g.tfs + g.tc + word) * g.th;
                        *tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], tm);
                    }
                }
            }
        }
        if !buf.is_empty() {
            out.push(Object::String(buf, StringFormat::Hexadecimal));
        }
        removed.then_some(out)
    }

    fn font(
        &self,
        resources: &Dictionary,
        cache: &mut HashMap<Vec<u8>, Rc<FontInfo>>,
        name: &[u8],
    ) -> Rc<FontInfo> {
        if let Some(f) = cache.get(name) {
            return f.clone();
        }
        let info = resources
            .get(b"Font")
            .ok()
            .map(|o| resolve(self.doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(name).ok())
            .map(|o| resolve(self.doc, o))
            .and_then(|o| o.as_dict().ok())
            .map(|d| font_info(self.doc, d))
            .unwrap_or_else(FontInfo::unknown);
        let rc = Rc::new(info);
        cache.insert(name.to_vec(), rc.clone());
        rc
    }

    fn xobject<'b>(&'b self, resources: &'b Dictionary, name: &[u8]) -> Option<&'b Stream> {
        resources
            .get(b"XObject")
            .ok()
            .map(|o| resolve(self.doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(name).ok())
            .map(|o| resolve(self.doc, o))
            .and_then(|o| o.as_stream().ok())
    }

    /// Walks drawing instructions. Returns the new instructions and what to
    /// add to this scope's resources, or `None` when nothing changed.
    fn walk(
        &mut self,
        ops: &[Operation],
        resources: &Dictionary,
        ctm: Mat,
        depth: u32,
    ) -> Option<(Vec<Operation>, Additions)> {
        let mut out: Vec<Operation> = Vec::with_capacity(ops.len());
        let mut adds: Additions = Vec::new();
        let mut changed = false;
        let mut stack: Vec<Gs> = Vec::new();
        let mut g = Gs::new(ctm);
        let mut tm = IDENTITY;
        let mut tlm = IDENTITY;
        let mut fonts: HashMap<Vec<u8>, Rc<FontInfo>> = HashMap::new();
        let mut path: Vec<(f64, f64)> = Vec::new();

        for op in ops {
            let o = &op.operands;
            match op.operator.as_str() {
                "q" => stack.push(g.clone()),
                "Q" => {
                    if let Some(prev) = stack.pop() {
                        g = prev;
                    }
                }
                "cm" => g.ctm = mul(&matrix_of(o), &g.ctm),
                "BT" => {
                    tm = IDENTITY;
                    tlm = IDENTITY;
                }
                "Tc" => g.tc = operand(o, 0),
                "Tw" => g.tw = operand(o, 0),
                "Tz" => g.th = operand(o, 0) / 100.0,
                "TL" => g.tl = operand(o, 0),
                "Ts" => g.rise = operand(o, 0),
                "Tf" => {
                    g.tfs = operand(o, 1);
                    if let Some(Ok(n)) = o.first().map(|x| x.as_name()) {
                        g.font = Some(self.font(resources, &mut fonts, n));
                    }
                }
                "Td" | "TD" => {
                    let (tx, ty) = (operand(o, 0), operand(o, 1));
                    if op.operator == "TD" {
                        g.tl = -ty;
                    }
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], &tlm);
                    tm = tlm;
                }
                "Tm" => {
                    tlm = matrix_of(o);
                    tm = tlm;
                }
                "T*" => {
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -g.tl], &tlm);
                    tm = tlm;
                }
                "Tj" | "'" | "\"" | "TJ" => {
                    let mut prefix: Vec<Operation> = Vec::new();
                    let string_at = match op.operator.as_str() {
                        "'" => {
                            tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -g.tl], &tlm);
                            tm = tlm;
                            prefix.push(Operation::new("T*", vec![]));
                            0
                        }
                        "\"" => {
                            g.tw = operand(o, 0);
                            g.tc = operand(o, 1);
                            tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -g.tl], &tlm);
                            tm = tlm;
                            prefix.push(Operation::new("Tw", vec![o[0].clone()]));
                            prefix.push(Operation::new("Tc", vec![o[1].clone()]));
                            prefix.push(Operation::new("T*", vec![]));
                            2
                        }
                        _ => 0,
                    };
                    let elems: Vec<Elem> = if op.operator == "TJ" {
                        o.first()
                            .and_then(|a| a.as_array().ok())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| match x {
                                        Object::String(s, _) => Some(Elem::Str(s.clone())),
                                        other => num(other).map(Elem::Num),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    } else {
                        match o.get(string_at) {
                            Some(Object::String(s, _)) => vec![Elem::Str(s.clone())],
                            _ => Vec::new(),
                        }
                    };
                    if let Some(arr) = self.show(&g, &mut tm, elems) {
                        changed = true;
                        out.extend(prefix);
                        out.push(Operation::new("TJ", vec![Object::Array(arr)]));
                        continue;
                    }
                }
                "m" | "l" => {
                    path.push(apply(&g.ctm, operand(o, 0), operand(o, 1)));
                }
                "c" => {
                    for i in 0..3 {
                        path.push(apply(&g.ctm, operand(o, i * 2), operand(o, i * 2 + 1)));
                    }
                }
                "v" | "y" => {
                    for i in 0..2 {
                        path.push(apply(&g.ctm, operand(o, i * 2), operand(o, i * 2 + 1)));
                    }
                }
                "re" => {
                    let (x, y, w, h) = (operand(o, 0), operand(o, 1), operand(o, 2), operand(o, 3));
                    for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
                        path.push(apply(&g.ctm, px, py));
                    }
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                    let points = std::mem::take(&mut path);
                    if op.operator != "n" && !points.is_empty() {
                        let b = FBox::around(
                            &points
                                .iter()
                                .map(|&(x, y)| {
                                    let (sx, sy) = apply(&self.to_shown, x, y);
                                    (sx / self.shown.0, sy / self.shown.1)
                                })
                                .collect::<Vec<_>>(),
                        );
                        let remove = self
                            .boxes
                            .iter()
                            .any(|x| b.inside(x) || (b.intersects(x) && b.area() <= x.area()));
                        if remove {
                            changed = true;
                            out.push(Operation::new("n", vec![]));
                            continue;
                        }
                    }
                }
                "Do" => {
                    let Some(Ok(name)) = o.first().map(|x| x.as_name()) else {
                        out.push(op.clone());
                        continue;
                    };
                    let name = name.to_vec();
                    let Some(xo) = self.xobject(resources, &name).cloned() else {
                        out.push(op.clone());
                        continue;
                    };
                    let subtype = xo
                        .dict
                        .get(b"Subtype")
                        .ok()
                        .and_then(|s| s.as_name().ok())
                        .unwrap_or(b"")
                        .to_vec();
                    if subtype == b"Image" {
                        let b = self.fbox(&g.ctm, 0.0, 0.0, 1.0, 1.0);
                        if !self.touches(&b) {
                            out.push(op.clone());
                            continue;
                        }
                        changed = true;
                        if self.boxes.iter().any(|x| b.inside(x)) {
                            // Wholly covered: left out.
                            continue;
                        }
                        match paint_image(
                            self.doc,
                            &xo,
                            &g.ctm,
                            &self.to_shown,
                            self.shown,
                            self.boxes,
                        ) {
                            Some(stream) => {
                                let n = self.new_name("LbRed");
                                adds.push((b"XObject", n.clone(), stream));
                                out.push(Operation::new("Do", vec![Object::Name(n)]));
                            }
                            None => {
                                self.flatten = true;
                                self.notes
                                    .push("a picture in a format Libreri cannot edit".into());
                                out.push(op.clone());
                            }
                        }
                        continue;
                    }
                    if subtype == b"Form" && depth < 12 {
                        let fm = xo
                            .dict
                            .get(b"Matrix")
                            .ok()
                            .and_then(|m| m.as_array().ok())
                            .map(|a| matrix_of(a))
                            .unwrap_or(IDENTITY);
                        let inner = mul(&fm, &g.ctm);
                        let bbox = xo
                            .dict
                            .get(b"BBox")
                            .ok()
                            .and_then(|b| crate::geom::rect(self.doc, b));
                        let touches = bbox.is_none_or(|b| {
                            self.touches(&self.fbox(&inner, b[0], b[1], b[2], b[3]))
                        });
                        if !touches {
                            out.push(op.clone());
                            continue;
                        }
                        let form_res = xo
                            .dict
                            .get(b"Resources")
                            .ok()
                            .map(|r| resolve(self.doc, r))
                            .and_then(|r| r.as_dict().ok())
                            .cloned()
                            .unwrap_or_else(|| resources.clone());
                        let bytes = xo
                            .decompressed_content()
                            .unwrap_or_else(|_| xo.content.clone());
                        let Ok(content) = Content::decode(&bytes) else {
                            self.flatten = true;
                            out.push(op.clone());
                            continue;
                        };
                        if let Some((new_ops, form_adds)) =
                            self.walk(&content.operations, &form_res, inner, depth + 1)
                        {
                            changed = true;
                            let mut res = form_res.clone();
                            add_inline(&mut res, form_adds);
                            let mut dict = xo.dict.clone();
                            dict.remove(b"Filter");
                            dict.remove(b"DecodeParms");
                            dict.remove(b"Length");
                            dict.set("Resources", res);
                            let mut stream = Stream::new(dict, encode(&new_ops));
                            let _ = stream.compress();
                            let n = self.new_name("LbForm");
                            adds.push((b"XObject", n.clone(), stream));
                            out.push(Operation::new("Do", vec![Object::Name(n)]));
                            continue;
                        }
                    }
                }
                "BI" => match o.first() {
                    Some(Object::Stream(_)) => {
                        let b = self.fbox(&g.ctm, 0.0, 0.0, 1.0, 1.0);
                        if self.touches(&b) {
                            // Small pictures inside the page: left out.
                            changed = true;
                            continue;
                        }
                    }
                    _ => {
                        self.flatten = true;
                        self.notes
                            .push("an inline picture Libreri could not read".into());
                    }
                },
                _ => {}
            }
            out.push(op.clone());
        }
        changed.then_some((out, adds))
    }
}

/// Adds new XObjects (or other resources) under their names.
/// Adds new XObjects inside a form's resources; [`apply_additions`] later
/// stores them as objects of their own.
fn add_inline(res: &mut Dictionary, adds: Additions) {
    for (cat, name, stream) in adds {
        let mut sub = match res.get(cat) {
            Ok(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        };
        sub.set(name, Object::Stream(stream));
        res.set(cat, sub);
    }
}

/// Moves streams nested in dictionaries out into objects of their own.
fn lift(doc: &mut Document, o: &mut Object) {
    match o {
        Object::Dictionary(d) => d.iter_mut().for_each(|(_, v)| lift_value(doc, v)),
        Object::Array(a) => a.iter_mut().for_each(|v| lift_value(doc, v)),
        Object::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| lift_value(doc, v)),
        _ => {}
    }
}

fn lift_value(doc: &mut Document, v: &mut Object) {
    lift(doc, v);
    if matches!(v, Object::Stream(_)) {
        let s = std::mem::replace(v, Object::Null);
        *v = Object::Reference(doc.add_object(s));
    }
}

/// Streams are always stored as objects of their own.
pub(crate) fn apply_additions(doc: &mut Document, res: &mut Dictionary, adds: Additions) {
    for (cat, name, stream) in adds {
        let mut sub = match res.get(cat) {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(r)) => doc.get_dictionary(*r).cloned().unwrap_or_default(),
            _ => Dictionary::new(),
        };
        let mut stream = Object::Stream(stream);
        lift(doc, &mut stream);
        let id = doc.add_object(stream);
        sub.set(name, Object::Reference(id));
        res.set(cat, sub);
    }
}

/// Writes drawing instructions back out, including inline pictures (which
/// lopdf keeps as a stream operand).
pub(crate) fn encode(ops: &[Operation]) -> Vec<u8> {
    let mut out = Vec::new();
    let one = |op: &Operation| {
        Content {
            operations: vec![op.clone()],
        }
        .encode()
        .unwrap_or_default()
    };
    for op in ops {
        if op.operator == "BI" {
            if let Some(Object::Stream(s)) = op.operands.first() {
                out.extend_from_slice(b"BI\n");
                for (k, v) in s.dict.iter() {
                    out.push(b'/');
                    out.extend_from_slice(k);
                    out.push(b' ');
                    let val = one(&Operation::new("", vec![v.clone()]));
                    out.extend_from_slice(val.trim_ascii_end());
                    out.push(b'\n');
                }
                out.extend_from_slice(b"ID ");
                out.extend_from_slice(&s.content);
                out.extend_from_slice(b"\nEI\n");
            }
            continue;
        }
        out.extend(one(op));
        out.push(b'\n');
    }
    out
}

/// Decodes a picture, paints the covered parts and returns it as a new
/// stream; `None` when its format cannot be edited here.
fn paint_image(
    doc: &Document,
    xo: &Stream,
    ctm: &Mat,
    to_shown: &Mat,
    shown: (f64, f64),
    boxes: &[FBox],
) -> Option<Stream> {
    let d = &xo.dict;
    let get = |k: &[u8]| d.get(k).ok().map(|o| resolve(doc, o));
    let w = get(b"Width").and_then(num)? as usize;
    let h = get(b"Height").and_then(num)? as usize;
    if w == 0 || h == 0 || w * h > 80_000_000 {
        return None;
    }
    let mask = matches!(get(b"ImageMask"), Some(Object::Boolean(true)));
    let filters: Vec<Vec<u8>> = match get(b"Filter") {
        Some(Object::Name(n)) => vec![n.clone()],
        Some(Object::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_name().ok().map(<[u8]>::to_vec))
            .collect(),
        _ => Vec::new(),
    };
    let components = |cs: Option<&Object>| -> Option<usize> {
        match cs.map(|o| resolve(doc, o)) {
            Some(Object::Name(n)) if n == b"DeviceGray" || n == b"CalGray" => Some(1),
            Some(Object::Name(n)) if n == b"DeviceRGB" || n == b"CalRGB" => Some(3),
            Some(Object::Array(a))
                if a.first().and_then(|x| x.as_name().ok()) == Some(b"ICCBased") =>
            {
                let s = a
                    .get(1)
                    .map(|x| resolve(doc, x))
                    .and_then(|x| x.as_stream().ok())?;
                let n = s.dict.get(b"N").ok().and_then(num)? as usize;
                (n == 1 || n == 3).then_some(n)
            }
            Some(Object::Array(a))
                if a.first().and_then(|x| x.as_name().ok()) == Some(b"CalRGB") =>
            {
                Some(3)
            }
            Some(Object::Array(a))
                if a.first().and_then(|x| x.as_name().ok()) == Some(b"CalGray") =>
            {
                Some(1)
            }
            _ => None,
        }
    };

    // Pixels of the box in the picture: unit square → rows from the top.
    let inv = invert(ctm)?;
    let from_shown = invert(to_shown)?;
    let mut rects = Vec::new();
    for b in boxes {
        let corners = [(b.x0, b.y0), (b.x1, b.y0), (b.x1, b.y1), (b.x0, b.y1)];
        let pts: Vec<(f64, f64)> = corners
            .iter()
            .map(|&(u, v)| {
                let (x, y) = apply(&from_shown, u * shown.0, v * shown.1);
                let (s, t) = apply(&inv, x, y);
                (s * w as f64, (1.0 - t) * h as f64)
            })
            .collect();
        let bb = FBox::around(&pts);
        let x0 = bb.x0.floor().max(0.0) as usize;
        let y0 = bb.y0.floor().max(0.0) as usize;
        let x1 = (bb.x1.ceil().max(0.0) as usize).min(w);
        let y1 = (bb.y1.ceil().max(0.0) as usize).min(h);
        if x1 > x0 && y1 > y0 {
            rects.push((x0, y0, x1, y1));
        }
    }

    let mut dict = d.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    dict.remove(b"Length");

    let is_dct = filters.last().is_some_and(|f| f == b"DCTDecode");
    let (mut data, comps, bpc) = if is_dct {
        let raw = if filters.len() > 1 {
            // Other filters around the JPEG: undo them first.
            let mut s = xo.clone();
            s.dict.set(
                "Filter",
                Object::Array(
                    filters[..filters.len() - 1]
                        .iter()
                        .map(|f| Object::Name(f.clone()))
                        .collect(),
                ),
            );
            s.decompressed_content().ok()?
        } else {
            xo.content.clone()
        };
        let img = image::load_from_memory_with_format(&raw, image::ImageFormat::Jpeg).ok()?;
        if img.color().channel_count() == 1 {
            dict.set("ColorSpace", "DeviceGray");
            (img.to_luma8().into_raw(), 1usize, 8usize)
        } else {
            dict.set("ColorSpace", "DeviceRGB");
            dict.remove(b"Decode");
            (img.to_rgb8().into_raw(), 3, 8)
        }
    } else {
        if filters.iter().any(|f| {
            matches!(
                f.as_slice(),
                b"JBIG2Decode" | b"CCITTFaxDecode" | b"JPXDecode"
            )
        }) {
            return None;
        }
        let raw = if filters.is_empty() {
            xo.content.clone()
        } else {
            xo.decompressed_content().ok()?
        };
        let bpc = if mask {
            1
        } else {
            get(b"BitsPerComponent").and_then(num)? as usize
        };
        let comps = if mask {
            1
        } else {
            components(get(b"ColorSpace"))?
        };
        (raw, comps, bpc)
    };
    if bpc != 8 && !(bpc == 1 && comps == 1) {
        return None;
    }
    let row = if bpc == 8 { w * comps } else { w.div_ceil(8) };
    if data.len() < row * h {
        return None;
    }
    // Image masks paint where a sample is 0; set 1 so nothing is painted.
    // Other pictures become black where covered.
    let decode_inverted = get(b"Decode")
        .and_then(|o| {
            o.as_array()
                .ok()
                .map(|a| a.first().and_then(num) == Some(1.0))
        })
        .unwrap_or(false);
    let bit_value = if mask {
        !decode_inverted
    } else {
        decode_inverted
    };
    for &(x0, y0, x1, y1) in &rects {
        for y in y0..y1 {
            let line = &mut data[y * row..(y + 1) * row];
            if bpc == 8 {
                line[x0 * comps..x1 * comps].fill(0);
            } else {
                for x in x0..x1 {
                    let (byte, bit) = (x / 8, 7 - (x % 8));
                    if bit_value {
                        line[byte] |= 1 << bit;
                    } else {
                        line[byte] &= !(1 << bit);
                    }
                }
            }
        }
    }
    dict.set("BitsPerComponent", bpc as i64);
    let mut stream = Stream::new(dict, data);
    let _ = stream.compress();
    Some(stream)
}

/// Removes annotations (links, notes, form fields) that touch the boxes.
fn remove_annotations(doc: &mut Document, page: ObjectId, geo: &Geometry, boxes: &[FBox]) -> usize {
    let Ok(dict) = doc.get_dictionary(page) else {
        return 0;
    };
    let Some(annots) = dict
        .get(b"Annots")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .cloned()
    else {
        return 0;
    };
    let mut keep = Vec::new();
    let mut removed = Vec::new();
    for a in annots {
        let d = resolve(doc, &a).as_dict().ok();
        let r = d
            .and_then(|d| d.get(b"Rect").ok())
            .and_then(|r| crate::geom::rect(doc, r));
        let hit = r.is_some_and(|r| {
            let b = FBox::around(&[geo.fraction_of(r[0], r[1]), geo.fraction_of(r[2], r[3])]);
            boxes.iter().any(|x| b.intersects(x))
        });
        if hit {
            if let Object::Reference(id) = a {
                removed.push(id);
            }
        } else {
            keep.push(a);
        }
    }
    let count = removed.len();
    if let Ok(d) = doc.get_dictionary_mut(page) {
        d.set("Annots", keep);
    }
    // Form fields: also out of the form, so their values leave the file.
    if !removed.is_empty() {
        let acro = doc
            .catalog()
            .ok()
            .and_then(|c| c.get(b"AcroForm").ok())
            .and_then(|o| o.as_reference().ok());
        let fields_owner = acro.or_else(|| {
            doc.trailer
                .get(b"Root")
                .ok()
                .and_then(|o| o.as_reference().ok())
        });
        if let Some(owner) = fields_owner {
            let prune = |arr: &mut Vec<Object>| {
                arr.retain(|o| !matches!(o, Object::Reference(r) if removed.contains(r)))
            };
            if let Ok(Object::Dictionary(d)) = doc.get_object_mut(owner) {
                if let Ok(Object::Array(f)) = d.get_mut(b"Fields") {
                    prune(f);
                }
                if let Ok(Object::Dictionary(form)) = d.get_mut(b"AcroForm") {
                    if let Ok(Object::Array(f)) = form.get_mut(b"Fields") {
                        prune(f);
                    }
                }
            }
            // Kids of other fields.
            let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
            for id in ids {
                if let Ok(Object::Dictionary(d)) = doc.get_object_mut(id) {
                    if let Ok(Object::Array(k)) = d.get_mut(b"Kids") {
                        prune(k);
                    }
                }
            }
        }
    }
    count
}

/// Removes everything under `boxes` (fractions of the shown page) on one
/// page. `resources` is the page's own copy of its resources, which is
/// updated with new pictures and forms.
pub fn remove_under(
    doc: &mut Document,
    page: ObjectId,
    resources: &mut Dictionary,
    boxes: &[FBox],
) -> Result<Outcome> {
    let geo = Geometry::of(doc, page);
    let to_shown = invert(&geo.matrix()).unwrap_or(IDENTITY);
    let bytes = doc.get_page_content(page);
    let content = Content::decode(&bytes).map_err(|e| crate::Error::Read(e.to_string()))?;
    let mut w = Walker {
        doc,
        to_shown,
        shown: geo.shown(),
        boxes,
        flatten: false,
        notes: Vec::new(),
        counter: 0,
    };
    // Names already used on the page.
    if let Ok(Object::Dictionary(x)) = resources.get(b"XObject") {
        w.counter = x.len();
    }
    let result = w.walk(&content.operations, resources, IDENTITY, 0);
    let mut outcome = Outcome {
        changed: result.is_some(),
        needs_flatten: w.flatten,
        notes: w.notes,
    };
    if let Some((ops, adds)) = result {
        apply_additions(doc, resources, adds);
        let mut stream = Stream::new(Dictionary::new(), encode(&ops));
        let _ = stream.compress();
        let id = doc.add_object(stream);
        let d = doc
            .get_dictionary_mut(page)
            .map_err(|e| crate::Error::Read(e.to_string()))?;
        d.set("Contents", Object::Reference(id));
    }
    if remove_annotations(doc, page, &geo, boxes) > 0 {
        outcome.changed = true;
    }
    Ok(outcome)
}
