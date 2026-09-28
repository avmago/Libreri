//! Cover images and thumbnails.
//!
//! Any cover (embedded in an EPUB, the first page of a comic, a PDF page
//! rendered by the interface, or later a download) goes through
//! [`make_covers`], which produces two JPEGs: a cover for the details panel
//! and a small thumbnail for the grid.

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageReader, Limits, Rgb, RgbImage};
use std::io::Cursor;

/// Longest side of the stored cover.
pub const COVER_MAX: u32 = 1200;
/// Bounding box of the grid thumbnail (2× the largest grid card).
pub const THUMB_WIDTH: u32 = 360;
pub const THUMB_HEIGHT: u32 = 540;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the image could not be read: {0}")]
    Decode(#[from] image::ImageError),
    #[error("the image is empty")]
    Empty,
}

/// A cover and its thumbnail, both JPEG.
#[derive(Debug, Clone)]
pub struct Covers {
    pub cover: Vec<u8>,
    pub thumbnail: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// True if the image is one plain colour (a cover drawn from a page that
/// could not be rendered).
pub fn is_blank(bytes: &[u8]) -> bool {
    let Ok(img) = image::load_from_memory(bytes) else {
        return false;
    };
    let small = img.thumbnail(64, 64).to_rgb8();
    let first = *small.get_pixel(0, 0);
    small
        .pixels()
        .all(|p| (0..3).all(|c| p[c].abs_diff(first[c]) <= 12))
}

fn flatten(img: &DynamicImage) -> RgbImage {
    // Transparent PNG covers go on white, like paper.
    let rgba = img.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = u16::from(p[3]);
        let mix = |c: u8| ((u16::from(c) * a + 255 * (255 - a)) / 255) as u8;
        out.put_pixel(x, y, Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
    }
    out
}

fn jpeg(img: &RgbImage, quality: u8) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality).encode_image(img)?;
    Ok(buf)
}

/// Decodes any supported image (including the PNM pages DjVuLibre writes)
/// and encodes it as JPEG, for pages shown in the reader.
pub fn to_jpeg(bytes: &[u8], quality: u8) -> Result<Vec<u8>, Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Decode(e.into()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    reader.limits(limits);
    let img = reader.decode()?;
    jpeg(&flatten(&img), quality)
}

/// Decodes `bytes` (JPEG, PNG, GIF, WebP or BMP) and makes the cover and
/// thumbnail.
pub fn make_covers(bytes: &[u8]) -> Result<Covers, Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Decode(e.into()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    reader.limits(limits);
    let img = reader.decode()?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(Error::Empty);
    }
    let cover = if w.max(h) > COVER_MAX {
        img.resize(COVER_MAX, COVER_MAX, FilterType::Triangle)
    } else {
        img.clone()
    };
    let thumb = img.resize(THUMB_WIDTH, THUMB_HEIGHT, FilterType::Triangle);
    let cover_rgb = flatten(&cover);
    Ok(Covers {
        width: cover_rgb.width(),
        height: cover_rgb.height(),
        cover: jpeg(&cover_rgb, 88)?,
        thumbnail: jpeg(&flatten(&thumb), 82)?,
    })
}

/// A picture's bytes, media type, width and height.
pub type Picture = (Vec<u8>, &'static str, u32, u32);

/// A picture made ready to place on a page: at most `max` pixels on its
/// longest side, PNG when it has transparency, JPEG otherwise. Returns the
/// bytes, their media type, and the width and height.
pub fn picture(bytes: &[u8], max: u32) -> Result<Picture, Error> {
    let img = image::load_from_memory(bytes)?;
    let img = if img.width().max(img.height()) > max {
        img.resize(max, max, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let (w, h) = (img.width(), img.height());
    let mut out = std::io::Cursor::new(Vec::new());
    if img.color().has_alpha() {
        img.write_to(&mut out, image::ImageFormat::Png)?;
        Ok((out.into_inner(), "image/png", w, h))
    } else {
        let rgb = img.to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&rgb)?;
        Ok((out.into_inner(), "image/jpeg", w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnm_pages_become_jpeg() {
        let pnm = b"P6\n2 1\n255\n\xff\x00\x00\x00\xff\x00".to_vec();
        let jpg = to_jpeg(&pnm, 85).unwrap();
        assert!(jpg.starts_with(&[0xFF, 0xD8]));
        assert!(to_jpeg(b"nope", 85).is_err());
    }

    #[test]
    fn blank_covers_are_recognised() {
        let white = make_covers(&png_of(RgbImage::from_pixel(
            300,
            450,
            Rgb([250, 248, 243]),
        )))
        .unwrap();
        assert!(is_blank(&white.thumbnail));
        let mut page = RgbImage::from_pixel(300, 450, Rgb([255, 255, 255]));
        for x in 20..280 {
            page.put_pixel(x, 100, Rgb([0, 0, 0]));
            page.put_pixel(x, 101, Rgb([0, 0, 0]));
        }
        let text = make_covers(&png_of(page)).unwrap();
        assert!(!is_blank(&text.thumbnail));
        assert!(!is_blank(b"not an image"));
    }
    use image::{ImageFormat, Rgba, RgbaImage};

    fn png_of(img: RgbImage) -> Vec<u8> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([200, 30, 30, 128]));
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn makes_bounded_jpegs() {
        let c = make_covers(&png(1600, 2400)).unwrap();
        assert_eq!((c.width, c.height), (800, 1200));
        let t = image::load_from_memory(&c.thumbnail).unwrap();
        assert_eq!(t.dimensions(), (360, 540));
        assert_eq!(&c.cover[..2], &[0xFF, 0xD8]); // JPEG magic
    }

    #[test]
    fn small_images_are_not_enlarged_for_the_cover() {
        let c = make_covers(&png(100, 150)).unwrap();
        assert_eq!((c.width, c.height), (100, 150));
    }

    #[test]
    fn rejects_garbage() {
        assert!(make_covers(b"not an image").is_err());
    }

    #[test]
    fn pictures_are_made_smaller_and_keep_transparency() {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(3000, 1000, image::Rgba([0, 0, 0, 0]))
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let (bytes, mime, w, h) = super::picture(&out.into_inner(), 1600).unwrap();
        assert_eq!((mime, w, h), ("image/png", 1600, 533));
        assert!(bytes.starts_with(b"\x89PNG"));
    }
}
