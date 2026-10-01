//! PaddleOCR-VL with PP-DocLayoutV3, through `oar-ocr-vl`.

use crate::text::region_text;
use libreri_formats::ocr::{words_in_region, OcrPage, PageReader};
use oar_ocr_vl::utils::image::load_image_from_memory;
use oar_ocr_vl::utils::parse_device;
use oar_ocr_vl::{DocParser, PaddleOcrVl, PpDocLayout};
use std::path::Path;
use std::sync::Mutex;

pub struct Paddle {
    /// One page at a time: the model keeps its working memory between
    /// the steps of reading.
    inner: Mutex<(PaddleOcrVl, PpDocLayout)>,
}

impl Paddle {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let open = |device_name: &str| -> Result<(PaddleOcrVl, PpDocLayout), String> {
            let device = parse_device(device_name).map_err(|e| e.to_string())?;
            let layout = PpDocLayout::from_dir(dir.join("layout"), device.clone())
                .map_err(|e| e.to_string())?;
            let model = PaddleOcrVl::from_dir(dir, device).map_err(|e| e.to_string())?;
            Ok((model, layout))
        };
        // The graphics chip of Apple computers, else the processor.
        let loaded = if cfg!(target_os = "macos") {
            open("metal").or_else(|_| open("cpu"))
        } else {
            open("cpu")
        };
        let inner = loaded.map_err(|e| format!("PaddleOCR-VL could not load: {e}"))?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }
}

impl PageReader for Paddle {
    fn engine(&self) -> String {
        "PaddleOCR-VL 1.6".into()
    }

    fn read_page(&self, image: &[u8], page: u32) -> Result<OcrPage, String> {
        let img = load_image_from_memory(image).map_err(|e| e.to_string())?;
        let (w, h) = (img.width() as f32, img.height() as f32);
        if w < 1.0 || h < 1.0 {
            return Err("the page picture is empty".into());
        }
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (model, layout) = &*guard;
        let result = DocParser::new(model)
            .parse(layout, img)
            .map_err(|e| format!("PaddleOCR-VL could not read the page: {e}"))?;
        drop(guard);
        let mut elements = result.layout_elements;
        elements.sort_by_key(|e| e.order_index.unwrap_or(u32::MAX));
        let mut out = OcrPage {
            page,
            confidence: 90.0,
            ..Default::default()
        };
        for el in elements {
            let kind = format!("{:?}", el.element_type);
            let kind = snake(&kind);
            let text = region_text(&kind, el.text.as_deref().unwrap_or(""));
            if text.is_empty() {
                continue;
            }
            let xs = el.bbox.points.iter().map(|p| p.x);
            let ys = el.bbox.points.iter().map(|p| p.y);
            let (x0, x1) = xs.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
            let (y0, y1) = ys.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let region = [
                (x0 / w).clamp(0.0, 1.0),
                (y0 / h).clamp(0.0, 1.0),
                ((x1 - x0) / w).clamp(0.0, 1.0),
                ((y1 - y0) / h).clamp(0.0, 1.0),
            ];
            out.words.extend(words_in_region(&text, region, h / w));
            if !out.text.is_empty() {
                out.text.push_str("\n\n");
            }
            out.text.push_str(&text);
        }
        Ok(out)
    }
}

/// `HeaderImage` → `header_image`.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn snake_case() {
        assert_eq!(super::snake("HeaderImage"), "header_image");
        assert_eq!(super::snake("Text"), "text");
    }
}
