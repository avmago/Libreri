//! Finding the words in what someone wrote: Markdown code, maths, links
//! and addresses are left out, and places are counted in UTF-16 units
//! (what the interface's text boxes use).

/// A word and where it is (UTF-16 offsets, end exclusive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word<'a> {
    pub start: usize,
    pub end: usize,
    pub text: &'a str,
}

/// Byte ranges not to check: code, maths, link targets, addresses.
fn skipped(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    // Fenced code blocks.
    let mut line_start = 0;
    let mut fence: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            match fence {
                None => fence = Some(line_start),
                Some(s) => {
                    out.push((s, line_start + line.len()));
                    fence = None;
                }
            }
        }
        line_start += line.len();
    }
    if let Some(s) = fence {
        out.push((s, text.len()));
    }
    let find = |from: usize, pat: &[u8]| -> Option<usize> {
        b[from..]
            .windows(pat.len())
            .position(|w| w == pat)
            .map(|p| p + from)
    };
    while i < b.len() {
        let rest = &b[i..];
        let (open, close): (&[u8], &[u8]) = if rest.starts_with(b"$$") {
            (b"$$", b"$$")
        } else if rest.starts_with(b"`") {
            (b"`", b"`")
        } else if rest.starts_with(b"$") && rest.get(1).is_some_and(|c| !c.is_ascii_whitespace()) {
            (b"$", b"$")
        } else if rest.starts_with(b"](") {
            (b"](", b")")
        } else if rest.starts_with(b"\\(") {
            (b"\\(", b"\\)")
        } else if rest.starts_with(b"\\[") {
            (b"\\[", b"\\]")
        } else {
            i += 1;
            continue;
        };
        let from = i + open.len();
        match find(from, close) {
            Some(end) if close != b"$" || !text[from..end].contains('\n') => {
                out.push((i, end + close.len()));
                i = end + close.len();
            }
            _ => i = from,
        }
    }
    // Addresses and anything with "://", "@" or "www.".
    let mut start = None;
    for (k, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if c.is_whitespace() || c == '<' || c == '>' || c == '(' || c == ')' {
            if let Some(s) = start.take() {
                let chunk = &text[s..k];
                if chunk.contains("://") || chunk.contains('@') || chunk.starts_with("www.") {
                    out.push((s, k));
                }
            }
        } else if start.is_none() {
            start = Some(k);
        }
    }
    out.sort_unstable();
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c.is_numeric() || is_mark(c)
}

/// Combining marks (Hindi vowel signs and the like) belong to the word.
fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x0900..=0x0DFF | 0x1AB0..=0x1AFF | 0x200C | 0x200D)
}

/// Whether a word is worth checking: not a number, an acronym, a single
/// letter or a CamelCase name.
fn checkable(w: &str) -> bool {
    let letters = w.chars().filter(|c| c.is_alphabetic()).count();
    if letters < 2 || w.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let upper = w.chars().filter(|c| c.is_uppercase()).count();
    if upper == letters {
        return false;
    }
    // "iPhone", "McDonald": an uppercase letter after the first.
    !w.chars().skip(1).any(char::is_uppercase)
}

/// Ends the word being read (if any) at byte `end_byte`.
fn finish<'a>(
    text: &'a str,
    cur: &mut Option<(usize, usize)>,
    end_byte: usize,
    out: &mut Vec<Word<'a>>,
) {
    if let Some((sb, su)) = cur.take() {
        // Apostrophes at the ends are quotes, not part of the word.
        let raw = &text[sb..end_byte];
        let lead = raw.len() - raw.trim_start_matches(['\'', '’']).len();
        let trimmed = raw.trim_matches(['\'', '’']);
        if !trimmed.is_empty() && checkable(trimmed) {
            let start = su + raw[..lead].encode_utf16().count();
            let end = start + trimmed.encode_utf16().count();
            out.push(Word {
                start,
                end,
                text: trimmed,
            });
        }
    }
}

/// The words to check, in order.
pub fn words(text: &str) -> Vec<Word<'_>> {
    let skip = skipped(text);
    let mut out = Vec::new();
    let mut u16_at = 0usize;
    let mut cur: Option<(usize, usize)> = None; // (byte start, utf16 start)
    let mut skip_i = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut n = 0;
    while n < chars.len() {
        let (i, c) = chars[n];
        while skip_i < skip.len() && skip[skip_i].1 <= i {
            skip_i += 1;
        }
        let in_skip = skip.get(skip_i).is_some_and(|&(s, e)| s <= i && i < e);
        let joins = (c == '\'' || c == '’')
            && cur.is_some()
            && chars.get(n + 1).is_some_and(|&(_, d)| is_word_char(d));
        if !in_skip && (is_word_char(c) || joins) {
            if cur.is_none() {
                cur = Some((i, u16_at));
            }
        } else {
            finish(text, &mut cur, i, &mut out);
        }
        u16_at += c.len_utf16();
        n += 1;
    }
    finish(text, &mut cur, text.len(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(t: &str) -> Vec<&str> {
        words(t).into_iter().map(|w| w.text).collect()
    }

    #[test]
    fn finds_words_and_skips_the_rest() {
        assert_eq!(
            list(
                "The keeper’s log, 'dusk' — see `code here` and $x^2$ or [a link](https://x.y/zz)."
            ),
            ["The", "keeper’s", "log", "dusk", "see", "and", "or", "link"]
        );
        assert_eq!(
            list("NASA and iPhone and a 3rd e-mail me@x.org"),
            ["and", "and", "mail"]
        );
        assert_eq!(list("```\nlet wrng = 1;\n```\nafter"), ["after"]);
        assert_eq!(list("नमस्ते दुनिया"), ["नमस्ते", "दुनिया"]);
    }

    #[test]
    fn counts_places_like_the_interface() {
        // "é" is one UTF-16 unit; "𝒜" is two.
        let w = words("𝒜 café wrold");
        assert_eq!((w[0].text, w[0].start, w[0].end), ("café", 3, 7));
        assert_eq!((w[1].start, w[1].end), (8, 13));
    }
}
