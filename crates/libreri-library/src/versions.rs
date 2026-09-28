//! Earlier versions of edited books (Phase 6b).
//!
//! When Libreri changes a book's file (page edits, redaction, a filled-in
//! form, markup saved into the PDF), the old file is kept in
//! `.library-data/versions/<current book id>/<old id>.<ext>`, listed in
//! `versions.json` next to it. The book gets a new id (its content hash,
//! ADR 0004); the old id stays as an alias, so links keep working.
//! Versions are kept until the user deletes them, and travel in backups
//! and exports that include book files.
//!
//! Notes, markup and reading positions follow their pages: each version
//! records how its pages became the next version's pages (`next`), so
//! restoring an older version can send them back. Notes on pages that were
//! removed are kept with the version and come back when it is restored.

use crate::paths::{self, write_atomic};
use crate::{covers, sidecar, Error, Library, Result};
use libreri_core::{Annotation, Book, BookId, FileType, ProfileId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "versions.json";
const MANIFEST_VERSION: u32 = 1;

/// One change to a page's geometry, applied in order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Step {
    /// Turned clockwise by `deg`.
    Rotate { deg: i32 },
    /// Cut to x, y, w, h (fractions of the page as it was).
    Crop { r: [f64; 4] },
    /// The opposite of a crop.
    Uncrop { r: [f64; 4] },
}

impl Step {
    fn inverse(self) -> Step {
        match self {
            Step::Rotate { deg } => Step::Rotate { deg: -deg },
            Step::Crop { r } => Step::Uncrop { r },
            Step::Uncrop { r } => Step::Crop { r },
        }
    }

    fn point(self, (x, y): (f64, f64)) -> (f64, f64) {
        match self {
            Step::Rotate { deg } => match deg.rem_euclid(360) / 90 {
                1 => (1.0 - y, x),
                2 => (1.0 - x, 1.0 - y),
                3 => (y, 1.0 - x),
                _ => (x, y),
            },
            Step::Crop {
                r: [cx, cy, cw, ch],
            } => ((x - cx) / cw.max(1e-6), (y - cy) / ch.max(1e-6)),
            Step::Uncrop {
                r: [cx, cy, cw, ch],
            } => (x * cw + cx, y * ch + cy),
        }
    }
}

/// Page `from` of one version is page `to` of the next.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLink {
    pub from: u32,
    pub to: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
}

/// How the pages of one file became the pages of the next.
pub type PageMap = Vec<PageLink>;

/// The page map of a page edit.
pub fn page_map(moves: &[libreri_pdf_edit::PageMove]) -> PageMap {
    moves
        .iter()
        .filter_map(|m| {
            let from = m.from?;
            let mut steps = Vec::new();
            if m.rotate.rem_euclid(360) != 0 {
                steps.push(Step::Rotate { deg: m.rotate });
            }
            if let Some(r) = m.crop {
                steps.push(Step::Crop { r });
            }
            Some(PageLink {
                from,
                to: m.to,
                steps,
            })
        })
        .collect()
}

fn compose(a: Option<&PageMap>, b: Option<&PageMap>) -> Option<PageMap> {
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (Some(a), Some(b)) => Some(
            a.iter()
                .flat_map(|x| {
                    b.iter()
                        .filter(move |y| y.from == x.to)
                        .map(move |y| PageLink {
                            from: x.from,
                            to: y.to,
                            steps: x.steps.iter().chain(&y.steps).copied().collect(),
                        })
                })
                .collect(),
        ),
    }
}

fn invert(map: Option<&PageMap>) -> Option<PageMap> {
    map.map(|m| {
        m.iter()
            .map(|l| PageLink {
                from: l.to,
                to: l.from,
                steps: l.steps.iter().rev().map(|s| s.inverse()).collect(),
            })
            .collect()
    })
}

/// A note that could not follow its page (the page was removed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptNote {
    pub profile: ProfileId,
    pub annotation: Annotation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: BookId,
    /// File name inside the versions folder.
    file: String,
    /// The book's file name when this version was current.
    name: String,
    saved_at: String,
    /// Why the file changed: "Edited pages", "Filled in form", …
    reason: String,
    size: u64,
    /// How this version's pages became the next one's (None: the same).
    #[serde(default)]
    next: Option<PageMap>,
    /// Notes left behind on pages the next version does not have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    kept: Vec<KeptNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format_version: u32,
    /// Oldest first.
    versions: Vec<Entry>,
}

/// An earlier version, for the version history.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub id: BookId,
    pub file_name: String,
    pub saved_at: String,
    pub reason: String,
    pub size: u64,
    /// Library-relative path, to open or show the version.
    pub rel_path: String,
}

/// A changed file to save as the book's new version.
pub struct NewVersion<'a> {
    /// The new file (it is moved into place).
    pub file: &'a Path,
    pub reason: &'a str,
    /// How the pages moved (None: the same pages).
    pub pages: Option<PageMap>,
    /// The first page looks different, so the cover is made again.
    pub cover_changed: bool,
}

pub(crate) fn versions_dir(lib: &Library, id: &BookId) -> PathBuf {
    lib.layout().data_dir().join("versions").join(id.as_str())
}

fn read_manifest(dir: &Path) -> Manifest {
    fs::read_to_string(dir.join(MANIFEST))
        .ok()
        .and_then(|t| serde_json::from_str::<Manifest>(&t).ok())
        .filter(|m| m.format_version <= MANIFEST_VERSION)
        .unwrap_or(Manifest {
            format_version: MANIFEST_VERSION,
            versions: Vec::new(),
        })
}

fn write_manifest(dir: &Path, m: &Manifest) -> Result<()> {
    if m.versions.is_empty() {
        let _ = fs::remove_file(dir.join(MANIFEST));
        let _ = fs::remove_dir(dir);
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(m).map_err(std::io::Error::other)?;
    write_atomic(&dir.join(MANIFEST), &json)?;
    Ok(())
}

// ---------- moving positions ----------

/// Moves one page position (fractions) through the steps.
fn move_point(steps: &[Step], p: (f64, f64)) -> (f64, f64) {
    steps.iter().fold(p, |p, s| s.point(p))
}

fn move_box(steps: &[Step], b: [f64; 4]) -> [f64; 4] {
    let a = move_point(steps, (b[0], b[1]));
    let c = move_point(steps, (b[0] + b[2], b[1] + b[3]));
    [
        a.0.min(c.0),
        a.1.min(c.1),
        (a.0 - c.0).abs(),
        (a.1 - c.1).abs(),
    ]
}

fn as_f64s(v: &Value) -> Option<Vec<f64>> {
    v.as_array()?.iter().map(Value::as_f64).collect()
}

fn round(v: f64) -> Value {
    serde_json::json!((v * 10000.0).round() / 10000.0)
}

/// Rewrites a point `[x, y, …]` in place.
fn move_pt(v: &mut Value, steps: &[Step]) {
    if let Some(p) = as_f64s(v).filter(|p| p.len() >= 2) {
        let (x, y) = move_point(steps, (p[0], p[1]));
        let mut out = vec![round(x), round(y)];
        out.extend(p[2..].iter().map(|z| serde_json::json!(z)));
        *v = Value::Array(out);
    }
}

fn move_rect(v: &mut Value, steps: &[Step]) {
    if let Some(b) = as_f64s(v).filter(|b| b.len() == 4) {
        let r = move_box(steps, [b[0], b[1], b[2], b[3]]);
        *v = Value::Array(r.into_iter().map(round).collect());
    }
}

/// Page geometry around one link: how widths scale, and the new page size.
#[derive(Clone, Copy)]
struct Sizes {
    old: (f64, f64),
    new: (f64, f64),
}

/// A locator on page `from` moved to `link`'s page. `None` when it is not
/// a page locator.
fn move_locator(locator: &str, link: &PageLink, sizes: Option<Sizes>) -> Option<String> {
    let mut v: Value = serde_json::from_str(locator).ok()?;
    let obj = v.as_object_mut()?;
    obj.insert("page".into(), serde_json::json!(link.to));
    let steps = &link.steps;
    match obj.get("type").and_then(Value::as_str) {
        Some("pdf") => {
            if let Some(top) = obj.get("top").and_then(Value::as_f64) {
                let (_, y) = move_point(steps, (0.5, top));
                obj.insert("top".into(), round(y.clamp(0.0, 1.0)));
            }
        }
        Some("pdf-highlight") => {
            if let Some(Value::Array(rects)) = obj.get_mut("rects") {
                rects.iter_mut().for_each(|r| move_rect(r, steps));
            }
        }
        Some("markup") => {
            let item = obj.get_mut("item")?.as_object_mut()?;
            if let Some(Value::Array(points)) = item.get_mut("points") {
                points.iter_mut().for_each(|p| move_pt(p, steps));
            }
            for key in ["from", "to", "at"] {
                if let Some(p) = item.get_mut(key) {
                    move_pt(p, steps);
                }
            }
            if let Some(b) = item.get_mut("box") {
                move_rect(b, steps);
            }
            if let Some(s) = sizes {
                let factor = s.old.0 / s.new.0.max(1e-6);
                for key in ["width", "size"] {
                    if let Some(w) = item.get(key).and_then(Value::as_f64) {
                        item.insert(key.into(), serde_json::json!(w * factor));
                    }
                }
                if item.get("tool").and_then(Value::as_str) == Some("measure") {
                    item.insert("page".into(), serde_json::json!([s.new.0, s.new.1]));
                }
            }
        }
        _ => return None,
    }
    serde_json::to_string(&v).ok()
}

fn page_of(locator: &str) -> Option<u32> {
    let v: Value = serde_json::from_str(locator).ok()?;
    let t = v.get("type")?.as_str()?;
    if !matches!(t, "pdf" | "pdf-highlight" | "markup") {
        return None;
    }
    v.get("page")?.as_u64().map(|p| p as u32)
}

/// Where the pages of two files are, for widths that must keep their size.
struct PageSizes {
    old: Vec<(f64, f64)>,
    new: Vec<(f64, f64)>,
}

impl PageSizes {
    fn read(old: &Path, new: &Path, pdf: bool) -> Self {
        let sizes = |p: &Path| {
            if pdf {
                libreri_pdf_edit::page_sizes(p).unwrap_or_default()
            } else {
                Vec::new()
            }
        };
        PageSizes {
            old: sizes(old),
            new: sizes(new),
        }
    }

    fn at(&self, link: &PageLink) -> Option<Sizes> {
        Some(Sizes {
            old: *self.old.get(link.from as usize - 1)?,
            new: *self.new.get(link.to as usize - 1)?,
        })
    }
}

fn link_for(map: &PageMap, page: u32) -> Option<&PageLink> {
    map.iter().find(|l| l.from == page)
}

impl Library {
    /// Moves every profile's notes and reading positions from `old` to
    /// `new` pages. Returns the notes whose page is gone (already removed
    /// from the book).
    fn move_notes(&self, book: &BookId, map: &PageMap, sizes: &PageSizes) -> Result<Vec<KeptNote>> {
        let mut kept = Vec::new();
        for (mut a, profile) in self.with_db(|db| db.book_annotations(book))? {
            let Some(page) = page_of(&a.locator) else {
                continue;
            };
            match link_for(map, page) {
                Some(link) => {
                    if let Some(loc) = move_locator(&a.locator, link, sizes.at(link)) {
                        a.locator = loc;
                        if a.label.as_deref().is_some_and(|l| l.starts_with("p. ")) {
                            a.label = Some(format!("p. {}", link.to));
                        }
                        self.with_db(|db| db.save_annotation(&a, &profile))?;
                    }
                }
                None => {
                    self.with_db(|db| db.delete_annotation(&a.id))?;
                    kept.push(KeptNote {
                        profile,
                        annotation: a,
                    });
                }
            }
        }
        for (profile, pos) in self.with_db(|db| db.book_positions(book))? {
            let Some(page) = page_of(&pos) else { continue };
            // A removed page: read on from the page that took its place.
            let link = link_for(map, page).cloned().or_else(|| {
                map.iter()
                    .filter(|l| l.from > page)
                    .min_by_key(|l| l.from)
                    .map(|l| PageLink {
                        steps: Vec::new(),
                        ..l.clone()
                    })
            });
            if let Some(loc) = link.and_then(|l| move_locator(&pos, &l, None)) {
                self.with_db(|db| db.replace_position(book, &profile, &loc))?;
            }
        }
        Ok(kept)
    }

    /// Adds back notes kept with a version (their ids must be free).
    fn bring_back(
        &self,
        book: &BookId,
        notes: Vec<KeptNote>,
        map: Option<&PageMap>,
        sizes: &PageSizes,
    ) -> Result<()> {
        for k in notes {
            let mut a = k.annotation;
            if self.with_db(|db| db.annotation(&a.id))?.is_some() {
                continue;
            }
            if let (Some(map), Some(page)) = (map, page_of(&a.locator)) {
                let Some(link) = link_for(map, page) else {
                    continue;
                };
                match move_locator(&a.locator, link, sizes.at(link)) {
                    Some(loc) => a.locator = loc,
                    None => continue,
                }
            }
            a.book_id = book.clone();
            self.with_db(|db| db.save_annotation(&a, &k.profile))?;
        }
        Ok(())
    }

    /// Moves saved OCR text along with its pages.
    fn move_ocr(&self, book: &BookId, map: &PageMap) -> Result<()> {
        let Some(mut ocr) = self.ocr_text(book)? else {
            return Ok(());
        };
        let mut pages = Vec::new();
        for link in map {
            let Some(p) = ocr.page(link.from) else {
                continue;
            };
            let mut p = p.clone();
            p.page = link.to;
            if !link.steps.is_empty() {
                let cropped = link.steps.iter().any(|s| matches!(s, Step::Crop { .. }));
                p.words = p
                    .words
                    .into_iter()
                    .filter_map(|mut w| {
                        let r = move_box(&link.steps, w.rect.map(f64::from));
                        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                        if !(0.0..=1.0).contains(&cx) || !(0.0..=1.0).contains(&cy) {
                            return None;
                        }
                        w.rect = r.map(|v| ((v * 10000.0).round() / 10000.0) as f32);
                        Some(w)
                    })
                    .collect();
                if cropped {
                    p.text = p
                        .words
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                }
            }
            pages.push(p);
        }
        pages.sort_by_key(|p| p.page);
        pages.dedup_by_key(|p| p.page);
        ocr.pages = pages;
        self.save_ocr(book, &ocr)
    }

    /// Replaces the book's file with `v.file`, keeping the old one as a
    /// version. Returns the book under its new id.
    pub fn save_version(&self, id: &BookId, v: NewVersion<'_>) -> Result<Book> {
        self.require_edit()?;
        let book = self.book(id)?;
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .filter(|p| p.is_file())
            .ok_or(Error::BookNotFound)?;
        let new_id = paths::hash_file(v.file)?;
        if new_id == book.id {
            let _ = fs::remove_file(v.file);
            return Ok(book);
        }
        let taken = self.with_db(|db| db.resolve_book_id(&new_id))?;
        if taken.is_some_and(|t| t != book.id) {
            return Err(Error::InvalidInput(
                "the changed file is the same as another book in the library".into(),
            ));
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|| "bin".into());
        let old_dir = versions_dir(self, &book.id);
        let dir = versions_dir(self, &new_id);
        if old_dir.is_dir() {
            fs::create_dir_all(dir.parent().unwrap_or(&dir))?;
            fs::rename(&old_dir, &dir)?;
        }
        fs::create_dir_all(&dir)?;
        let file = format!("{}.{ext}", book.id);
        let kept_copy = dir.join(&file);
        fs::copy(&path, &kept_copy)?;
        let size = fs::metadata(&kept_copy)?.len();

        // Put the new file in place.
        let staged = path.with_extension(format!("{ext}.libreri-new"));
        paths::move_file(v.file, &staged)?;
        fs::rename(&staged, &path)?;
        let meta = fs::metadata(&path)?;
        let old = book.id.clone();
        self.with_db(|db| {
            db.change_book_id(
                &old,
                &new_id,
                meta.len(),
                paths::mtime_secs(&meta),
                &crate::now(),
            )
        })?;
        sidecar::rename(self.layout(), &old, &new_id);
        crate::text::rename(self.layout(), &old, &new_id);
        self.rename_annotation_backups(&old, &new_id);
        if v.cover_changed {
            let _ = fs::remove_file(covers::cover_path(self.layout(), &old));
            let _ = fs::remove_file(covers::thumbnail_path(self.layout(), &old));
            self.with_db(|db| db.set_has_cover(&new_id, false, &crate::now()))?;
        } else {
            covers::rename(self.layout(), &old, &new_id);
        }

        let mut kept = Vec::new();
        if let Some(map) = &v.pages {
            let sizes = PageSizes::read(&kept_copy, &path, book.file_type == FileType::Pdf);
            kept = self.move_notes(&new_id, map, &sizes)?;
            self.move_ocr(&new_id, map)?;
        }
        let mut m = read_manifest(&dir);
        m.versions.push(Entry {
            id: old,
            file,
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            saved_at: crate::now(),
            reason: v.reason.to_owned(),
            size,
            next: v.pages,
            kept,
        });
        write_manifest(&dir, &m)?;
        self.after_change(&new_id)?;
        self.record(&new_id)
    }

    /// Rewrites the sidecar and every personal backup of a book.
    fn after_change(&self, id: &BookId) -> Result<()> {
        sidecar::write(self, &self.record(id)?)?;
        let profiles: HashSet<ProfileId> = self
            .with_db(|db| db.book_annotations(id))?
            .into_iter()
            .map(|(_, p)| p)
            .chain(
                self.with_db(|db| db.book_positions(id))?
                    .into_iter()
                    .map(|(p, _)| p),
            )
            .collect();
        let dir = self.layout().data_dir().join("annotations");
        for p in profiles {
            // Only profiles that keep backups have a folder there.
            if dir.join(p.to_string()).is_dir() {
                self.backup_personal_of(&p, id)?;
            }
        }
        Ok(())
    }

    /// The book's earlier versions, newest first.
    pub fn versions(&self, id: &BookId) -> Result<Vec<VersionInfo>> {
        let book = self.book(id)?;
        let dir = versions_dir(self, &book.id);
        let m = read_manifest(&dir);
        Ok(m.versions
            .iter()
            .rev()
            .filter(|e| dir.join(&e.file).is_file())
            .map(|e| VersionInfo {
                id: e.id.clone(),
                file_name: e.name.clone(),
                saved_at: e.saved_at.clone(),
                reason: e.reason.clone(),
                size: e.size,
                rel_path: paths::rel_of(self.layout(), &dir.join(&e.file)).unwrap_or_default(),
            })
            .collect())
    }

    /// Makes an earlier version the current file again. The current file
    /// becomes a version, so nothing is lost.
    pub fn restore_version(&self, id: &BookId, version: &BookId) -> Result<Book> {
        self.require_edit()?;
        let book = self.book(id)?;
        let dir = versions_dir(self, &book.id);
        let mut m = read_manifest(&dir);
        let k = m
            .versions
            .iter()
            .position(|e| &e.id == version)
            .ok_or_else(|| Error::InvalidInput("that version is gone".into()))?;
        let src = dir.join(&m.versions[k].file);
        if !src.is_file() {
            return Err(Error::InvalidInput("that version's file is gone".into()));
        }
        // Pages: current → version k is the inverse of k → … → current.
        let mut forward: Option<PageMap> = None;
        let mut identity = true;
        for e in &m.versions[k..] {
            identity &= e.next.is_none();
            forward = compose(forward.as_ref(), e.next.as_ref());
        }
        let back = if identity {
            None
        } else {
            invert(forward.as_ref())
        };
        // Notes kept with the versions from k on, as seen from version k.
        let mut returning: Vec<(Vec<KeptNote>, Option<PageMap>)> = Vec::new();
        for j in k..m.versions.len() {
            let mut to_k: Option<PageMap> = None;
            let mut same = true;
            for e in m.versions[k..j].iter().rev() {
                same &= e.next.is_none();
                to_k = compose(to_k.as_ref(), invert(e.next.as_ref()).as_ref());
            }
            returning.push((
                std::mem::take(&mut m.versions[j].kept),
                if same { None } else { to_k },
            ));
        }
        let restored = m.versions.remove(k);
        // The version before the restored one now leads to the one after it.
        if k > 0 {
            let before = &mut m.versions[k - 1];
            before.next = compose(before.next.as_ref(), restored.next.as_ref());
        }
        write_manifest(&dir, &m)?;

        // Copy it out, then save it like any other change.
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .ok_or(Error::BookNotFound)?;
        let tmp = path.with_extension("libreri-restore");
        fs::copy(&src, &tmp)?;
        let new = self.save_version(
            &book.id,
            NewVersion {
                file: &tmp,
                reason: &format!(
                    "Before restoring the version of {}",
                    short_date(&restored.saved_at)
                ),
                pages: back,
                cover_changed: true,
            },
        );
        if new.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        let new = new?;
        let _ = fs::remove_file(versions_dir(self, &new.id).join(&restored.file));
        let no_sizes = PageSizes {
            old: Vec::new(),
            new: Vec::new(),
        };
        for (notes, map) in returning {
            self.bring_back(&new.id, notes, map.as_ref(), &no_sizes)?;
        }
        self.after_change(&new.id)?;
        self.record(&new.id)
    }

    /// Deletes one earlier version for good.
    pub fn delete_version(&self, id: &BookId, version: &BookId) -> Result<()> {
        self.require_edit()?;
        let book = self.book(id)?;
        let dir = versions_dir(self, &book.id);
        let mut m = read_manifest(&dir);
        let Some(k) = m.versions.iter().position(|e| &e.id == version) else {
            return Ok(());
        };
        let gone = m.versions.remove(k);
        if k > 0 {
            let before = &mut m.versions[k - 1];
            before.next = compose(before.next.as_ref(), gone.next.as_ref());
        }
        let _ = fs::remove_file(dir.join(&gone.file));
        write_manifest(&dir, &m)
    }

    /// The file of an earlier version.
    pub fn version_path(&self, id: &BookId, version: &BookId) -> Result<PathBuf> {
        let book = self.book(id)?;
        let dir = versions_dir(self, &book.id);
        read_manifest(&dir)
            .versions
            .iter()
            .find(|e| &e.id == version)
            .map(|e| dir.join(&e.file))
            .filter(|p| p.is_file())
            .ok_or_else(|| Error::InvalidInput("that version is gone".into()))
    }

    /// How many earlier versions the library keeps, and their bytes.
    pub fn versions_usage(&self) -> (u32, u64) {
        let mut count = 0;
        let mut bytes = 0;
        for e in walkdir::WalkDir::new(self.layout().data_dir().join("versions"))
            .into_iter()
            .flatten()
        {
            if e.file_type().is_file() && e.file_name() != MANIFEST {
                count += 1;
                bytes += e.metadata().map_or(0, |m| m.len());
            }
        }
        (count, bytes)
    }

    /// Deletes every earlier version of every book.
    pub fn delete_all_versions(&self) -> Result<u32> {
        self.require_edit()?;
        let (count, _) = self.versions_usage();
        let dir = self.layout().data_dir().join("versions");
        if dir.is_dir() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
        Ok(count)
    }

    /// Files to put in an archive for one book: (name inside the archive,
    /// file on disk).
    pub(crate) fn version_files(&self, id: &BookId) -> Vec<(String, PathBuf)> {
        let dir = versions_dir(self, id);
        let m = read_manifest(&dir);
        if m.versions.is_empty() {
            return Vec::new();
        }
        let base = format!("{}/versions/{id}", libreri_core::layout::DATA_DIR);
        let mut out = vec![(format!("{base}/{MANIFEST}"), dir.join(MANIFEST))];
        out.extend(
            m.versions
                .iter()
                .map(|e| (format!("{base}/{}", e.file), dir.join(&e.file)))
                .filter(|(_, p)| p.is_file()),
        );
        out
    }

    /// Adds versions from an archive (manifest JSON and a way to fetch each
    /// file) to a book's versions, skipping ones already here.
    pub(crate) fn merge_versions(
        &self,
        id: &BookId,
        manifest: &[u8],
        mut fetch: impl FnMut(&str, &Path) -> Result<()>,
    ) -> Result<u32> {
        let Ok(incoming) = serde_json::from_slice::<Manifest>(manifest) else {
            return Ok(0);
        };
        if incoming.format_version > MANIFEST_VERSION {
            return Ok(0);
        }
        let dir = versions_dir(self, id);
        let mut m = read_manifest(&dir);
        let mut added = 0;
        for e in incoming.versions {
            let safe = Path::new(&e.file)
                .file_name()
                .is_some_and(|n| n.to_string_lossy() == e.file);
            if !safe || m.versions.iter().any(|x| x.id == e.id) || &e.id == id {
                continue;
            }
            fs::create_dir_all(&dir)?;
            let dest = dir.join(&e.file);
            if fetch(&e.file, &dest).is_err() || paths::hash_file(&dest)? != e.id {
                let _ = fs::remove_file(&dest);
                continue;
            }
            m.versions.push(e);
            added += 1;
        }
        m.versions.sort_by(|a, b| a.saved_at.cmp(&b.saved_at));
        write_manifest(&dir, &m)?;
        Ok(added)
    }

    /// Version folders of books no longer in the library: (count, bytes).
    pub(crate) fn unused_versions(&self, known: &HashSet<String>) -> Vec<(PathBuf, u64)> {
        let Ok(entries) = fs::read_dir(self.layout().data_dir().join("versions")) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|e| !known.contains(&e.file_name().to_string_lossy().into_owned()))
            .map(|e| {
                let bytes = walkdir::WalkDir::new(e.path())
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f.metadata().ok())
                    .filter(|m| m.is_file())
                    .map(|m| m.len())
                    .sum();
                (e.path(), bytes)
            })
            .collect()
    }
}

fn short_date(when: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(when)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| when.chars().take(16).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::library;
    use libreri_core::{AnnotationKind, BookQuery};
    use libreri_pdf_edit::{EditPlan, OutPage};

    fn pdf(dir: &Path, pages: &[&str]) -> PathBuf {
        let p = dir.join("book.pdf");
        libreri_formats::test_text_pdf(&p, pages);
        p
    }

    fn note(book: &BookId, page: u32, locator: Value) -> Annotation {
        Annotation {
            id: uuid::Uuid::new_v4().to_string(),
            book_id: book.clone(),
            kind: AnnotationKind::Bookmark,
            color: None,
            locator: {
                let mut l = locator;
                l["page"] = serde_json::json!(page);
                l.to_string()
            },
            quote: None,
            note: Some(format!("on {page}")),
            label: Some(format!("p. {page}")),
            position: 0.0,
            created_at: String::new(),
            modified_at: String::new(),
        }
    }

    #[test]
    fn steps_move_points() {
        let r = Step::Rotate { deg: 90 };
        assert_eq!(r.point((0.0, 0.0)), (1.0, 0.0));
        assert_eq!(r.inverse().point(r.point((0.2, 0.7))), (0.2, 0.7));
        let c = Step::Crop {
            r: [0.5, 0.5, 0.5, 0.5],
        };
        assert_eq!(c.point((0.75, 0.5)), (0.5, 0.0));
        let map = vec![PageLink {
            from: 2,
            to: 1,
            steps: vec![r, c],
        }];
        let back = invert(Some(&map)).unwrap();
        let p = move_point(&back[0].steps, move_point(&map[0].steps, (0.9, 0.1)));
        assert!((p.0 - 0.9).abs() < 1e-9 && (p.1 - 0.1).abs() < 1e-9);
        let two = compose(
            Some(&map),
            Some(&vec![PageLink {
                from: 1,
                to: 3,
                steps: vec![],
            }]),
        )
        .unwrap();
        assert_eq!((two[0].from, two[0].to, two[0].steps.len()), (2, 3, 2));
    }

    #[test]
    fn markup_follows_turned_pages() {
        let link = PageLink {
            from: 3,
            to: 1,
            steps: vec![Step::Rotate { deg: 90 }],
        };
        let loc = r##"{"type":"markup","page":3,"layer":"Markup","item":{"tool":"rect","box":[0.1,0.2,0.3,0.1],"width":0.01,"color":"#000","opacity":1,"dash":"solid","fill":null}}"##;
        let sizes = Sizes {
            old: (600.0, 800.0),
            new: (800.0, 600.0),
        };
        let out: Value =
            serde_json::from_str(&move_locator(loc, &link, Some(sizes)).unwrap()).unwrap();
        assert_eq!(out["page"], 1);
        assert_eq!(out["item"]["box"], serde_json::json!([0.7, 0.1, 0.1, 0.3]));
        assert_eq!(out["item"]["width"], serde_json::json!(0.0075));
    }

    #[test]
    fn a_new_version_keeps_the_old_file_and_moves_the_notes() {
        let (dir, lib) = library();
        pdf(&lib.layout().books_dir(), &["one", "two", "three"]);
        lib.scan(&crate::NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap()[0].clone();
        let n3 = lib
            .save_annotation(note(&book.id, 3, serde_json::json!({"type":"pdf"})))
            .unwrap();
        let n2 = lib
            .save_annotation(note(&book.id, 2, serde_json::json!({"type":"pdf"})))
            .unwrap();

        // Three, one (turned); two is removed.
        let plan = EditPlan {
            pages: vec![
                OutPage::Page {
                    page: 3,
                    rotate: 0,
                    crop: None,
                },
                OutPage::Page {
                    page: 1,
                    rotate: 90,
                    crop: None,
                },
            ],
            ..Default::default()
        };
        let path = lib.layout().resolve_relative(&book.rel_path).unwrap();
        let out = dir.path().join("edited.pdf");
        let report = libreri_pdf_edit::apply(&path, &plan, &[], &out).unwrap();
        let new = lib
            .save_version(
                &book.id,
                NewVersion {
                    file: &out,
                    reason: "Edited pages",
                    pages: Some(page_map(&report.page_map)),
                    cover_changed: true,
                },
            )
            .unwrap();
        assert_ne!(new.id, book.id);
        // The old id still finds the book.
        assert_eq!(lib.book(&book.id).unwrap().id, new.id);
        let notes = lib.annotations(&new.id).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, n3.id);
        assert_eq!(page_of(&notes[0].locator), Some(1));
        assert_eq!(notes[0].label.as_deref(), Some("p. 1"));
        let versions = lib.versions(&new.id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].id, book.id);
        assert!(lib
            .layout()
            .resolve_relative(&versions[0].rel_path)
            .unwrap()
            .is_file());
        assert_eq!(lib.versions_usage().0, 1);

        // Restoring brings back the pages and the note on page two.
        let back = lib.restore_version(&new.id, &book.id).unwrap();
        assert_eq!(back.id, book.id);
        let mut pages: Vec<(String, Option<u32>)> = lib
            .annotations(&back.id)
            .unwrap()
            .into_iter()
            .map(|a| (a.id.clone(), page_of(&a.locator)))
            .collect();
        pages.sort();
        let mut want = vec![(n3.id.clone(), Some(3)), (n2.id.clone(), Some(2))];
        want.sort();
        assert_eq!(pages, want);
        // The edited file is now a version itself.
        let versions = lib.versions(&back.id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].id, new.id);
        lib.delete_version(&back.id, &new.id).unwrap();
        assert!(lib.versions(&back.id).unwrap().is_empty());
        assert_eq!(lib.versions_usage(), (0, 0));
    }
}
