//! Scanning book barcodes.
//!
//! [`decode`] reads the EAN-13 (ISBN) or UPC barcode in a picture: a frame
//! from the computer's camera, a photo file, or a photo sent by a phone.
//! [`PhoneScanner`] serves a one-time page on the local network so a phone
//! can take the photo (see docs/adr/0013-online-details.md).

pub mod paper;
mod phone;

pub use phone::{PhoneEvent, PhoneMode, PhonePairing, PhoneScanner};

use image::{DynamicImage, GenericImageView};
use rxing::{BarcodeFormat, DecodeHints};
use std::collections::HashSet;

/// A barcode read from a picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scanned {
    /// The digits as printed.
    pub code: String,
    /// The ISBN-13, if the code is one (978… or 979…, or an ISBN-10).
    pub isbn13: Option<String>,
}

impl Scanned {
    pub fn new(code: &str) -> Self {
        let code: String = code
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        let isbn13 = libreri_core::isbn::normalize_isbn13(&code)
            .ok()
            .or_else(|| libreri_core::isbn::isbn10_to_13(&code));
        Self { code, isbn13 }
    }
}

/// Longest side a picture is scaled down to before reading; phone photos
/// are far larger than a barcode needs.
const WORK_SIZE: u32 = 1600;

fn formats() -> HashSet<BarcodeFormat> {
    HashSet::from([
        BarcodeFormat::EAN_13,
        BarcodeFormat::EAN_8,
        BarcodeFormat::UPC_A,
        BarcodeFormat::UPC_E,
    ])
}

fn read_luma(img: &DynamicImage) -> Option<String> {
    let luma = img.to_luma8();
    let (w, h) = luma.dimensions();
    let mut hints = DecodeHints {
        PossibleFormats: Some(formats()),
        TryHarder: Some(true),
        ..Default::default()
    };
    rxing::helpers::detect_in_luma_with_hints(luma.into_raw(), w, h, None, &mut hints)
        .ok()
        .map(|r| r.getText().to_owned())
}

/// Reads a barcode from an image file's bytes (JPEG, PNG, WebP, GIF, BMP).
/// Returns `Ok(None)` if the picture has no readable barcode.
pub fn decode(bytes: &[u8]) -> Result<Option<Scanned>, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|_| "that is not a picture Libreri can read".to_owned())?;
    Ok(decode_image(&img))
}

pub fn decode_image(img: &DynamicImage) -> Option<Scanned> {
    let (w, h) = img.dimensions();
    let small = if w.max(h) > WORK_SIZE {
        img.resize(WORK_SIZE, WORK_SIZE, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    // Barcodes photographed sideways, and small barcodes lost when scaling.
    let tries = [
        Some(small.clone()),
        Some(small.rotate90()),
        (w.max(h) > WORK_SIZE).then(|| img.clone()),
    ];
    tries
        .iter()
        .flatten()
        .find_map(read_luma)
        .map(|c| Scanned::new(&c))
}

#[cfg(test)]
pub(crate) mod testing {
    use image::{GrayImage, Luma};
    use rxing::Writer;

    /// A picture of an EAN-13 barcode on a white page, `scale` pixels per
    /// bar.
    pub fn barcode_png(code: &str, rotate: bool) -> Vec<u8> {
        let matrix = rxing::MultiFormatWriter
            .encode(code, &rxing::BarcodeFormat::EAN_13, 400, 160)
            .unwrap();
        let (mw, mh) = (matrix.getWidth(), matrix.getHeight());
        let mut img = GrayImage::from_pixel(mw + 200, mh + 300, Luma([255]));
        for y in 0..mh {
            for x in 0..mw {
                if matrix.get(x, y) {
                    img.put_pixel(x + 100, y + 150, Luma([20]));
                }
            }
        }
        let img = image::DynamicImage::ImageLuma8(img);
        let img = if rotate { img.rotate90() } else { img };
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::barcode_png;
    use super::*;

    #[test]
    fn reads_isbn_barcodes() {
        let s = decode(&barcode_png("9780306406157", false))
            .unwrap()
            .unwrap();
        assert_eq!(s.code, "9780306406157");
        assert_eq!(s.isbn13.as_deref(), Some("9780306406157"));
        let s = decode(&barcode_png("9780306406157", true))
            .unwrap()
            .unwrap();
        assert_eq!(s.code, "9780306406157", "sideways");
    }

    #[test]
    fn pictures_without_barcodes() {
        let mut blank = Vec::new();
        image::DynamicImage::new_luma8(300, 200)
            .write_to(
                &mut std::io::Cursor::new(&mut blank),
                image::ImageFormat::Png,
            )
            .unwrap();
        assert_eq!(decode(&blank).unwrap(), None);
        assert!(decode(b"not a picture").is_err());
    }

    #[test]
    fn non_book_codes_have_no_isbn() {
        let s = Scanned::new("4006381333931");
        assert_eq!(s.isbn13, None);
        assert_eq!(
            Scanned::new("0-306-40615-2").isbn13.as_deref(),
            Some("9780306406157")
        );
    }
}
