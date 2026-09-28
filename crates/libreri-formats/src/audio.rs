//! Audiobooks (MP3, M4B, M4A, AAC, OGG, Opus, FLAC): details, cover,
//! length and chapters.
//!
//! Tags and length come from lofty (MIT/Apache-2.0). Chapters are read
//! here: ID3v2 `CHAP` frames (MP3), Nero `chpl` and QuickTime chapter
//! tracks (M4B/M4A), and `CHAPTERnnn` comments (Ogg, Opus, FLAC).

use crate::Extracted;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey, Tag};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// A chapter and where it starts, in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub title: String,
    pub start: f64,
}

/// Length and chapters of an audio file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    /// Seconds, when known.
    pub duration: Option<f64>,
    pub chapters: Vec<Chapter>,
}

fn tag_of(path: &Path) -> Result<(lofty::file::TaggedFile, Option<Tag>), String> {
    let file = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let tag = file.primary_tag().or_else(|| file.first_tag()).cloned();
    Ok((file, tag))
}

/// Details and cover for the import.
pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let (_, tag) = tag_of(path)?;
    let Some(tag) = tag else { return Ok(()) };
    let m = &mut out.metadata;
    // Audiobooks often put the book in the album and the chapter in the title.
    let album = tag
        .album()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let title = tag
        .title()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    if let Some(t) = album.or(title) {
        m.title = t;
    }
    let artist = tag
        .get_string(ItemKey::AlbumArtist)
        .or_else(|| tag.get_string(ItemKey::TrackArtist))
        .map(str::to_owned);
    if let Some(a) = artist {
        m.authors = a
            .split([';', '/'])
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Some(y) = tag
        .get_string(ItemKey::Year)
        .or_else(|| tag.get_string(ItemKey::RecordingDate))
    {
        m.year = y.get(..4).and_then(|y| y.parse().ok());
    }
    if let Some(p) = tag
        .get_string(ItemKey::Publisher)
        .or_else(|| tag.get_string(ItemKey::Label))
    {
        m.publisher = Some(p.to_owned());
    }
    if let Some(c) = tag.comment() {
        let c = c.trim();
        if c.len() > 40 {
            m.about = Some(c.to_owned());
        }
    }
    if let Some(pic) = tag.pictures().first() {
        out.cover = Some(pic.data().to_vec());
    }
    Ok(())
}

/// Length and chapters.
pub fn info(path: &Path) -> Result<AudioInfo, String> {
    let (file, _) = tag_of(path)?;
    let duration = Some(file.properties().duration().as_secs_f64()).filter(|d| *d > 0.0);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut chapters = match ext.as_str() {
        "mp3" => id3_chapters(path).unwrap_or_default(),
        "m4b" | "m4a" | "aac" | "mp4" => mp4_chapters(path).unwrap_or_default(),
        _ => Vec::new(),
    };
    if chapters.is_empty() {
        chapters = vorbis_chapters(path, &ext).unwrap_or_default();
    }
    chapters.sort_by(|a, b| a.start.total_cmp(&b.start));
    chapters.dedup_by(|b, a| (a.start - b.start).abs() < 0.01);
    Ok(AudioInfo { duration, chapters })
}

// ---------- Ogg / Opus / FLAC ----------

fn vorbis_chapters(path: &Path, ext: &str) -> Result<Vec<Chapter>, String> {
    use lofty::config::ParseOptions;
    use lofty::flac::FlacFile;
    use lofty::ogg::{OpusFile, VorbisFile};
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let opts = ParseOptions::new().read_properties(false);
    let items: Vec<(String, String)> = {
        let collect = |c: Option<&lofty::ogg::tag::VorbisComments>| {
            c.map(|c| {
                c.items()
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
        };
        match ext {
            "flac" => collect(
                FlacFile::read_from(&mut f, opts)
                    .map_err(|e| e.to_string())?
                    .vorbis_comments(),
            ),
            "opus" => collect(Some(
                OpusFile::read_from(&mut f, opts)
                    .map_err(|e| e.to_string())?
                    .vorbis_comments(),
            )),
            "ogg" | "oga" => collect(Some(
                VorbisFile::read_from(&mut f, opts)
                    .map_err(|e| e.to_string())?
                    .vorbis_comments(),
            )),
            _ => Vec::new(),
        }
    };
    Ok(comment_chapters(
        items.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    ))
}

/// `CHAPTER001=00:01:02.500`, `CHAPTER001NAME=Title`.
fn comment_chapters<'a>(items: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<Chapter> {
    let mut starts: Vec<(String, f64)> = Vec::new();
    let mut names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (key, value) in items {
        let key = key.to_ascii_uppercase();
        let Some(rest) = key.strip_prefix("CHAPTER") else {
            continue;
        };
        if let Some(n) = rest.strip_suffix("NAME") {
            names.insert(n.to_owned(), value.to_owned());
        } else if rest.chars().all(|c| c.is_ascii_digit()) {
            if let Some(t) = parse_clock(value) {
                starts.push((rest.to_owned(), t));
            }
        }
    }
    starts
        .into_iter()
        .map(|(n, start)| Chapter {
            title: names
                .get(&n)
                .cloned()
                .unwrap_or_else(|| format!("Chapter {}", n.trim_start_matches('0'))),
            start,
        })
        .collect()
}

/// "01:02:03.500" or "02:03.5" → seconds.
fn parse_clock(s: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in s.trim().split(':') {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

// ---------- MP3 (ID3v2) ----------

fn synchsafe(b: &[u8]) -> usize {
    b.iter()
        .fold(0usize, |acc, &x| (acc << 7) | usize::from(x & 0x7f))
}

fn be32(b: &[u8]) -> usize {
    b.iter()
        .take(4)
        .fold(0usize, |acc, &x| (acc << 8) | usize::from(x))
}

/// Text of an ID3 text frame body (encoding byte, then text).
fn id3_text(body: &[u8]) -> String {
    let Some((&enc, text)) = body.split_first() else {
        return String::new();
    };
    let s = match enc {
        1 | 2 => {
            let (units, be) = match text {
                [0xFF, 0xFE, rest @ ..] => (rest, false),
                [0xFE, 0xFF, rest @ ..] => (rest, true),
                rest => (rest, enc == 2),
            };
            let u: Vec<u16> = units
                .chunks_exact(2)
                .map(|c| {
                    if be {
                        u16::from_be_bytes([c[0], c[1]])
                    } else {
                        u16::from_le_bytes([c[0], c[1]])
                    }
                })
                .collect();
            String::from_utf16_lossy(&u)
        }
        3 => String::from_utf8_lossy(text).into_owned(),
        _ => text.iter().map(|&b| char::from(b)).collect(),
    };
    s.trim_matches(char::from(0)).trim().to_owned()
}

/// Frames in an ID3v2 frame area: (id, body).
fn id3_frames(data: &[u8], version: u8) -> Vec<(&[u8], &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 10 <= data.len() {
        let id = &data[i..i + 4];
        if id[0] == 0 {
            break;
        }
        let size = if version >= 4 {
            synchsafe(&data[i + 4..i + 8])
        } else {
            be32(&data[i + 4..i + 8])
        };
        let start = i + 10;
        let end = start.saturating_add(size).min(data.len());
        out.push((id, &data[start..end]));
        i = end;
    }
    out
}

fn id3_chapters(path: &Path) -> Result<Vec<Chapter>, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut head = [0u8; 10];
    f.read_exact(&mut head).map_err(|e| e.to_string())?;
    if &head[..3] != b"ID3" {
        return Ok(Vec::new());
    }
    let version = head[3];
    let size = synchsafe(&head[6..10]).min(64 * 1024 * 1024);
    let mut tag = vec![0u8; size];
    f.read_exact(&mut tag).map_err(|e| e.to_string())?;
    let mut data: &[u8] = &tag;
    if head[5] & 0x40 != 0 && data.len() >= 4 {
        // Extended header.
        let ext = if version >= 4 {
            synchsafe(&data[..4])
        } else {
            be32(&data[..4]) + 4
        };
        data = data.get(ext..).unwrap_or(&[]);
    }
    let mut out = Vec::new();
    for (id, body) in id3_frames(data, version) {
        if id != b"CHAP" {
            continue;
        }
        let Some(zero) = body.iter().position(|&b| b == 0) else {
            continue;
        };
        let rest = &body[zero + 1..];
        if rest.len() < 16 {
            continue;
        }
        let start_ms = be32(&rest[..4]);
        let title = id3_frames(&rest[16..], version)
            .into_iter()
            .find(|(id, _)| *id == b"TIT2")
            .map(|(_, b)| id3_text(b))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("Chapter {}", out.len() + 1));
        out.push(Chapter {
            title,
            start: start_ms as f64 / 1000.0,
        });
    }
    Ok(out)
}

// ---------- MP4 / M4B ----------

struct Atom {
    kind: [u8; 4],
    /// Where the body starts and ends in the file.
    start: u64,
    end: u64,
}

fn atoms(f: &mut File, from: u64, to: u64) -> Vec<Atom> {
    let mut out = Vec::new();
    let mut pos = from;
    while pos + 8 <= to {
        let mut h = [0u8; 8];
        if f.seek(SeekFrom::Start(pos)).is_err() || f.read_exact(&mut h).is_err() {
            break;
        }
        let mut size = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let kind = [h[4], h[5], h[6], h[7]];
        let mut body = pos + 8;
        if size == 1 {
            let mut big = [0u8; 8];
            if f.read_exact(&mut big).is_err() {
                break;
            }
            size = u64::from_be_bytes(big);
            body += 8;
        } else if size == 0 {
            size = to - pos;
        }
        if size < 8 || pos + size > to {
            break;
        }
        out.push(Atom {
            kind,
            start: body,
            end: pos + size,
        });
        pos += size;
    }
    out
}

fn child(f: &mut File, parent: &Atom, kind: &[u8; 4]) -> Option<Atom> {
    atoms(f, parent.start, parent.end)
        .into_iter()
        .find(|a| &a.kind == kind)
}

fn body(f: &mut File, a: &Atom, max: u64) -> Option<Vec<u8>> {
    let len = (a.end - a.start).min(max);
    let mut buf = vec![0u8; len as usize];
    f.seek(SeekFrom::Start(a.start)).ok()?;
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], i: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(i..i + 8)?.try_into().ok()?))
}

fn mp4_chapters(path: &Path) -> Result<Vec<Chapter>, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let Some(moov) = atoms(&mut f, 0, len)
        .into_iter()
        .find(|a| &a.kind == b"moov")
    else {
        return Ok(Vec::new());
    };
    // QuickTime chapter track (iTunes and most audiobook tools).
    if let Some(list) = quicktime_chapters(&mut f, &moov) {
        if !list.is_empty() {
            return Ok(list);
        }
    }
    // Nero chapters.
    let Some(udta) = child(&mut f, &moov, b"udta") else {
        return Ok(Vec::new());
    };
    let Some(chpl) = child(&mut f, &udta, b"chpl") else {
        return Ok(Vec::new());
    };
    let b = body(&mut f, &chpl, 4 * 1024 * 1024).unwrap_or_default();
    let mut i = 4 + usize::from(b.first().copied().unwrap_or(0) != 0) * 4;
    let count = usize::from(*b.get(i).unwrap_or(&0));
    i += 1;
    let mut out = Vec::new();
    for _ in 0..count {
        let (Some(start), Some(&n)) = (u64_at(&b, i), b.get(i + 8)) else {
            break;
        };
        let n = usize::from(n);
        let title = String::from_utf8_lossy(b.get(i + 9..i + 9 + n).unwrap_or(&[]))
            .trim()
            .to_owned();
        out.push(Chapter {
            title,
            start: start as f64 / 10_000_000.0,
        });
        i += 9 + n;
    }
    Ok(out)
}

fn quicktime_chapters(f: &mut File, moov: &Atom) -> Option<Vec<Chapter>> {
    let traks: Vec<Atom> = atoms(f, moov.start, moov.end)
        .into_iter()
        .filter(|a| &a.kind == b"trak")
        .collect();
    let track_id = |f: &mut File, t: &Atom| -> Option<u32> {
        let tkhd = child(f, t, b"tkhd")?;
        let b = body(f, &tkhd, 64)?;
        u32_at(&b, if b[0] == 1 { 20 } else { 12 })
    };
    // Which track holds the chapters.
    let mut chapter_id = None;
    for t in &traks {
        if let Some(tref) = child(f, t, b"tref") {
            if let Some(chap) = child(f, &tref, b"chap") {
                chapter_id = body(f, &chap, 16).and_then(|b| u32_at(&b, 0));
            }
        }
    }
    let chapter_id = chapter_id?;
    let mut trak = None;
    for t in traks {
        if track_id(f, &t) == Some(chapter_id) {
            trak = Some(t);
            break;
        }
    }
    let trak = trak?;
    let mdia = child(f, &trak, b"mdia")?;
    let mdhd_atom = child(f, &mdia, b"mdhd")?;
    let mdhd = body(f, &mdhd_atom, 64)?;
    let timescale = f64::from(u32_at(&mdhd, if mdhd[0] == 1 { 20 } else { 12 })?.max(1));
    let minf = child(f, &mdia, b"minf")?;
    let stbl = child(f, &minf, b"stbl")?;
    let table = |f: &mut File, kind: &[u8; 4]| {
        child(f, &stbl, kind).and_then(|a| body(f, &a, 16 * 1024 * 1024))
    };
    // Sample times.
    let stts = table(f, b"stts")?;
    let mut starts = Vec::new();
    let mut t = 0u64;
    for e in 0..u32_at(&stts, 4)? as usize {
        let (Some(count), Some(delta)) = (u32_at(&stts, 8 + e * 8), u32_at(&stts, 12 + e * 8))
        else {
            break;
        };
        for _ in 0..count.min(100_000) {
            starts.push(t);
            t += u64::from(delta);
        }
    }
    // Sample sizes and where they are.
    let stsz = table(f, b"stsz")?;
    let fixed = u32_at(&stsz, 4)?;
    let n = u32_at(&stsz, 8)? as usize;
    let sizes: Vec<u32> = (0..n)
        .map(|i| {
            if fixed != 0 {
                fixed
            } else {
                u32_at(&stsz, 12 + i * 4).unwrap_or(0)
            }
        })
        .collect();
    let offsets: Vec<u64> = if let Some(stco) = table(f, b"stco") {
        (0..u32_at(&stco, 4)? as usize)
            .filter_map(|i| u32_at(&stco, 8 + i * 4).map(u64::from))
            .collect()
    } else {
        let co64 = table(f, b"co64")?;
        (0..u32_at(&co64, 4)? as usize)
            .filter_map(|i| u64_at(&co64, 8 + i * 8))
            .collect()
    };
    let stsc = table(f, b"stsc")?;
    let runs: Vec<(u32, u32)> = (0..u32_at(&stsc, 4)? as usize)
        .filter_map(|i| Some((u32_at(&stsc, 8 + i * 12)?, u32_at(&stsc, 12 + i * 12)?)))
        .collect();
    let mut sample_pos = Vec::with_capacity(n);
    let mut s = 0usize;
    for (ci, &off) in offsets.iter().enumerate() {
        let chunk = ci as u32 + 1;
        let per = runs
            .iter()
            .rev()
            .find(|(first, _)| *first <= chunk)
            .map_or(1, |r| r.1);
        let mut pos = off;
        for _ in 0..per {
            if s >= n {
                break;
            }
            sample_pos.push(pos);
            pos += u64::from(sizes[s]);
            s += 1;
        }
    }
    let mut out = Vec::new();
    for (i, &pos) in sample_pos.iter().enumerate() {
        let size = sizes.get(i).copied().unwrap_or(0).min(4096) as usize;
        if size < 2 {
            continue;
        }
        let mut buf = vec![0u8; size];
        if f.seek(SeekFrom::Start(pos)).is_err() || f.read_exact(&mut buf).is_err() {
            continue;
        }
        let len = usize::from(u16::from_be_bytes([buf[0], buf[1]])).min(size - 2);
        let text = &buf[2..2 + len];
        let title = if text.starts_with(&[0xFE, 0xFF]) {
            let u: Vec<u16> = text[2..]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u)
        } else {
            String::from_utf8_lossy(text).into_owned()
        };
        let start = starts.get(i).copied().unwrap_or(0) as f64 / timescale;
        out.push(Chapter {
            title: title.trim().to_owned(),
            start,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny MP3: an ID3v2.3 tag with two chapters, then one silent frame.
    fn mp3_with_chapters(path: &Path) {
        fn frame(id: &[u8], body: &[u8]) -> Vec<u8> {
            let mut f = id.to_vec();
            f.extend((body.len() as u32).to_be_bytes());
            f.extend([0, 0]);
            f.extend(body);
            f
        }
        let tit2 = |t: &str| frame(b"TIT2", &[&[3u8][..], t.as_bytes()].concat());
        let chap = |id: &str, start: u32, end: u32, title: &str| {
            let mut b = id.as_bytes().to_vec();
            b.push(0);
            b.extend(start.to_be_bytes());
            b.extend(end.to_be_bytes());
            b.extend([0xFF; 8]);
            b.extend(tit2(title));
            frame(b"CHAP", &b)
        };
        let mut frames = Vec::new();
        frames.extend(frame(b"TALB", &[&[3u8][..], b"The Lighthouse"].concat()));
        frames.extend(frame(b"TPE1", &[&[3u8][..], b"Jane Smith"].concat()));
        frames.extend(chap("c1", 0, 60_000, "Arrival"));
        frames.extend(chap("c2", 60_000, 125_500, "The first storm"));
        let size = frames.len();
        let ss = [
            (size >> 21) as u8 & 0x7f,
            (size >> 14) as u8 & 0x7f,
            (size >> 7) as u8 & 0x7f,
            size as u8 & 0x7f,
        ];
        let mut data = b"ID3\x03\x00\x00".to_vec();
        data.extend(ss);
        data.extend(frames);
        // MPEG-1 Layer III, 128 kbit/s, 44.1 kHz frames of silence.
        for _ in 0..40 {
            let mut fr = vec![0u8; 417];
            fr[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            data.extend(fr);
        }
        std::fs::write(path, data).unwrap();
    }

    #[test]
    fn reads_mp3_chapters_and_details() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("book.mp3");
        mp3_with_chapters(&p);
        let info = info(&p).unwrap();
        assert_eq!(
            info.chapters,
            [
                Chapter {
                    title: "Arrival".into(),
                    start: 0.0
                },
                Chapter {
                    title: "The first storm".into(),
                    start: 60.0
                }
            ]
        );
        let mut out = Extracted::default();
        read(&p, &mut out).unwrap();
        assert_eq!(out.metadata.title, "The Lighthouse");
        assert_eq!(out.metadata.authors, ["Jane Smith"]);
    }

    #[test]
    fn reads_comment_chapters() {
        let items = [
            ("CHAPTER001", "00:00:00.000"),
            ("CHAPTER001NAME", "One"),
            ("CHAPTER002", "00:10:05.5"),
            ("chapter002name", "Two"),
        ];
        let mut c = comment_chapters(items.into_iter());
        c.sort_by(|a, b| a.start.total_cmp(&b.start));
        assert_eq!(
            c,
            [
                Chapter {
                    title: "One".into(),
                    start: 0.0
                },
                Chapter {
                    title: "Two".into(),
                    start: 605.5
                }
            ]
        );
    }

    #[test]
    fn reads_clock_times() {
        assert_eq!(parse_clock("01:02:03.5"), Some(3723.5));
        assert_eq!(parse_clock("2:03"), Some(123.0));
        assert_eq!(parse_clock("x"), None);
    }

    /// Checks files made by other tools: `LIBRERI_AUDIO_DIR=dir cargo test audio::tests::look`
    /// reads every `book.*` there, each with three chapters (ffmpeg writes
    /// none into FLAC).
    #[test]
    fn look() {
        let Some(dir) = std::env::var_os("LIBRERI_AUDIO_DIR") else {
            return;
        };
        for ext in ["m4b", "mp3", "opus", "flac"] {
            let p = std::path::Path::new(&dir).join(format!("book.{ext}"));
            let i = info(&p).unwrap();
            let mut out = Extracted::default();
            read(&p, &mut out).unwrap();
            println!(
                "{ext}: {:?} {:?} {:?}",
                i.duration, i.chapters, out.metadata.title
            );
            if ext != "flac" {
                assert_eq!(i.chapters.len(), 3, "{ext}");
            }
        }
    }
}
