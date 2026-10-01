//! Turning what a model read in a region into plain text for search and
//! reading aloud.

/// A table the model wrote as HTML: one line per row, cells apart.
pub fn html_table_text(html: &str) -> String {
    let mut out = String::new();
    let mut cell = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        cell.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
        let name = tag
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("");
        match name {
            "td" | "th" if tag.starts_with('/') => {
                row.push(unescape(cell.trim()));
                cell.clear();
            }
            "tr" if tag.starts_with('/') => {
                if !cell.trim().is_empty() {
                    row.push(unescape(cell.trim()));
                }
                cell.clear();
                let line = row.join("  ").trim().to_owned();
                if !line.is_empty() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&line);
                }
                row.clear();
            }
            "br" => cell.push(' '),
            _ => {}
        }
        rest = &rest[start + end + 1..];
    }
    cell.push_str(rest);
    if !row.is_empty() || !cell.trim().is_empty() {
        row.push(unescape(cell.trim()));
        let line = row.join("  ").trim().to_owned();
        if !line.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&line);
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// The text of a region: tables as rows, the rest as written. Pictures
/// have none.
pub fn region_text(kind: &str, text: &str) -> String {
    let t = text.trim();
    match kind {
        "image" | "header_image" | "footer_image" | "seal" => String::new(),
        "table" if t.contains('<') => html_table_text(t),
        _ => t.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_become_rows() {
        let html = "<table><tr><th>Name</th><th>Value</th></tr><tr><td>a &amp; b</td><td>1</td></tr></table>";
        assert_eq!(html_table_text(html), "Name  Value\na & b  1");
    }

    #[test]
    fn regions() {
        assert_eq!(region_text("image", "x"), "");
        assert_eq!(region_text("text", "  Hello\nworld "), "Hello\nworld");
        assert_eq!(region_text("table", "<tr><td>1</td></tr>"), "1");
    }
}
