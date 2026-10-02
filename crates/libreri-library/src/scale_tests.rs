//! Libraries of 10,000+ books (Phase 10): how long the everyday work takes.
//! Run with `cargo test -p libreri-library --release scale -- --ignored --nocapture`.

use crate::testutil::*;
use crate::NoProgress;
use libreri_core::BookQuery;
use std::time::Instant;

fn time<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    eprintln!("{what:<34} {:>8.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    out
}

#[test]
#[ignore = "slow: 10,000 books"]
fn scale_ten_thousand_books() {
    let (_d, lib) = library();
    let books = lib.layout().books_dir();
    for i in 0..10_000 {
        md_book(
            &books,
            &format!("Shelf {}/Book {i}.md", i % 50),
            &format!("Book number {i}"),
        );
    }
    time("first scan (10,000 new)", || lib.scan(&NoProgress).unwrap());
    time("scan again (nothing changed)", || {
        lib.scan(&NoProgress).unwrap()
    });
    let all = time("list all books", || {
        lib.books(&BookQuery::default()).unwrap()
    });
    assert_eq!(all.len(), 10_000);
    time("list all, sorted by title", || {
        lib.books(&BookQuery {
            sort: libreri_core::SortKey::Title,
            ..Default::default()
        })
        .unwrap()
    });
    time("search \"number 99\"", || {
        lib.books(&BookQuery {
            search: Some("number 99".into()),
            ..Default::default()
        })
        .unwrap()
    });
    time("one folder", || {
        lib.books(&BookQuery {
            folder: Some("Shelf 7".into()),
            ..Default::default()
        })
        .unwrap()
    });
    time("facets (sidebar counts)", || lib.facets().unwrap());
    time("folders tree", || lib.folders().unwrap());
    time("rebuild index", || lib.rebuild_index(&NoProgress).unwrap());
}
