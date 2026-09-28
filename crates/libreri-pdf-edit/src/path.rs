//! The subset of SVG path data the reader sends: absolute M, L, Q, C and Z
//! with numbers separated by spaces or commas.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Quad(f64, f64, f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

pub fn parse(d: &str) -> Result<Vec<Seg>> {
    let mut out = Vec::new();
    let mut tokens = tokenize(d).into_iter().peekable();
    let mut cmd = None;
    let bad = |m: &str| Error::Path(m.to_owned());
    while let Some(t) = tokens.peek().cloned() {
        if let Tok::Cmd(c) = t {
            tokens.next();
            cmd = Some(c);
            if c == 'Z' || c == 'z' {
                out.push(Seg::Close);
                continue;
            }
        }
        let mut num = || match tokens.next() {
            Some(Tok::Num(n)) if n.is_finite() => Ok(n),
            _ => Err(bad("a number was expected")),
        };
        match cmd.ok_or_else(|| bad("the path must start with a command"))? {
            'M' => {
                out.push(Seg::Move(num()?, num()?));
                // Pairs after a move are lines.
                cmd = Some('L');
            }
            'L' => out.push(Seg::Line(num()?, num()?)),
            'Q' => out.push(Seg::Quad(num()?, num()?, num()?, num()?)),
            'C' => out.push(Seg::Cubic(num()?, num()?, num()?, num()?, num()?, num()?)),
            c => return Err(bad(&format!("unsupported command {c}"))),
        }
        if out.len() > 500_000 {
            return Err(bad("the drawing is too large"));
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Cmd(char),
    Num(f64),
}

fn tokenize(d: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, out: &mut Vec<Tok>| {
        if !num.is_empty() {
            out.push(Tok::Num(num.parse().unwrap_or(f64::NAN)));
            num.clear();
        }
    };
    for ch in d.chars() {
        match ch {
            'M' | 'L' | 'Q' | 'C' | 'Z' | 'z' => {
                flush(&mut num, &mut out);
                out.push(Tok::Cmd(ch));
            }
            ' ' | ',' | '\n' | '\t' => flush(&mut num, &mut out),
            '-' if !num.is_empty() && !num.ends_with(['e', 'E']) => {
                flush(&mut num, &mut out);
                num.push(ch);
            }
            _ => num.push(ch),
        }
    }
    flush(&mut num, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paths() {
        let p = parse("M 0.1 0.2 L0.3,0.4 0.5 0.6 Q 1 2 3 4 C1 2 3 4 5 6 Z").unwrap();
        assert_eq!(
            p,
            vec![
                Seg::Move(0.1, 0.2),
                Seg::Line(0.3, 0.4),
                Seg::Line(0.5, 0.6),
                Seg::Quad(1.0, 2.0, 3.0, 4.0),
                Seg::Cubic(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
                Seg::Close
            ]
        );
        assert_eq!(parse("M1-2").unwrap(), vec![Seg::Move(1.0, -2.0)]);
        assert!(parse("1 2").is_err());
        assert!(parse("M 1").is_err());
        assert!(parse("A 1 2").is_err());
    }
}
