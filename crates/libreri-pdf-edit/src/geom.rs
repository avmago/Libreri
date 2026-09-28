//! Page geometry: the visible page (crop box and rotation) and mapping
//! between PDF user space and fractions of the page as shown.

use lopdf::{Document, Object, ObjectId};

/// An affine matrix `[a b c d e f]`, PDF convention (row vectors).
pub type Mat = [f64; 6];

pub const IDENTITY: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a` then `b` (PDF order: `a × b`).
pub fn mul(a: &Mat, b: &Mat) -> Mat {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

pub fn apply(m: &Mat, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

pub fn invert(m: &Mat) -> Option<Mat> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 {
        return None;
    }
    let a = m[3] / det;
    let b = -m[1] / det;
    let c = -m[2] / det;
    let d = m[0] / det;
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

pub fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference(r) => doc.get_object(*r).unwrap_or(o),
        _ => o,
    }
}

pub fn num(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

/// A page attribute, looking up the page tree for inherited ones.
pub fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page;
    for _ in 0..32 {
        let d = doc.get_dictionary(id).ok()?;
        if let Ok(v) = d.get(key) {
            return Some(resolve(doc, v).clone());
        }
        id = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

pub fn rect(doc: &Document, o: &Object) -> Option<[f64; 4]> {
    let a = resolve(doc, o).as_array().ok()?;
    let v: Vec<f64> = a.iter().filter_map(|x| num(resolve(doc, x))).collect();
    (v.len() == 4).then(|| {
        [
            v[0].min(v[2]),
            v[1].min(v[3]),
            v[0].max(v[2]),
            v[1].max(v[3]),
        ]
    })
}

/// The visible page: crop box within the media box, and rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub rotate: i64,
}

impl Geometry {
    pub fn of(doc: &Document, page: ObjectId) -> Self {
        let media = inherited(doc, page, b"MediaBox")
            .and_then(|o| rect(doc, &o))
            .unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let crop = inherited(doc, page, b"CropBox")
            .and_then(|o| rect(doc, &o))
            .map(|c| {
                [
                    c[0].max(media[0]),
                    c[1].max(media[1]),
                    c[2].min(media[2]),
                    c[3].min(media[3]),
                ]
            })
            .filter(|c| c[2] > c[0] && c[3] > c[1])
            .unwrap_or(media);
        let rotate = inherited(doc, page, b"Rotate")
            .and_then(|o| num(&o))
            .map_or(0, |r| (r as i64).rem_euclid(360) / 90 * 90);
        Geometry {
            x0: crop[0],
            y0: crop[1],
            x1: crop[2],
            y1: crop[3],
            rotate,
        }
    }

    /// Width and height as shown.
    pub fn shown(&self) -> (f64, f64) {
        let (w, h) = (self.x1 - self.x0, self.y1 - self.y0);
        if self.rotate % 180 == 0 {
            (w, h)
        } else {
            (h, w)
        }
    }

    /// Maps shown points (y down, from the top left) to PDF user space.
    pub fn matrix(&self) -> Mat {
        let w = self.x1 - self.x0;
        match self.rotate {
            90 => [0.0, 1.0, 1.0, 0.0, self.x0, self.y0],
            180 => [-1.0, 0.0, 0.0, 1.0, self.x0 + w, self.y0],
            270 => [0.0, -1.0, -1.0, 0.0, self.x0 + w, self.y1],
            _ => [1.0, 0.0, 0.0, -1.0, self.x0, self.y1],
        }
    }

    /// A point in user space as fractions of the shown page.
    pub fn fraction_of(&self, x: f64, y: f64) -> (f64, f64) {
        let inv = invert(&self.matrix()).unwrap_or(IDENTITY);
        let (sx, sy) = apply(&inv, x, y);
        let (w, h) = self.shown();
        (sx / w, sy / h)
    }

    /// A point given as fractions of the shown page, in user space.
    pub fn point_at(&self, u: f64, v: f64) -> (f64, f64) {
        let (w, h) = self.shown();
        apply(&self.matrix(), u * w, v * h)
    }
}

/// An axis-aligned box `[x0, y0, x1, y1]` in page fractions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FBox {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl FBox {
    /// From `[x, y, w, h]`.
    pub fn from_xywh(b: [f64; 4]) -> Self {
        Self {
            x0: b[0],
            y0: b[1],
            x1: b[0] + b[2],
            y1: b[1] + b[3],
        }
    }

    pub fn around(points: &[(f64, f64)]) -> Self {
        let mut b = Self {
            x0: f64::INFINITY,
            y0: f64::INFINITY,
            x1: f64::NEG_INFINITY,
            y1: f64::NEG_INFINITY,
        };
        for &(x, y) in points {
            b.x0 = b.x0.min(x);
            b.y0 = b.y0.min(y);
            b.x1 = b.x1.max(x);
            b.y1 = b.y1.max(y);
        }
        b
    }

    pub fn area(&self) -> f64 {
        (self.x1 - self.x0).max(0.0) * (self.y1 - self.y0).max(0.0)
    }

    pub fn overlap(&self, o: &FBox) -> f64 {
        let w = self.x1.min(o.x1) - self.x0.max(o.x0);
        let h = self.y1.min(o.y1) - self.y0.max(o.y0);
        if w <= 0.0 || h <= 0.0 {
            0.0
        } else {
            w * h
        }
    }

    pub fn intersects(&self, o: &FBox) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }

    pub fn inside(&self, o: &FBox) -> bool {
        self.x0 >= o.x0 - 1e-6
            && self.y0 >= o.y0 - 1e-6
            && self.x1 <= o.x1 + 1e-6
            && self.y1 <= o.y1 + 1e-6
    }

    pub fn center(&self) -> (f64, f64) {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }

    pub fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices_invert_and_compose() {
        let m = [2.0, 0.0, 0.0, 3.0, 10.0, 20.0];
        let inv = invert(&m).unwrap();
        assert_eq!(apply(&inv, 12.0, 23.0), (1.0, 1.0));
        let t = mul(&[1.0, 0.0, 0.0, 1.0, 5.0, 0.0], &m);
        assert_eq!(apply(&t, 0.0, 0.0), (20.0, 20.0));
    }

    #[test]
    fn fractions_round_trip_for_every_rotation() {
        for rotate in [0, 90, 180, 270] {
            let g = Geometry {
                x0: 10.0,
                y0: 20.0,
                x1: 622.0,
                y1: 812.0,
                rotate,
            };
            let (x, y) = g.point_at(0.25, 0.75);
            let (u, v) = g.fraction_of(x, y);
            assert!(
                (u - 0.25).abs() < 1e-9 && (v - 0.75).abs() < 1e-9,
                "{rotate}"
            );
        }
        // Top left as shown, unrotated, is the crop box's top left.
        let g = Geometry {
            x0: 0.0,
            y0: 0.0,
            x1: 612.0,
            y1: 792.0,
            rotate: 0,
        };
        assert_eq!(g.point_at(0.0, 0.0), (0.0, 792.0));
    }
}
