//! HTML-to-Markdown conversion.

use super::html::{decode_entities, extract_attr, strip_scripts_and_styles, strip_tags};

/// Convert HTML to a simple Markdown representation.
pub fn html_to_markdown(html: &str) -> String {
    let cleaned = strip_scripts_and_styles(html);
    let mut result = cleaned.clone();

    // Headings
    for level in (1..=6).rev() {
        let open = format!("<h{}", level);
        let close = format!("</h{}>", level);
        let prefix = "#".repeat(level);
        result = replace_tag_pair(&result, &open, &close, &format!("{} ", prefix), "\n\n");
    }

    // Bold
    result = replace_tag_pair(&result, "<strong", "</strong>", "**", "**");
    result = replace_tag_pair(&result, "<b>", "</b>", "**", "**");
    result = replace_tag_pair(&result, "<b ", "</b>", "**", "**");

    // Italic
    result = replace_tag_pair(&result, "<em", "</em>", "*", "*");
    result = replace_tag_pair(&result, "<i>", "</i>", "*", "*");
    result = replace_tag_pair(&result, "<i ", "</i>", "*", "*");

    // Line breaks and paragraphs
    result = result.replace("<br>", "\n");
    result = result.replace("<br/>", "\n");
    result = result.replace("<br />", "\n");
    result = replace_tag_pair(&result, "<p", "</p>", "", "\n\n");

    // List items
    result = replace_tag_pair(&result, "<li", "</li>", "- ", "\n");

    // Links: extract href and text
    result = convert_links_to_markdown(&result);

    // Strip remaining tags
    let mut out = String::with_capacity(result.len());
    let mut in_tag = false;
    for ch in result.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }

    out = decode_entities(&out);

    // Collapse multiple blank lines
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }

    out.trim().to_string()
}

/// Replace HTML tag pairs with prefix/suffix strings.
fn replace_tag_pair(html: &str, open: &str, close: &str, prefix: &str, suffix: &str) -> String {
    let lower = html.to_lowercase();
    let mut result = String::with_capacity(html.len());
    let mut search_from = 0;

    while search_from < html.len() {
        let tag_start = match lower[search_from..].find(open) {
            Some(p) => search_from + p,
            None => {
                result.push_str(&html[search_from..]);
                break;
            }
        };

        result.push_str(&html[search_from..tag_start]);

        let content_start = match lower[tag_start..].find('>') {
            Some(p) => tag_start + p + 1,
            None => {
                result.push_str(&html[tag_start..]);
                break;
            }
        };

        let close_start = match lower[content_start..].find(close) {
            Some(p) => content_start + p,
            None => {
                result.push_str(&html[tag_start..]);
                break;
            }
        };

        result.push_str(prefix);
        result.push_str(&html[content_start..close_start]);
        result.push_str(suffix);

        search_from = close_start + close.len();
    }

    result
}

/// Convert <a href="...">text</a> to [text](url) markdown links.
fn convert_links_to_markdown(html: &str) -> String {
    let lower = html.to_lowercase();
    let mut result = String::with_capacity(html.len());
    let mut search_from = 0;

    while search_from < html.len() {
        let a_start = match lower[search_from..].find("<a ") {
            Some(p) => search_from + p,
            None => match lower[search_from..].find("<a>") {
                Some(p) => search_from + p,
                None => {
                    result.push_str(&html[search_from..]);
                    break;
                }
            },
        };

        result.push_str(&html[search_from..a_start]);

        let tag_end = match lower[a_start..].find('>') {
            Some(p) => a_start + p + 1,
            None => {
                result.push_str(&html[a_start..]);
                break;
            }
        };

        let close_start = match lower[tag_end..].find("</a>") {
            Some(p) => tag_end + p,
            None => {
                result.push_str(&html[a_start..]);
                break;
            }
        };

        let href = extract_attr(&html[a_start..tag_end], "href").unwrap_or_default();
        let text = strip_tags(&html[tag_end..close_start]);

        if href.is_empty() {
            result.push_str(&text);
        } else {
            result.push('[');
            result.push_str(&text);
            result.push_str("](");
            result.push_str(&href);
            result.push(')');
        }

        search_from = close_start + 4;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_markdown_headings() {
        let html = "<h1>Title</h1><p>Some text</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("# Title"));
        assert!(md.contains("Some text"));
    }
}
