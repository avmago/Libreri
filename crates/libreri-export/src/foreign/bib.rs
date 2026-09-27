//! BibTeX and RIS files, as Mendeley, JabRef, Zotero and EndNote export
//! them, including the attached files they list (`file = {…}` in BibTeX,
//! `L1`/`L4` in RIS). Relative paths are read from the file's folder.

use super::{foreign_file, ForeignBook, ForeignError, ForeignLibrary, ForeignSource};
use libreri_core::{BookMetadata, ContentType};
use libreri_metadata::normalise;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------- BibTeX ----------

/// One entry: type, key and fields (names lower-case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibEntry {
    pub kind: String,
    pub key: String,
    pub fields: HashMap<String, String>,
}

/// Turns common LaTeX into plain text: accents, escapes, dashes, braces.
pub fn latex_to_text(s: &str) -> String {
    const ACCENTS: &[(char, &str, &str)] = &[
        ('"', "aeiouyAEIOU", "äëïöüÿÄËÏÖÜ"),
        ('\'', "aeiouycnszAEIOUYCNSZ", "áéíóúýćńśźÁÉÍÓÚÝĆŃŚŹ"),
        ('`', "aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
        ('^', "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        ('~', "anoANO", "ãñõÃÑÕ"),
        ('c', "csCS", "çşÇŞ"),
        ('v', "csznrCSZNR", "čšžňřČŠŽŇŘ"),
        ('u', "agAG", "ăğĂĞ"),
        ('=', "aeiouAEIOU", "āēīōūĀĒĪŌŪ"),
        ('.', "zZ", "żŻ"),
    ];
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            let n = chars[i + 1];
            // \"{o}, \"o, \c{c}
            if let Some((_, from, to)) = ACCENTS.iter().find(|(a, _, _)| *a == n) {
                let mut j = i + 2;
                let braced = chars.get(j) == Some(&'{');
                if braced || (n.is_alphabetic() && chars.get(j) == Some(&' ')) {
                    j += 1;
                }
                if let Some(&letter) = chars.get(j) {
                    if let Some(pos) = from.chars().position(|x| x == letter) {
                        out.push(to.chars().nth(pos).unwrap_or(letter));
                        i = j + 1;
                        if braced && chars.get(i) == Some(&'}') {
                            i += 1;
                        }
                        continue;
                    }
                }
            }
            // \ss, \o, \aa, \ae, \l
            let word: String = chars[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            let special = match word.as_str() {
                "ss" => Some("ß"),
                "o" => Some("ø"),
                "O" => Some("Ø"),
                "aa" => Some("å"),
                "AA" => Some("Å"),
                "ae" => Some("æ"),
                "AE" => Some("Æ"),
                "oe" => Some("œ"),
                "l" => Some("ł"),
                "L" => Some("Ł"),
                "i" => Some("ı"),
                "textendash" => Some("–"),
                "textemdash" => Some("—"),
                "textbackslash" => Some("\\"),
                "backslash" => Some("\\"),
                _ => None,
            };
            if let Some(sp) = special {
                out.push_str(sp);
                i += 1 + word.len();
                if chars.get(i) == Some(&'{') && chars.get(i + 1) == Some(&'}') {
                    i += 2;
                }
                continue;
            }
            if "&%$#_{}".contains(n) {
                out.push(n);
                i += 2;
                continue;
            }
            // Other commands (\emph, \textit…): drop the command, keep its text.
            i += 1 + word.len().max(1);
            continue;
        }
        match c {
            '{' | '}' | '$' => {}
            '~' => out.push(' '),
            '-' if chars.get(i + 1) == Some(&'-') => {
                if chars.get(i + 2) == Some(&'-') {
                    out.push('—');
                    i += 1;
                } else {
                    out.push('–');
                }
                i += 1;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parses BibTeX. Unknown syntax is skipped rather than refused.
pub fn parse_bibtex(text: &str) -> Vec<BibEntry> {
    let chars: Vec<char> = text.chars().collect();
    let mut strings: HashMap<String, String> = [
        ("jan", "January"),
        ("feb", "February"),
        ("mar", "March"),
        ("apr", "April"),
        ("may", "May"),
        ("jun", "June"),
        ("jul", "July"),
        ("aug", "August"),
        ("sep", "September"),
        ("oct", "October"),
        ("nov", "November"),
        ("dec", "December"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let mut out = Vec::new();
    let mut i = 0;
    let skip_ws = |i: &mut usize| {
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
    };
    // Reads {…} with nesting, or "…", or a bare word; returns raw text.
    let read_value = |i: &mut usize, strings: &HashMap<String, String>| -> String {
        let mut parts = Vec::new();
        loop {
            skip_ws(i);
            if *i >= chars.len() {
                break;
            }
            match chars[*i] {
                '{' => {
                    let mut depth = 0;
                    let start = *i + 1;
                    while *i < chars.len() {
                        match chars[*i] {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            '\\' => *i += 1,
                            _ => {}
                        }
                        *i += 1;
                    }
                    parts.push(
                        chars[start..(*i).min(chars.len())]
                            .iter()
                            .collect::<String>(),
                    );
                    *i += 1;
                }
                '"' => {
                    let start = *i + 1;
                    *i += 1;
                    let mut depth = 0;
                    while *i < chars.len() {
                        match chars[*i] {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            '"' if depth == 0 => break,
                            '\\' => *i += 1,
                            _ => {}
                        }
                        *i += 1;
                    }
                    parts.push(
                        chars[start..(*i).min(chars.len())]
                            .iter()
                            .collect::<String>(),
                    );
                    *i += 1;
                }
                _ => {
                    let start = *i;
                    while *i < chars.len()
                        && !matches!(chars[*i], ',' | '}' | ')' | '#')
                        && !chars[*i].is_whitespace()
                    {
                        *i += 1;
                    }
                    let word: String = chars[start..*i].iter().collect();
                    parts.push(strings.get(&word.to_lowercase()).cloned().unwrap_or(word));
                }
            }
            skip_ws(i);
            if *i < chars.len() && chars[*i] == '#' {
                *i += 1;
                continue;
            }
            break;
        }
        parts.concat()
    };

    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        while i < chars.len() && chars[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let kind: String = chars[start..i].iter().collect::<String>().to_lowercase();
        skip_ws(&mut i);
        if i >= chars.len() || !matches!(chars[i], '{' | '(') {
            continue;
        }
        i += 1;
        match kind.as_str() {
            "comment" | "preamble" => {
                // Skip to the matching brace.
                let mut depth = 1;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '{' | '(' => depth += 1,
                        '}' | ')' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                continue;
            }
            "string" => {
                skip_ws(&mut i);
                let s = i;
                while i < chars.len() && chars[i] != '=' {
                    i += 1;
                }
                let name: String = chars[s..i].iter().collect::<String>().trim().to_lowercase();
                i += 1;
                let value = read_value(&mut i, &strings);
                strings.insert(name, value);
                while i < chars.len() && !matches!(chars[i], '}' | ')') {
                    i += 1;
                }
                i += 1;
                continue;
            }
            _ => {}
        }
        skip_ws(&mut i);
        let s = i;
        while i < chars.len() && chars[i] != ',' && !matches!(chars[i], '}' | ')') {
            i += 1;
        }
        let key: String = chars[s..i].iter().collect::<String>().trim().to_owned();
        let mut fields = HashMap::new();
        loop {
            skip_ws(&mut i);
            if i >= chars.len() {
                break;
            }
            if matches!(chars[i], '}' | ')') {
                i += 1;
                break;
            }
            if chars[i] == ',' {
                i += 1;
                continue;
            }
            let s = i;
            while i < chars.len() && chars[i] != '=' && !matches!(chars[i], '}' | ')' | ',') {
                i += 1;
            }
            if i >= chars.len() || chars[i] != '=' {
                continue;
            }
            let name: String = chars[s..i].iter().collect::<String>().trim().to_lowercase();
            i += 1;
            let value = read_value(&mut i, &strings);
            if !name.is_empty() {
                fields.insert(name, value);
            }
        }
        out.push(BibEntry { kind, key, fields });
    }
    out
}

/// "Smith, John and Jane Doe and {World Health Organization}".
fn bib_people(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut current = String::new();
    let words: Vec<&str> = s.split_whitespace().collect();
    for w in words {
        depth += w.matches('{').count() as i32 - w.matches('}').count() as i32;
        if depth == 0 && w.eq_ignore_ascii_case("and") {
            out.push(std::mem::take(&mut current));
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(w);
    }
    out.push(current);
    out.into_iter()
        .map(|p| normalise::person(&latex_to_text(p.trim())))
        .filter(|p| !p.is_empty() && p != "others")
        .collect()
}

/// Files listed in a `file` field: Mendeley ":path:pdf", JabRef
/// "desc:path:PDF", Zotero "Full Text:path:application/pdf", `;`-separated,
/// `\:` and `$\backslash$` escaped.
fn bib_files(field: &str, base: &Path) -> Vec<PathBuf> {
    let unescaped = field
        .replace("$\\backslash$", "\\")
        .replace("{\\textbackslash}", "\\");
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut chars = unescaped.chars().peekable();
    // Split on unescaped ';', keeping "\:" as a marker for ':'.
    while let Some(c) = chars.next() {
        match c {
            '\\' if matches!(chars.peek(), Some(':') | Some(';')) => {
                let n = chars.next().unwrap_or(':');
                current.push(if n == ':' { '\u{1}' } else { ';' });
            }
            ';' => entries.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    entries.push(current);
    let mut out = Vec::new();
    for e in entries {
        let parts: Vec<&str> = e.split(':').collect();
        let raw = match parts.len() {
            0 => continue,
            1 => parts[0].to_owned(),
            2 => parts[1].to_owned(),
            // "desc:C:/path:pdf" — a Windows drive letter split the path.
            n if n >= 4 && parts[1].len() == 1 => {
                format!("{}:{}", parts[1], parts[2..n - 1].join(":"))
            }
            n => parts[1..n - 1].join(":"),
        };
        let raw = raw.replace('\u{1}', ":").trim().to_owned();
        if raw.is_empty() {
            continue;
        }
        let raw = raw.strip_prefix("file://").unwrap_or(&raw).to_owned();
        let p = PathBuf::from(&raw);
        let looks_absolute =
            p.is_absolute() || raw.starts_with('/') || raw.as_bytes().get(1) == Some(&b':');
        out.push(if looks_absolute {
            p
        } else if !base.join(&p).exists() && Path::new("/").join(&p).exists() {
            // Mendeley on macOS drops the leading "/".
            Path::new("/").join(p)
        } else {
            base.join(p)
        });
    }
    out
}

fn bib_type(kind: &str, fields: &HashMap<String, String>) -> ContentType {
    match kind {
        "article" => ContentType::ResearchPaper,
        "inproceedings" | "conference" | "proceedings" => ContentType::ConferencePaper,
        "book" | "inbook" | "incollection" | "booklet" | "mvbook" => ContentType::Book,
        "phdthesis" | "mastersthesis" | "thesis" => ContentType::Thesis,
        "techreport" | "report" => ContentType::TechnicalReport,
        "manual" => ContentType::Manual,
        "online" | "electronic" | "www" => ContentType::Article,
        _ if fields.contains_key("eprint") || fields.contains_key("archiveprefix") => {
            ContentType::Preprint
        }
        _ => ContentType::Other,
    }
}

/// Book details from one BibTeX entry.
pub fn bib_metadata(e: &BibEntry) -> BookMetadata {
    let f = |k: &str| {
        e.fields
            .get(k)
            .map(|v| latex_to_text(v))
            .filter(|v| !v.is_empty())
    };
    let raw = |k: &str| {
        e.fields
            .get(k)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let arxiv = match (f("archiveprefix"), f("eprinttype"), f("eprint")) {
        (Some(p), _, Some(id)) | (_, Some(p), Some(id)) if p.eq_ignore_ascii_case("arxiv") => {
            Some(id)
        }
        _ => None,
    };
    let isbn = f("isbn").map(|s| {
        s.split([' ', ','])
            .next()
            .unwrap_or_default()
            .replace('-', "")
    });
    let (isbn13, isbn10) = match isbn {
        Some(s) if s.len() == 10 => (None, Some(s)),
        Some(s) => (Some(s), None),
        None => (None, None),
    };
    let mut contributors: Vec<String> = e
        .fields
        .get("editor")
        .map(|v| bib_people(v))
        .unwrap_or_default()
        .into_iter()
        .map(|n| format!("{n} (editor)"))
        .collect();
    contributors.extend(
        e.fields
            .get("translator")
            .map(|v| bib_people(v))
            .unwrap_or_default()
            .into_iter()
            .map(|n| format!("{n} (translator)")),
    );
    BookMetadata {
        title: f("title").unwrap_or_default(),
        subtitle: f("subtitle"),
        authors: e
            .fields
            .get("author")
            .map(|v| bib_people(v))
            .unwrap_or_default(),
        contributors,
        about: f("abstract").and_then(|a| normalise::about(&a)),
        tags: f("keywords")
            .map(|k| {
                k.split([',', ';'])
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        year: f("year")
            .or_else(|| f("date"))
            .as_deref()
            .and_then(normalise::year),
        publisher: f("publisher")
            .or_else(|| f("school"))
            .or_else(|| f("institution"))
            .or_else(|| f("organization")),
        pages: f("pagetotal").and_then(|p| p.parse().ok()),
        isbn13,
        isbn10,
        edition: f("edition"),
        language: f("language")
            .or_else(|| f("langid"))
            .and_then(|l| normalise::language(&l)),
        content_type: bib_type(&e.kind, &e.fields),
        series: f("series"),
        doi: raw("doi").map(|d| {
            d.trim_start_matches("https://doi.org/")
                .trim_start_matches("http://dx.doi.org/")
                .to_owned()
        }),
        arxiv_id: arxiv,
        journal: f("journal")
            .or_else(|| f("journaltitle"))
            .or_else(|| f("booktitle")),
        volume: f("volume"),
        issue: f("number").or_else(|| f("issue")),
        url: raw("url"),
        ..Default::default()
    }
}

pub fn read_bibtex_file(path: &Path) -> Result<ForeignLibrary, ForeignError> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let base = path.parent().unwrap_or(Path::new("."));
    let entries = parse_bibtex(&text);
    if entries.is_empty() {
        return Err(ForeignError::NotRecognised(
            "no BibTeX entries were found in this file".into(),
        ));
    }
    let books = entries
        .iter()
        .map(|e| {
            let mut b = ForeignBook::new(format!("bibtex:{}", e.key), bib_metadata(e));
            b.files = e
                .fields
                .get("file")
                .or_else(|| e.fields.get("pdf"))
                .map(|f| bib_files(f, base))
                .unwrap_or_default()
                .into_iter()
                .filter_map(foreign_file)
                .collect();
            b
        })
        .collect();
    Ok(ForeignLibrary {
        source: ForeignSource::Bibtex,
        books,
        warnings: Vec::new(),
    })
}

// ---------- RIS ----------

/// Parses RIS: records of `TAG  - value` lines, ended by `ER  -`.
pub fn parse_ris(text: &str) -> Vec<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut current: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}');
        if line.len() >= 5 && &line[2..4] == "  " && line.as_bytes().get(4) == Some(&b'-') {
            let tag = line[..2].to_owned();
            let value = line[5..].trim().to_owned();
            if tag == "ER" {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            } else {
                current.push((tag, value));
            }
        } else if let Some(last) = current.last_mut() {
            // A continuation line.
            if !line.trim().is_empty() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn ris_metadata(record: &[(String, String)]) -> BookMetadata {
    let all = |tags: &[&str]| -> Vec<String> {
        record
            .iter()
            .filter(|(t, _)| tags.contains(&t.as_str()))
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
            .collect()
    };
    let first = |tags: &[&str]| all(tags).into_iter().next();
    let ty = first(&["TY"]).unwrap_or_default();
    let content_type = match ty.as_str() {
        "BOOK" | "EBOOK" | "CHAP" | "ECHAP" | "EDBOOK" => ContentType::Book,
        "JOUR" | "EJOUR" | "JFULL" => ContentType::ResearchPaper,
        "CONF" | "CPAPER" => ContentType::ConferencePaper,
        "THES" => ContentType::Thesis,
        "RPRT" => ContentType::TechnicalReport,
        "MGZN" | "NEWS" | "ELEC" | "BLOG" => ContentType::Article,
        "STAND" => ContentType::Standard,
        "SLIDE" => ContentType::Slides,
        _ => ContentType::Other,
    };
    let isbn = first(&["SN"]).map(|s| {
        s.split([' ', ','])
            .next()
            .unwrap_or_default()
            .replace('-', "")
    });
    let (isbn13, isbn10) = match isbn {
        Some(s) if s.len() == 13 => (Some(s), None),
        Some(s) if s.len() == 10 => (None, Some(s)),
        _ => (None, None), // ISSNs and others
    };
    BookMetadata {
        title: first(&["TI", "T1", "CT"]).unwrap_or_default(),
        authors: all(&["AU", "A1"])
            .iter()
            .map(|a| normalise::person(a))
            .collect(),
        contributors: all(&["A2", "ED", "A3"])
            .iter()
            .map(|a| format!("{} (editor)", normalise::person(a)))
            .collect(),
        about: first(&["AB", "N2"]).and_then(|a| normalise::about(&a)),
        tags: all(&["KW"]),
        year: first(&["PY", "Y1", "DA"])
            .as_deref()
            .and_then(normalise::year),
        publisher: first(&["PB"]),
        isbn13,
        isbn10,
        edition: first(&["ET"]),
        language: first(&["LA"]).and_then(|l| normalise::language(&l)),
        content_type,
        series: first(&["T3"]),
        doi: first(&["DO"]),
        journal: first(&["T2", "JO", "JF", "BT", "J2"]),
        volume: first(&["VL"]),
        issue: first(&["IS"]),
        url: first(&["UR"]),
        ..Default::default()
    }
}

pub fn read_ris_file(path: &Path) -> Result<ForeignLibrary, ForeignError> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let base = path.parent().unwrap_or(Path::new("."));
    let records = parse_ris(&text);
    if records.is_empty() {
        return Err(ForeignError::NotRecognised(
            "no RIS records were found in this file".into(),
        ));
    }
    let books = records
        .iter()
        .enumerate()
        .map(|(n, r)| {
            let mut b = ForeignBook::new(format!("ris:{n}"), ris_metadata(r));
            b.files = r
                .iter()
                .filter(|(t, _)| t == "L1" || t == "L4")
                .flat_map(|(_, v)| {
                    v.split(';')
                        .map(str::trim)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .filter(|v| !v.is_empty() && !v.starts_with("http"))
                .map(|v| {
                    let v = v.strip_prefix("file://").unwrap_or(&v).to_owned();
                    let v = v.replace("%20", " ");
                    let p = PathBuf::from(&v);
                    if p.is_absolute() {
                        p
                    } else {
                        base.join(p)
                    }
                })
                .filter_map(foreign_file)
                .collect();
            b
        })
        .collect();
    Ok(ForeignLibrary {
        source: ForeignSource::Ris,
        books,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIB: &str = r#"
@string{nips = "Advances in Neural Information Processing Systems"}
@comment{jabref-meta: databaseType:bibtex;}
@inproceedings{vaswani2017,
  author = {Vaswani, Ashish and Shazeer, Noam and {Google Brain}},
  title = {Attention Is {All} You Need},
  booktitle = nips,
  year = 2017,
  eprint = {1706.03762},
  archivePrefix = {arXiv},
  keywords = {transformers; attention},
  file = {:Users/me/Papers/Vaswani\:2017.pdf:pdf}
}
@book{gödel,
  author = "Kurt G{\"o}del and Ren\'{e} Descartes",
  title = "{\"U}ber formal unentscheidbare S{\"a}tze -- Teil I",
  publisher = {Springer},
  year = {1931},
  isbn = {978-0-441-01359-3},
  file = {Full Text:papers/godel.pdf:application/pdf;Snapshot:papers/snap.html:text/html},
}
"#;

    #[test]
    fn parses_bibtex_with_files_and_latex() {
        let e = parse_bibtex(BIB);
        assert_eq!(e.len(), 2);
        let m = bib_metadata(&e[0]);
        assert_eq!(m.title, "Attention Is All You Need");
        assert_eq!(
            m.authors,
            ["Ashish Vaswani", "Noam Shazeer", "Google Brain"]
        );
        assert_eq!(
            m.journal.as_deref(),
            Some("Advances in Neural Information Processing Systems")
        );
        assert_eq!(m.year, Some(2017));
        assert_eq!(m.arxiv_id.as_deref(), Some("1706.03762"));
        assert_eq!(m.content_type, ContentType::ConferencePaper);
        assert_eq!(m.tags, ["transformers", "attention"]);
        let files = bib_files(&e[0].fields["file"], Path::new("/base"));
        assert_eq!(
            files,
            [PathBuf::from("/base/Users/me/Papers/Vaswani:2017.pdf")]
        );

        let m = bib_metadata(&e[1]);
        assert_eq!(m.authors, ["Kurt Gödel", "René Descartes"]);
        assert_eq!(m.title, "Über formal unentscheidbare Sätze – Teil I");
        assert_eq!(m.isbn13.as_deref(), Some("9780441013593"));
        let files = bib_files(&e[1].fields["file"], Path::new("/lib"));
        assert_eq!(files[0], PathBuf::from("/lib/papers/godel.pdf"));
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn mendeley_and_windows_paths() {
        let f = bib_files(":C$\\backslash$:/Users/a/b.pdf:pdf", Path::new("/x"));
        assert_eq!(f, [PathBuf::from("C:/Users/a/b.pdf")]);
        let f = bib_files(":/Users/a/My Paper.pdf:PDF", Path::new("/x"));
        assert_eq!(f, [PathBuf::from("/Users/a/My Paper.pdf")]);
    }

    #[test]
    fn parses_ris() {
        let text = "TY  - JOUR\r\nAU  - Smith, Jane\r\nAU  - Doe, John\r\nTI  - Light\r\n  and shade\r\nT2  - Optics Letters\r\nPY  - 2019///\r\nSN  - 0146-9592\r\nDO  - 10.1364/OL.1\r\nKW  - optics\r\nL1  - file:///Users/a/light.pdf\r\nER  - \r\n\r\nTY  - BOOK\r\nTI  - Dune\r\nSN  - 9780441013593\r\nER  - \r\n";
        let r = parse_ris(text);
        assert_eq!(r.len(), 2);
        let m = ris_metadata(&r[0]);
        assert_eq!(m.title, "Light and shade");
        assert_eq!(m.authors, ["Jane Smith", "John Doe"]);
        assert_eq!(m.journal.as_deref(), Some("Optics Letters"));
        assert_eq!(m.year, Some(2019));
        assert_eq!(m.isbn13, None, "an ISSN is not an ISBN");
        assert_eq!(m.content_type, ContentType::ResearchPaper);
        let m = ris_metadata(&r[1]);
        assert_eq!(m.isbn13.as_deref(), Some("9780441013593"));
        assert_eq!(m.content_type, ContentType::Book);
    }
}
