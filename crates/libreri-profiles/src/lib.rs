//! Profile secrets: 6-digit PINs, how long to wait after wrong ones, and the
//! owner's recovery code.
//!
//! PINs are hashed with Argon2id and a random salt. They keep people who
//! share a computer out of each other's notes; they are not encryption, and
//! anyone who can read the library folder can read the book files and notes
//! (docs/adr/0010-profiles-and-pins.md).

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand_core::{OsRng, RngCore};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PinError {
    #[error("a PIN is exactly 6 digits")]
    NotSixDigits,
    #[error("that PIN is too easy to guess; avoid repeated digits and runs like 123456")]
    TooEasy,
    #[error("could not protect the PIN: {0}")]
    Hash(String),
}

/// Checks a new PIN: exactly six digits, and not trivial (one digit
/// repeated, a run up or down, or two digits alternating like 121212).
pub fn validate_pin(pin: &str) -> Result<(), PinError> {
    if pin.len() != 6 || !pin.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PinError::NotSixDigits);
    }
    let d: Vec<i8> = pin.bytes().map(|b| (b - b'0') as i8).collect();
    let steps: Vec<i8> = d.windows(2).map(|w| w[1] - w[0]).collect();
    let same_step = steps.iter().all(|s| *s == steps[0]);
    let runs = same_step && matches!(steps[0], -1..=1);
    let alternating = d[0] == d[2] && d[2] == d[4] && d[1] == d[3] && d[3] == d[5];
    if runs || alternating {
        return Err(PinError::TooEasy);
    }
    Ok(())
}

/// Hashes a PIN or recovery code as an Argon2id PHC string.
pub fn hash_secret(secret: &str) -> Result<String, PinError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| PinError::Hash(e.to_string()))
}

/// True if `secret` matches `hash`. A damaged hash never matches.
pub fn verify_secret(secret: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(secret.as_bytes(), &h)
            .is_ok()
    })
}

/// Wrong PINs allowed before the first wait.
pub const FREE_ATTEMPTS: u32 = 5;

/// How long to wait after `failed` wrong PINs in a row, in seconds (0 = no
/// wait): 30 s, then 5 min, then 1 hour.
pub fn lockout_seconds(failed: u32) -> u64 {
    match failed {
        0..FREE_ATTEMPTS => 0,
        5..=9 => 30,
        10..=14 => 300,
        _ => 3600,
    }
}

/// A recovery code like "K7QM-2XPD-9HVA-TR4C": 16 characters from an
/// alphabet without 0/O or 1/I, about 80 bits.
pub fn new_recovery_code() -> String {
    const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let chars: Vec<char> = bytes
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect();
    chars
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Recovery codes are compared without dashes, spaces or case.
pub fn normalise_code(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_must_be_six_digits_and_not_trivial() {
        assert_eq!(validate_pin("12345"), Err(PinError::NotSixDigits));
        assert_eq!(validate_pin("12345a"), Err(PinError::NotSixDigits));
        assert_eq!(validate_pin("１２３４５６"), Err(PinError::NotSixDigits));
        for easy in [
            "000000", "111111", "123456", "654321", "345678", "121212", "909090",
        ] {
            assert_eq!(validate_pin(easy), Err(PinError::TooEasy), "{easy}");
        }
        for ok in ["480715", "902364", "112233", "135790"] {
            assert!(validate_pin(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn hashes_verify_and_are_salted() {
        let a = hash_secret("480715").unwrap();
        let b = hash_secret("480715").unwrap();
        assert_ne!(a, b, "random salt");
        assert!(a.starts_with("$argon2id$"));
        assert!(verify_secret("480715", &a));
        assert!(!verify_secret("480716", &a));
        assert!(!verify_secret("480715", "not a hash"));
    }

    #[test]
    fn lockout_grows() {
        assert_eq!(lockout_seconds(4), 0);
        assert_eq!(lockout_seconds(5), 30);
        assert_eq!(lockout_seconds(10), 300);
        assert_eq!(lockout_seconds(20), 3600);
    }

    #[test]
    fn recovery_codes() {
        let c = new_recovery_code();
        assert_eq!(c.len(), 19);
        assert_eq!(normalise_code(&c.to_lowercase()), c.replace('-', ""));
        assert_ne!(new_recovery_code(), c);
    }
}
