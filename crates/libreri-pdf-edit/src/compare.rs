//! Comparing two documents: which pages belong together, which
//! words were removed, added or changed, and where drawings and pictures
//! look different.
//!
//! The caller reads each document's words (with boxes) and renders pages;
//! everything here works on those, so PDFs, DjVu scans and OCR text are
//! compared the same way. Boxes are fractions of the page as shown
//! (x, y, w, h, top-left origin).

use serde::{Deserialize, Serialize};
use similar::{Algorithm, DiffOp};
use std::time::{Duration, Instant};

/// A word and where it is on its page.
#[derive(Debug, Clone, PartialEq)]
pub struct CmpWord {
    pub text: String,
    pub rect: [f64; 4],
}

/// Pages that belong together (1-based); `None` on one side means the page
/// was added or removed.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PagePair {
    pub a: Option<u32>,
    pub b: Option<u32>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// Words only in the first document.
    Removed,
    /// Words only in the second.
    Added,
    /// Words replaced by others.
    Changed,
    /// Drawings or pictures look different.
    Look,
    PageRemoved,
    PageAdded,
}

/// One difference. `pair` is the index into the page pairs.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub kind: ChangeKind,
    pub pair: u32,
    pub a_rects: Vec<[f64; 4]>,
    pub b_rects: Vec<[f64; 4]>,
    pub a_text: String,
    pub b_text: String,
}

/// Words are compared without case and outer punctuation differences
/// mattering for the match; the shown text keeps them.
fn key(w: &str) -> String {
    let t = w.trim_matches(|c: char| !c.is_alphanumeric());
    if t.is_empty() { w } else { t }.to_lowercase()
}

/// Line boxes around words (neighbours on one line are joined).
pub fn line_rects(words: &[&CmpWord]) -> Vec<[f64; 4]> {
    let mut out: Vec<[f64; 4]> = Vec::new();
    for w in words {
        let [x, y, ww, h] = w.rect;
        if let Some(last) = out.last_mut() {
            let same_line = (last[1] - y).abs() < h.max(last[3]) * 0.5;
            let near = x <= last[0] + last[2] + h * 3.0 && x + ww >= last[0] - h;
            if same_line && near {
                let x1 = (last[0] + last[2]).max(x + ww);
                let y1 = (last[1] + last[3]).max(y + h);
                last[0] = last[0].min(x);
                last[1] = last[1].min(y);
                last[2] = x1 - last[0];
                last[3] = y1 - last[1];
                continue;
            }
        }
        out.push(w.rect);
    }
    out
}

fn join(words: &[&CmpWord]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pairs pages up: pages sharing the most unchanged words, in order;
/// between two such pairs, left-over pages pair in order and the rest were
/// added or removed.
pub fn pair_pages(a_pages: usize, b_pages: usize, votes: &[(u32, u32)]) -> Vec<PagePair> {
    use std::collections::HashMap;
    let mut score: HashMap<(u32, u32), u32> = HashMap::new();
    for &(a, b) in votes {
        *score.entry((a, b)).or_default() += 1;
    }
    // Best chain of pairs, increasing on both sides (longest weighted path).
    let mut cells: Vec<((u32, u32), u32)> = score.into_iter().filter(|(_, v)| *v >= 2).collect();
    cells.sort();
    let n = cells.len();
    let mut best = vec![0u64; n];
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        best[i] = u64::from(cells[i].1);
        for j in 0..i {
            if cells[j].0 .0 < cells[i].0 .0
                && cells[j].0 .1 < cells[i].0 .1
                && best[j] + u64::from(cells[i].1) > best[i]
            {
                best[i] = best[j] + u64::from(cells[i].1);
                prev[i] = j;
            }
        }
    }
    let mut chain = Vec::new();
    if let Some(mut i) = (0..n).max_by_key(|&i| best[i]) {
        loop {
            chain.push(cells[i].0);
            if prev[i] == usize::MAX {
                break;
            }
            i = prev[i];
        }
    }
    chain.reverse();
    // Fill the gaps.
    let mut out = Vec::new();
    let (mut ai, mut bi) = (1u32, 1u32);
    let ends = chain
        .iter()
        .copied()
        .chain(std::iter::once((a_pages as u32 + 1, b_pages as u32 + 1)));
    for (ta, tb) in ends {
        while ai < ta && bi < tb {
            out.push(PagePair {
                a: Some(ai),
                b: Some(bi),
            });
            ai += 1;
            bi += 1;
        }
        while ai < ta {
            out.push(PagePair {
                a: Some(ai),
                b: None,
            });
            ai += 1;
        }
        while bi < tb {
            out.push(PagePair {
                a: None,
                b: Some(bi),
            });
            bi += 1;
        }
        if ta as usize <= a_pages && tb as usize <= b_pages {
            out.push(PagePair {
                a: Some(ta),
                b: Some(tb),
            });
            ai = ta + 1;
            bi = tb + 1;
        }
    }
    out
}

/// Compares the words of two documents (one list per page). Returns the
/// page pairs and the word changes, in reading order.
fn word(doc: &[Vec<CmpWord>], (p, i): (u32, usize)) -> &CmpWord {
    &doc[p as usize - 1][i]
}

pub fn compare_words(a: &[Vec<CmpWord>], b: &[Vec<CmpWord>]) -> (Vec<PagePair>, Vec<Change>) {
    let flat = |doc: &[Vec<CmpWord>]| -> Vec<(u32, usize)> {
        doc.iter()
            .enumerate()
            .flat_map(|(p, ws)| (0..ws.len()).map(move |i| (p as u32 + 1, i)))
            .collect()
    };
    let (fa, fb) = (flat(a), flat(b));
    let ka: Vec<String> = fa.iter().map(|&at| key(&word(a, at).text)).collect();
    let kb: Vec<String> = fb.iter().map(|&at| key(&word(b, at).text)).collect();
    let deadline = Instant::now() + Duration::from_secs(20);
    let ops = similar::capture_diff_slices_deadline(Algorithm::Patience, &ka, &kb, Some(deadline));

    let mut votes = Vec::new();
    for op in &ops {
        if let DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = *op
        {
            for k in 0..len {
                votes.push((fa[old_index + k].0, fb[new_index + k].0));
            }
        }
    }
    let pairs = pair_pages(a.len(), b.len(), &votes);
    // Then each pair of pages on its own, so words never match across
    // pages that do not belong together.
    let mut changes = Vec::new();
    for (pi, pp) in pairs.iter().enumerate() {
        let (Some(pa), Some(pb)) = (pp.a, pp.b) else {
            continue;
        };
        let wa = &a[pa as usize - 1];
        let wb = &b[pb as usize - 1];
        let ka: Vec<String> = wa.iter().map(|w| key(&w.text)).collect();
        let kb: Vec<String> = wb.iter().map(|w| key(&w.text)).collect();
        let deadline = Instant::now() + Duration::from_secs(2);
        let ops = similar::capture_diff_slices_deadline(Algorithm::Myers, &ka, &kb, Some(deadline));
        for op in ops {
            let (ar, br) = match op {
                DiffOp::Equal { .. } => continue,
                DiffOp::Delete {
                    old_index, old_len, ..
                } => (old_index..old_index + old_len, 0..0),
                DiffOp::Insert {
                    new_index, new_len, ..
                } => (0..0, new_index..new_index + new_len),
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => (
                    old_index..old_index + old_len,
                    new_index..new_index + new_len,
                ),
            };
            let on_a: Vec<&CmpWord> = wa[ar].iter().collect();
            let on_b: Vec<&CmpWord> = wb[br].iter().collect();
            let kind = match (on_a.is_empty(), on_b.is_empty()) {
                (false, true) => ChangeKind::Removed,
                (true, false) => ChangeKind::Added,
                _ => ChangeKind::Changed,
            };
            // Only case or punctuation changed: not worth listing.
            if kind == ChangeKind::Changed
                && join(&on_a).to_lowercase() == join(&on_b).to_lowercase()
            {
                continue;
            }
            changes.push(Change {
                kind,
                pair: pi as u32,
                a_rects: line_rects(&on_a),
                b_rects: line_rects(&on_b),
                a_text: join(&on_a),
                b_text: join(&on_b),
            });
        }
    }
    for (i, pp) in pairs.iter().enumerate() {
        let (kind, text) = match (pp.a, pp.b) {
            (Some(p), None) => (
                ChangeKind::PageRemoved,
                join(&a[p as usize - 1].iter().collect::<Vec<_>>()),
            ),
            (None, Some(p)) => (
                ChangeKind::PageAdded,
                join(&b[p as usize - 1].iter().collect::<Vec<_>>()),
            ),
            _ => continue,
        };
        let short: String = text.chars().take(160).collect();
        changes.push(Change {
            kind,
            pair: i as u32,
            a_rects: Vec::new(),
            b_rects: Vec::new(),
            a_text: if kind == ChangeKind::PageRemoved {
                short.clone()
            } else {
                String::new()
            },
            b_text: if kind == ChangeKind::PageAdded {
                short
            } else {
                String::new()
            },
        });
    }
    changes.sort_by_key(|c| c.pair);
    (pairs, changes)
}

/// A page as grey pixels.
pub struct Grey<'a> {
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
}

/// Where two renderings of a page look different, outside `ignore` (the
/// boxes of words, which are compared as text). `a` and `b` are sampled
/// to the same grid, so they may differ in size.
pub fn look_changes(a: &Grey, b: &Grey, ignore: &[[f64; 4]]) -> Vec<[f64; 4]> {
    const CELL: u32 = 8;
    let (gw, gh) = (
        160u32,
        (160.0 * f64::from(a.height) / f64::from(a.width.max(1))).round() as u32,
    );
    let gh = gh.clamp(8, 400);
    let (cw, ch) = (f64::from(CELL), f64::from(CELL));
    let (cols, rows) = (gw.div_ceil(CELL), gh.div_ceil(CELL));
    let sample = |g: &Grey, fx: f64, fy: f64| -> i32 {
        let x = ((fx * f64::from(g.width)) as u32).min(g.width.saturating_sub(1));
        let y = ((fy * f64::from(g.height)) as u32).min(g.height.saturating_sub(1));
        i32::from(g.pixels[(y * g.width + x) as usize])
    };
    // Words, widened a little, are left to the text comparison.
    let pad = 0.006;
    let ignored = |fx: f64, fy: f64| {
        ignore.iter().any(|[x, y, w, h]| {
            fx >= x - pad && fx <= x + w + pad && fy >= y - pad && fy <= y + h + pad
        })
    };
    let mut hot = vec![false; (cols * rows) as usize];
    for r in 0..rows {
        for c in 0..cols {
            let mut differ = 0;
            let mut n = 0;
            for dy in 0..CELL {
                for dx in 0..CELL {
                    let (x, y) = (c * CELL + dx, r * CELL + dy);
                    if x >= gw || y >= gh {
                        continue;
                    }
                    let (fx, fy) = (
                        (f64::from(x) + 0.5) / f64::from(gw),
                        (f64::from(y) + 0.5) / f64::from(gh),
                    );
                    if ignored(fx, fy) {
                        continue;
                    }
                    n += 1;
                    if (sample(a, fx, fy) - sample(b, fx, fy)).abs() > 60 {
                        differ += 1;
                    }
                }
            }
            hot[(r * cols + c) as usize] = n > 0 && differ * 100 >= n * 12;
        }
    }
    // Neighbouring cells make one region.
    let mut seen = vec![false; hot.len()];
    let mut out = Vec::new();
    for start in 0..hot.len() {
        if !hot[start] || seen[start] {
            continue;
        }
        let (mut c0, mut r0, mut c1, mut r1) = (u32::MAX, u32::MAX, 0, 0);
        let mut stack = vec![start];
        seen[start] = true;
        let mut count = 0;
        while let Some(i) = stack.pop() {
            count += 1;
            let (c, r) = (i as u32 % cols, i as u32 / cols);
            (c0, r0, c1, r1) = (c0.min(c), r0.min(r), c1.max(c), r1.max(r));
            for (dc, dr) in [
                (-1i32, 0i32),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ] {
                let (nc, nr) = (c as i32 + dc, r as i32 + dr);
                if nc < 0 || nr < 0 || nc >= cols as i32 || nr >= rows as i32 {
                    continue;
                }
                let j = (nr as u32 * cols + nc as u32) as usize;
                if hot[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        // A single stray cell is noise (anti-aliasing, a speck in a scan).
        if count < 2 {
            continue;
        }
        let x = f64::from(c0) * cw / f64::from(gw);
        let y = f64::from(r0) * ch / f64::from(gh);
        let w = (f64::from(c1 + 1) * cw / f64::from(gw)).min(1.0) - x;
        let h = (f64::from(r1 + 1) * ch / f64::from(gh)).min(1.0) - y;
        out.push([x, y, w, h]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str) -> Vec<CmpWord> {
        text.split_whitespace()
            .enumerate()
            .map(|(i, t)| CmpWord {
                text: t.into(),
                rect: [
                    0.1 + (i % 8) as f64 * 0.1,
                    0.1 + (i / 8) as f64 * 0.05,
                    0.08,
                    0.02,
                ],
            })
            .collect()
    }

    #[test]
    fn finds_changed_words() {
        let a = vec![
            page("the keeper Samuel Harte wrote in the log at dusk"),
            page("storms of the year"),
        ];
        let b = vec![
            page("the keeper Ada Harte wrote in the big log at dusk"),
            page("storms of the year"),
        ];
        let (pairs, changes) = compare_words(&a, &b);
        assert_eq!(
            pairs,
            [
                PagePair {
                    a: Some(1),
                    b: Some(1)
                },
                PagePair {
                    a: Some(2),
                    b: Some(2)
                }
            ]
        );
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(
            (
                changes[0].kind,
                changes[0].a_text.as_str(),
                changes[0].b_text.as_str()
            ),
            (ChangeKind::Changed, "Samuel", "Ada")
        );
        assert_eq!(
            (changes[1].kind, changes[1].b_text.as_str()),
            (ChangeKind::Added, "big")
        );
        assert!(changes[1].a_rects.is_empty() && changes[1].b_rects.len() == 1);
    }

    #[test]
    fn pairs_pages_around_added_and_removed_ones() {
        let a = vec![
            page("one alpha beta gamma delta"),
            page("two epsilon zeta eta theta"),
            page("three iota kappa lambda mu"),
        ];
        let b = vec![
            page("one alpha beta gamma delta"),
            page("new page nothing like the others at all"),
            page("three iota kappa lambda mu"),
            page("four nu xi omicron pi"),
        ];
        let (pairs, changes) = compare_words(&a, &b);
        // Page two was replaced: an unmatched page on each side pairs up.
        assert_eq!(
            pairs,
            [
                PagePair {
                    a: Some(1),
                    b: Some(1)
                },
                PagePair {
                    a: Some(2),
                    b: Some(2)
                },
                PagePair {
                    a: Some(3),
                    b: Some(3)
                },
                PagePair {
                    a: None,
                    b: Some(4)
                },
            ]
        );
        assert!(changes
            .iter()
            .any(|c| c.kind == ChangeKind::PageAdded && c.pair == 3));
        assert!(changes.iter().all(|c| c.pair != 0 && c.pair != 2));
    }

    #[test]
    fn a_removed_page_is_one_change() {
        let a = vec![
            page("intro words here and more"),
            page("gone page with its own words"),
            page("end words stay here too"),
        ];
        let b = vec![
            page("intro words here and more"),
            page("end words stay here too"),
        ];
        let (pairs, changes) = compare_words(&a, &b);
        assert_eq!(
            pairs[1],
            PagePair {
                a: Some(2),
                b: None
            }
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::PageRemoved);
        assert!(changes[0].a_text.starts_with("gone page"));
    }

    #[test]
    fn finds_changed_drawings_outside_text() {
        let (w, h) = (200u32, 280u32);
        let blank = vec![255u8; (w * h) as usize];
        let mut drawn = blank.clone();
        // A dark box in the lower right, and a changed "word" in the top left.
        for y in 200..250 {
            for x in 120..180 {
                drawn[(y * w + x) as usize] = 0;
            }
        }
        for y in 20..30 {
            for x in 20..60 {
                drawn[(y * w + x) as usize] = 0;
            }
        }
        let a = Grey {
            pixels: &blank,
            width: w,
            height: h,
        };
        let b = Grey {
            pixels: &drawn,
            width: w,
            height: h,
        };
        let word = [[20.0 / 200.0, 20.0 / 280.0, 40.0 / 200.0, 10.0 / 280.0]];
        let regions = look_changes(&a, &b, &word);
        assert_eq!(regions.len(), 1, "{regions:?}");
        let [x, y, rw, rh] = regions[0];
        assert!(
            x <= 0.6 && x + rw >= 0.9 && y <= 0.72 && y + rh >= 0.89,
            "{regions:?}"
        );
        assert!(look_changes(&a, &a, &[]).is_empty());
    }
}
