//! ISBN-10 / ISBN-13 validation and conversion.
//!
//! Input may contain spaces and hyphens ("978-0-306-40615-7"); output is the
//! bare digits (with a trailing `X` allowed for ISBN-10).

/// Why an ISBN was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IsbnError {
    #[error("an ISBN-13 has 13 digits")]
    Length13,
    #[error("an ISBN-10 has 10 characters (digits, and X as the last one)")]
    Length10,
    #[error("the check digit does not match; please check for a typo")]
    Checksum,
    #[error("an ISBN-13 starts with 978 or 979")]
    Prefix,
}

fn clean(input: &str) -> String {
    input
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '\u{2010}'..='\u{2015}'))
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn isbn13_check(first12: &[u8]) -> u8 {
    let sum: u32 = first12
        .iter()
        .enumerate()
        .map(|(i, d)| u32::from(*d) * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    ((10 - sum % 10) % 10) as u8
}

fn isbn10_check(first9: &[u8]) -> u8 {
    let sum: u32 = first9
        .iter()
        .enumerate()
        .map(|(i, d)| u32::from(*d) * (10 - i as u32))
        .sum();
    ((11 - sum % 11) % 11) as u8
}

fn digits(s: &str) -> Option<Vec<u8>> {
    s.bytes()
        .map(|b| b.is_ascii_digit().then_some(b - b'0'))
        .collect()
}

/// Validates an ISBN-13 and returns it as 13 digits.
pub fn normalize_isbn13(input: &str) -> Result<String, IsbnError> {
    let s = clean(input);
    let d = digits(&s)
        .filter(|d| d.len() == 13)
        .ok_or(IsbnError::Length13)?;
    if !(s.starts_with("978") || s.starts_with("979")) {
        return Err(IsbnError::Prefix);
    }
    if isbn13_check(&d[..12]) != d[12] {
        return Err(IsbnError::Checksum);
    }
    Ok(s)
}

/// Validates an ISBN-10 and returns it as 10 characters.
pub fn normalize_isbn10(input: &str) -> Result<String, IsbnError> {
    let s = clean(input);
    // Byte slicing below needs plain ASCII; anything else is not an ISBN.
    if !s.is_ascii() || s.len() != 10 {
        return Err(IsbnError::Length10);
    }
    let d = digits(&s[..9]).ok_or(IsbnError::Length10)?;
    let last = match s.as_bytes()[9] {
        b'X' => 10,
        b @ b'0'..=b'9' => b - b'0',
        _ => return Err(IsbnError::Length10),
    };
    if isbn10_check(&d) != last {
        return Err(IsbnError::Checksum);
    }
    Ok(s)
}

/// Converts a valid ISBN-10 to its ISBN-13 (978 prefix).
pub fn isbn10_to_13(isbn10: &str) -> Option<String> {
    let s = normalize_isbn10(isbn10).ok()?;
    let mut d = vec![9, 7, 8];
    d.extend(digits(&s[..9])?);
    let check = isbn13_check(&d);
    d.push(check);
    Some(d.iter().map(|n| char::from(b'0' + n)).collect())
}

/// Converts a valid 978-prefixed ISBN-13 to ISBN-10. 979 numbers have no
/// ISBN-10.
pub fn isbn13_to_10(isbn13: &str) -> Option<String> {
    let s = normalize_isbn13(isbn13).ok()?;
    if !s.starts_with("978") {
        return None;
    }
    let d = digits(&s[3..12])?;
    let check = isbn10_check(&d);
    let mut out: String = d.iter().map(|n| char::from(b'0' + n)).collect();
    out.push(if check == 10 {
        'X'
    } else {
        char::from(b'0' + check)
    });
    Some(out)
}

/// Finds the first plausible ISBN in free text (such as an EPUB identifier
/// "urn:isbn:9780306406157") and returns `(isbn13, isbn10)`.
pub fn find_isbn(text: &str) -> Option<(String, Option<String>)> {
    let mut run = String::new();
    let flush = |run: &str| -> Option<(String, Option<String>)> {
        let c = clean(run);
        if let Ok(i13) = normalize_isbn13(&c) {
            let i10 = isbn13_to_10(&i13);
            return Some((i13, i10));
        }
        if let Ok(i10) = normalize_isbn10(&c) {
            return isbn10_to_13(&i10).map(|i13| (i13, Some(i10)));
        }
        None
    };
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() || ch == '-' || ch == 'X' || ch == 'x' {
            run.push(ch);
        } else {
            if let Some(found) = flush(&run) {
                return Some(found);
            }
            run.clear();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_isbn13() {
        assert_eq!(
            normalize_isbn13("978-0-306-40615-7").unwrap(),
            "9780306406157"
        );
        assert_eq!(normalize_isbn13("9780306406158"), Err(IsbnError::Checksum));
        assert_eq!(normalize_isbn13("12345"), Err(IsbnError::Length13));
        assert_eq!(normalize_isbn13("1230306406157"), Err(IsbnError::Prefix));
    }

    #[test]
    fn validates_isbn10_including_x() {
        assert_eq!(normalize_isbn10("0-306-40615-2").unwrap(), "0306406152");
        assert_eq!(normalize_isbn10("080442957x").unwrap(), "080442957X");
        assert_eq!(normalize_isbn10("0306406153"), Err(IsbnError::Checksum));
    }

    #[test]
    fn rejects_non_ascii_without_panicking() {
        // 8 digits + a 2-byte letter = 10 bytes; slicing at 9 used to panic.
        assert_eq!(normalize_isbn10("12345678é"), Err(IsbnError::Length10));
        assert_eq!(
            normalize_isbn10("１２３４５６７８９"),
            Err(IsbnError::Length10)
        );
        assert_eq!(normalize_isbn13("978030640615é"), Err(IsbnError::Length13));
    }

    #[test]
    fn converts_both_ways() {
        assert_eq!(isbn10_to_13("0306406152").unwrap(), "9780306406157");
        assert_eq!(isbn13_to_10("9780306406157").unwrap(), "0306406152");
        assert_eq!(isbn13_to_10("9798886450028"), None);
    }

    #[test]
    fn finds_isbns_in_text() {
        assert_eq!(
            find_isbn("urn:isbn:978-0-306-40615-7").unwrap().0,
            "9780306406157"
        );
        assert_eq!(
            find_isbn("ISBN 0-306-40615-2 (paperback)").unwrap().0,
            "9780306406157"
        );
        assert_eq!(find_isbn("no numbers 12345 here"), None);
    }
}
