//! Looking up a word or a name while reading: its meanings from Wiktionary
//! and a summary from Wikipedia (both free, no key). Only the selected
//! words and the book's language are sent.

use crate::http::{enc, json, Http};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meaning {
    /// "Noun", "Verb"…
    pub part_of_speech: String,
    pub definitions: Vec<Definition>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub text: String,
    pub examples: Vec<String>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiSummary {
    pub title: String,
    pub description: Option<String>,
    pub extract: String,
    /// The page's picture as a data: URL (small), so nothing else loads.
    pub image: Option<String>,
    pub url: String,
    /// A page listing several meanings of the name.
    pub disambiguation: bool,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordLookup {
    pub query: String,
    /// The language the meanings are in ("en"), when found.
    pub language: Option<String>,
    pub meanings: Vec<Meaning>,
    pub wiktionary_url: String,
    pub wikipedia: Option<WikiSummary>,
    /// What could not be reached (shown, not fatal).
    pub problems: Vec<String>,
}

/// "en" from "en-GB", "English"…; None for anything odd.
fn lang_code(lang: Option<&str>) -> String {
    let l = lang.unwrap_or("en").trim().to_ascii_lowercase();
    let code = l.split(['-', '_']).next().unwrap_or("en");
    let code = match code {
        "english" => "en",
        "french" => "fr",
        "german" => "de",
        "spanish" => "es",
        "italian" => "it",
        c => c,
    };
    if (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase()) {
        code.to_owned()
    } else {
        "en".to_owned()
    }
}

/// Text from a bit of Wiktionary HTML: tags out, entities read.
fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Meanings from Wiktionary's definition answer, in `lang` (else English,
/// else the first language given).
pub(crate) fn read_meanings(v: &Value, lang: &str) -> (Option<String>, Vec<Meaning>) {
    let Some(map) = v.as_object() else {
        return (None, Vec::new());
    };
    let pick = [lang, "en"]
        .iter()
        .find_map(|l| map.get(*l).map(|x| (l.to_string(), x)))
        .or_else(|| map.iter().next().map(|(k, x)| (k.clone(), x)));
    let Some((code, entries)) = pick else {
        return (None, Vec::new());
    };
    let mut out = Vec::new();
    for e in entries.as_array().into_iter().flatten() {
        let pos = e["partOfSpeech"].as_str().unwrap_or("").to_owned();
        let defs: Vec<Definition> = e["definitions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| {
                let text = plain(d["definition"].as_str()?);
                if text.is_empty() {
                    return None;
                }
                let examples = d["examples"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(plain))
                    .filter(|x| !x.is_empty())
                    .take(2)
                    .collect();
                Some(Definition { text, examples })
            })
            .take(6)
            .collect();
        if !defs.is_empty() {
            out.push(Meaning {
                part_of_speech: pos,
                definitions: defs,
            });
        }
    }
    (Some(code), out)
}

fn read_summary(http: &dyn Http, v: &Value) -> Option<WikiSummary> {
    let title = v["title"].as_str()?.to_owned();
    let extract = v["extract"].as_str().unwrap_or("").trim().to_owned();
    if extract.is_empty() {
        return None;
    }
    let image = v["thumbnail"]["source"]
        .as_str()
        .filter(|u| u.starts_with("https://upload.wikimedia.org/"))
        .and_then(|u| http.get(u, &[]).ok())
        .filter(|r| r.ok() && r.body.len() < 400_000)
        .map(|r| {
            let kind = r.content_type.unwrap_or_else(|| "image/jpeg".into());
            format!(
                "data:{};base64,{}",
                kind.split(';').next().unwrap_or("image/jpeg"),
                base64::engine::general_purpose::STANDARD.encode(&r.body)
            )
        });
    Some(WikiSummary {
        title,
        description: v["description"].as_str().map(str::to_owned),
        extract,
        image,
        url: v["content_urls"]["desktop"]["page"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        disambiguation: v["type"].as_str() == Some("disambiguation"),
    })
}

/// Looks up `query` (a word or a few) in the book's language.
pub fn lookup(http: &dyn Http, query: &str, lang: Option<&str>) -> WordLookup {
    let q = query
        .trim()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_owned();
    let lang = lang_code(lang);
    let mut problems = Vec::new();
    let word = q.replace(' ', "_");
    // One word: as written, then in lower case (start of a sentence).
    let mut meanings = Vec::new();
    let mut language = None;
    if !q.contains(' ') || q.split(' ').count() <= 3 {
        let mut tries = vec![word.clone()];
        let lower = word.to_lowercase();
        if lower != word {
            tries.push(lower);
        }
        for w in tries {
            let url = format!(
                "https://en.wiktionary.org/api/rest_v1/page/definition/{}",
                enc(&w).replace("%5F", "_")
            );
            match http
                .get(&url, &[("Accept", "application/json")])
                .and_then(json)
            {
                Ok(v) => {
                    let (l, m) = read_meanings(&v, &lang);
                    if !m.is_empty() {
                        language = l;
                        meanings = m;
                        break;
                    }
                }
                Err(e) => {
                    problems.push(format!("Wiktionary: {e}"));
                    break;
                }
            }
        }
    }
    let wiki_url = format!(
        "https://{lang}.wikipedia.org/api/rest_v1/page/summary/{}",
        enc(&word).replace("%5F", "_")
    );
    let wikipedia = match http
        .get(&wiki_url, &[("Accept", "application/json")])
        .and_then(json)
    {
        Ok(v) => read_summary(http, &v),
        Err(e) => {
            problems.push(format!("Wikipedia: {e}"));
            None
        }
    };
    WordLookup {
        wiktionary_url: format!(
            "https://en.wiktionary.org/wiki/{}",
            enc(&word).replace("%5F", "_")
        ),
        query: q,
        language,
        meanings,
        wikipedia,
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_wiktionary_in_the_books_language() {
        let v = json!({
            "en": [{"partOfSpeech": "Noun", "language": "English", "definitions": [
                {"definition": "A <a href=\"/wiki/tower\">tower</a> with a &amp; light.", "examples": ["The <b>lighthouse</b> shone."]},
                {"definition": ""}
            ]}],
            "fr": [{"partOfSpeech": "Nom", "definitions": [{"definition": "Phare."}]}]
        });
        let (l, m) = read_meanings(&v, "fr");
        assert_eq!(l.as_deref(), Some("fr"));
        assert_eq!(m[0].definitions[0].text, "Phare.");
        let (l, m) = read_meanings(&v, "de");
        assert_eq!(l.as_deref(), Some("en"));
        assert_eq!(m[0].part_of_speech, "Noun");
        assert_eq!(m[0].definitions.len(), 1);
        assert_eq!(m[0].definitions[0].text, "A tower with a & light.");
        assert_eq!(m[0].definitions[0].examples, ["The lighthouse shone."]);
        assert_eq!(lang_code(Some("en-GB")), "en");
        assert_eq!(lang_code(Some("weird!")), "en");
    }
}
