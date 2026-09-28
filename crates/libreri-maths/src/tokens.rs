//! pix2tex's tokens back to LaTeX, tidied as pix2tex does.

use regex::Regex;
use std::sync::OnceLock;

fn vocab() -> &'static Vec<String> {
    static V: OnceLock<Vec<String>> = OnceLock::new();
    V.get_or_init(|| {
        let doc: serde_json::Value =
            serde_json::from_str(include_str!("../data/tokenizer.json")).unwrap_or_default();
        let mut out = vec![String::new(); 8000];
        if let Some(map) = doc["model"]["vocab"].as_object() {
            for (tok, id) in map {
                if let Some(i) = id.as_u64().map(|i| i as usize).filter(|&i| i < out.len()) {
                    out[i] = tok.clone();
                }
            }
        }
        out
    })
}

/// Token ids as the LaTeX they spell.
pub fn decode(ids: &[u32]) -> String {
    let v = vocab();
    let mut s = String::new();
    for &id in ids {
        let t = v.get(id as usize).map(String::as_str).unwrap_or("");
        if !t.starts_with('[') || !t.ends_with(']') || t.len() < 3 {
            s.push_str(t);
        }
    }
    tidy(&s.replace('Ġ', " "))
}

/// Removes spaces the model writes between tokens, keeping those between
/// two letters (`\alpha b`) and escaped ones (`\ `); names in `\mathrm{…}`
/// and the like lose their spaces.
pub fn tidy(s: &str) -> String {
    static NAMES: OnceLock<Regex> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        Regex::new(r"\\(operatorname|mathrm|text|mathbf)\s?\*? \{.*?\}").expect("regex")
    });
    let s = names.replace_all(s.trim(), |c: &regex::Captures| c[0].replace(' ', ""));
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let before = out.chars().last();
            let after = chars.get(j).copied();
            let keep = match (before, after) {
                (Some(a), Some(b)) => {
                    (a.is_ascii_alphabetic() && b.is_ascii_alphabetic()) || a == '\\'
                }
                _ => false,
            };
            if keep {
                out.push(' ');
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_are_tidied() {
        assert_eq!(tidy(r"\frac { a } { b } + 1"), r"\frac{a}{b}+1");
        assert_eq!(tidy(r"\alpha b"), r"\alpha b");
        assert_eq!(tidy(r"x \ y"), r"x\ y");
        assert_eq!(tidy(r"\mathrm {d x}"), r"\mathrm{dx}");
    }

    #[test]
    fn ids_decode() {
        // "[BOS]" is skipped; ids 3.. are single characters.
        assert_eq!(decode(&[1, 3, 2]), "!");
    }
}
