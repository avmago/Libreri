//! PDF text for the search index and page images for OCR, both through
//! hayro (a PDF interpreter and rasterizer in safe Rust).
//!
//! Text comes from the glyphs a page draws: each glyph's Unicode value and
//! position, put back into words and lines by distance. That also catches
//! invisible OCR text layers, so a PDF that was made searchable elsewhere
//! counts as having text.

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt,
};
use kurbo::{Affine, BezPath, Point, Rect};
use std::path::Path;

/// Largest PDF read whole into memory for text or OCR.
const MAX_BYTES: u64 = 1024 * 1024 * 1024;

fn load(path: &Path) -> Result<Pdf, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_BYTES {
        return Err("the PDF is too large to read".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Pdf::new(bytes).map_err(|e| match e {
        hayro::hayro_syntax::LoadPdfError::Decryption(_) => "the PDF is password-protected".into(),
        hayro::hayro_syntax::LoadPdfError::Invalid => "the PDF could not be read".into(),
    })
}

/// One character where the page draws it (top-left origin, points).
#[derive(Debug, Clone, Copy)]
struct Placed {
    ch: char,
    x: f64,
    y: f64,
    end: f64,
    size: f64,
}

#[derive(Default)]
struct TextDevice {
    chars: Vec<Placed>,
}

impl<'a> Device<'a> for TextDevice {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        let Some(text) = glyph.as_unicode() else {
            return;
        };
        let m = transform * glyph_transform;
        let origin = m * Point::ZERO;
        let advance = match glyph {
            Glyph::Outline(g) => g.advance_width().unwrap_or(0.0) as f64,
            Glyph::Type3(_) => 0.0,
        };
        let end = (m * Point::new(advance, 0.0)).x;
        let size = (m * Point::new(0.0, 1000.0) - origin).hypot().max(1.0);
        let chars: Vec<char> = match text {
            BfString::Char(c) => vec![c],
            BfString::String(s) => s.chars().collect(),
        };
        let n = chars.len().max(1) as f64;
        let step = (end - origin.x) / n;
        for (i, ch) in chars.into_iter().enumerate() {
            if ch == '\0' {
                continue;
            }
            let x = origin.x + step * i as f64;
            self.chars.push(Placed {
                ch,
                x,
                y: origin.y,
                end: x + step,
                size,
            });
        }
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// Puts characters back into lines and words, in drawing order.
fn assemble(chars: &[Placed]) -> String {
    let mut out = String::new();
    let mut prev: Option<Placed> = None;
    for &c in chars {
        if let Some(p) = prev {
            let line_gap = (c.y - p.y).abs();
            let size = p.size.min(c.size);
            if line_gap > size * 0.6 || c.x < p.x - size {
                // A new line: hyphenated words are joined back together.
                if out.ends_with('-') && c.ch.is_alphabetic() {
                    let before = out[..out.len() - 1].chars().last();
                    if before.is_some_and(char::is_alphabetic) {
                        out.pop();
                        prev = Some(c);
                        out.push(c.ch);
                        continue;
                    }
                }
                if line_gap > size * 2.0 {
                    out.push_str("\n\n");
                } else {
                    out.push('\n');
                }
            } else if c.x - p.end > size * 0.2 && !c.ch.is_whitespace() && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        if c.ch.is_whitespace() {
            if !out.ends_with([' ', '\n']) && !out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(c.ch);
        }
        prev = Some(c);
    }
    out.trim().to_owned()
}

fn page_context<'a>(
    page: &hayro::hayro_syntax::page::Page<'a>,
    cache: &InterpreterCache<'a>,
) -> Context<'a> {
    let (w, h) = page.render_dimensions();
    Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, w as f64, h as f64),
        cache,
        page.xref(),
        InterpreterSettings {
            render_annotations: false,
            ..Default::default()
        },
    )
}

/// The text of every page, in page order (empty for pages without text).
pub fn page_texts(path: &Path) -> Result<Vec<String>, String> {
    let pdf = load(path)?;
    let cache = InterpreterCache::new();
    let mut out = Vec::with_capacity(pdf.pages().len());
    for page in pdf.pages().iter() {
        let mut ctx = page_context(page, &cache);
        let mut dev = TextDevice::default();
        interpret_page(page, &mut ctx, &mut dev);
        out.push(assemble(&dev.chars));
    }
    Ok(out)
}

/// A PDF opened once for rendering many pages (shared between threads).
pub struct PdfDoc {
    pdf: Pdf,
}

impl PdfDoc {
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self { pdf: load(path)? })
    }

    pub fn page_count(&self) -> u32 {
        self.pdf.pages().len() as u32
    }

    /// Renders one page (1-based) as a grey PNG for OCR at about `dpi` dots
    /// per inch, never wider or taller than `max_side` pixels. Returns the
    /// PNG and the resolution actually used.
    pub fn render_png(&self, page: u32, dpi: f32, max_side: u32) -> Result<(Vec<u8>, u32), String> {
        let pages = self.pdf.pages();
        let page = pages
            .get(page.saturating_sub(1) as usize)
            .ok_or("the page does not exist")?;
        let (w, h) = page.render_dimensions();
        let mut scale = dpi / 72.0;
        let longest = w.max(h) * scale;
        if longest > max_side as f32 {
            scale *= max_side as f32 / longest;
        }
        let cache = hayro::RenderCache::new();
        let settings = hayro::RenderSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            ..Default::default()
        };
        let pixmap = hayro::render(page, &cache, &InterpreterSettings::default(), &settings);
        let (pw, ph) = (pixmap.width() as u32, pixmap.height() as u32);
        // Grey is all OCR needs, and a quarter of the size.
        let grey: Vec<u8> = pixmap
            .data_as_u8_slice()
            .chunks_exact(4)
            .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
            .collect();
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, pw, ph);
            enc.set_color(png::ColorType::Grayscale);
            enc.set_depth(png::BitDepth::Eight);
            enc.set_compression(png::Compression::Fast);
            let mut writer = enc.write_header().map_err(|e| e.to_string())?;
            writer.write_image_data(&grey).map_err(|e| e.to_string())?;
        }
        Ok((png, (scale * 72.0).round() as u32))
    }
}

/// Renders one page; see [`PdfDoc::render_png`].
pub fn render_page_png(path: &Path, page: u32, dpi: f32, max_side: u32) -> Result<Vec<u8>, String> {
    Ok(PdfDoc::open(path)?.render_png(page, dpi, max_side)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_of_generated_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.pdf");
        crate::test_text_pdf(
            &path,
            &[
                "The lighthouse keeper\nwrote every night",
                "",
                "Second-\nhand news",
            ],
        );
        let texts = page_texts(&path).unwrap();
        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0], "The lighthouse keeper\nwrote every night");
        assert_eq!(texts[1], "");
        assert_eq!(texts[2], "Secondhand news");
    }

    #[test]
    fn renders_a_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.pdf");
        crate::test_pdf(&path, 1);
        let png = render_page_png(&path, 1, 150.0, 4000).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(render_page_png(&path, 5, 150.0, 4000).is_err());
    }

    #[test]
    fn words_and_lines() {
        let at = |ch, x: f64, y: f64| Placed {
            ch,
            x,
            y,
            end: x + 6.0,
            size: 12.0,
        };
        let chars = vec![
            at('a', 0.0, 10.0),
            at('b', 6.0, 10.0),
            at('c', 20.0, 10.0),
            at('d', 0.0, 25.0),
            at('e', 6.0, 25.0),
        ];
        assert_eq!(assemble(&chars), "ab c\nde");
    }
}
