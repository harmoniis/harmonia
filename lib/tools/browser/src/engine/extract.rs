//! HTML structured data extraction: title, links, headings, meta, lists.

use serde_json::{json, Value};
use std::collections::HashMap;

use super::extract_tables::{extract_forms, extract_lists, extract_tables};
use super::html::{extract_attr, strip_tags};

/// Extract the page title from HTML.
pub fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")?;
    let content_start = start + 7;
    let end_rel = lower[content_start..].find("</title>")?;
    let title = html[content_start..content_start + end_rel].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

/// Extract all href values from anchor tags.
pub fn extract_links(html: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut search_from = 0;

    while search_from < html.len() {
        let rest = &html[search_from..];

        let href_pos = match rest.find("href=\"") {
            Some(p) => p,
            None => break,
        };

        let value_start = search_from + href_pos + 6;
        if value_start >= html.len() {
            break;
        }

        let value_rest = &html[value_start..];
        let value_end = match value_rest.find('"') {
            Some(p) => p,
            None => break,
        };

        let link = &html[value_start..value_start + value_end];
        if !link.is_empty() {
            links.push(link.to_string());
        }

        search_from = value_start + value_end + 1;
    }

    links
}

/// Extract headings (h1-h6) with their level.
pub fn extract_headings(html: &str) -> Vec<(u8, String)> {
    let mut headings = Vec::new();
    let lower = html.to_lowercase();

    for level in 1..=6u8 {
        let open_tag = format!("<h{}", level);
        let close_tag = format!("</h{}>", level);
        let mut search_from = 0;

        while search_from < lower.len() {
            let tag_start = match lower[search_from..].find(&open_tag) {
                Some(p) => search_from + p,
                None => break,
            };

            let content_start = match lower[tag_start..].find('>') {
                Some(p) => tag_start + p + 1,
                None => break,
            };

            let content_end = match lower[content_start..].find(&close_tag) {
                Some(p) => content_start + p,
                None => break,
            };

            let text = strip_tags(&html[content_start..content_end])
                .trim()
                .to_string();
            if !text.is_empty() {
                headings.push((level, text));
            }

            search_from = content_end + close_tag.len();
        }
    }

    headings
}

/// Extract meta tags as key-value pairs.
pub fn extract_meta(html: &str) -> HashMap<String, String> {
    let mut meta = HashMap::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    while search_from < lower.len() {
        let meta_start = match lower[search_from..].find("<meta") {
            Some(p) => search_from + p,
            None => break,
        };

        let meta_end = match lower[meta_start..].find('>') {
            Some(p) => meta_start + p + 1,
            None => break,
        };

        let tag = &html[meta_start..meta_end];
        let name = extract_attr(tag, "name")
            .or_else(|| extract_attr(tag, "property"))
            .unwrap_or_default();
        let content = extract_attr(tag, "content").unwrap_or_default();

        if !name.is_empty() && !content.is_empty() {
            meta.insert(name, content);
        }

        search_from = meta_end;
    }

    meta
}

/// Extract structured data based on a hint string.
///
/// Supported hints: "tables", "headings", "lists", "forms", "meta"
pub fn extract_structured(html: &str, selector_hint: &str) -> Value {
    match selector_hint.to_lowercase().as_str() {
        "tables" => json!(extract_tables(html)),
        "headings" => {
            let headings = extract_headings(html);
            json!(headings
                .iter()
                .map(|(level, text)| json!({"level": level, "text": text}))
                .collect::<Vec<_>>())
        }
        "lists" => json!(extract_lists(html)),
        "forms" => {
            let forms = extract_forms(html);
            json!(forms
                .iter()
                .map(|f| json!({
                    "action": f.action,
                    "method": f.method,
                    "fields": f.fields.iter().map(|field| json!({
                        "name": field.name,
                        "field_type": field.field_type,
                        "placeholder": field.placeholder,
                    })).collect::<Vec<_>>()
                }))
                .collect::<Vec<_>>())
        }
        "meta" => json!(extract_meta(html)),
        _ => json!({"error": format!("unknown selector hint: {}", selector_hint)}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_title_works() {
        let html = "<html><head><title>Hello World</title></head><body></body></html>";
        assert_eq!(extract_title(html), Some("Hello World".to_string()));
    }

    #[test]
    fn extract_title_missing() {
        assert_eq!(extract_title("<html><body>no title</body></html>"), None);
    }

    #[test]
    fn extract_links_works() {
        let html = r#"<a href="https://example.com">Example</a> <a href="/about">About</a>"#;
        let links = extract_links(html);
        assert_eq!(links, vec!["https://example.com", "/about"]);
    }

    #[test]
    fn extract_headings_finds_all_levels() {
        let html = "<h1>Title</h1><h2>Subtitle</h2><h3>Section</h3>";
        let headings = extract_headings(html);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0], (1, "Title".to_string()));
        assert_eq!(headings[1], (2, "Subtitle".to_string()));
        assert_eq!(headings[2], (3, "Section".to_string()));
    }

    #[test]
    fn extract_meta_works() {
        let html =
            r#"<meta name="description" content="A test page"><meta name="author" content="Test">"#;
        let meta = extract_meta(html);
        assert_eq!(meta.get("description").unwrap(), "A test page");
        assert_eq!(meta.get("author").unwrap(), "Test");
    }
}
