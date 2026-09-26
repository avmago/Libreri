//! Developer tool: creates a library and imports a folder into it, copying
//! the files. Handy for trying the app with many books.
//!
//! cargo run -p libreri-library --release --example seed -- <books folder> <new library folder>

use libreri_library::{ImportMode, ImportRequest, Library, NoProgress};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [source, target] = args.as_slice() else {
        eprintln!("usage: seed <books folder> <new library folder>");
        std::process::exit(2);
    };
    let library = Library::create(&PathBuf::from(target), None, "seed").expect("create library");
    let started = Instant::now();
    let sources: Vec<PathBuf> = std::fs::read_dir(source)
        .expect("read source folder")
        .flatten()
        .map(|e| e.path())
        .collect();
    let report = library
        .import(
            &ImportRequest {
                sources,
                folder: String::new(),
                mode: ImportMode::Copy,
            },
            &NoProgress,
        )
        .expect("import");
    println!(
        "imported {} books in {:.2?} ({} duplicates, {} unsupported, {} failed, {} warnings)",
        report.added_ids.len(),
        started.elapsed(),
        report.duplicates.len(),
        report.unsupported,
        report.failed.len(),
        report.warnings.len()
    );
    for (file, why) in &report.failed {
        println!("  failed: {file}: {why}");
    }
    library.close().expect("close");
}
