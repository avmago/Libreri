//! Libreri archives (`.libreri`): a ZIP file that mirrors the library
//! folder, plus `manifest.json`.
//!
//! ```text
//! manifest.json                          what is inside (see [`Manifest`])
//! .library-data/metadata/<book>.json     book details (sidecars)
//! .library-data/annotations/<profile>/<book>.json
//! .library-data/profiles/<profile>.json  (and .collections.json)
//! .library-data/covers/<book>.jpg
//! .library-data/text/<book>.json         OCR text (Phase 5)
//! Notes/<profile name>/…                 notebooks and notes, as Markdown
//! Books/…                                book files, when included
//! database/library.db                    a copy of the catalogue, for reference
//! ```
//!
//! Books are identified by content hash, so notes find their books again in
//! any library (docs/data-portability.md). Reading refuses entry names that
//! could escape the destination folder and caps what it reads.

use libreri_core::{Alias, AnnotationKind, BookId, FileType, ProfileId, ProfileKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub const ARCHIVE_FORMAT: &str = "libreri-archive";
pub const ARCHIVE_VERSION: u32 = 1;
pub const MANIFEST: &str = "manifest.json";
pub const DATABASE: &str = "database/library.db";

/// Top-level folders an archive may contain.
const ALLOWED: &[&str] = &[
    ".library-data/metadata/",
    ".library-data/annotations/",
    ".library-data/profiles/",
    ".library-data/covers/",
    ".library-data/text/",
    "Notes/",
    "Books/",
    "database/",
];

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveKind {
    /// Made with Export; never carries PINs.
    Export,
    /// Made by a backup; carries PINs so a restore brings them back.
    Backup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveLibrary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveProfile {
    pub id: ProfileId,
    pub name: String,
    pub kind: ProfileKind,
    /// Folder under `Notes/` in the archive.
    pub notes_folder: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveBook {
    /// BLAKE3 hash of the file: the book's identity everywhere.
    pub id: BookId,
    #[serde(default)]
    pub aliases: Vec<Alias>,
    pub rel_path: String,
    pub file_type: FileType,
    pub file_size: u64,
    /// The file itself is in the archive (at `rel_path`).
    pub file_included: bool,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub isbn13: Option<String>,
    pub isbn10: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub pages: Option<u32>,
}

/// One highlight or bookmark, with the link that points at it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveNote {
    pub id: String,
    pub profile: ProfileId,
    pub book: BookId,
    pub kind: AnnotationKind,
    pub label: Option<String>,
    pub link: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveNotebook {
    pub profile: ProfileId,
    pub book: BookId,
    /// Path inside the archive ("Notes/Jane/Optics.md").
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveFile {
    pub path: String,
    pub size: u64,
}

/// What an archive holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub kind: ArchiveKind,
    pub created_at: String,
    /// "Libreri 0.1.0".
    pub created_by: String,
    pub library: ArchiveLibrary,
    pub includes_book_files: bool,
    pub includes_pins: bool,
    pub profiles: Vec<ArchiveProfile>,
    pub books: Vec<ArchiveBook>,
    #[serde(default)]
    pub notes: Vec<ArchiveNote>,
    #[serde(default)]
    pub notebooks: Vec<ArchiveNotebook>,
    #[serde(default)]
    pub files: Vec<ArchiveFile>,
}

impl Manifest {
    pub fn new(kind: ArchiveKind, created_at: &str, app: &str, library: ArchiveLibrary) -> Self {
        Self {
            format: ARCHIVE_FORMAT.to_owned(),
            format_version: ARCHIVE_VERSION,
            kind,
            created_at: created_at.to_owned(),
            created_by: format!("Libreri {app}"),
            library,
            includes_book_files: false,
            includes_pins: false,
            profiles: Vec::new(),
            books: Vec::new(),
            notes: Vec::new(),
            notebooks: Vec::new(),
            files: Vec::new(),
        }
    }
}

/// Why an archive could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("this is not a Libreri archive")]
    NotAnArchive,
    #[error("this archive was made by a newer version of Libreri (archive format {0}); update Libreri to open it")]
    TooNew(u32),
    #[error("the archive is damaged: {0}")]
    Damaged(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<zip::result::ZipError> for ArchiveError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            zip::result::ZipError::Io(e) => Self::Io(e),
            zip::result::ZipError::InvalidArchive(_) => Self::NotAnArchive,
            other => Self::Damaged(other.to_string()),
        }
    }
}

/// True for entry names that stay inside the archive's own folders.
pub fn is_safe_name(name: &str) -> bool {
    if name == MANIFEST {
        return true;
    }
    if name.contains('\\') || name.contains(':') || name.starts_with('/') {
        return false;
    }
    if !ALLOWED.iter().any(|p| name.starts_with(p)) {
        return false;
    }
    name.split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

/// Writes an archive to `<dest>.part` and renames it into place when
/// finished, so a failed or cancelled export never leaves half a file.
pub struct ArchiveWriter {
    zip: Option<zip::ZipWriter<BufWriter<File>>>,
    dest: PathBuf,
    part: PathBuf,
    written: BTreeSet<String>,
    files: Vec<ArchiveFile>,
}

impl ArchiveWriter {
    pub fn create(dest: &Path) -> io::Result<Self> {
        let part = part_path(dest);
        let file = File::create(&part)?;
        Ok(Self {
            zip: Some(zip::ZipWriter::new(BufWriter::new(file))),
            dest: dest.to_path_buf(),
            part,
            written: BTreeSet::new(),
            files: Vec::new(),
        })
    }

    fn options(compress: bool, size: u64) -> zip::write::SimpleFileOptions {
        let method = if compress {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .large_file(size >= u64::from(u32::MAX) - 1024)
            .unix_permissions(0o644)
    }

    fn start(&mut self, name: &str, compress: bool, size: u64) -> io::Result<()> {
        if !is_safe_name(name) || name == MANIFEST {
            return Err(io::Error::other(format!(
                "not allowed in an archive: {name}"
            )));
        }
        if !self.written.insert(name.to_owned()) {
            return Err(io::Error::other(format!("already in the archive: {name}")));
        }
        self.files.push(ArchiveFile {
            path: name.to_owned(),
            size,
        });
        let zip = self.zip.as_mut().expect("open until finished");
        zip.start_file(name, Self::options(compress, size))
            .map_err(io::Error::other)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.written.contains(name)
    }

    pub fn add_bytes(&mut self, name: &str, bytes: &[u8]) -> io::Result<()> {
        self.start(name, true, bytes.len() as u64)?;
        self.zip.as_mut().expect("open").write_all(bytes)
    }

    /// Copies a file in. Book files are stored as they are (PDFs, EPUBs and
    /// images are compressed already); text is compressed.
    pub fn add_file(&mut self, name: &str, src: &Path) -> io::Result<()> {
        let size = fs::metadata(src)?.len();
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let compress = matches!(ext.as_str(), "json" | "md" | "txt" | "fb2" | "db" | "csv");
        self.start(name, compress, size)?;
        let mut from = BufReader::new(File::open(src)?);
        io::copy(&mut from, self.zip.as_mut().expect("open"))?;
        Ok(())
    }

    /// Writes the manifest (with the list of files) and puts the archive in
    /// place.
    pub fn finish(mut self, mut manifest: Manifest) -> io::Result<u64> {
        manifest.files = std::mem::take(&mut self.files);
        let json = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
        let mut zip = self.zip.take().expect("open");
        zip.start_file(MANIFEST, Self::options(true, json.len() as u64))
            .map_err(io::Error::other)?;
        zip.write_all(&json)?;
        let mut out = zip.finish().map_err(io::Error::other)?;
        out.flush()?;
        out.get_ref().sync_all()?;
        drop(out);
        fs::rename(&self.part, &self.dest)?;
        Ok(fs::metadata(&self.dest)?.len())
    }
}

impl Drop for ArchiveWriter {
    fn drop(&mut self) {
        if self.zip.take().is_some() {
            let _ = fs::remove_file(&self.part);
        }
    }
}

/// Largest manifest or JSON entry read into memory.
pub const MAX_JSON: u64 = 64 * 1024 * 1024;

/// Reads an archive.
pub struct ArchiveReader {
    zip: zip::ZipArchive<BufReader<File>>,
    pub manifest: Manifest,
}

impl ArchiveReader {
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        let file = File::open(path)?;
        let mut zip = zip::ZipArchive::new(BufReader::new(file))?;
        let manifest = {
            let entry = zip
                .by_name(MANIFEST)
                .map_err(|_| ArchiveError::NotAnArchive)?;
            let mut bytes = Vec::new();
            entry.take(MAX_JSON).read_to_end(&mut bytes)?;
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| ArchiveError::NotAnArchive)?;
            if value.get("format").and_then(|f| f.as_str()) != Some(ARCHIVE_FORMAT) {
                return Err(ArchiveError::NotAnArchive);
            }
            let version = value
                .get("formatVersion")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            if version > ARCHIVE_VERSION {
                return Err(ArchiveError::TooNew(version));
            }
            serde_json::from_value::<Manifest>(value)
                .map_err(|e| ArchiveError::Damaged(format!("manifest: {e}")))?
        };
        Ok(Self { zip, manifest })
    }

    /// Names of every allowed entry (others are ignored).
    pub fn names(&self) -> Vec<String> {
        self.zip
            .file_names()
            .filter(|n| is_safe_name(n) && !n.ends_with('/') && *n != MANIFEST)
            .map(str::to_owned)
            .collect()
    }

    pub fn has(&self, name: &str) -> bool {
        is_safe_name(name) && self.zip.index_for_name(name).is_some()
    }

    /// Reads a small entry (JSON, Markdown) into memory, at most `MAX_JSON`.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        if !is_safe_name(name) {
            return Err(ArchiveError::Damaged(format!("unsafe name {name}")));
        }
        let entry = self
            .zip
            .by_name(name)
            .map_err(|_| ArchiveError::Damaged(format!("{name} is missing")))?;
        let mut bytes = Vec::new();
        entry.take(MAX_JSON).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Writes an entry to `dest` (through `dest.part`), never more bytes
    /// than the entry says it has. Returns the size written.
    pub fn extract(&mut self, name: &str, dest: &Path) -> Result<u64, ArchiveError> {
        if !is_safe_name(name) {
            return Err(ArchiveError::Damaged(format!("unsafe name {name}")));
        }
        let entry = self
            .zip
            .by_name(name)
            .map_err(|_| ArchiveError::Damaged(format!("{name} is missing")))?;
        let size = entry.size();
        if let Some(dir) = dest.parent() {
            fs::create_dir_all(dir)?;
        }
        let part = part_path(dest);
        let result = (|| -> io::Result<u64> {
            let mut out = BufWriter::new(File::create(&part)?);
            let n = io::copy(&mut entry.take(size), &mut out)?;
            out.flush()?;
            Ok(n)
        })();
        match result {
            Ok(n) if n == size => {
                fs::rename(&part, dest)?;
                Ok(n)
            }
            Ok(_) => {
                let _ = fs::remove_file(&part);
                Err(ArchiveError::Damaged(format!("{name} is incomplete")))
            }
            Err(e) => {
                let _ = fs::remove_file(&part);
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest::new(
            ArchiveKind::Export,
            "2026-09-28T10:00:00Z",
            "0.1.0",
            ArchiveLibrary {
                id: "lib".into(),
                name: "Home".into(),
            },
        )
    }

    #[test]
    fn names_that_escape_are_refused() {
        assert!(is_safe_name("Books/Physics/a.pdf"));
        assert!(is_safe_name(".library-data/metadata/x.json"));
        assert!(!is_safe_name("Books/../../etc/passwd"));
        assert!(!is_safe_name("/Books/a.pdf"));
        assert!(!is_safe_name("Books\\a.pdf"));
        assert!(!is_safe_name("C:/Books/a.pdf"));
        assert!(!is_safe_name(".library-data/library.db"));
        assert!(!is_safe_name("Other/a.txt"));
        assert!(!is_safe_name("Books//a.pdf"));
    }

    #[test]
    fn round_trip_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        fs::write(&src, b"%PDF-1.7 hello").unwrap();
        let dest = dir.path().join("out.libreri");

        let mut w = ArchiveWriter::create(&dest).unwrap();
        w.add_file("Books/Physics/a.pdf", &src).unwrap();
        w.add_bytes("Notes/Me/n.md", b"# Note").unwrap();
        assert!(w.add_bytes("../evil", b"x").is_err());
        assert!(w.add_bytes("Notes/Me/n.md", b"again").is_err());
        assert!(!dest.exists(), "only the .part file exists until finished");
        w.finish(manifest()).unwrap();
        assert!(dest.exists());
        assert!(!part_path(&dest).exists());

        let mut r = ArchiveReader::open(&dest).unwrap();
        assert_eq!(r.manifest.library.name, "Home");
        assert_eq!(r.manifest.files.len(), 2);
        assert_eq!(r.read("Notes/Me/n.md").unwrap(), b"# Note");
        let out = dir.path().join("x/a.pdf");
        assert_eq!(r.extract("Books/Physics/a.pdf", &out).unwrap(), 14);
        assert_eq!(fs::read(&out).unwrap(), b"%PDF-1.7 hello");
        assert_eq!(r.names().len(), 2);

        // Dropped without finishing: nothing is left behind.
        let other = dir.path().join("broken.libreri");
        let mut w = ArchiveWriter::create(&other).unwrap();
        w.add_bytes("Notes/a.md", b"x").unwrap();
        drop(w);
        assert!(!other.exists() && !part_path(&other).exists());
    }

    #[test]
    fn refuses_other_zips_and_newer_versions() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain.zip");
        {
            let mut z = zip::ZipWriter::new(File::create(&plain).unwrap());
            z.start_file("a.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(matches!(
            ArchiveReader::open(&plain),
            Err(ArchiveError::NotAnArchive)
        ));
        let text = dir.path().join("t.libreri");
        fs::write(&text, "not a zip").unwrap();
        assert!(matches!(
            ArchiveReader::open(&text),
            Err(ArchiveError::NotAnArchive)
        ));

        let newer = dir.path().join("new.libreri");
        {
            let mut z = zip::ZipWriter::new(File::create(&newer).unwrap());
            z.start_file(MANIFEST, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(br#"{"format":"libreri-archive","formatVersion":99}"#)
                .unwrap();
            z.finish().unwrap();
        }
        assert!(matches!(
            ArchiveReader::open(&newer),
            Err(ArchiveError::TooNew(99))
        ));
    }
}
