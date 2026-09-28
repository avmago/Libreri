//! Looking at a PDF as it will be seen, with hayro: where every character
//! is drawn (to check that nothing is left under a redaction) and the page
//! as a picture (to flatten a page when needed).

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt,
};
use hayro::hayro_syntax::Pdf;
use kurbo::{Affine, BezPath, Point, Rect};

#[derive(Default)]
struct Glyphs {
    /// Glyph origins (and a point half an em up) in shown page points.
    at: Vec<(f64, f64)>,
}

impl<'a> Device<'a> for Glyphs {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph(
        &mut self,
        g: &Glyph<'a>,
        t: Affine,
        gt: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        let m = t * gt;
        let advance = match g {
            Glyph::Outline(o) => f64::from(o.advance_width().unwrap_or(500.0)),
            Glyph::Type3(_) => 500.0,
        };
        // The middle of the glyph: half its advance along, a third of an em up.
        let c = m * Point::new(advance / 2.0, 300.0);
        self.at.push((c.x, c.y));
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

pub fn open(bytes: Vec<u8>) -> Option<Pdf> {
    Pdf::new(bytes).ok()
}

/// Middles of every character drawn on a page (0-based), as fractions of
/// the shown page. Invisible text counts too.
pub fn glyph_middles(pdf: &Pdf, index: usize) -> Vec<(f64, f64)> {
    let pages = pdf.pages();
    let Some(page) = pages.get(index) else {
        return Vec::new();
    };
    let (w, h) = page.render_dimensions();
    let cache = InterpreterCache::new();
    let mut ctx = Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, f64::from(w), f64::from(h)),
        &cache,
        page.xref(),
        InterpreterSettings {
            render_annotations: false,
            ..Default::default()
        },
    );
    let mut dev = Glyphs::default();
    interpret_page(page, &mut ctx, &mut dev);
    dev.at
        .into_iter()
        .map(|(x, y)| (x / f64::from(w), y / f64::from(h)))
        .collect()
}

/// A page (0-based) as a JPEG at about `dpi`, as shown (rotation applied).
pub fn page_jpeg(pdf: &Pdf, index: usize, dpi: f32) -> Option<Vec<u8>> {
    let pages = pdf.pages();
    let page = pages.get(index)?;
    let (w, h) = page.render_dimensions();
    let mut scale = dpi / 72.0;
    let longest = w.max(h) * scale;
    if longest > 6000.0 {
        scale *= 6000.0 / longest;
    }
    let cache = hayro::RenderCache::new();
    let settings = hayro::RenderSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        ..Default::default()
    };
    let pixmap = hayro::render(page, &cache, &InterpreterSettings::default(), &settings);
    let (pw, ph) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
    let rgb: Vec<u8> = pixmap
        .data_as_u8_slice()
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let img = image::RgbImage::from_raw(pw, ph, rgb)?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88)
        .encode_image(&img)
        .ok()?;
    Some(out.into_inner())
}
