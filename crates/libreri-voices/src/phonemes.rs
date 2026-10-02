//! Text to phonemes with eSpeak NG, run as its own program (it stays a
//! separate helper, so Libreri itself carries no GPL code). Punctuation is
//! kept, as the voices were trained with it: the text is cut at each mark,
//! every piece is turned into IPA, and the marks are put back.

use std::process::Command;

/// The marks kept between pieces (as `phonemizer` keeps them).
const MARKS: &str = ";:,.!?¡¿—…\"«»“”(){}[]";

/// One piece of text, and the marks after it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub text: String,
    pub marks: String,
}

/// Cuts `text` at punctuation, keeping the marks with the piece before.
pub(crate) fn pieces(text: &str) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    let mut words = String::new();
    let mut marks = String::new();
    for c in text.chars() {
        if MARKS.contains(c) {
            marks.push(c);
        } else if c.is_whitespace() {
            if !marks.is_empty() {
                out.push(Piece {
                    text: std::mem::take(&mut words).trim().to_owned(),
                    marks: std::mem::take(&mut marks),
                });
            } else {
                words.push(' ');
            }
        } else {
            if !marks.is_empty() {
                if words.trim().is_empty() || words.ends_with(' ') {
                    // Opening marks: “Hi
                    out.push(Piece {
                        text: std::mem::take(&mut words).trim().to_owned(),
                        marks: std::mem::take(&mut marks),
                    });
                } else {
                    // A mark inside a word ("U.S.", "e.g."): keep it for eSpeak.
                    words.push_str(&std::mem::take(&mut marks));
                }
            }
            words.push(c);
        }
    }
    let words = words.trim().to_owned();
    if !words.is_empty() || !marks.is_empty() {
        out.push(Piece { text: words, marks });
    }
    out.retain(|p| !p.text.is_empty() || !p.marks.is_empty());
    out
}

/// eSpeak's IPA for one piece, on one line: clause breaks become spaces,
/// language-switch notes ("(en)") and tie marks go.
pub(crate) fn clean(espeak: &str) -> String {
    let mut s = String::new();
    let mut skip = false;
    for c in espeak.chars() {
        match c {
            '(' => skip = true,
            ')' if skip => skip = false,
            _ if skip => {}
            '\u{200d}' | '\u{361}' | '\u{35c}' => {}
            '\n' | '\r' | '\t' => s.push(' '),
            c => s.push(c),
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Puts pieces' phonemes and their marks back together.
pub(crate) fn join(parts: &[(String, &str)]) -> String {
    let mut out = String::new();
    for (ph, marks) in parts {
        if !ph.is_empty() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(ph);
        }
        if !marks.is_empty() {
            // Opening marks stand before the next word, closing ones after.
            out.push_str(marks);
            out.push(' ');
        }
    }
    out.trim().to_owned()
}

/// Phonemes for `text` in eSpeak's `voice` (a language such as "en-us").
pub fn phonemize(text: &str, voice: &str) -> Result<String, String> {
    let ps = pieces(text);
    let mut parts = Vec::with_capacity(ps.len());
    for p in &ps {
        let ph = if p.text.is_empty() {
            String::new()
        } else {
            espeak(&p.text, voice)?
        };
        parts.push((ph, p.marks.as_str()));
    }
    Ok(join(&parts))
}

fn espeak(text: &str, voice: &str) -> Result<String, String> {
    let mut cmd: Command = libreri_helpers::command("espeak-ng").ok_or(
        "natural voices need eSpeak NG to turn words into sounds; install it in Settings › Reader",
    )?;
    let out = cmd
        .args(["-q", "--ipa", "-b", "1", "-v", voice, "--", text])
        .output()
        .map_err(|e| format!("eSpeak NG could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "eSpeak NG could not read this: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(clean(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_at_punctuation() {
        let p = pieces("The harbour, and Dr. Smith read 3 chapters.");
        let t: Vec<_> = p
            .iter()
            .map(|p| (p.text.as_str(), p.marks.as_str()))
            .collect();
        assert_eq!(
            t,
            [
                ("The harbour", ","),
                ("and Dr", "."),
                ("Smith read 3 chapters", ".")
            ]
        );
        // Marks inside a word stay with it.
        assert_eq!(pieces("e.g. this")[0].text, "e.g");
        assert_eq!(
            pieces("“Hi,” she said")[0],
            Piece {
                text: String::new(),
                marks: "“".into()
            }
        );
    }

    #[test]
    fn cleans_espeak_output() {
        assert_eq!(clean("bɔːnʒˈʊɹ (fr)ab(en)\nænd\n"), "bɔːnʒˈʊɹ ab ænd");
        assert_eq!(clean("t\u{200d}ʃˈæp"), "tʃˈæp");
    }

    #[test]
    fn joins_with_marks() {
        let parts = [
            ("ðə hˈɑːɹbɚ".to_owned(), ","),
            ("ænd dˈɑːktɚ".to_owned(), "."),
            ("smˈɪθ".to_owned(), "."),
        ];
        assert_eq!(join(&parts), "ðə hˈɑːɹbɚ, ænd dˈɑːktɚ. smˈɪθ.");
    }

    #[test]
    fn matches_kokoro_onnx_when_espeak_is_here() {
        if libreri_helpers::command("espeak-ng").is_none() {
            return;
        }
        let got = phonemize(
            "The harbour was quiet that morning, and Dr. Smith read 3 chapters.",
            "en-us",
        )
        .unwrap();
        // What kokoro-onnx (phonemizer) gives for the same sentence.
        assert_eq!(
            got,
            "ðə hˈɑːɹbɚ wʌz kwˈaɪət ðæt mˈɔːɹnɪŋ, ænd dˈɑːktɚ. smˈɪθ ɹˈiːd θɹˈiː tʃˈæptɚz."
        );
    }
}
