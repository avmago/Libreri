//! Reading the text of a page image with Tesseract (a helper program).
//!
//! Tesseract writes TSV: one row per page, block, paragraph, line and word
//! with pixel boxes. Words keep their boxes as fractions of the page (top
//! left origin), the same shape as DjVu text, so the page reader can select,
//! highlight and find in OCR text too.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const NOT_INSTALLED: &str =
    "Tesseract is not installed. Install it in Settings › Helper programs to read scanned pages.";

/// One recognised word and where it is on the page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrWord {
    #[serde(rename = "t")]
    pub text: String,
    /// x, y, width, height as fractions of the page, top-left origin,
    /// rounded to 1/10000.
    #[serde(rename = "r")]
    pub rect: [f32; 4],
}

/// The text of one page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OcrPage {
    pub page: u32,
    /// Lines separated by `\n`, paragraphs by a blank line.
    pub text: String,
    pub words: Vec<OcrWord>,
    /// Tesseract's mean word confidence, 0–100.
    #[serde(default)]
    pub confidence: f32,
}

fn round4(v: f32) -> f32 {
    (v * 10000.0).round() / 10000.0
}

/// Parses Tesseract's TSV output.
pub fn parse_tsv(tsv: &str, page: u32) -> OcrPage {
    let mut out = OcrPage {
        page,
        ..Default::default()
    };
    let (mut pw, mut ph) = (0f32, 0f32);
    let mut last: Option<(u32, u32, u32)> = None;
    let mut conf_sum = 0f32;
    for row in tsv.lines().skip(1) {
        let cols: Vec<&str> = row.splitn(12, '\t').collect();
        if cols.len() < 11 {
            continue;
        }
        let num = |i: usize| cols[i].trim().parse::<f32>().unwrap_or(0.0);
        match cols[0] {
            "1" => {
                pw = num(8);
                ph = num(9);
            }
            "5" => {
                let text = cols.get(11).map(|t| t.trim()).unwrap_or("");
                if text.is_empty() || pw <= 0.0 || ph <= 0.0 {
                    continue;
                }
                let key = (num(2) as u32, num(3) as u32, num(4) as u32);
                if let Some(prev) = last {
                    if prev.0 != key.0 || prev.1 != key.1 {
                        out.text.push_str("\n\n");
                    } else if prev.2 != key.2 {
                        out.text.push('\n');
                    } else {
                        out.text.push(' ');
                    }
                }
                last = Some(key);
                out.text.push_str(text);
                conf_sum += num(10).max(0.0);
                out.words.push(OcrWord {
                    text: text.to_owned(),
                    rect: [
                        round4(num(6) / pw),
                        round4(num(7) / ph),
                        round4(num(8) / pw),
                        round4(num(9) / ph),
                    ],
                });
            }
            _ => {}
        }
    }
    if !out.words.is_empty() {
        out.confidence = conf_sum / out.words.len() as f32;
    }
    out
}

/// Runs Tesseract on one image file. `languages` are Tesseract codes
/// ("eng", "fra"); `tessdata` is the folder with their data files, or
/// `None` for Tesseract's own.
pub fn recognize(
    image: &Path,
    page: u32,
    languages: &[String],
    tessdata: Option<&Path>,
    dpi: Option<u32>,
) -> Result<OcrPage, String> {
    let mut cmd = libreri_helpers::command("tesseract").ok_or(NOT_INSTALLED)?;
    cmd.arg(image).arg("stdout");
    if let Some(dir) = tessdata {
        cmd.arg("--tessdata-dir").arg(dir);
    }
    let langs = if languages.is_empty() {
        "eng".to_owned()
    } else {
        languages.join("+")
    };
    cmd.args(["-l", &langs, "--psm", "3"]);
    if let Some(dpi) = dpi {
        cmd.args(["--dpi", &dpi.to_string()]);
    }
    cmd.arg("tsv");
    // Several pages run side by side; one thread each is faster overall.
    cmd.env("OMP_THREAD_LIMIT", "1");
    let out = cmd
        .output()
        .map_err(|e| format!("Tesseract could not start: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .find(|l| l.contains("Error") || l.contains("Failed") || l.contains("failed"))
            .unwrap_or_else(|| err.lines().last().unwrap_or("unknown error"));
        return Err(format!(
            "Tesseract could not read the page: {}",
            line.trim()
        ));
    }
    Ok(parse_tsv(&String::from_utf8_lossy(&out.stdout), page))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TSV: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext
1\t1\t0\t0\t0\t0\t0\t0\t1000\t2000\t-1\t
2\t1\t1\t0\t0\t0\t100\t100\t800\t300\t-1\t
5\t1\t1\t1\t1\t1\t100\t100\t200\t50\t96\tThe
5\t1\t1\t1\t1\t2\t320\t100\t300\t50\t90\tkeeper
5\t1\t1\t1\t2\t1\t100\t200\t200\t50\t80\twrote
5\t1\t1\t2\t1\t1\t100\t400\t200\t50\t70\tAgain
5\t1\t1\t2\t1\t2\t100\t400\t200\t50\t-1\t ";

    #[test]
    fn parses_words_lines_and_paragraphs() {
        let p = parse_tsv(TSV, 3);
        assert_eq!(p.page, 3);
        assert_eq!(p.text, "The keeper\nwrote\n\nAgain");
        assert_eq!(p.words.len(), 4);
        assert_eq!(p.words[1].rect, [0.32, 0.05, 0.3, 0.025]);
        assert_eq!(p.confidence, 84.0);
    }

    #[test]
    fn reads_a_rendered_page() {
        if libreri_helpers::find_program("tesseract").is_none() {
            eprintln!("Tesseract is not installed; skipped");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("t.pdf");
        crate::test_text_pdf(&pdf, &["The lighthouse keeper wrote every night"]);
        let png = crate::pdftext::render_page_png(&pdf, 1, 300.0, 4000).unwrap();
        let img = dir.path().join("p.png");
        std::fs::write(&img, png).unwrap();
        let page = recognize(&img, 1, &["eng".into()], None, Some(300)).unwrap();
        assert!(page.text.contains("lighthouse keeper"), "{page:?}");
        let w = &page.words[0];
        assert!(w.rect[0] > 0.1 && w.rect[0] < 0.14, "{w:?}");
    }
}
