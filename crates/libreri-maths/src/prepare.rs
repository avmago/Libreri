//! Getting a picture ready for the model, as pix2tex does: greyscale, the
//! writing cropped out, sized to fit 672 × 192 and padded to multiples of
//! 32, dark writing on white, normalised.

use candle_core::{Device, Result, Tensor};
use image::{imageops::FilterType, DynamicImage, GrayImage, Luma};

const MAX_W: u32 = 672;
const MAX_H: u32 = 192;
const MEAN: f32 = 0.7931;
const STD: f32 = 0.1738;

/// Pixels of the writing (dark on light), as a mask.
fn ink(img: &GrayImage) -> Vec<bool> {
    let (lo, hi) = img
        .pixels()
        .fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let mid = (u16::from(lo) + u16::from(hi)) / 2;
    let mean = img.pixels().map(|p| f64::from(p[0])).sum::<f64>() / img.len().max(1) as f64;
    let dark = mean > f64::from(mid);
    img.pixels()
        .map(|p| (u16::from(p[0]) < mid) == dark)
        .collect()
}

/// The typical height of a mark in the picture (the median height of its
/// connected pieces), used to bring writing to the size the model knows.
pub fn mark_height(img: &DynamicImage) -> Option<f32> {
    let grey = img.to_luma8();
    let (w, h) = (grey.width() as usize, grey.height() as usize);
    let mut mask = ink(&grey);
    let mut heights = Vec::new();
    let mut stack = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] {
            continue;
        }
        mask[start] = false;
        stack.push(start);
        let (mut top, mut bottom, mut count) = (h, 0, 0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            top = top.min(y);
            bottom = bottom.max(y);
            count += 1;
            for (dx, dy) in [
                (-1i64, 0i64),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if mask[j] {
                    mask[j] = false;
                    stack.push(j);
                }
            }
        }
        if count >= 4 {
            heights.push((bottom - top + 1) as f32);
        }
    }
    if heights.is_empty() {
        return None;
    }
    heights.sort_by(f32::total_cmp);
    Some(heights[heights.len() / 2])
}

/// Crops to the writing and pads with white to multiples of 32. Writing
/// is made dark on light first.
fn pad(img: &GrayImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let (lo, hi) = img
        .pixels()
        .fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let span = f32::from(hi.saturating_sub(lo)).max(1.0);
    let mut data: Vec<u8> = img
        .pixels()
        .map(|p| ((f32::from(p[0] - lo) / span) * 255.0).round() as u8)
        .collect();
    let mean = data.iter().map(|&v| f64::from(v)).sum::<f64>() / data.len().max(1) as f64;
    let dark_on_light = mean > 128.0;
    if !dark_on_light {
        for v in &mut data {
            *v = 255 - *v;
        }
    }
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if data[(y * w + x) as usize] < 128 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 {
        (x0, y0, x1, y1) = (0, 0, w - 1, h - 1);
    }
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    let (pw, ph) = (cw.div_ceil(32) * 32, ch.div_ceil(32) * 32);
    let mut out = GrayImage::from_pixel(pw, ph, Luma([255]));
    for y in 0..ch {
        for x in 0..cw {
            out.put_pixel(x, y, Luma([data[((y + y0) * w + x + x0) as usize]]));
        }
    }
    out
}

/// Shrinks to fit the largest size the model reads, then pads to at least
/// 32 × 32.
fn fit(img: GrayImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let r = (w as f32 / MAX_W as f32).max(h as f32 / MAX_H as f32);
    let img = if r > 1.0 {
        let nw = ((w as f32 / r) as u32).max(1);
        let nh = ((h as f32 / r) as u32).max(1);
        image::imageops::resize(&img, nw, nh, FilterType::Triangle)
    } else {
        img
    };
    let (w, h) = img.dimensions();
    if w >= 32 && h >= 32 {
        return img;
    }
    let mut out = GrayImage::from_pixel(w.max(32), h.max(32), Luma([255]));
    image::imageops::overlay(&mut out, &img, 0, 0);
    out
}

/// A picture as the model's input, 1 × 1 × H × W.
pub fn prepare(img: &DynamicImage, scale: f32, device: &Device) -> Result<Tensor> {
    let grey = img.to_luma8();
    let grey = if (scale - 1.0).abs() > 0.01 {
        let (w, h) = grey.dimensions();
        image::imageops::resize(
            &grey,
            ((w as f32 * scale) as u32).max(1),
            ((h as f32 * scale) as u32).max(1),
            FilterType::Lanczos3,
        )
    } else {
        grey
    };
    let img = pad(&fit(pad(&grey)));
    let (w, h) = img.dimensions();
    let (w, h) = (w.min(MAX_W), h.min(MAX_H));
    let img = image::imageops::crop_imm(&img, 0, 0, w, h).to_image();
    let data: Vec<f32> = img
        .pixels()
        .map(|p| (f32::from(p[0]) / 255.0 - MEAN) / STD)
        .collect();
    Tensor::from_vec(data, (1, 1, h as usize, w as usize), device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_and_pads_to_32() {
        let mut img = GrayImage::from_pixel(300, 120, Luma([250]));
        for x in 50..130 {
            for y in 40..60 {
                img.put_pixel(x, y, Luma([10]));
            }
        }
        let t = prepare(&DynamicImage::ImageLuma8(img), 1.0, &Device::Cpu).unwrap();
        assert_eq!(t.dims(), &[1, 1, 32, 96]);
    }

    #[test]
    fn light_writing_on_dark_is_turned_round() {
        let mut img = GrayImage::from_pixel(64, 64, Luma([0]));
        img.put_pixel(10, 10, Luma([255]));
        let p = pad(&img);
        assert_eq!(p.get_pixel(0, 0)[0], 0);
        assert_eq!(p.get_pixel(5, 5)[0], 255);
    }
}
