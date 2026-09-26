//! A tiny, forgiving XML tree for the small metadata files inside ebooks
//! (container.xml, OPF, FB2 headers, ComicInfo.xml). Namespaces are ignored:
//! elements and attributes are matched by their local name, lowercased.

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct Element {
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub children: Vec<Element>,
    pub text: String,
}

fn local(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase()
}

fn resolve_ref(name: &str) -> String {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => num.parse().ok(),
        };
        return code
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default();
    }
    quick_xml::escape::resolve_predefined_entity(name)
        .map(str::to_owned)
        .or_else(|| {
            // A few HTML entities that appear in ebook descriptions.
            Some(
                match name {
                    "nbsp" => "\u{a0}",
                    "mdash" => "—",
                    "ndash" => "–",
                    "hellip" => "…",
                    "rsquo" => "’",
                    "lsquo" => "‘",
                    "rdquo" => "”",
                    "ldquo" => "“",
                    _ => "",
                }
                .to_owned(),
            )
        })
        .unwrap_or_default()
}

/// Parses `text` into a tree. Stops quietly at the first syntax error and
/// returns what was read so far; ebook metadata is often slightly broken.
pub fn parse(text: &str) -> Element {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Element> = vec![Element::default()];
    let open = |e: &quick_xml::events::BytesStart<'_>| {
        let mut el = Element {
            name: local(e.name().as_ref()),
            ..Default::default()
        };
        for attr in e.attributes().flatten() {
            if let Ok(v) = attr.normalized_value(XmlVersion::Implicit1_0) {
                el.attrs.insert(local(attr.key.as_ref()), v.into_owned());
            }
        }
        el
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(open(&e)),
            Ok(Event::Empty(e)) => {
                let el = open(&e);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(el);
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let el = stack.pop().expect("stack has a child");
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(el);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(el) = stack.last_mut() {
                    el.text.push_str(&t.xml10_content());
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(el) = stack.last_mut() {
                    el.text.push_str(&t.xml10_content());
                }
            }
            Ok(Event::GeneralRef(r)) => {
                if let Some(el) = stack.last_mut() {
                    el.text.push_str(&resolve_ref(&r.into_inner()));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    // Close anything left open by a truncated file.
    while stack.len() > 1 {
        let el = stack.pop().expect("stack has a child");
        if let Some(parent) = stack.last_mut() {
            parent.children.push(el);
        }
    }
    stack.pop().unwrap_or_default()
}

impl Element {
    /// First descendant (depth-first) with this local name.
    pub fn find(&self, name: &str) -> Option<&Element> {
        for c in &self.children {
            if c.name == name {
                return Some(c);
            }
            if let Some(found) = c.find(name) {
                return Some(found);
            }
        }
        None
    }

    /// Every descendant with this local name, in document order.
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Element>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.find_all(name, out);
        }
    }

    pub fn all(&self, name: &str) -> Vec<&Element> {
        let mut out = Vec::new();
        self.find_all(name, &mut out);
        out
    }

    /// Direct children with this local name.
    pub fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// All text inside this element, whitespace collapsed.
    pub fn text_deep(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push(' ');
            s.push_str(&c.text_deep());
        }
        collapse(&s)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }
}

pub fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Removes HTML tags from a description and tidies whitespace, keeping
/// paragraph breaks.
pub fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for ch in s.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim_start_matches('/').to_ascii_lowercase();
                if t.starts_with('p') || t.starts_with("br") || t.starts_with("div") {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(ch),
            _ => out.push(ch),
        }
    }
    // Descriptions are often double-escaped HTML.
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    out.lines()
        .map(collapse)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaced_and_broken_xml() {
        let root = parse(
            r#"<?xml version="1.0"?><package xmlns:dc="x"><metadata>
            <dc:title>War &amp; Peace</dc:title><dc:creator opf:role="aut">Leo</dc:creator>
            <meta name="cover" content="img"/><dc:description>unclosed"#,
        );
        assert_eq!(root.find("title").unwrap().text_deep(), "War & Peace");
        assert_eq!(root.find("creator").unwrap().attr("role"), Some("aut"));
        assert_eq!(root.all("meta").len(), 1);
        assert_eq!(root.find("description").unwrap().text_deep(), "unclosed");
    }

    #[test]
    fn strips_html_descriptions() {
        assert_eq!(
            strip_html("<p>One &amp; <b>two</b></p><p>Three</p>"),
            "One & two\n\nThree"
        );
    }
}
