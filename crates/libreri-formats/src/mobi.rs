//! MOBI and AZW3 (Kindle) books: details, cover and text.
//!
//! A Kindle book is a Palm database: record 0 holds the PalmDOC and MOBI
//! headers and the EXTH metadata block; the text follows in records
//! compressed with PalmDOC LZ77 or HUFF/CDIC; images come after the text.
//! AZW3 (KF8) uses the same layout. Books with DRM keep their details but
//! their text cannot be read.

use crate::{find_year, split_keywords, xml, Extracted};
use libreri_core::isbn::find_isbn;
use std::path::Path;

/// Largest Kindle file read whole.
const MAX_BYTES: u64 = 512 * 1024 * 1024;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A parsed Kindle file.
pub struct Mobi {
    data: Vec<u8>,
    offsets: Vec<usize>,
    compression: u16,
    text_records: usize,
    encryption: u16,
    utf8: bool,
    extra_flags: u16,
    first_image: Option<usize>,
    huff_first: usize,
    huff_count: usize,
    pub title: String,
    exth: Vec<(u32, Vec<u8>)>,
}

impl Mobi {
    pub fn open(path: &Path) -> Result<Self, String> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if size > MAX_BYTES {
            return Err("the file is too large".into());
        }
        Self::parse(std::fs::read(path).map_err(|e| e.to_string())?)
    }

    pub fn parse(data: Vec<u8>) -> Result<Self, String> {
        let bad = || "not a Kindle book".to_owned();
        if data.get(60..68) != Some(b"BOOKMOBI") && data.get(60..68) != Some(b"TEXtREAd") {
            return Err(bad());
        }
        let count = u16_at(&data, 76).ok_or_else(bad)? as usize;
        let mut offsets = Vec::with_capacity(count + 1);
        for i in 0..count {
            let off = u32_at(&data, 78 + i * 8).ok_or_else(bad)? as usize;
            offsets.push(off.min(data.len()));
        }
        offsets.push(data.len());
        let r0 = *offsets.first().ok_or_else(bad)?;
        let rec0 = data.get(r0..offsets[1]).ok_or_else(bad)?;
        let compression = u16_at(rec0, 0).ok_or_else(bad)?;
        let text_records = u16_at(rec0, 8).ok_or_else(bad)? as usize;
        let encryption = u16_at(rec0, 12).unwrap_or(0);
        let mut m = Mobi {
            compression,
            text_records: text_records.min(count.saturating_sub(1)),
            encryption,
            utf8: false,
            extra_flags: 0,
            first_image: None,
            huff_first: 0,
            huff_count: 0,
            title: String::new(),
            exth: Vec::new(),
            offsets,
            data: Vec::new(),
        };
        if rec0.get(16..20) == Some(b"MOBI") {
            let header_len = u32_at(rec0, 20).unwrap_or(0) as usize;
            m.utf8 = u32_at(rec0, 28) == Some(65001);
            let name_off = u32_at(rec0, 84).unwrap_or(0) as usize;
            let name_len = u32_at(rec0, 88).unwrap_or(0) as usize;
            if let Some(name) = rec0.get(name_off..name_off.saturating_add(name_len)) {
                m.title = m.decode(name);
            }
            m.first_image = u32_at(rec0, 108)
                .filter(|&i| i != u32::MAX && (i as usize) < count)
                .map(|i| i as usize);
            m.huff_first = u32_at(rec0, 112).unwrap_or(0) as usize;
            m.huff_count = u32_at(rec0, 116).unwrap_or(0) as usize;
            if header_len >= 0xE4 {
                m.extra_flags = u16_at(rec0, 16 + 0xE2).unwrap_or(0);
            }
            let exth_flags = u32_at(rec0, 128).unwrap_or(0);
            let at = 16 + header_len;
            if exth_flags & 0x40 != 0 && rec0.get(at..at + 4) == Some(b"EXTH") {
                let n = u32_at(rec0, at + 8).unwrap_or(0) as usize;
                let mut p = at + 12;
                for _ in 0..n.min(4096) {
                    let (Some(kind), Some(len)) = (u32_at(rec0, p), u32_at(rec0, p + 4)) else {
                        break;
                    };
                    let len = len as usize;
                    if len < 8 {
                        break;
                    }
                    if let Some(v) = rec0.get(p + 8..p + len) {
                        m.exth.push((kind, v.to_vec()));
                    }
                    p += len;
                }
            }
        } else {
            // A plain PalmDOC text: the database name is the title.
            m.title = String::from_utf8_lossy(&data[..32])
                .trim_end_matches('\0')
                .to_owned();
        }
        m.data = data;
        Ok(m)
    }

    fn record(&self, i: usize) -> Option<&[u8]> {
        let start = *self.offsets.get(i)?;
        let end = *self.offsets.get(i + 1)?;
        self.data.get(start..end.max(start))
    }

    fn decode(&self, bytes: &[u8]) -> String {
        if self.utf8 {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            bytes.iter().map(|&b| cp1252(b)).collect()
        }
    }

    fn exth_str(&self, kind: u32) -> Vec<String> {
        self.exth
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, v)| xml::collapse(&self.decode(v)))
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// True when the book is locked with DRM.
    pub fn protected(&self) -> bool {
        self.encryption != 0
    }

    /// Bytes of the cover image, if the book names one.
    pub fn cover(&self) -> Option<Vec<u8>> {
        let first = self.first_image?;
        let pick = |kind| {
            self.exth
                .iter()
                .find(|(k, _)| *k == kind)
                .and_then(|(_, v)| u32_at(v, 0))
        };
        let index = pick(201).or_else(|| pick(202))?;
        let rec = self.record(first + index as usize)?;
        crate::is_image_bytes(rec).then(|| rec.to_vec())
    }

    /// The book's text as the markup it is stored in (HTML-like).
    pub fn markup(&self) -> Result<String, String> {
        if self.protected() {
            return Err("the book is protected by DRM".into());
        }
        let mut huff = match self.compression {
            17480 => Some(Huffcdic::load(self)?),
            1 | 2 => None,
            other => return Err(format!("unknown compression {other}")),
        };
        let mut out = Vec::new();
        for i in 1..=self.text_records {
            let Some(rec) = self.record(i) else { break };
            let rec = &rec[..rec.len() - trailing_size(rec, self.extra_flags).min(rec.len())];
            match (self.compression, &mut huff) {
                (1, _) => out.extend_from_slice(rec),
                (2, _) => palmdoc(rec, &mut out),
                (_, Some(h)) => h.unpack(rec, &mut out, 0)?,
                _ => {}
            }
        }
        Ok(self.decode(&out))
    }
}

/// Windows-1252, the encoding of older MOBI files.
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Size of the extra data some writers append to each text record.
fn trailing_size(rec: &[u8], flags: u16) -> usize {
    let mut size = 0usize;
    let mut f = flags >> 1;
    while f != 0 {
        if f & 1 != 0 {
            let end = rec.len().saturating_sub(size);
            let tail = &rec[end.saturating_sub(4)..end];
            let mut n = 0usize;
            for &v in tail {
                if v & 0x80 != 0 {
                    n = 0;
                }
                n = (n << 7) | (v & 0x7F) as usize;
            }
            size += n;
        }
        f >>= 1;
    }
    if flags & 1 != 0 {
        if let Some(&b) = rec.len().checked_sub(size + 1).and_then(|i| rec.get(i)) {
            size += (b & 0x3) as usize + 1;
        }
    }
    size
}

/// PalmDOC's LZ77 variant.
fn palmdoc(src: &[u8], out: &mut Vec<u8>) {
    let start = out.len();
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        i += 1;
        match c {
            0x01..=0x08 => {
                let end = (i + c as usize).min(src.len());
                out.extend_from_slice(&src[i..end]);
                i = end;
            }
            0x00 | 0x09..=0x7F => out.push(c),
            0xC0..=0xFF => {
                out.push(b' ');
                out.push(c ^ 0x80);
            }
            0x80..=0xBF => {
                let Some(&c2) = src.get(i) else { break };
                i += 1;
                let v = ((c as usize) << 8) | c2 as usize;
                let dist = (v >> 3) & 0x7FF;
                let len = (v & 7) + 3;
                if dist == 0 || dist > out.len() - start {
                    continue;
                }
                let from = out.len() - dist;
                for k in 0..len {
                    let b = out[from + k];
                    out.push(b);
                }
            }
        }
    }
}

/// HUFF/CDIC compression (used by some Kindle books).
struct Huffcdic {
    dict1: Vec<(u32, bool, u64)>,
    mincode: Vec<u64>,
    maxcode: Vec<u64>,
    dictionary: Vec<(Vec<u8>, bool)>,
}

impl Huffcdic {
    fn load(m: &Mobi) -> Result<Self, String> {
        let bad = || "damaged HUFF/CDIC data".to_owned();
        let huff = m.record(m.huff_first).ok_or_else(bad)?;
        if huff.get(0..8) != Some(b"HUFF\0\0\0\x18") {
            return Err(bad());
        }
        let off1 = u32_at(huff, 8).ok_or_else(bad)? as usize;
        let off2 = u32_at(huff, 12).ok_or_else(bad)? as usize;
        let mut dict1 = Vec::with_capacity(256);
        for i in 0..256 {
            let v = u32_at(huff, off1 + i * 4).ok_or_else(bad)?;
            let codelen = v & 0x1F;
            let term = v & 0x80 != 0;
            let maxcode = (((v >> 8) as u64 + 1) << (32 - codelen.min(32))) - 1;
            if codelen == 0 {
                return Err(bad());
            }
            dict1.push((codelen, term, maxcode));
        }
        let mut mincode = vec![0u64];
        let mut maxcode = vec![0u64];
        for i in 0..32 {
            let lo = u32_at(huff, off2 + i * 8).ok_or_else(bad)? as u64;
            let hi = u32_at(huff, off2 + i * 8 + 4).ok_or_else(bad)? as u64;
            let len = i as u32 + 1;
            mincode.push(lo << (32 - len));
            maxcode.push(((hi + 1) << (32 - len)) - 1);
        }
        let mut dictionary = Vec::new();
        for r in 1..m.huff_count {
            let cdic = m.record(m.huff_first + r).ok_or_else(bad)?;
            if cdic.get(0..8) != Some(b"CDIC\0\0\0\x10") {
                return Err(bad());
            }
            let phrases = u32_at(cdic, 8).ok_or_else(bad)? as usize;
            let bits = u32_at(cdic, 12).ok_or_else(bad)?;
            let n = (1usize << bits.min(20)).min(phrases.saturating_sub(dictionary.len()));
            for k in 0..n {
                let off = u16_at(cdic, 16 + k * 2).ok_or_else(bad)? as usize;
                let blen = u16_at(cdic, 16 + off).ok_or_else(bad)?;
                let len = (blen & 0x7FFF) as usize;
                let slice = cdic.get(18 + off..18 + off + len).ok_or_else(bad)?;
                dictionary.push((slice.to_vec(), blen & 0x8000 != 0));
            }
        }
        Ok(Self {
            dict1,
            mincode,
            maxcode,
            dictionary,
        })
    }

    fn unpack(&mut self, data: &[u8], out: &mut Vec<u8>, depth: u32) -> Result<(), String> {
        if depth > 32 {
            return Err("damaged HUFF/CDIC data".into());
        }
        let mut bits_left = data.len() as i64 * 8;
        let mut padded = data.to_vec();
        padded.extend_from_slice(&[0; 8]);
        let q = |pos: usize| u64::from_be_bytes(padded[pos..pos + 8].try_into().unwrap_or([0; 8]));
        let mut pos = 0usize;
        let mut x = q(pos);
        let mut n: i64 = 32;
        loop {
            if n <= 0 {
                pos += 4;
                if pos + 8 > padded.len() {
                    break;
                }
                x = q(pos);
                n += 32;
            }
            let code = (x >> n) & 0xFFFF_FFFF;
            let (mut codelen, term, mut maxcode) = self.dict1[(code >> 24) as usize];
            if !term {
                while (codelen as usize) < self.mincode.len()
                    && code < self.mincode[codelen as usize]
                {
                    codelen += 1;
                }
                maxcode = *self
                    .maxcode
                    .get(codelen as usize)
                    .ok_or("damaged HUFF/CDIC data")?;
            }
            n -= codelen as i64;
            bits_left -= codelen as i64;
            if bits_left < 0 {
                break;
            }
            let r = ((maxcode - code) >> (32 - codelen)) as usize;
            let Some((slice, done)) = self.dictionary.get(r).cloned() else {
                return Err("damaged HUFF/CDIC data".into());
            };
            if done {
                out.extend_from_slice(&slice);
            } else {
                let mut expanded = Vec::new();
                self.unpack(&slice, &mut expanded, depth + 1)?;
                out.extend_from_slice(&expanded);
                self.dictionary[r] = (expanded, true);
            }
        }
        Ok(())
    }
}

pub fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let book = Mobi::open(path)?;
    let m = &mut out.metadata;
    let updated = book.exth_str(503);
    m.title = updated
        .into_iter()
        .next()
        .unwrap_or_else(|| book.title.clone());
    m.authors = book
        .exth_str(100)
        .iter()
        .flat_map(|a| crate::split_people(a))
        .collect();
    m.publisher = book.exth_str(101).into_iter().next();
    m.about = book
        .exth_str(103)
        .into_iter()
        .next()
        .map(|d| xml::strip_html(&d))
        .filter(|d| !d.is_empty());
    if let Some((i13, i10)) = book.exth_str(104).iter().find_map(|s| find_isbn(s)) {
        m.isbn13 = Some(i13);
        m.isbn10 = i10;
    }
    m.tags = book
        .exth_str(105)
        .iter()
        .flat_map(|s| split_keywords(s))
        .collect();
    m.year = book.exth_str(106).iter().find_map(|d| find_year(d));
    m.language = book.exth_str(524).into_iter().next();
    m.contributors = book.exth_str(108);
    out.cover = book.cover();
    if book.protected() {
        out.warnings
            .push("the book is protected by DRM; it can be listed but not read".into());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a small MOBI file with PalmDOC-compressed text records.
    pub fn make_mobi(
        title: &str,
        author: &str,
        records: &[&[u8]],
        cover: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut exth = Vec::new();
        let mut entries: Vec<(u32, Vec<u8>)> = vec![
            (100, author.as_bytes().to_vec()),
            (101, b"Harbour Press".to_vec()),
            (104, b"978-0-306-40615-7".to_vec()),
            (106, b"2019-05-01".to_vec()),
            (524, b"en".to_vec()),
        ];
        if cover.is_some() {
            entries.push((201, 0u32.to_be_bytes().to_vec()));
        }
        for (k, v) in &entries {
            exth.extend_from_slice(&k.to_be_bytes());
            exth.extend_from_slice(&(v.len() as u32 + 8).to_be_bytes());
            exth.extend_from_slice(v);
        }
        let mut exth_block = b"EXTH".to_vec();
        exth_block.extend_from_slice(&(exth.len() as u32 + 12).to_be_bytes());
        exth_block.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        exth_block.extend_from_slice(&exth);

        let header_len = 0xE8u32;
        let mut rec0 = vec![0u8; 16 + header_len as usize];
        rec0[0..2].copy_from_slice(&2u16.to_be_bytes());
        rec0[8..10].copy_from_slice(&(records.len() as u16).to_be_bytes());
        rec0[10..12].copy_from_slice(&4096u16.to_be_bytes());
        rec0[16..20].copy_from_slice(b"MOBI");
        rec0[20..24].copy_from_slice(&header_len.to_be_bytes());
        rec0[28..32].copy_from_slice(&65001u32.to_be_bytes());
        let first_image = records.len() as u32 + 1;
        rec0[108..112].copy_from_slice(&first_image.to_be_bytes());
        rec0[128..132].copy_from_slice(&0x40u32.to_be_bytes());
        rec0.extend_from_slice(&exth_block);
        let name_off = rec0.len() as u32;
        rec0[84..88].copy_from_slice(&name_off.to_be_bytes());
        rec0[88..92].copy_from_slice(&(title.len() as u32).to_be_bytes());
        rec0.extend_from_slice(title.as_bytes());
        rec0.extend_from_slice(&[0, 0]);

        let mut recs: Vec<Vec<u8>> = vec![rec0];
        for r in records {
            recs.push(r.to_vec());
        }
        if let Some(c) = cover {
            recs.push(c.to_vec());
        }
        let mut out = vec![0u8; 78];
        out[..title.len().min(31)].copy_from_slice(&title.as_bytes()[..title.len().min(31)]);
        out[60..68].copy_from_slice(b"BOOKMOBI");
        out[76..78].copy_from_slice(&(recs.len() as u16).to_be_bytes());
        let mut offset = 78 + recs.len() * 8 + 2;
        for (i, r) in recs.iter().enumerate() {
            out.extend_from_slice(&(offset as u32).to_be_bytes());
            out.extend_from_slice(&[0, 0, 0, i as u8]);
            offset += r.len();
        }
        out.extend_from_slice(&[0, 0]);
        for r in recs {
            out.extend_from_slice(&r);
        }
        out
    }

    #[test]
    fn palmdoc_decompresses() {
        let mut out = Vec::new();
        // "abc" literal, then a back reference of distance 3 length 3, then
        // " x" as a space pair, then a 2-byte literal run.
        let dist_len: u16 = 0x8000 | (3 << 3);
        let mut src = b"abc".to_vec();
        src.extend_from_slice(&dist_len.to_be_bytes());
        src.push(b'x' ^ 0x80);
        src.extend_from_slice(&[2, 0xE9, 0xEA]);
        palmdoc(&src, &mut out);
        assert_eq!(out, b"abcabc x\xE9\xEA");
    }

    #[test]
    fn reads_details_cover_and_text() {
        let png = b"\x89PNG\r\n\x1a\nnot really a picture".to_vec();
        let bytes = make_mobi(
            "Harbour Lights",
            "Jane Smith",
            &[
                b"<html><body><h1>One</h1><p>The keeper wrote</p>",
                b"<p>every night.</p></body></html>",
            ],
            Some(&png),
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.mobi");
        std::fs::write(&path, &bytes).unwrap();
        let x = crate::extract(&path, libreri_core::FileType::Mobi);
        assert_eq!(x.metadata.title, "Harbour Lights");
        assert_eq!(x.metadata.authors, vec!["Jane Smith"]);
        assert_eq!(x.metadata.publisher.as_deref(), Some("Harbour Press"));
        assert_eq!(x.metadata.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(x.metadata.year, Some(2019));
        assert_eq!(x.metadata.language.as_deref(), Some("en"));
        assert_eq!(x.cover.as_deref(), Some(&png[..]));
        let book = Mobi::open(&path).unwrap();
        let text = book.markup().unwrap();
        assert!(
            text.contains("The keeper wrote</p><p>every night."),
            "{text}"
        );
    }

    #[test]
    fn trailing_entries_are_removed() {
        // One trailing entry of 3 bytes (size byte 0x83 counts itself).
        let rec = b"hello\x01\x02\x83";
        assert_eq!(trailing_size(rec, 0b10), 3);
        // Multibyte flag: last byte & 3, plus one.
        assert_eq!(trailing_size(b"hi\x01", 0b1), 2);
    }
}
