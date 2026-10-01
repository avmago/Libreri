//! RTF: turned into Markdown, which Libreri reads, searches and marks like
//! any Markdown book.
//!
//! Kept: paragraphs, headings (from the document's heading styles or
//! outline levels, or large bold lines), bold, italic and struck-through
//! text, bulleted and numbered lists, tables, links and line breaks; the
//! title, author, subject and keywords from the document's `\info`.
//! Left out: fonts, colours, sizes, pictures, headers and footers.

use crate::{split_keywords, split_people, Extracted};
use encoding_rs::Encoding;
use std::collections::HashMap;
use std::path::Path;

/// RTF files larger than this are not converted (they are mostly pictures).
const MAX_RTF: u64 = 64 * 1024 * 1024;

/// What `\info` says about the document.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RtfInfo {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
}

/// Reads an RTF file as Markdown.
pub fn read_markdown(path: &Path) -> Result<String, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_RTF {
        return Err("the RTF file is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(to_markdown(&bytes).0)
}

pub(crate) fn read(path: &Path, out: &mut Extracted) -> Result<(), String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_RTF {
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if !bytes.starts_with(b"{\\rtf") {
        return Err("this is not an RTF file".into());
    }
    let (md, info) = to_markdown(&bytes);
    let m = &mut out.metadata;
    m.title = if info.title.trim().is_empty() {
        // The first heading, as for Markdown.
        md.lines()
            .find_map(|l| l.strip_prefix("# "))
            .map(|t| t.replace('\\', ""))
            .unwrap_or_default()
    } else {
        info.title.trim().to_owned()
    };
    if !info.author.trim().is_empty() {
        m.authors = split_people(&info.author);
    }
    if !info.subject.trim().is_empty() {
        m.about = Some(info.subject.trim().to_owned());
    }
    if !info.keywords.trim().is_empty() {
        m.tags = split_keywords(&info.keywords);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Dest {
    /// Shown text.
    Text,
    /// Not shown (fonts, colours, pictures, headers…).
    Skip,
    /// `\info` fields.
    Title,
    Author,
    Subject,
    Keywords,
    /// A field's instruction (`HYPERLINK "…"`).
    FieldInst,
    /// A list item's number or bullet.
    ListText,
    /// The style sheet: style numbers and names.
    StyleSheet,
}

#[derive(Debug, Clone)]
struct State {
    dest: Dest,
    style: Style,
    hidden: bool,
    /// Characters to skip after `\uN`.
    uc: usize,
    /// Font size in half-points.
    fs: u32,
    link: Option<String>,
    /// In the style sheet: the style this group defines.
    style_no: Option<u32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            dest: Dest::Text,
            style: Style::default(),
            hidden: false,
            uc: 1,
            fs: 24,
            link: None,
            style_no: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Run {
    text: String,
    style: Style,
    link: Option<String>,
    fs: u32,
}

#[derive(Debug, Default)]
struct Para {
    runs: Vec<Run>,
    /// Heading level from a style or `\outlinelevel` (1–6).
    heading: Option<u8>,
    in_table: bool,
    /// In a list (`\ls`), at this level (`\ilvl`).
    list: Option<u32>,
    list_text: String,
}

struct Converter {
    stack: Vec<State>,
    cur: State,
    para: Para,
    out: Vec<String>,
    /// Table rows (cells) waiting to be written.
    rows: Vec<Vec<String>>,
    cells: Vec<String>,
    cell_text: Vec<String>,
    info: RtfInfo,
    encoding: &'static Encoding,
    /// Bytes waiting to be decoded together (multi-byte code pages).
    bytes: Vec<u8>,
    skip: usize,
    /// A high surrogate from `\uN`, waiting for its pair.
    high: Option<u16>,
    field_inst: String,
    /// Heading levels of the document's styles (`\s2` → 1).
    heading_styles: HashMap<u32, u8>,
    style_name: String,
    style_s: u32,
}

/// Converts RTF to Markdown; also returns the document's `\info`.
pub fn to_markdown(rtf: &[u8]) -> (String, RtfInfo) {
    let mut c = Converter {
        stack: Vec::new(),
        cur: State::default(),
        para: Para::default(),
        out: Vec::new(),
        rows: Vec::new(),
        cells: Vec::new(),
        cell_text: Vec::new(),
        info: RtfInfo::default(),
        encoding: encoding_rs::WINDOWS_1252,
        bytes: Vec::new(),
        skip: 0,
        high: None,
        field_inst: String::new(),
        heading_styles: HashMap::new(),
        style_name: String::new(),
        style_s: 0,
    };
    c.run(rtf);
    c.end_para();
    c.flush_table();
    let mut md = c.out.join("\n\n");
    md.push('\n');
    (md, c.info)
}

fn is_letter(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

impl Converter {
    fn run(&mut self, s: &[u8]) {
        let mut i = 0;
        // Ignore anything before the first group.
        while i < s.len() && s[i] != b'{' {
            i += 1;
        }
        while i < s.len() {
            let b = s[i];
            match b {
                b'{' => {
                    self.flush_bytes();
                    self.stack.push(self.cur.clone());
                    i += 1;
                }
                b'}' => {
                    self.flush_bytes();
                    self.close_group();
                    i += 1;
                }
                b'\\' => {
                    i += 1;
                    let Some(&n) = s.get(i) else { break };
                    if is_letter(n) {
                        let start = i;
                        while i < s.len() && is_letter(s[i]) {
                            i += 1;
                        }
                        let word = std::str::from_utf8(&s[start..i]).unwrap_or("").to_owned();
                        let num_start = i;
                        if i < s.len() && s[i] == b'-' {
                            i += 1;
                        }
                        while i < s.len() && s[i].is_ascii_digit() {
                            i += 1;
                        }
                        let param = std::str::from_utf8(&s[num_start..i])
                            .ok()
                            .and_then(|p| p.parse::<i64>().ok());
                        if i < s.len() && s[i] == b' ' {
                            i += 1;
                        }
                        if word == "bin" {
                            // Binary data: skip it whole.
                            i += param.unwrap_or(0).max(0) as usize;
                            continue;
                        }
                        self.flush_bytes();
                        self.word(&word, param);
                    } else {
                        i += 1;
                        match n {
                            b'\'' => {
                                let hex = s.get(i..i + 2).and_then(|h| std::str::from_utf8(h).ok());
                                if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                                    self.byte(v);
                                }
                                i += 2;
                            }
                            b'*' => {
                                // An optional destination: skipped unless known.
                                self.flush_bytes();
                                if !matches!(self.peek_word(s, i).as_deref(), Some("fldinst")) {
                                    self.cur.dest = Dest::Skip;
                                }
                            }
                            b'\\' | b'{' | b'}' => self.byte(n),
                            b'~' => self.text("\u{a0}"),
                            b'_' => self.text("\u{2011}"),
                            b'-' => {}
                            b'\n' | b'\r' => self.word("par", None),
                            b'\t' => self.text("\t"),
                            _ => {}
                        }
                    }
                }
                b'\r' | b'\n' => i += 1,
                _ => {
                    self.byte(b);
                    i += 1;
                }
            }
        }
        self.flush_bytes();
    }

    fn peek_word(&self, s: &[u8], mut i: usize) -> Option<String> {
        while i < s.len() && s[i].is_ascii_whitespace() {
            i += 1;
        }
        if s.get(i) != Some(&b'\\') {
            return None;
        }
        let start = i + 1;
        let mut j = start;
        while j < s.len() && is_letter(s[j]) {
            j += 1;
        }
        std::str::from_utf8(&s[start..j]).ok().map(str::to_owned)
    }

    fn close_group(&mut self) {
        if self.cur.dest == Dest::StyleSheet
            && (self.cur.style_no.is_some() || !self.style_name.is_empty())
        {
            self.end_style();
        }
        self.cur = self.stack.pop().unwrap_or_default();
    }

    /// A style of the style sheet ends: is it a heading?
    fn end_style(&mut self) {
        let name = self.style_name.trim().trim_end_matches(';').to_lowercase();
        let level = name
            .strip_prefix("heading")
            .or_else(|| name.strip_prefix("überschrift"))
            .or_else(|| name.strip_prefix("titre"))
            .and_then(|r| r.trim().parse::<u8>().ok())
            .or(match name.as_str() {
                "title" => Some(1),
                "subtitle" => Some(2),
                _ => None,
            });
        if let Some(l) = level {
            self.heading_styles.insert(self.style_s, l.clamp(1, 6));
        }
        self.style_name.clear();
        self.style_s = 0;
    }

    fn word(&mut self, w: &str, p: Option<i64>) {
        let on = p != Some(0);
        match w {
            // Destinations not shown.
            "fonttbl" | "colortbl" | "pict" | "object" | "header" | "footer" | "headerl"
            | "headerr" | "headerf" | "footerl" | "footerr" | "footerf" | "listtable"
            | "listoverridetable" | "rsidtbl" | "generator" | "xmlnsdecl" | "themedata"
            | "colorschememapping" | "datastore" | "latentstyles" | "pgdsctbl" | "footnote"
            | "annotation" | "atnid" | "atnauthor" | "comment" | "operator" | "company"
            | "doccomm" | "revtbl" | "filetbl" | "nonshppict" | "shp" | "xe" | "tc" => {
                self.cur.dest = Dest::Skip;
            }
            "info" => self.cur.dest = Dest::Skip,
            "title" => self.cur.dest = Dest::Title,
            "author" => self.cur.dest = Dest::Author,
            "subject" => self.cur.dest = Dest::Subject,
            "keywords" => self.cur.dest = Dest::Keywords,
            "stylesheet" => self.cur.dest = Dest::StyleSheet,
            "fldinst" => {
                self.cur.dest = Dest::FieldInst;
                self.field_inst.clear();
            }
            "fldrslt" => {
                self.cur.dest = Dest::Text;
                self.cur.link = hyperlink(&self.field_inst);
            }
            "listtext" | "pntext" => {
                self.cur.dest = Dest::ListText;
                self.para.list_text.clear();
            }
            "ansicpg" => {
                if let Some(cp) = p {
                    if let Some(e) = Encoding::for_label(format!("windows-{cp}").as_bytes())
                        .or_else(|| Encoding::for_label(format!("cp{cp}").as_bytes()))
                    {
                        self.encoding = e;
                    }
                    if cp == 932 {
                        self.encoding = encoding_rs::SHIFT_JIS;
                    } else if cp == 936 {
                        self.encoding = encoding_rs::GBK;
                    } else if cp == 949 {
                        self.encoding = encoding_rs::EUC_KR;
                    } else if cp == 950 {
                        self.encoding = encoding_rs::BIG5;
                    } else if cp == 65001 {
                        self.encoding = encoding_rs::UTF_8;
                    }
                }
            }
            "mac" => self.encoding = encoding_rs::MACINTOSH,
            "s" => {
                if self.cur.dest == Dest::StyleSheet {
                    self.cur.style_no = p.map(|v| v as u32);
                    self.style_s = p.unwrap_or(0) as u32;
                } else if self.cur.dest == Dest::Text {
                    if let Some(l) = p.and_then(|v| self.heading_styles.get(&(v as u32))) {
                        self.para.heading = Some(*l);
                    }
                }
            }
            "outlinelevel" => {
                if let Some(l) = p.filter(|l| (0..6).contains(l)) {
                    self.para.heading = Some(l as u8 + 1);
                }
            }
            "pard" => {
                self.para.heading = None;
                self.para.in_table = false;
                self.para.list = None;
            }
            "intbl" => self.para.in_table = true,
            "ls" => {
                self.para.list.get_or_insert(0);
            }
            "ilvl" => self.para.list = Some(p.unwrap_or(0).max(0) as u32),
            "par" | "sect" | "page" if self.cur.dest == Dest::Text => {
                if self.para.in_table {
                    self.cell_break();
                } else {
                    self.end_para();
                }
            }
            "line" => self.text("\n"),
            "tab" => self.text("\t"),
            "cell" | "nestcell" => self.end_cell(),
            "row" | "nestrow" => self.end_row(),
            "plain" => {
                self.cur.style = Style::default();
                self.cur.hidden = false;
                self.cur.fs = 24;
            }
            "b" => self.cur.style.bold = on,
            "i" => self.cur.style.italic = on,
            "strike" | "striked" => self.cur.style.strike = on,
            "v" => self.cur.hidden = on,
            "fs" => self.cur.fs = p.unwrap_or(24).clamp(1, 2000) as u32,
            "uc" => self.cur.uc = p.unwrap_or(1).max(0) as usize,
            "u" => {
                if let Some(v) = p {
                    let unit = if v < 0 { (v + 65536) as u16 } else { v as u16 };
                    self.unicode(unit);
                    self.skip = self.cur.uc;
                }
            }
            "emdash" => self.text("—"),
            "endash" => self.text("–"),
            "bullet" => self.text("•"),
            "lquote" => self.text("‘"),
            "rquote" => self.text("’"),
            "ldblquote" => self.text("“"),
            "rdblquote" => self.text("”"),
            "emspace" | "enspace" | "qmspace" => self.text(" "),
            _ => {}
        }
    }

    fn unicode(&mut self, unit: u16) {
        if (0xD800..0xDC00).contains(&unit) {
            self.high = Some(unit);
            return;
        }
        let c = if (0xDC00..0xE000).contains(&unit) {
            match self.high.take() {
                Some(h) => {
                    char::from_u32(0x10000 + (((h as u32) - 0xD800) << 10) + (unit as u32 - 0xDC00))
                }
                None => None,
            }
        } else {
            char::from_u32(unit as u32)
        };
        if let Some(c) = c {
            self.text(&c.to_string());
        }
    }

    fn byte(&mut self, b: u8) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        self.bytes.push(b);
    }

    fn flush_bytes(&mut self) {
        if self.bytes.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.bytes);
        let (text, _, _) = self.encoding.decode(&bytes);
        let text = text.into_owned();
        self.text(&text);
    }

    fn text(&mut self, t: &str) {
        // A `\u` replacement character counts as one to skip.
        let t = if self.skip > 0 && !t.is_empty() {
            let mut cs = t.chars();
            while self.skip > 0 && cs.next().is_some() {
                self.skip -= 1;
            }
            cs.as_str().to_owned()
        } else {
            t.to_owned()
        };
        if t.is_empty() {
            return;
        }
        match self.cur.dest {
            Dest::Skip => {}
            Dest::Title => self.info.title.push_str(&t),
            Dest::Author => self.info.author.push_str(&t),
            Dest::Subject => self.info.subject.push_str(&t),
            Dest::Keywords => self.info.keywords.push_str(&t),
            Dest::FieldInst => self.field_inst.push_str(&t),
            Dest::ListText => self.para.list_text.push_str(&t),
            Dest::StyleSheet => self.style_name.push_str(&t),
            Dest::Text => {
                if self.cur.hidden {
                    return;
                }
                let run = Run {
                    text: t,
                    style: self.cur.style,
                    link: self.cur.link.clone(),
                    fs: self.cur.fs,
                };
                match self.para.runs.last_mut() {
                    Some(last)
                        if last.style == run.style
                            && last.link == run.link
                            && last.fs == run.fs =>
                    {
                        last.text.push_str(&run.text);
                    }
                    _ => self.para.runs.push(run),
                }
            }
        }
    }

    /// A paragraph inside a table cell.
    fn cell_break(&mut self) {
        let t = render_runs(&std::mem::take(&mut self.para.runs), true);
        if !t.trim().is_empty() {
            self.cell_text.push(t.trim().to_owned());
        }
    }

    fn end_cell(&mut self) {
        self.flush_bytes();
        self.cell_break();
        let text = std::mem::take(&mut self.cell_text).join(" ");
        self.cells.push(text);
        self.para.list_text.clear();
    }

    fn end_row(&mut self) {
        if !self.cells.is_empty() {
            let row = std::mem::take(&mut self.cells);
            self.rows.push(row);
        }
    }

    fn flush_table(&mut self) {
        if !self.cells.is_empty() {
            self.end_row();
        }
        if self.rows.is_empty() {
            return;
        }
        let rows = std::mem::take(&mut self.rows);
        let width = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let line = |cells: &[String]| {
            let mut l = String::from("|");
            for i in 0..width {
                l.push(' ');
                l.push_str(
                    &cells
                        .get(i)
                        .cloned()
                        .unwrap_or_default()
                        .replace('|', "\\|"),
                );
                l.push_str(" |");
            }
            l
        };
        let mut table = vec![line(&rows[0])];
        table.push(format!("|{}", " --- |".repeat(width)));
        for r in &rows[1..] {
            table.push(line(r));
        }
        self.out.push(table.join("\n"));
    }

    fn end_para(&mut self) {
        let mut runs = std::mem::take(&mut self.para.runs);
        let mut list_text = std::mem::take(&mut self.para.list_text);
        // Lists typed as text: "•<tab>item", "1.<tab>item".
        if list_text.trim().is_empty() && self.para.list.is_none() {
            if let Some((marker, len)) = typed_marker(&runs) {
                list_text = marker;
                strip_chars(&mut runs, len);
            }
        }
        let text = render_runs(&runs, false);
        if text.trim().is_empty() {
            return;
        }
        // A paragraph after a table ends it.
        self.flush_table();
        let level = self.para.heading.or_else(|| looks_like_heading(&runs));
        let block = if let Some(l) = level.filter(|_| self.para.list.is_none()) {
            let plain = render_runs(&strip_bold(&runs), false);
            format!(
                "{} {}",
                "#".repeat(l as usize),
                plain.trim().replace('\n', " ")
            )
        } else if self.para.list.is_some() || !list_text.trim().is_empty() {
            let depth = self.para.list.unwrap_or(0) as usize;
            let marker = list_text.trim();
            let numbered = marker
                .trim_end_matches(['.', ')'])
                .chars()
                .all(|c| c.is_ascii_digit())
                && !marker.is_empty();
            let prefix = if numbered {
                format!("{}.", marker.trim_end_matches(['.', ')']))
            } else {
                "-".to_owned()
            };
            format!("{}{} {}", "  ".repeat(depth), prefix, text.trim())
        } else {
            text.trim().to_owned()
        };
        // Items of one list stay together (a new kind of list starts apart).
        let kind = list_kind(&block);
        if kind.is_some() {
            if let Some(last) = self.out.last_mut() {
                if last.lines().last().map(list_kind) == Some(kind) {
                    last.push('\n');
                    last.push_str(&block);
                    return;
                }
            }
        }
        self.out.push(block);
    }
}

/// "-" or "1." for a list item's line, at any depth.
fn list_kind(line: &str) -> Option<&'static str> {
    let l = line.trim_start();
    if l.starts_with("- ") {
        Some("-")
    } else if starts_numbered(l) {
        Some("1.")
    } else {
        None
    }
}

/// A bullet or number typed at the start of a paragraph, and its length
/// (in characters, with the space or tab after it).
fn typed_marker(runs: &[Run]) -> Option<(String, usize)> {
    let raw: String = runs.iter().map(|r| r.text.as_str()).collect();
    let lead = raw.chars().take_while(|c| *c == ' ').count();
    let t: String = raw.chars().skip(lead).collect();
    let mut cs = t.chars();
    let first = cs.next()?;
    if matches!(first, '•' | '◦' | '▪' | '·' | '‣' | '○' | '■') {
        let next = cs.next()?;
        return matches!(next, '\t' | ' ').then(|| ("•".to_owned(), lead + 2));
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits < 4 {
        let rest: String = t.chars().skip(digits).take(2).collect();
        if rest == ".\t" || rest == ")\t" {
            return Some((format!("{}.", &t[..digits]), lead + digits + 2));
        }
    }
    None
}

/// Removes the first `n` characters of a paragraph's text.
fn strip_chars(runs: &mut Vec<Run>, mut n: usize) {
    while n > 0 && !runs.is_empty() {
        let len = runs[0].text.chars().count();
        if len <= n {
            runs.remove(0);
            n -= len;
        } else {
            runs[0].text = runs[0].text.chars().skip(n).collect();
            n = 0;
        }
    }
}

fn starts_numbered(s: &str) -> bool {
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && s[digits..].starts_with(". ")
}

/// `HYPERLINK "https://…"` → the address.
fn hyperlink(inst: &str) -> Option<String> {
    let rest = inst.trim().strip_prefix("HYPERLINK")?.trim();
    let url = if let Some(q) = rest.strip_prefix('"') {
        q.split('"').next()?
    } else {
        rest.split_whitespace().next()?
    };
    let ok =
        url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:");
    ok.then(|| url.to_owned())
}

/// A short line all in bold or in large type reads as a heading.
fn looks_like_heading(runs: &[Run]) -> Option<u8> {
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    let t = text.trim();
    if t.is_empty()
        || t.chars().count() > 100
        || t.contains('\n')
        || t.ends_with(['.', ',', ':', ';'])
    {
        return None;
    }
    let visible: Vec<&Run> = runs.iter().filter(|r| !r.text.trim().is_empty()).collect();
    let min_fs = visible.iter().map(|r| r.fs).min()?;
    let all_bold = visible.iter().all(|r| r.style.bold);
    match min_fs {
        36.. => Some(1),
        30..=35 => Some(2),
        26..=29 if all_bold => Some(3),
        _ => None,
    }
}

fn strip_bold(runs: &[Run]) -> Vec<Run> {
    runs.iter()
        .map(|r| Run {
            style: Style {
                bold: false,
                ..r.style
            },
            ..r.clone()
        })
        .collect()
}

/// Characters that mean something in Markdown.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '#' | '|' | '~' | '$'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Runs as Markdown: bold, italic, struck through and linked text.
fn render_runs(runs: &[Run], in_cell: bool) -> String {
    let mut out = String::new();
    for r in runs {
        let text = r.text.replace('\t', " ");
        // Spaces stay outside the markers (`** a**` is not bold).
        let lead = text.len() - text.trim_start().len();
        let trail = text.len() - text.trim_end().len();
        let core = text.trim();
        if core.is_empty() {
            out.push_str(&text);
            continue;
        }
        let mut s = escape(core);
        if in_cell {
            s = s.replace('\n', " ");
        } else {
            s = s.replace('\n', "  \n");
        }
        if r.style.strike {
            s = format!("~~{s}~~");
        }
        if r.style.italic {
            s = format!("*{s}*");
        }
        if r.style.bold {
            s = format!("**{s}**");
        }
        if let Some(url) = &r.link {
            s = format!("[{s}]({url})");
        }
        out.push_str(&text[..lead]);
        out.push_str(&s);
        out.push_str(&text[text.len() - trail..]);
    }
    // Text that would read as a list item at the start of a line.
    let lead = out.len() - out.trim_start().len();
    let t = &out[lead..];
    if t.starts_with("- ") || t.starts_with("+ ") {
        out.insert(lead, '\\');
    } else if starts_numbered(t) {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        out.insert(lead + digits, '\\');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(s: &str) -> String {
        to_markdown(s.as_bytes()).0
    }

    #[test]
    fn paragraphs_and_styles() {
        let r = md(
            r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times;}}{\colortbl;\red0\green0\blue0;}
\pard Hello {\b bold} and {\i italic} world.\par
Second \b strong\b0  text.\par}",
        );
        assert_eq!(
            r,
            "Hello **bold** and *italic* world.\n\nSecond **strong** text.\n"
        );
    }

    #[test]
    fn headings_from_styles_and_size() {
        let r = md(
            r"{\rtf1\ansi{\stylesheet{\s0 Normal;}{\s1\b\fs32 heading 1;}}
\pard\s1 Chapter One\par
\pard\s0 Text.\par
\pard\fs40\b Big Title\b0\fs24\par}",
        );
        assert!(r.contains("# Chapter One"), "{r}");
        assert!(r.contains("Text."), "{r}");
        assert!(r.contains("# Big Title"), "{r}");
    }

    #[test]
    fn unicode_hex_and_code_page() {
        let r = md(r"{\rtf1\ansi\ansicpg1252\uc1 Caf\'e9 \u8364? \u-10179?\u-8704? done\par}");
        assert_eq!(r, "Café € 😀 done\n");
    }

    #[test]
    fn typed_lists() {
        let r = md("{\\rtf1 \\bullet\\tab One\\par \\bullet\\tab Two\\par 1.\\tab Three\\par 2.\\tab Four\\par}");
        assert_eq!(r, "- One\n- Two\n\n1. Three\n2. Four\n");
    }

    #[test]
    fn lists_tables_and_links() {
        let r = md(r#"{\rtf1\ansi
{\listtext\'95\tab}\pard\ls1 First\par
{\listtext\'95\tab}\pard\ls1 Second\par
\pard {\field{\*\fldinst HYPERLINK "https://example.org"}{\fldrslt Example}}\par
\trowd\cellx1000\cellx2000\pard\intbl A\cell B\cell\row
\trowd\cellx1000\cellx2000\pard\intbl 1\cell 2\cell\row
\pard After.\par}"#);
        assert!(r.contains("- First\n- Second"), "{r}");
        assert!(r.contains("[Example](https://example.org)"), "{r}");
        assert!(r.contains("| A | B |\n| --- | --- |\n| 1 | 2 |"), "{r}");
        assert!(r.ends_with("After.\n"), "{r}");
    }

    #[test]
    fn info_and_hidden_parts() {
        let (r, info) = to_markdown(
            br"{\rtf1{\info{\title My Paper}{\author Ada Lovelace}{\keywords maths; engines}}{\*\generator Word;}{\header Page}\pard Body {\v secret}text\par}",
        );
        assert_eq!(info.title, "My Paper");
        assert_eq!(info.author, "Ada Lovelace");
        assert_eq!(r, "Body text\n");
    }

    #[test]
    fn escapes_markdown() {
        assert_eq!(
            md(r"{\rtf1 2 * 3 = 6 #1 [x]\par}"),
            "2 \\* 3 = 6 \\#1 \\[x\\]\n"
        );
    }
}
