//! Tries feeds on the internet: `cargo run -p libreri-feeds --example live [urls…]`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let urls: Vec<String> = if args.is_empty() {
        let mut v = vec![libreri_feeds::sources::arxiv_feed_url("cs.AI")];
        v.push(libreri_feeds::sources::arxiv_search_url("diffusion models").unwrap());
        v.extend(
            libreri_feeds::sources::suggested()
                .iter()
                .map(|s| s.url.to_owned()),
        );
        v
    } else {
        args
    };
    for u in urls {
        match libreri_feeds::fetch::discover(&u) {
            Ok(f) => {
                let e = f.parsed.entries.first();
                println!(
                    "OK   {} → {} | {} items | first: {:?} | authors {:?} | pdf {:?} | topics {:?} | date {:?} | summary {:.60}",
                    u,
                    f.parsed.title,
                    f.parsed.entries.len(),
                    e.map(|e| &e.title),
                    e.map(|e| e.authors.iter().take(3).collect::<Vec<_>>()),
                    e.and_then(|e| e.pdf.as_ref()),
                    e.map(|e| e.topics.iter().take(3).collect::<Vec<_>>()),
                    e.and_then(|e| e.published.as_ref()),
                    e.map(|e| e.summary.as_str()).unwrap_or(""),
                );
            }
            Err(e) => println!("FAIL {u}: {e}"),
        }
    }
}
