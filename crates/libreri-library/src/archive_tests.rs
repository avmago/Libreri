//! Round trips through Libreri archives: every note must still open the
//! same place in the same book in another library, at another path
//! (docs/data-portability.md, "Verification").

use crate::testutil::*;
use crate::*;
use libreri_core::{Annotation, AnnotationKind, BookId, BookQuery, ProfileKind, TextQuote};
use libreri_export::archive::ArchiveReader;
use libreri_export::ExportFormat;
use std::fs;
use std::path::Path;

const H1: &str = "0b7f5a3e-1f7e-4d4c-9d34-5d0a8d8f2c10";
const H2: &str = "5c1d8a2e-3b4f-4e6a-8c9d-0e1f2a3b4c5d";
const PIN: &str = "284715";

fn highlight(book: &BookId, id: &str, note: &str) -> Annotation {
    Annotation {
        id: id.into(),
        book_id: book.clone(),
        kind: AnnotationKind::Highlight,
        color: None,
        locator: r#"{"type":"text","start":1,"end":5}"#.into(),
        quote: Some(TextQuote {
            exact: "Body".into(),
            ..Default::default()
        }),
        note: Some(note.into()),
        label: Some("p. 1".into()),
        position: 0.1,
        created_at: String::new(),
        modified_at: String::new(),
    }
}

fn by_title(lib: &Library, title: &str) -> libreri_core::Book {
    lib.books(&BookQuery::default())
        .unwrap()
        .into_iter()
        .find(|b| b.metadata.title == title)
        .unwrap_or_else(|| panic!("no book {title}"))
}

fn owner(lib: &Library) -> libreri_core::Profile {
    lib.profiles()
        .unwrap()
        .into_iter()
        .find(|p| p.kind == ProfileKind::Owner)
        .unwrap()
}

/// Library A: two books, the owner's highlight, notebook, rating and
/// collection, and Sam (with a PIN) who highlighted too.
fn source() -> (tempfile::TempDir, Library, BookId) {
    let (dir, lib) = library();
    md_book(&lib.layout().books_dir(), "Physics/optics.md", "Optics");
    md_book(&lib.layout().books_dir(), "waves.md", "Waves");
    lib.scan(&NoProgress).unwrap();
    let optics = by_title(&lib, "Optics");
    let mut m = optics.metadata.clone();
    m.authors = vec!["Jane Smith".into()];
    m.isbn13 = Some("9780131103627".into());
    m.tags = vec!["Light".into()];
    lib.update_metadata(&optics.id, m).unwrap();
    lib.save_annotation(highlight(&optics.id, H1, "Owner's note"))
        .unwrap();
    let nb = lib.notebook(&optics.id).unwrap();
    lib.save_notebook(&optics.id, &format!("{}My thoughts\n", nb.content))
        .unwrap();
    let mut state = lib.book(&optics.id).unwrap().user;
    state.rating = 5;
    lib.set_user_state(&optics.id, &state).unwrap();
    lib.save_collection(
        "",
        "Light",
        &BookQuery {
            search: Some("light".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let sam = lib
        .create_profile("Sam", "blue", ProfileKind::Standard, Some(PIN))
        .unwrap();
    lib.sign_in(&sam.id, Some(PIN)).unwrap();
    lib.save_annotation(highlight(&optics.id, H2, "Sam's note"))
        .unwrap();
    lib.sign_out().unwrap();
    lib.sign_in(&owner(&lib).id, None).unwrap();
    // OCR text read earlier travels with the book.
    let ocr = OcrText {
        format_version: 1,
        engine: "tesseract 5".into(),
        languages: vec!["eng".into()],
        updated_at: String::new(),
        pages: vec![libreri_formats::ocr::OcrPage {
            page: 1,
            text: "Rays of light".into(),
            ..Default::default()
        }],
    };
    lib.save_ocr(&optics.id, &ocr).unwrap();
    (dir, lib, optics.id)
}

fn export(lib: &Library, dest: &Path, files: bool, everyone: bool) {
    lib.export(
        &ExportRequest {
            format: ExportFormat::Archive,
            books: None,
            dest: dest.to_path_buf(),
            personal: true,
            notes: true,
            book_files: files,
            everyone,
            app_version: V.into(),
        },
        &NoProgress,
    )
    .unwrap();
}

fn new_library(dir: &Path, name: &str) -> Library {
    Library::create(&dir.join(name), None, V).unwrap()
}

#[test]
fn everything_comes_back_in_a_new_library_at_another_path() {
    let (dir, a, optics) = source();
    let archive = dir.path().join("all.libreri");
    export(&a, &archive, true, true);

    let manifest = ArchiveReader::open(&archive).unwrap().manifest;
    assert_eq!(manifest.books.len(), 2);
    assert_eq!(manifest.notes.len(), 2);
    assert!(manifest.notes.iter().any(|n| n.link.ends_with(H1)));
    assert!(!manifest.includes_pins);
    assert!(manifest
        .files
        .iter()
        .any(|f| f.path == "database/library.db"));

    let b = new_library(dir.path(), "elsewhere/B");
    let summary = b.inspect_archive(&archive).unwrap();
    assert_eq!(
        (summary.books, summary.with_file, summary.missing),
        (2, 2, 0)
    );
    let owner_choice = summary
        .profiles
        .iter()
        .find(|p| p.archive.kind == ProfileKind::Owner)
        .unwrap();
    assert_eq!(
        owner_choice.suggestion,
        ProfileTarget::Existing(owner(&b).id),
        "the archive's owner goes to this library's owner"
    );

    let r = b
        .import_archive(&archive, &ArchiveImport::default(), &NoProgress)
        .unwrap();
    assert_eq!(r.added, 2);
    assert_eq!(r.notes_added, 2);
    assert_eq!(r.profiles_created, vec!["Sam".to_owned()]);

    // The same book (same id), same details, same notes, same links.
    let book = b.book(&optics).unwrap();
    assert_eq!(book.rel_path, "Books/Physics/optics.md");
    assert_eq!(book.metadata.isbn13.as_deref(), Some("9780131103627"));
    assert_eq!(book.user.rating, 5);
    let notes = b.annotations(&optics).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, H1);
    assert_eq!(
        notes[0].link(),
        format!("libreri://book/{optics}#annotation={H1}")
    );
    let nb = b.notebook(&optics).unwrap();
    assert!(nb.content.contains("My thoughts"), "{}", nb.content);
    let ocr = b.ocr_text(&optics).unwrap().expect("OCR text came along");
    assert_eq!(ocr.pages[0].text, "Rays of light");
    assert_eq!(b.collections().unwrap()[0].name, "Light");

    // Sam's notes went to a new Sam, without the PIN (exports never carry it).
    let sam = b
        .profiles()
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Sam")
        .unwrap();
    assert!(!sam.has_pin());
    b.sign_in(&sam.id, None).unwrap();
    assert_eq!(b.annotations(&optics).unwrap()[0].id, H2);

    // Importing again changes nothing.
    b.sign_out().unwrap();
    b.sign_in(&owner(&b).id, None).unwrap();
    let again = b
        .import_archive(&archive, &ArchiveImport::default(), &NoProgress)
        .unwrap();
    assert_eq!((again.added, again.linked, again.notes_added), (0, 2, 0));
    assert_eq!(again.notes_kept, 2);
    assert!(again.note_conflicts.is_empty());
    assert!(again.profiles_created.is_empty());
    assert_eq!(b.annotations(&optics).unwrap().len(), 1);
    assert_eq!(b.books(&BookQuery::default()).unwrap().len(), 2);

    // A rebuilt index keeps it all.
    b.rebuild_index(&NoProgress).unwrap();
    assert_eq!(b.annotations(&optics).unwrap().len(), 1);
}

#[test]
fn backups_keep_pins_and_restore_the_owner() {
    let (dir, a, optics) = source();
    let backup = dir.path().join("backup.libreri");
    a.backup_to(&backup, false, V, &NoProgress).unwrap();
    assert!(ArchiveReader::open(&backup).unwrap().manifest.includes_pins);

    let c = new_library(dir.path(), "C");
    let r = c
        .import_archive(
            &backup,
            &ArchiveImport {
                adopt_owner: true,
                ..Default::default()
            },
            &NoProgress,
        )
        .unwrap();
    assert_eq!(r.missing, 2, "no files in this backup");
    assert_eq!(
        c.profiles().unwrap().iter().filter(|p| p.has_pin()).count(),
        1
    );
    assert_eq!(owner(&c).name, owner(&a).name);

    // Notes are kept for the missing book, and reconnect when it comes back.
    let book = c.book(&optics).unwrap();
    assert!(book.missing);
    assert_eq!(c.annotations(&optics).unwrap().len(), 1);
    let file = dir.path().join("optics.md");
    fs::copy(a.layout().books_dir().join("Physics/optics.md"), &file).unwrap();
    let rep = c
        .import(
            &ImportRequest {
                sources: vec![file],
                folder: String::new(),
                mode: ImportMode::Copy,
            },
            &NoProgress,
        )
        .unwrap();
    assert_eq!(rep.relinked, 1);
    assert!(!c.book(&optics).unwrap().missing);
    assert_eq!(c.annotations(&optics).unwrap()[0].id, H1);
}

#[test]
fn notes_move_to_another_copy_of_the_same_book() {
    let (dir, a, optics) = source();
    let archive = dir.path().join("mine.libreri");
    export(&a, &archive, false, false);

    // Library D has Optics too, but a different file (another edition).
    let d = new_library(dir.path(), "D");
    fs::create_dir_all(d.layout().books_dir()).unwrap();
    fs::write(
        d.layout().books_dir().join("optics-2e.md"),
        "---\ntitle: Optics\n---\nSecond edition\n",
    )
    .unwrap();
    d.scan(&NoProgress).unwrap();
    let here = by_title(&d, "Optics");
    let mut m = here.metadata.clone();
    m.isbn13 = Some("9780131103627".into());
    d.update_metadata(&here.id, m).unwrap();
    assert_ne!(here.id, optics);

    let s = d.inspect_archive(&archive).unwrap();
    assert_eq!((s.other_file, s.missing), (1, 1));
    let r = d
        .import_archive(&archive, &ArchiveImport::default(), &NoProgress)
        .unwrap();
    assert_eq!(r.other_file, 1);
    assert!(r.profiles_created.is_empty(), "only the owner's own notes");
    let notes = d.annotations(&here.id).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].quote.as_ref().unwrap().exact, "Body");
    // Old links still open the book here.
    assert_eq!(
        d.with_db(|db| db.resolve_book_id(&optics)).unwrap(),
        Some(here.id.clone())
    );
    // The old id survives a rebuild (it is in the sidecar).
    d.rebuild_index(&NoProgress).unwrap();
    assert_eq!(
        d.with_db(|db| db.resolve_book_id(&optics)).unwrap(),
        Some(here.id)
    );
}

#[test]
fn only_the_owner_imports_and_bad_files_are_refused() {
    let (dir, a, _) = source();
    let bad = dir.path().join("bad.libreri");
    fs::write(&bad, "nope").unwrap();
    assert!(matches!(
        a.inspect_archive(&bad),
        Err(Error::InvalidInput(_))
    ));

    let archive = dir.path().join("a.libreri");
    export(&a, &archive, false, true);
    let sam = a
        .profiles()
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Sam")
        .unwrap();
    a.sign_in(&sam.id, Some(PIN)).unwrap();
    assert!(a.inspect_archive(&archive).is_err());
}

#[test]
fn earlier_versions_travel_with_the_book_files() {
    let (dir, a) = library();
    let books = a.layout().books_dir();
    libreri_formats::test_text_pdf(&books.join("report.pdf"), &["first draft"]);
    a.scan(&NoProgress).unwrap();
    let old = a.books(&BookQuery::default()).unwrap().remove(0);
    let edited = dir.path().join("edited.pdf");
    libreri_formats::test_text_pdf(&edited, &["second draft"]);
    let new = a
        .save_version(
            &old.id,
            NewVersion {
                file: &edited,
                reason: "Edited pages",
                pages: None,
                cover_changed: false,
            },
        )
        .unwrap();

    // Without book files, no versions either.
    let light = dir.path().join("light.libreri");
    export(&a, &light, false, false);
    let manifest_name = format!(".library-data/versions/{}/versions.json", new.id);
    assert!(!ArchiveReader::open(&light).unwrap().has(&manifest_name));

    let archive = dir.path().join("all.libreri");
    export(&a, &archive, true, false);
    assert!(ArchiveReader::open(&archive).unwrap().has(&manifest_name));
    let b = new_library(dir.path(), "B");
    b.import_archive(&archive, &ArchiveImport::default(), &NoProgress)
        .unwrap();
    let versions = b.versions(&new.id).unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].id, old.id);
    assert_eq!(versions[0].reason, "Edited pages");
    let file = b.version_path(&new.id, &old.id).unwrap();
    assert_eq!(paths::hash_file(&file).unwrap(), old.id);
}
