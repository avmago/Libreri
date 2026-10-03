//! Photos of paper pages: finding the page in a photo,
//! straightening it, and cleaning it up so it reads like a scan.

use image::{DynamicImage, GrayImage, ImageBuffer, Luma, Rgb, RgbImage};
use serde::{Deserialize, Serialize};

/// A corner as fractions of the photo (0–1), top-left origin.
pub type Corner = [f64; 2];

/// How a page is cleaned.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Clean {
    /// Colours kept, shadows and a grey cast removed.
    #[default]
    Colour,
    /// Grey, with even lighting.
    Grey,
    /// Black ink on white paper.
    BlackWhite,
    /// Left as photographed.
    None,
}

/// Longest side of a straightened page, in pixels.
const LONGEST: u32 = 2600;

fn luma(p: &Rgb<u8>) -> f32 {
    0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])
}

/// Otsu's threshold for a histogram.
fn otsu(hist: &[u32; 256], total: u32) -> u8 {
    let sum: f64 = hist
        .iter()
        .enumerate()
        .map(|(i, &c)| i as f64 * f64::from(c))
        .sum();
    let (mut sum_b, mut w_b) = (0.0, 0.0);
    let (mut best, mut best_var) = (128u8, -1.0);
    for (t, &c) in hist.iter().enumerate() {
        w_b += f64::from(c);
        if w_b == 0.0 {
            continue;
        }
        let w_f = f64::from(total) - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * f64::from(c);
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;
        let var = w_b * w_f * (m_b - m_f).powi(2);
        if var > best_var {
            best_var = var;
            best = t as u8;
        }
    }
    best
}

/// Where the page is in a photo: its corners (top-left, top-right,
/// bottom-right, bottom-left). Paper is found as the largest bright area;
/// when there is none, the whole photo is the page.
pub fn detect(photo: &DynamicImage) -> [Corner; 4] {
    let whole = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let small = photo.thumbnail(480, 480).to_rgb8();
    let (w, h) = small.dimensions();
    if w < 8 || h < 8 {
        return whole;
    }
    let mut gray = GrayImage::new(w, h);
    let mut hist = [0u32; 256];
    for (x, y, p) in small.enumerate_pixels() {
        let v = luma(p) as u8;
        gray.put_pixel(x, y, Luma([v]));
        hist[v as usize] += 1;
    }
    let t = otsu(&hist, w * h);
    // Largest bright area (4-connected).
    let mut label = vec![0u32; (w * h) as usize];
    let mut best: (u32, usize) = (0, 0);
    let mut next = 0u32;
    let mut stack = Vec::new();
    for start in 0..(w * h) as usize {
        if label[start] != 0 || gray.as_raw()[start] <= t {
            continue;
        }
        next += 1;
        let mut size = 0usize;
        stack.push(start);
        label[start] = next;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = ((i as u32) % w, (i as u32) / w);
            let mut visit = |nx: u32, ny: u32| {
                let j = (ny * w + nx) as usize;
                if label[j] == 0 && gray.as_raw()[j] > t {
                    label[j] = next;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(x - 1, y);
            }
            if x + 1 < w {
                visit(x + 1, y);
            }
            if y > 0 {
                visit(x, y - 1);
            }
            if y + 1 < h {
                visit(x, y + 1);
            }
        }
        if size > best.1 {
            best = (next, size);
        }
    }
    let area = (w * h) as usize;
    // Too small, or the whole photo: nothing to straighten.
    if best.1 < area / 8 || best.1 > area * 97 / 100 {
        return whole;
    }
    // Corners: the points furthest along the four diagonals.
    let mut ext = [(f64::MIN, [0.0, 0.0]); 4];
    for (i, &l) in label.iter().enumerate() {
        if l != best.0 {
            continue;
        }
        let (x, y) = ((i as u32 % w) as f64, (i as u32 / w) as f64);
        let scores = [-x - y, x - y, x + y, -x + y];
        for (k, s) in scores.into_iter().enumerate() {
            if s > ext[k].0 {
                ext[k] = (s, [x, y]);
            }
        }
    }
    let (fw, fh) = (f64::from(w - 1), f64::from(h - 1));
    ext.map(|(_, [x, y])| [(x / fw).clamp(0.0, 1.0), (y / fh).clamp(0.0, 1.0)])
}

/// Solves the 8×8 system for the homography that maps the output
/// rectangle's corners to `src` (pixel coordinates).
fn homography(dst: [[f64; 2]; 4], src: [[f64; 2]; 4]) -> Option<[f64; 9]> {
    let mut a = [[0.0f64; 9]; 8];
    for k in 0..4 {
        let [u, v] = dst[k];
        let [x, y] = src[k];
        a[2 * k] = [u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x, x];
        a[2 * k + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y, y];
    }
    for col in 0..8 {
        let pivot = (col..8).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        let p = a[col][col];
        for v in a[col].iter_mut() {
            *v /= p;
        }
        for row in 0..8 {
            if row != col {
                let f = a[row][col];
                if f != 0.0 {
                    let pivot_row = a[col];
                    for (v, pv) in a[row].iter_mut().zip(pivot_row) {
                        *v -= f * pv;
                    }
                }
            }
        }
    }
    let h: Vec<f64> = (0..8).map(|i| a[i][8]).collect();
    Some([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0])
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Straightens the page with `corners` (fractions) into a flat rectangle.
pub fn straighten(photo: &DynamicImage, corners: [Corner; 4]) -> RgbImage {
    let src = photo.to_rgb8();
    let (w, h) = (f64::from(src.width()), f64::from(src.height()));
    let px = corners.map(|[x, y]| [x.clamp(0.0, 1.0) * (w - 1.0), y.clamp(0.0, 1.0) * (h - 1.0)]);
    let out_w = dist(px[0], px[1]).max(dist(px[3], px[2]));
    let out_h = dist(px[0], px[3]).max(dist(px[1], px[2]));
    let k = (f64::from(LONGEST) / out_w.max(out_h)).min(1.0);
    let (ow, oh) = (
        ((out_w * k).round() as u32).max(16),
        ((out_h * k).round() as u32).max(16),
    );
    let dst = [
        [0.0, 0.0],
        [f64::from(ow - 1), 0.0],
        [f64::from(ow - 1), f64::from(oh - 1)],
        [0.0, f64::from(oh - 1)],
    ];
    let Some(m) = homography(dst, px) else {
        return src;
    };
    let mut out = RgbImage::new(ow, oh);
    for (u, v, p) in out.enumerate_pixels_mut() {
        let (u, v) = (f64::from(u), f64::from(v));
        let d = m[6] * u + m[7] * v + m[8];
        let x = (m[0] * u + m[1] * v + m[2]) / d;
        let y = (m[3] * u + m[4] * v + m[5]) / d;
        *p = bilinear(&src, x, y);
    }
    out
}

fn bilinear(img: &RgbImage, x: f64, y: f64) -> Rgb<u8> {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let x0 = (x.floor() as i64).clamp(0, w - 1);
    let y0 = (y.floor() as i64).clamp(0, h - 1);
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = (x - x0 as f64).clamp(0.0, 1.0);
    let fy = (y - y0 as f64).clamp(0.0, 1.0);
    let g = |xx: i64, yy: i64| img.get_pixel(xx as u32, yy as u32);
    let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
    let mut out = [0u8; 3];
    for i in 0..3 {
        let top = f64::from(a[i]) * (1.0 - fx) + f64::from(b[i]) * fx;
        let bot = f64::from(c[i]) * (1.0 - fx) + f64::from(d[i]) * fx;
        out[i] = (top * (1.0 - fy) + bot * fy).round() as u8;
    }
    Rgb(out)
}

/// A box blur of a luminance grid, with an integral image.
fn blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut sum = vec![0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0f64;
        for x in 0..w {
            row += f64::from(src[y * w + x]);
            sum[(y + 1) * (w + 1) + x + 1] = sum[y * (w + 1) + x + 1] + row;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let s = sum[y1 * (w + 1) + x1] - sum[y0 * (w + 1) + x1] - sum[y1 * (w + 1) + x0]
                + sum[y0 * (w + 1) + x0];
            out[y * w + x] = (s / ((y1 - y0) * (x1 - x0)) as f64) as f32;
        }
    }
    out
}

/// Evens out the lighting (shadows, a grey or yellow cast) and cleans the
/// page as chosen.
pub fn clean(page: &RgbImage, how: Clean) -> DynamicImage {
    if how == Clean::None {
        return DynamicImage::ImageRgb8(page.clone());
    }
    let (w, h) = (page.width() as usize, page.height() as usize);
    let lum: Vec<f32> = page.pixels().map(luma).collect();
    // The paper's brightness around each point: a wide blur of a "closed"
    // image (ink removed by taking the brightest nearby value first).
    let r_small = (w.max(h) / 150).max(2);
    let mut bright = lum.clone();
    for _ in 0..2 {
        // A cheap maximum filter: rows then columns.
        let mut tmp = bright.clone();
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (x.saturating_sub(r_small), (x + r_small).min(w - 1));
                tmp[y * w + x] = (a..=b).map(|i| bright[y * w + i]).fold(0.0, f32::max);
            }
        }
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (y.saturating_sub(r_small), (y + r_small).min(h - 1));
                bright[y * w + x] = (a..=b).map(|j| tmp[j * w + x]).fold(0.0, f32::max);
            }
        }
    }
    let paper = blur(&bright, w, h, (w.max(h) / 40).max(4));
    let level = |i: usize, v: f32| -> f32 {
        let bg = paper[i].max(24.0);
        // Paper becomes white; ink keeps its darkness relative to it.
        let t = (v / bg).min(1.0);
        // A little more contrast in the ink.
        (t.powf(1.6) * 255.0).clamp(0.0, 255.0)
    };
    match how {
        Clean::Grey | Clean::BlackWhite => {
            let mut out: GrayImage = ImageBuffer::new(w as u32, h as u32);
            for (i, p) in out.pixels_mut().enumerate() {
                let v = level(i, lum[i]);
                p[0] = if how == Clean::BlackWhite {
                    if v < 170.0 {
                        0
                    } else {
                        255
                    }
                } else {
                    v as u8
                };
            }
            DynamicImage::ImageLuma8(out)
        }
        _ => {
            let mut out = RgbImage::new(w as u32, h as u32);
            for (i, (p, src)) in out.pixels_mut().zip(page.pixels()).enumerate() {
                let bg = paper[i].max(24.0);
                let t = (lum[i] / bg).min(1.0);
                if t >= 0.92 {
                    *p = Rgb([255, 255, 255]);
                    continue;
                }
                // The same contrast as grey pages, keeping the ink's colour.
                let f = level(i, lum[i]) / (t * 255.0).max(1.0);
                for c in 0..3 {
                    p[c] = (f32::from(src[c]) * 255.0 / bg * f).clamp(0.0, 255.0) as u8;
                }
            }
            DynamicImage::ImageRgb8(out)
        }
    }
}

/// Turns a page by quarter turns (clockwise).
pub fn turn(img: DynamicImage, quarter_turns: i32) -> DynamicImage {
    match quarter_turns.rem_euclid(4) {
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => img,
    }
}

/// A page ready for a PDF: straightened, cleaned, turned, as JPEG.
pub fn process(
    photo: &DynamicImage,
    corners: [Corner; 4],
    how: Clean,
    quarter_turns: i32,
    quality: u8,
    max_side: Option<u32>,
) -> Result<Vec<u8>, String> {
    let flat = straighten(photo, corners);
    let flat = match max_side {
        Some(m) if flat.width().max(flat.height()) > m => {
            DynamicImage::ImageRgb8(flat).thumbnail(m, m).to_rgb8()
        }
        _ => flat,
    };
    let page = turn(clean(&flat, how), quarter_turns);
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    page.write_with_encoder(enc).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dark desk with a light, slightly turned sheet with some "ink".
    fn photo() -> DynamicImage {
        let (w, h) = (800u32, 600u32);
        let corners = [
            [200.0, 90.0],
            [620.0, 130.0],
            [590.0, 540.0],
            [160.0, 500.0],
        ];
        let inside = |x: f64, y: f64| {
            (0..4).all(|i| {
                let [ax, ay] = corners[i];
                let [bx, by] = corners[(i + 1) % 4];
                (bx - ax) * (y - ay) - (by - ay) * (x - ax) >= 0.0
            })
        };
        let img = RgbImage::from_fn(w, h, |x, y| {
            let (xf, yf) = (f64::from(x), f64::from(y));
            if inside(xf, yf) {
                // A shadow over the lower right, and a line of ink.
                let shade = 235.0 - (xf + yf) / 40.0;
                if (300..500).contains(&x) && (290..296).contains(&y) {
                    Rgb([30, 30, 60])
                } else {
                    Rgb([shade as u8, (shade - 5.0) as u8, (shade - 20.0) as u8])
                }
            } else {
                Rgb([60, 45, 40])
            }
        });
        DynamicImage::ImageRgb8(img)
    }

    /// `LIBRERI_LOOK=/some/dir cargo test -p libreri-scan paper::tests::look`
    /// writes a real photo's pages, to look at.
    #[test]
    fn look() {
        let (Some(dir), Some(src)) = (
            std::env::var_os("LIBRERI_LOOK"),
            std::env::var_os("LIBRERI_PHOTO"),
        ) else {
            return;
        };
        let p = image::open(src).unwrap();
        let c = detect(&p);
        println!("{c:?}");
        for (name, how) in [
            ("colour", Clean::Colour),
            ("grey", Clean::Grey),
            ("bw", Clean::BlackWhite),
        ] {
            let jpeg = process(&p, c, how, 0, 82, Some(1600)).unwrap();
            std::fs::write(std::path::Path::new(&dir).join(format!("{name}.jpg")), jpeg).unwrap();
        }
    }

    #[test]
    fn finds_the_page() {
        let c = detect(&photo());
        let want = [[0.25, 0.15], [0.775, 0.217], [0.738, 0.9], [0.2, 0.833]];
        for (got, want) in c.iter().zip(want) {
            assert!(
                (got[0] - want[0]).abs() < 0.03 && (got[1] - want[1]).abs() < 0.03,
                "{c:?}"
            );
        }
        // A plain photo: the whole of it.
        let plain = DynamicImage::ImageRgb8(RgbImage::from_pixel(100, 80, Rgb([200, 200, 200])));
        assert_eq!(
            detect(&plain),
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        );
    }

    #[test]
    fn straightens_and_cleans() {
        let p = photo();
        let flat = straighten(&p, detect(&p));
        // About as big as the sheet's edges.
        assert!((380..460).contains(&flat.width()), "{}", flat.width());
        assert!((380..460).contains(&flat.height()), "{}", flat.height());
        let bw = clean(&flat, Clean::BlackWhite).to_luma8();
        let (w, h) = bw.dimensions();
        // Paper (even in the shadow) is white; the ink line stays black.
        assert_eq!(bw.get_pixel(w / 10, h / 10)[0], 255);
        assert_eq!(bw.get_pixel(w * 8 / 10, h * 85 / 100)[0], 255);
        let dark = bw.pixels().filter(|p| p[0] == 0).count();
        assert!(dark > 200 && dark < (w * h / 10) as usize, "{dark}");
        let jpeg = process(&p, detect(&p), Clean::Colour, 1, 80, Some(600)).unwrap();
        let back = image::load_from_memory(&jpeg).unwrap();
        // Turned a quarter.
        assert!(back.width().abs_diff(flat.height()) < 30);
    }
}
