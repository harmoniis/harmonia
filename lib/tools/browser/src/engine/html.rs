//! HTML processing: tag stripping, text extraction, entity decoding.

/// Remove all `<script>` and `<style>` blocks from HTML.
pub fn strip_scripts_and_styles(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let bytes = html.as_bytes();
    let lower_bytes = lower.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 7 < len && lower_bytes[i] == b'<' {
            let rest = &lower[i..];
            if rest.starts_with("<script") {
                if let Some(end) = lower[i..].find("</script>") {
                    i += end + 9;
                    continue;
                }
            } else if rest.starts_with("<style") {
                if let Some(end) = lower[i..].find("</style>") {
                    i += end + 8;
                    continue;
                }
            }
        }
        result.push(html.as_bytes()[i] as char);
        i += 1;
    }

    result
}

/// Strip all HTML tags and return clean text.
pub fn extract_text(html: &str) -> String {
    let cleaned = strip_scripts_and_styles(html);
    let mut result = String::with_capacity(cleaned.len());
    let mut in_tag = false;
    let mut last_was_space = false;

    for ch in cleaned.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
            continue;
        }
        if in_tag {
            continue;
        }

        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }

    decode_entities(&result).trim().to_string()
}

// ---- Internal utilities ----

/// Strip all HTML tags from a string.
pub(crate) fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

/// Extract an attribute value from an HTML tag string.
pub(crate) fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let search = format!("{}=\"", attr);
    let pos = lower.find(&search)?;
    let value_start = pos + search.len();
    let rest = &tag[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Decode common HTML entities.
pub(crate) fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_scripts_removes_script_tags() {
        let html = "<p>Hello</p><script>alert('xss')</script><p>World</p>";
        let result = strip_scripts_and_styles(html);
        assert!(result.contains("<p>Hello</p>"));
        assert!(result.contains("<p>World</p>"));
        assert!(!result.contains("alert"));
        assert!(!result.contains("<script"));
    }

    #[test]
    fn strip_styles_removes_style_tags() {
        let html = "<p>Hello</p><style>body { color: red; }</style><p>World</p>";
        let result = strip_scripts_and_styles(html);
        assert!(!result.contains("color: red"));
    }

    #[test]
    fn extract_text_strips_tags() {
        let html = "<p>Hello <strong>World</strong></p>";
        let text = extract_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<p>"));
        assert!(!text.contains("<strong>"));
    }
}
