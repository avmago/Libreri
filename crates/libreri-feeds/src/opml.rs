//! OPML: the subscription lists feed readers import and export.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

/// A folder or a feed in an OPML file.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    FeedFolder {
        name: String,
        children: Vec<Node>,
    },
    Feed {
        title: String,
        url: String,
        site: Option<String>,
    },
}

/// Reads an OPML file's outline.
pub fn read(xml: &str) -> Result<Vec<Node>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // A stack of open folders; the bottom one holds the top level.
    let mut stack: Vec<(String, Vec<Node>)> = vec![(String::new(), Vec::new())];
    let mut in_body = false;
    let mut feeds = 0usize;
    loop {
        match reader.read_event() {
            Err(e) => return Err(format!("this OPML file could not be read ({e})")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) if e.name().as_ref() == b"body" => in_body = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"body" => in_body = false,
            Ok(Event::Start(e)) if in_body && e.name().as_ref() == b"outline" => {
                match outline(&e) {
                    Some(feed) => {
                        feeds += 1;
                        // A feed written with children: keep the feed, and
                        // treat what is inside as its own folder.
                        stack.last_mut().expect("stack").1.push(feed);
                        stack.push((String::new(), Vec::new()));
                    }
                    None => stack.push((title_of(&e), Vec::new())),
                }
            }
            Ok(Event::Empty(e)) if in_body && e.name().as_ref() == b"outline" => {
                if let Some(feed) = outline(&e) {
                    feeds += 1;
                    stack.last_mut().expect("stack").1.push(feed);
                }
            }
            Ok(Event::End(e)) if in_body && e.name().as_ref() == b"outline" && stack.len() > 1 => {
                let (name, children) = stack.pop().expect("stack");
                let parent = &mut stack.last_mut().expect("stack").1;
                if name.is_empty() {
                    parent.extend(children);
                } else if !children.is_empty() {
                    parent.push(Node::FeedFolder { name, children });
                }
            }
            _ => {}
        }
    }
    if feeds == 0 {
        return Err("there are no feeds in this OPML file".into());
    }
    while stack.len() > 1 {
        let (name, children) = stack.pop().expect("stack");
        let parent = &mut stack.last_mut().expect("stack").1;
        if name.is_empty() {
            parent.extend(children);
        } else {
            parent.push(Node::FeedFolder { name, children });
        }
    }
    Ok(stack.pop().map(|(_, c)| c).unwrap_or_default())
}

fn attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.as_ref().eq_ignore_ascii_case(name))
            .then(|| a.unescape_value().ok().map(|v| v.trim().to_owned()))
            .flatten()
    })
}

fn title_of(e: &BytesStart) -> String {
    attr(e, b"title")
        .filter(|t| !t.is_empty())
        .or_else(|| attr(e, b"text"))
        .unwrap_or_default()
}

fn outline(e: &BytesStart) -> Option<Node> {
    let url = attr(e, b"xmlUrl").filter(|u| u.starts_with("http"))?;
    let title = title_of(e);
    Some(Node::Feed {
        title: if title.is_empty() { url.clone() } else { title },
        url,
        site: attr(e, b"htmlUrl").filter(|u| u.starts_with("http")),
    })
}

/// Writes an OPML file.
pub fn write(title: &str, nodes: &[Node]) -> String {
    let mut w = Writer::new_with_indent(Vec::new(), b' ', 2);
    let _ = w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));
    let _ = w.write_event(Event::Start(
        BytesStart::new("opml").with_attributes([("version", "2.0")]),
    ));
    let _ = w.write_event(Event::Start(BytesStart::new("head")));
    let _ = w.write_event(Event::Start(BytesStart::new("title")));
    let _ = w.write_event(Event::Text(BytesText::new(title)));
    let _ = w.write_event(Event::End(BytesEnd::new("title")));
    let _ = w.write_event(Event::End(BytesEnd::new("head")));
    let _ = w.write_event(Event::Start(BytesStart::new("body")));
    fn put(w: &mut Writer<Vec<u8>>, nodes: &[Node]) {
        for n in nodes {
            match n {
                Node::FeedFolder { name, children } => {
                    let e = BytesStart::new("outline")
                        .with_attributes([("text", name.as_str()), ("title", name.as_str())]);
                    let _ = w.write_event(Event::Start(e));
                    put(w, children);
                    let _ = w.write_event(Event::End(BytesEnd::new("outline")));
                }
                Node::Feed { title, url, site } => {
                    let mut e = BytesStart::new("outline").with_attributes([
                        ("type", "rss"),
                        ("text", title.as_str()),
                        ("title", title.as_str()),
                        ("xmlUrl", url.as_str()),
                    ]);
                    if let Some(s) = site {
                        e.push_attribute(("htmlUrl", s.as_str()));
                    }
                    let _ = w.write_event(Event::Empty(e));
                }
            }
        }
    }
    put(&mut w, nodes);
    let _ = w.write_event(Event::End(BytesEnd::new("body")));
    let _ = w.write_event(Event::End(BytesEnd::new("opml")));
    String::from_utf8(w.into_inner()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes() {
        let xml = r#"<?xml version="1.0"?>
<opml version="1.0"><head><title>Subs</title></head><body>
  <outline text="Science">
    <outline text="Physics">
      <outline type="rss" text="hep-th" xmlUrl="https://rss.arxiv.org/rss/hep-th"/>
    </outline>
    <outline type="rss" title="Nature" text="nat" xmlUrl="https://www.nature.com/nature.rss" htmlUrl="https://www.nature.com"/>
  </outline>
  <outline type="rss" text="HN &amp; more" xmlUrl="https://news.ycombinator.com/rss"/>
  <outline text="Empty folder"></outline>
</body></opml>"#;
        let nodes = read(xml).unwrap();
        let expected = vec![
            Node::FeedFolder {
                name: "Science".into(),
                children: vec![
                    Node::FeedFolder {
                        name: "Physics".into(),
                        children: vec![Node::Feed {
                            title: "hep-th".into(),
                            url: "https://rss.arxiv.org/rss/hep-th".into(),
                            site: None,
                        }],
                    },
                    Node::Feed {
                        title: "Nature".into(),
                        url: "https://www.nature.com/nature.rss".into(),
                        site: Some("https://www.nature.com".into()),
                    },
                ],
            },
            Node::Feed {
                title: "HN & more".into(),
                url: "https://news.ycombinator.com/rss".into(),
                site: None,
            },
        ];
        assert_eq!(nodes, expected);
        let again = read(&write("Libreri", &nodes)).unwrap();
        assert_eq!(again, expected);
        assert!(read("<opml><body></body></opml>").is_err());
    }
}
