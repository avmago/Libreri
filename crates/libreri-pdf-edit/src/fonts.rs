//! How wide each character of a PDF font is, enough to know where the text
//! of a page lies: `/Widths` of simple fonts, `/W` and `/DW` of CID fonts,
//! and built-in metrics for the standard 14 fonts when a PDF leaves them
//! out.

use crate::geom::{num, resolve};
use crate::std_fonts;
use lopdf::{Dictionary, Document, Object};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FontInfo {
    /// Two bytes per character (Type0 / CID fonts).
    pub two_byte: bool,
    /// Width per character code, in thousandths of the font size.
    widths: HashMap<u32, f64>,
    default: f64,
}

impl FontInfo {
    pub fn width(&self, code: u32) -> f64 {
        self.widths.get(&code).copied().unwrap_or(self.default)
    }

    /// Splits a string operand into character codes.
    pub fn codes(&self, bytes: &[u8]) -> Vec<u32> {
        if self.two_byte {
            bytes
                .chunks(2)
                .map(|c| u32::from(c[0]) << 8 | u32::from(*c.get(1).unwrap_or(&0)))
                .collect()
        } else {
            bytes.iter().map(|&b| u32::from(b)).collect()
        }
    }

    /// The bytes of one code.
    pub fn bytes(&self, code: u32) -> Vec<u8> {
        if self.two_byte {
            vec![(code >> 8) as u8, code as u8]
        } else {
            vec![code as u8]
        }
    }

    /// A font Libreri knows nothing about: every character half an em.
    pub fn unknown() -> Self {
        Self {
            two_byte: false,
            widths: HashMap::new(),
            default: 500.0,
        }
    }
}

fn get<'a>(doc: &'a Document, d: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    d.get(key).ok().map(|o| resolve(doc, o))
}

fn name(o: &Object) -> Option<String> {
    o.as_name()
        .ok()
        .map(|n| String::from_utf8_lossy(n).into_owned())
}

/// The standard font a base font name stands for, if any.
fn standard(base: &str) -> Option<&'static [(&'static str, u16)]> {
    let base = base
        .split_once('+')
        .map_or(base, |(_, b)| b)
        .replace([' ', ','], "-");
    let b = base.to_ascii_lowercase();
    let bold = b.contains("bold");
    let italic = b.contains("italic") || b.contains("oblique");
    if b.starts_with("courier") {
        return None; // all 600, handled by the caller
    }
    if b.starts_with("helvetica") || b.starts_with("arial") {
        return Some(if bold {
            std_fonts::HELVETICA_BOLD
        } else {
            std_fonts::HELVETICA
        });
    }
    if b.starts_with("times") {
        return Some(match (bold, italic) {
            (true, true) => std_fonts::TIMES_BOLD_ITALIC,
            (true, false) => std_fonts::TIMES_BOLD,
            (false, true) => std_fonts::TIMES_ITALIC,
            (false, false) => std_fonts::TIMES_ROMAN,
        });
    }
    None
}

/// Glyph names by code for a simple font's encoding.
fn encoding(doc: &Document, font: &Dictionary) -> Vec<String> {
    let base_of = |n: &str| -> &'static [&'static str; 256] {
        match n {
            "WinAnsiEncoding" => &std_fonts::WIN_ANSI,
            "MacRomanEncoding" => &std_fonts::MAC_ROMAN,
            _ => &std_fonts::STANDARD,
        }
    };
    let mut names: Vec<String> = std_fonts::STANDARD.iter().map(|s| s.to_string()).collect();
    match get(doc, font, b"Encoding") {
        Some(o @ Object::Name(_)) => {
            names = base_of(&name(o).unwrap_or_default())
                .iter()
                .map(|s| s.to_string())
                .collect();
        }
        Some(Object::Dictionary(d)) => {
            if let Some(b) = get(doc, d, b"BaseEncoding").and_then(name) {
                names = base_of(&b).iter().map(|s| s.to_string()).collect();
            }
            if let Some(Object::Array(diffs)) = get(doc, d, b"Differences") {
                let mut code = 0usize;
                for item in diffs {
                    match resolve(doc, item) {
                        Object::Integer(i) => code = (*i).clamp(0, 255) as usize,
                        o => {
                            if let (Some(n), true) = (name(o), code < 256) {
                                names[code] = n;
                                code += 1;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    names
}

/// Reads what is needed from a font dictionary.
pub fn font_info(doc: &Document, font: &Dictionary) -> FontInfo {
    let subtype = get(doc, font, b"Subtype")
        .and_then(name)
        .unwrap_or_default();
    if subtype == "Type0" {
        let mut info = FontInfo {
            two_byte: true,
            widths: HashMap::new(),
            default: 1000.0,
        };
        let desc = get(doc, font, b"DescendantFonts")
            .and_then(|o| o.as_array().ok())
            .and_then(|a| a.first())
            .and_then(|o| resolve(doc, o).as_dict().ok());
        if let Some(desc) = desc {
            if let Some(dw) = get(doc, desc, b"DW").and_then(num) {
                info.default = dw;
            }
            if let Some(Object::Array(w)) = get(doc, desc, b"W") {
                let mut i = 0;
                while i < w.len() {
                    let first = num(resolve(doc, &w[i])).unwrap_or(0.0) as u32;
                    match w.get(i + 1).map(|o| resolve(doc, o)) {
                        Some(Object::Array(list)) => {
                            for (k, v) in list.iter().enumerate() {
                                if let Some(v) = num(resolve(doc, v)) {
                                    info.widths.insert(first + k as u32, v);
                                }
                            }
                            i += 2;
                        }
                        Some(o) => {
                            let last = num(o).unwrap_or(0.0) as u32;
                            let v = w
                                .get(i + 2)
                                .and_then(|x| num(resolve(doc, x)))
                                .unwrap_or(info.default);
                            for c in first..=last.min(first + 65_535) {
                                info.widths.insert(c, v);
                            }
                            i += 3;
                        }
                        None => break,
                    }
                }
            }
        }
        return info;
    }

    let mut info = FontInfo {
        two_byte: false,
        widths: HashMap::new(),
        default: 0.0,
    };
    // Type 3 widths are in glyph space.
    let scale = if subtype == "Type3" {
        get(doc, font, b"FontMatrix")
            .and_then(|o| o.as_array().ok())
            .and_then(|a| a.first())
            .and_then(|o| num(resolve(doc, o)))
            .map_or(1.0, |a| a * 1000.0)
    } else {
        1.0
    };
    if let Some(missing) = get(doc, font, b"FontDescriptor")
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| get(doc, d, b"MissingWidth"))
        .and_then(num)
    {
        info.default = missing * scale;
    }
    if let Some(Object::Array(widths)) = get(doc, font, b"Widths") {
        let first = get(doc, font, b"FirstChar").and_then(num).unwrap_or(0.0) as u32;
        for (k, v) in widths.iter().enumerate() {
            if let Some(v) = num(resolve(doc, v)) {
                info.widths.insert(first + k as u32, v * scale);
            }
        }
        if info.default == 0.0 {
            info.default = 500.0;
        }
        return info;
    }
    // No widths: a standard font.
    let base = get(doc, font, b"BaseFont")
        .and_then(name)
        .unwrap_or_default();
    if base.to_ascii_lowercase().contains("courier") {
        info.default = 600.0;
        return info;
    }
    match standard(&base) {
        Some(table) => {
            let names = encoding(doc, font);
            for (code, n) in names.iter().enumerate() {
                if let Ok(i) = table.binary_search_by(|(k, _)| k.cmp(&n.as_str())) {
                    info.widths.insert(code as u32, f64::from(table[i].1));
                }
            }
            info.default = 500.0;
        }
        None => info.default = 500.0,
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn standard_font_widths() {
        let doc = Document::new();
        let helv = dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" };
        let f = font_info(&doc, &helv);
        assert_eq!(f.width(u32::from(b'A')), 667.0);
        assert_eq!(f.width(u32::from(b' ')), 278.0);
        let times =
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Bold" };
        assert_eq!(font_info(&doc, &times).width(u32::from(b'M')), 944.0);
        let courier = dictionary! { "Subtype" => "Type1", "BaseFont" => "Courier" };
        assert_eq!(font_info(&doc, &courier).width(65), 600.0);
    }

    #[test]
    fn widths_arrays() {
        let doc = Document::new();
        let simple = dictionary! {
            "Subtype" => "TrueType", "FirstChar" => 65, "Widths" => vec![Object::Integer(700), Object::Integer(710)],
        };
        let f = font_info(&doc, &simple);
        assert_eq!(
            (f.width(65), f.width(66), f.width(90)),
            (700.0, 710.0, 500.0)
        );
        let cid = dictionary! {
            "Subtype" => "Type0",
            "DescendantFonts" => vec![Object::Dictionary(dictionary! {
                "DW" => 900,
                "W" => vec![Object::Integer(3), Object::Array(vec![Object::Integer(250), Object::Integer(300)]),
                            Object::Integer(10), Object::Integer(12), Object::Integer(400)],
            })],
        };
        let f = font_info(&doc, &cid);
        assert!(f.two_byte);
        assert_eq!(
            (f.width(3), f.width(4), f.width(11), f.width(99)),
            (250.0, 300.0, 400.0, 900.0)
        );
        assert_eq!(f.codes(&[0, 3, 0, 4]), vec![3, 4]);
    }
}
