//! Anki decks (`.apkg`): review cards made from highlights, for people who
//! review in Anki. The file is a zip of `collection.anki2` (the older
//! SQLite collection every Anki version imports) and an empty media list.
//! Two note types: "Libreri" (front, back, source) and "Libreri Cloze"
//! (text with `{{c1::…}}`, back extra, source).

use rusqlite::{params, Connection};
use serde::Deserialize;
use std::io::Write;
use std::path::Path;

/// One note to write.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnkiNote {
    /// A stable id (the card's key in Libreri), so importing again updates
    /// the note instead of adding it twice.
    pub id: String,
    /// "basic" or "cloze".
    pub model: String,
    /// Basic: front, back, source. Cloze: text, back extra, source (HTML).
    pub fields: Vec<String>,
    pub tags: Vec<String>,
}

const BASIC_ID: i64 = 1_727_000_000_001;
const CLOZE_ID: i64 = 1_727_000_000_002;

const CSS: &str = ".card { font-family: Georgia, serif; font-size: 20px; line-height: 1.55; text-align: left; color: #18181b; background: #fff; max-width: 40em; margin: 0 auto; }\n.src { margin-top: 1em; font: 13px -apple-system, system-ui, sans-serif; color: #71717a; }\n.cloze { font-weight: bold; color: #2563eb; }\n.nightMode .card { color: #e4e4e7; background: #18181b; }";

fn field(name: &str, ord: usize) -> serde_json::Value {
    serde_json::json!({
        "name": name, "ord": ord, "sticky": false, "rtl": false,
        "font": "Arial", "size": 20, "media": []
    })
}

fn model(id: i64, name: &str, cloze: bool, deck: i64, now: i64) -> serde_json::Value {
    let (flds, tmpl) = if cloze {
        (
            vec![field("Text", 0), field("Back Extra", 1), field("Source", 2)],
            serde_json::json!({
                "name": "Cloze", "ord": 0,
                "qfmt": "{{cloze:Text}}",
                "afmt": "{{cloze:Text}}<br>{{Back Extra}}<div class=src>{{Source}}</div>",
                "did": null, "bqfmt": "", "bafmt": ""
            }),
        )
    } else {
        (
            vec![field("Front", 0), field("Back", 1), field("Source", 2)],
            serde_json::json!({
                "name": "Card 1", "ord": 0,
                "qfmt": "{{Front}}",
                "afmt": "{{FrontSide}}<hr id=answer>{{Back}}<div class=src>{{Source}}</div>",
                "did": null, "bqfmt": "", "bafmt": ""
            }),
        )
    };
    serde_json::json!({
        "id": id, "name": name, "type": i32::from(cloze), "mod": now, "usn": -1,
        "sortf": 0, "did": deck, "tmpls": [tmpl], "flds": flds, "css": CSS,
        "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
        "latexPost": "\\end{document}", "latexsvg": false,
        "req": [[0, "any", [0]]], "tags": [], "vers": []
    })
}

fn deck(id: i64, name: &str, now: i64) -> serde_json::Value {
    serde_json::json!({
        "id": id, "name": name, "mod": now, "usn": -1,
        "lrnToday": [0, 0], "revToday": [0, 0], "newToday": [0, 0], "timeToday": [0, 0],
        "collapsed": false, "browserCollapsed": false, "desc": "Highlights from Libreri",
        "dyn": 0, "conf": 1, "extendNew": 0, "extendRev": 50
    })
}

fn deck_conf() -> serde_json::Value {
    serde_json::json!({ "1": {
        "id": 1, "name": "Default", "mod": 0, "usn": 0, "maxTaken": 60, "autoplay": true,
        "timer": 0, "replayq": true, "dyn": false,
        "new": { "bury": false, "delays": [1.0, 10.0], "initialFactor": 2500,
                 "ints": [1, 4, 0], "order": 1, "perDay": 20 },
        "rev": { "bury": false, "ease4": 1.3, "ivlFct": 1.0, "maxIvl": 36500,
                 "perDay": 200, "hardFactor": 1.2 },
        "lapse": { "delays": [10.0], "leechAction": 1, "leechFails": 8,
                   "minInt": 1, "mult": 0.0 }
    }})
}

/// The checksum Anki keeps of a note's first field (without HTML).
fn checksum(text: &str) -> i64 {
    let mut h = sha1_smol::Sha1::new();
    h.update(strip_html(text).as_bytes());
    let hex = h.digest().to_string();
    i64::from_str_radix(&hex[..8], 16).unwrap_or(0)
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// A note's guid from our id: stable across exports.
fn guid(id: &str) -> String {
    let mut h = sha1_smol::Sha1::new();
    h.update(b"libreri-anki:");
    h.update(id.as_bytes());
    h.digest().to_string()[..10].to_owned()
}

/// Writes `notes` as a deck named `deck_name` to `path` (.apkg).
pub fn write_apkg(path: &Path, deck_name: &str, notes: &[AnkiNote]) -> Result<usize, String> {
    let dir = tempfile_dir(path)?;
    let db_path = dir.join("collection.anki2");
    let _ = std::fs::remove_file(&db_path);
    let written = write_collection(&db_path, deck_name, notes).map_err(|e| e.to_string());
    let result = written.and_then(|n| {
        let bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("collection.anki2", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
        zip.start_file("media", opts).map_err(|e| e.to_string())?;
        zip.write_all(b"{}").map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        Ok(n)
    });
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn tempfile_dir(near: &Path) -> Result<std::path::PathBuf, String> {
    let base = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = near
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = base.join(format!(
        "libreri-anki-{}-{stamp}",
        name.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn write_collection(
    db_path: &Path,
    deck_name: &str,
    notes: &[AnkiNote],
) -> rusqlite::Result<usize> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch(
        "CREATE TABLE col (id integer primary key, crt integer not null, mod integer not null, scm integer not null, ver integer not null, dty integer not null, usn integer not null, ls integer not null, conf text not null, models text not null, decks text not null, dconf text not null, tags text not null);
         CREATE TABLE notes (id integer primary key, guid text not null, mid integer not null, mod integer not null, usn integer not null, tags text not null, flds text not null, sfld integer not null, csum integer not null, flags integer not null, data text not null);
         CREATE TABLE cards (id integer primary key, nid integer not null, did integer not null, ord integer not null, mod integer not null, usn integer not null, type integer not null, queue integer not null, due integer not null, ivl integer not null, factor integer not null, reps integer not null, lapses integer not null, left integer not null, odue integer not null, odid integer not null, flags integer not null, data text not null);
         CREATE TABLE revlog (id integer primary key, cid integer not null, usn integer not null, ease integer not null, ivl integer not null, lastIvl integer not null, factor integer not null, time integer not null, type integer not null);
         CREATE TABLE graves (usn integer not null, oid integer not null, type integer not null);
         CREATE INDEX ix_notes_usn on notes (usn);
         CREATE INDEX ix_cards_usn on cards (usn);
         CREATE INDEX ix_revlog_usn on revlog (usn);
         CREATE INDEX ix_cards_nid on cards (nid);
         CREATE INDEX ix_cards_sched on cards (did, queue, due);
         CREATE INDEX ix_revlog_cid on revlog (cid);
         CREATE INDEX ix_notes_csum on notes (csum);",
    )?;
    let now_s = chrono::Utc::now().timestamp();
    let now_ms = chrono::Utc::now().timestamp_millis();
    // A deck id from its name, so exporting again goes into the same deck.
    let deck_id = 1_700_000_000_000 + (checksum(deck_name) % 1_000_000_000);
    let models = serde_json::json!({
        BASIC_ID.to_string(): model(BASIC_ID, "Libreri", false, deck_id, now_s),
        CLOZE_ID.to_string(): model(CLOZE_ID, "Libreri Cloze", true, deck_id, now_s),
    });
    let decks = serde_json::json!({
        "1": deck(1, "Default", now_s),
        deck_id.to_string(): deck(deck_id, deck_name, now_s),
    });
    let conf = serde_json::json!({
        "nextPos": notes.len() + 1, "estTimes": true, "activeDecks": [deck_id],
        "sortType": "noteFld", "timeLim": 0, "sortBackwards": false, "addToCur": true,
        "curDeck": deck_id, "newBust": true, "newSpread": 0, "dueCounts": true,
        "curModel": BASIC_ID.to_string(), "collapseTime": 1200
    });
    conn.execute(
        "INSERT INTO col VALUES (1, ?1, ?2, ?2, 11, 0, 0, 0, ?3, ?4, ?5, ?6, '{}')",
        params![
            now_s,
            now_ms,
            conf.to_string(),
            models.to_string(),
            decks.to_string(),
            deck_conf().to_string()
        ],
    )?;
    let tx = conn.unchecked_transaction()?;
    let mut n = 0usize;
    for (i, note) in notes.iter().enumerate() {
        let cloze = note.model == "cloze";
        let mut fields = note.fields.clone();
        fields.resize(3, String::new());
        let nid = now_ms + i as i64;
        let tags = if note.tags.is_empty() {
            String::new()
        } else {
            format!(
                " {} ",
                note.tags
                    .iter()
                    .map(|t| t.replace(char::is_whitespace, "_"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        tx.execute(
            "INSERT INTO notes VALUES (?1, ?2, ?3, ?4, -1, ?5, ?6, ?7, ?8, 0, '')",
            params![
                nid,
                guid(&note.id),
                if cloze { CLOZE_ID } else { BASIC_ID },
                now_s,
                tags,
                fields.join("\u{1f}"),
                strip_html(&fields[0]),
                checksum(&fields[0])
            ],
        )?;
        tx.execute(
            "INSERT INTO cards VALUES (?1, ?2, ?3, 0, ?4, -1, 0, 0, ?5, 0, 0, 0, 0, 0, 0, 0, 0, '')",
            params![nid, nid, deck_id, now_s, i as i64 + 1],
        )?;
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_deck_anki_can_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Libreri.apkg");
        let notes = vec![
            AnkiNote {
                id: "a:passage".into(),
                model: "basic".into(),
                fields: vec![
                    "The keeper climbed…".into(),
                    "The keeper climbed the long stair.".into(),
                    "<i>The Lighthouse</i>, p. 84".into(),
                ],
                tags: vec!["The Lighthouse".into()],
            },
            AnkiNote {
                id: "a:cloze".into(),
                model: "cloze".into(),
                fields: vec![
                    "the {{c1::lantern room}} had to be lit".into(),
                    String::new(),
                    "p. 84".into(),
                ],
                tags: vec![],
            },
        ];
        assert_eq!(write_apkg(&path, "Libreri", &notes).unwrap(), 2);
        // Unzip and read it back as Anki would.
        let f = std::fs::File::open(&path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        let mut db = Vec::new();
        std::io::Read::read_to_end(&mut z.by_name("collection.anki2").unwrap(), &mut db).unwrap();
        let out = dir.path().join("c.anki2");
        std::fs::write(&out, db).unwrap();
        let conn = Connection::open(&out).unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let (flds, tags): (String, String) = conn
            .query_row(
                "SELECT flds, tags FROM notes ORDER BY id LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(flds.contains('\u{1f}') && flds.contains("long stair"));
        assert_eq!(tags, " The_Lighthouse ");
        let models: String = conn
            .query_row("SELECT models FROM col", [], |r| r.get(0))
            .unwrap();
        assert!(models.contains("Libreri Cloze") && models.contains("{{cloze:Text}}"));
        assert_eq!(guid("x"), guid("x"));
        assert_ne!(guid("x"), guid("y"));
    }
}
