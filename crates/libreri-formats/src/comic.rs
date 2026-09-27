//! Comic archives: CBZ (ZIP), CBR (RAR), CB7 (7-Zip), CBT (TAR) and, when
//! the `unar` helper is installed, CBA (ACE).
//!
//! Pages are the images in the archive in natural order ("p2" before
//! "p10"), skipping macOS resource forks and hidden files. `ComicInfo.xml`
//! gives the details, and says whether the comic reads right to left.
//! ZIP pages are read one by one; the other kinds are solid or sequential,
//! so the reader unpacks all pages once into a cache (see [`extract_pages`]).

use crate::xml::{self, Element};
use crate::{
    is_image_name, natural_key, split_keywords, split_people, zip_read, Extracted, MAX_ENTRY,
};
use libreri_core::FileType;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Largest page image read (bigger entries are skipped as not pages).
const MAX_PAGE: u64 = 64 * 1024 * 1024;

fn is_page(name: &str) -> bool {
    let lower = name.to_lowercase();
    is_image_name(name)
        && !lower.starts_with("__macosx")
        && !lower.contains("/.")
        && !lower.starts_with('.')
}

/// Every entry name, and the named entries that were read.
type Scanned = (Vec<String>, Vec<(String, Vec<u8>)>);

fn sort_pages(pages: &mut [String]) {
    pages.sort_by_key(|n| natural_key(n));
}

/// Everything in one pass over a sequential archive: every entry name,
/// and the bytes of those `wanted` returns true for.
fn scan(
    path: &Path,
    kind: FileType,
    mut wanted: impl FnMut(&str) -> bool,
) -> Result<Scanned, String> {
    let mut names = Vec::new();
    let mut found = Vec::new();
    match kind {
        FileType::Cbz => {
            let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("not a valid CBZ ({e})"))?;
            names = zip.file_names().map(str::to_owned).collect();
            for n in names.clone() {
                if wanted(&n) {
                    if let Some(b) = zip_read(&mut zip, &n) {
                        found.push((n, b));
                    }
                }
            }
        }
        FileType::Cbr => {
            let mut archive = unrar::Archive::new(path)
                .open_for_processing()
                .map_err(|e| format!("not a valid CBR ({e})"))?;
            while let Some(header) = archive.read_header().map_err(|e| e.to_string())? {
                let entry = header.entry();
                let name = entry.filename.to_string_lossy().replace('\\', "/");
                let is_file = entry.is_file();
                let size = entry.unpacked_size;
                names.push(name.clone());
                archive = if is_file && size <= MAX_PAGE && wanted(&name) {
                    let (bytes, rest) = header.read().map_err(|e| e.to_string())?;
                    found.push((name, bytes));
                    rest
                } else {
                    header.skip().map_err(|e| e.to_string())?
                };
            }
        }
        FileType::Cb7 => {
            let mut reader =
                sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty())
                    .map_err(|e| format!("not a valid CB7 ({e})"))?;
            reader
                .for_each_entries(|entry, r| {
                    if entry.is_directory {
                        return Ok(true);
                    }
                    names.push(entry.name.clone());
                    if entry.size <= MAX_PAGE && wanted(&entry.name) {
                        let mut buf = Vec::with_capacity(entry.size as usize);
                        r.read_to_end(&mut buf)?;
                        found.push((entry.name.clone(), buf));
                    } else {
                        std::io::copy(r, &mut std::io::sink())?;
                    }
                    Ok(true)
                })
                .map_err(|e| e.to_string())?;
        }
        FileType::Cbt => {
            let mut archive = tar::Archive::new(File::open(path).map_err(|e| e.to_string())?);
            for entry in archive
                .entries()
                .map_err(|e| format!("not a valid CBT ({e})"))?
            {
                let mut entry = entry.map_err(|e| e.to_string())?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let name = entry
                    .path()
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .map_err(|e| e.to_string())?;
                names.push(name.clone());
                if entry.size() <= MAX_PAGE && wanted(&name) {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                    found.push((name, buf));
                }
            }
        }
        FileType::Cba => return ace::scan(path, wanted),
        other => return Err(format!("{} is not a comic archive", other.as_str())),
    }
    Ok((names, found))
}

/// Details, page count and cover of any comic archive.
pub fn read(path: &Path, kind: FileType, out: &mut Extracted) -> Result<(), String> {
    // Pass 1: names. Pass 2 (ZIP reads directly): ComicInfo and the first page.
    let (names, _) = scan(path, kind, |_| false)?;
    let mut pages: Vec<String> = names.iter().filter(|n| is_page(n)).cloned().collect();
    sort_pages(&mut pages);
    if !pages.is_empty() {
        out.metadata.pages = Some(pages.len() as u32);
    }
    let first = pages.first().cloned();
    let info_name = names
        .iter()
        .find(|n| {
            n.eq_ignore_ascii_case("ComicInfo.xml") || n.to_lowercase().ends_with("/comicinfo.xml")
        })
        .cloned();
    let (_, found) = scan(path, kind, |n| {
        Some(n) == first.as_deref() || Some(n) == info_name.as_deref()
    })?;
    for (name, bytes) in found {
        if Some(&name) == info_name.as_ref() {
            if bytes.len() as u64 <= MAX_ENTRY {
                let root = xml::parse(&String::from_utf8_lossy(&bytes));
                if let Some(info) = root.find("comicinfo") {
                    apply_comic_info(info, out);
                }
            }
        } else {
            out.cover = Some(bytes);
        }
    }
    Ok(())
}

/// A comic's pages as the reader needs them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicPages {
    /// Page names inside the archive, in reading order.
    pub pages: Vec<String>,
    /// ComicInfo says it reads right to left (manga).
    pub right_to_left: bool,
}

fn manga(names_and_info: Option<&[u8]>) -> bool {
    names_and_info
        .map(|b| {
            let root = xml::parse(&String::from_utf8_lossy(b));
            root.find("comicinfo")
                .and_then(|i| i.find("manga"))
                .map(Element::text_deep)
                .is_some_and(|m| m.eq_ignore_ascii_case("YesAndRightToLeft"))
        })
        .unwrap_or(false)
}

/// The pages of a comic (names only; ZIP pages are then read with
/// [`read_zip_page`]).
pub fn list_pages(path: &Path, kind: FileType) -> Result<ComicPages, String> {
    let mut info_name = None;
    let (names, found) = scan(path, kind, |n| {
        let hit =
            n.eq_ignore_ascii_case("ComicInfo.xml") || n.to_lowercase().ends_with("/comicinfo.xml");
        if hit {
            info_name = Some(n.to_owned());
        }
        hit
    })?;
    let mut pages: Vec<String> = names.into_iter().filter(|n| is_page(n)).collect();
    sort_pages(&mut pages);
    Ok(ComicPages {
        pages,
        right_to_left: manga(found.first().map(|(_, b)| b.as_slice())),
    })
}

/// One page of a CBZ, read directly.
pub fn read_zip_page(path: &Path, name: &str) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let entry = zip.by_name(name).map_err(|e| e.to_string())?;
    if entry.size() > MAX_PAGE {
        return Err("the page is too large".into());
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_PAGE)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Unpacks every page into `dest` as `0001.jpg`, `0002.png`… (numbered in
/// reading order, original extension) and writes `pages.json`. Returns the
/// file names. Pages already there are kept, so this is cheap to repeat.
pub fn extract_pages(
    path: &Path,
    kind: FileType,
    dest: &Path,
) -> Result<(ComicPages, Vec<PathBuf>), String> {
    let index = dest.join("pages.json");
    if let Ok(text) = fs::read_to_string(&index) {
        if let Ok(pages) = serde_json::from_str::<ComicPages>(&text) {
            let files = page_files(&pages, dest);
            if files.iter().all(|f| f.is_file()) {
                return Ok((pages, files));
            }
        }
    }
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let listed = list_pages(path, kind)?;
    let position: std::collections::HashMap<&str, usize> = listed
        .pages
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let files = page_files(&listed, dest);
    let (_, found) = scan(path, kind, |n| position.contains_key(n))?;
    for (name, bytes) in found {
        if let Some(i) = position.get(name.as_str()) {
            let tmp = files[*i].with_extension("part");
            fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
            fs::rename(&tmp, &files[*i]).map_err(|e| e.to_string())?;
        }
    }
    let json = serde_json::to_string(&listed).map_err(|e| e.to_string())?;
    fs::write(&index, json).map_err(|e| e.to_string())?;
    Ok((listed, files))
}

fn page_files(pages: &ComicPages, dest: &Path) -> Vec<PathBuf> {
    pages
        .pages
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let ext = n.rsplit('.').next().unwrap_or("jpg").to_lowercase();
            dest.join(format!("{:04}.{ext}", i + 1))
        })
        .collect()
}

/// ACE archives, through the `unar`/`lsar` helper when it is installed.
mod ace {
    use std::path::Path;

    pub fn scan(
        path: &Path,
        mut wanted: impl FnMut(&str) -> bool,
    ) -> Result<super::Scanned, String> {
        let out = libreri_helpers::command("lsar")
            .ok_or("CBA comics need the unar helper; install it in Settings › Helpers")?
            .arg("-j")
            .arg(path)
            .output()
            .map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|_| "unar could not read this CBA".to_owned())?;
        let names: Vec<String> = json["lsarContents"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|e| e["XADIsDirectory"].as_bool() != Some(true))
                    .filter_map(|e| e["XADFileName"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let want: Vec<String> = names.iter().filter(|n| wanted(n)).cloned().collect();
        let mut found = Vec::new();
        if !want.is_empty() {
            let tmp = std::env::temp_dir().join(format!(
                "libreri-cba-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos())
            ));
            let status = libreri_helpers::command("unar")
                .ok_or("CBA comics need the unar helper")?
                .args(["-q", "-f", "-D", "-o"])
                .arg(&tmp)
                .arg(path)
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                for n in want {
                    let file = n.rsplit('/').next().unwrap_or(&n);
                    if let Ok(bytes) = std::fs::read(tmp.join(file)) {
                        found.push((n, bytes));
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&tmp);
        }
        Ok((names, found))
    }
}

fn apply_comic_info(info: &Element, out: &mut Extracted) {
    let get = |n: &str| {
        info.find(n)
            .map(Element::text_deep)
            .filter(|t| !t.is_empty())
    };
    let m = &mut out.metadata;
    if let Some(t) = get("title") {
        m.title = t;
    }
    m.series = get("series");
    m.series_number = get("number").and_then(|n| n.parse().ok());
    if m.title.is_empty() {
        if let Some(series) = &m.series {
            m.title = match get("number") {
                Some(n) => format!("{series} #{n}"),
                None => series.clone(),
            };
        }
    }
    if let Some(w) = get("writer") {
        m.authors = w.split(',').flat_map(split_people).collect();
    }
    for (field, role) in [
        ("penciller", "artist"),
        ("colorist", "colourist"),
        ("editor", "editor"),
    ] {
        if let Some(v) = get(field) {
            m.contributors
                .extend(v.split(',').map(|p| format!("{} ({role})", p.trim())));
        }
    }
    m.about = get("summary");
    m.publisher = get("publisher");
    m.year = get("year").and_then(|y| y.parse().ok());
    m.language = get("languageiso");
    if let Some(g) = get("genre") {
        m.tags = split_keywords(&g);
    }
    if let Some(p) = get("pagecount").and_then(|p| p.parse().ok()) {
        m.pages = Some(p);
    }
}

#[cfg(test)]
mod tests {
    use crate::extract;
    use crate::testutil::zip_file;
    use libreri_core::FileType;

    #[test]
    fn reads_pages_cover_and_comic_info() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.cbz");
        zip_file(
            &p,
            &[
                ("p10.jpg", b"TEN"),
                ("p2.jpg", b"TWO"),
                ("p1.jpg", b"ONE"),
                ("__MACOSX/._p1.jpg", b"junk"),
                (
                    "ComicInfo.xml",
                    b"<ComicInfo><Series>Night Owl</Series><Number>3</Number>\
                      <Writer>John Smith, Jane Smith</Writer><Year>2020</Year></ComicInfo>",
                ),
            ],
        );
        let e = extract(&p, FileType::Cbz);
        assert_eq!(e.metadata.title, "Night Owl #3");
        assert_eq!(e.metadata.authors, vec!["John Smith", "Jane Smith"]);
        assert_eq!(e.metadata.pages, Some(3));
        assert_eq!(e.metadata.year, Some(2020));
        assert_eq!(e.cover.as_deref(), Some(&b"ONE"[..]));
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn reads_rar_comics() {
        let p = fixture("sample.cbr");
        let e = extract(&p, FileType::Cbr);
        assert_eq!(e.metadata.title, "Night Owl #4");
        assert_eq!(e.metadata.pages, Some(3));
        assert!(e
            .cover
            .as_deref()
            .is_some_and(|c| c.starts_with(b"\x89PNG")));
        let pages = crate::list_pages(&p, FileType::Cbr).unwrap();
        assert_eq!(pages.pages, ["p1.png", "p2.png", "p10.png"]);
        assert!(pages.right_to_left, "ComicInfo says manga");

        let dir = tempfile::tempdir().unwrap();
        let (again, files) = crate::extract_pages(&p, FileType::Cbr, dir.path()).unwrap();
        assert_eq!(again, pages);
        assert_eq!(files[2].file_name().unwrap(), "0003.png");
        assert!(files.iter().all(|f| f.is_file()));
        // A second call uses what is already there.
        assert!(crate::extract_pages(&p, FileType::Cbr, dir.path()).is_ok());
    }

    #[test]
    fn reads_7z_and_tar_comics() {
        let dir = tempfile::tempdir().unwrap();
        // 7-Zip
        let p7 = dir.path().join("c.cb7");
        {
            let mut w = sevenz_rust2::ArchiveWriter::create(&p7).unwrap();
            for (name, bytes) in [
                ("b/p2.jpg", &b"TWO"[..]),
                ("b/p1.jpg", b"ONE"),
                ("b/notes.txt", b"x"),
            ] {
                w.push_archive_entry(sevenz_rust2::ArchiveEntry::new_file(name), Some(bytes))
                    .unwrap();
            }
            w.finish().unwrap();
        }
        let e = extract(&p7, FileType::Cb7);
        assert_eq!(e.metadata.pages, Some(2));
        assert_eq!(e.cover.as_deref(), Some(&b"ONE"[..]));
        let out = dir.path().join("pages7");
        let (pages, files) = crate::extract_pages(&p7, FileType::Cb7, &out).unwrap();
        assert_eq!(pages.pages, ["b/p1.jpg", "b/p2.jpg"]);
        assert_eq!(std::fs::read(&files[1]).unwrap(), b"TWO");

        // TAR
        let pt = dir.path().join("c.cbt");
        {
            let mut b = tar::Builder::new(std::fs::File::create(&pt).unwrap());
            for (name, bytes) in [("p02.png", &b"B"[..]), ("p01.png", b"A")] {
                let mut h = tar::Header::new_gnu();
                h.set_size(bytes.len() as u64);
                h.set_mode(0o644);
                h.set_cksum();
                b.append_data(&mut h, name, bytes).unwrap();
            }
            b.finish().unwrap();
        }
        let e = extract(&pt, FileType::Cbt);
        assert_eq!(e.metadata.pages, Some(2));
        assert_eq!(e.cover.as_deref(), Some(&b"A"[..]));
        assert!(!crate::list_pages(&pt, FileType::Cbt).unwrap().right_to_left);
    }
}
