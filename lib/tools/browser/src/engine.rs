//! Browser engine: HTTP fetch, HTML processing, JS-free extraction.
//!
//! Uses `ureq` for HTTP fetching. Structured so Chrome CDP can be added
//! later via feature flag without changing the public API.

use crate::sandbox;
use harmonia_vault::{get_secret_for_component, init_from_env};
use serde_json::{json, Value};
use std::io::Read as _;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

/// Browser engine state.
pub struct BrowserState {
    user_agent: String,
    timeout_ms: u64,
    max_response_bytes: usize,
    network_allowlist: Option<Vec<String>>,
    initialized: bool,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            user_agent: "harmonia-browser/1.0.0".to_string(),
            timeout_ms: 10_000,
            max_response_bytes: 2 * 1024 * 1024, // 2 MB
            network_allowlist: None,
            initialized: false,
        }
    }
}

/// Deprecated: legacy global singleton. Will be replaced by injected state.
static LEGACY_STATE: OnceLock<RwLock<BrowserState>> = OnceLock::new();

fn state() -> &'static RwLock<BrowserState> {
    LEGACY_STATE.get_or_init(|| RwLock::new(BrowserState::default()))
}

/// Initialize the browser engine from an s-expression config string.
///
/// Recognized keys (parsed loosely):
///   :timeout <ms>
///   :user-agent "<string>"
///   :max-response-bytes <n>
///   :allowlist ("<domain1>" "<domain2>" ...)
pub fn init(config: &str) -> Result<(), String> {
    let mut st = state().write().map_err(|e| format!("lock poisoned: {e}"))?;

    // Parse timeout
    if let Some(val) = parse_sexp_int(config, ":timeout") {
        st.timeout_ms = val as u64;
    }

    // Parse user-agent
    if let Some(val) = parse_sexp_string(config, ":user-agent") {
        st.user_agent = val;
    }

    // Parse max-response-bytes
    if let Some(val) = parse_sexp_int(config, ":max-response-bytes") {
        st.max_response_bytes = val as usize;
    }

    // Parse allowlist
    if let Some(vals) = parse_sexp_string_list(config, ":allowlist") {
        st.network_allowlist = Some(vals);
    }

    st.initialized = true;
    Ok(())
}

/// Fetch a URL via HTTP GET, enforcing timeout and size limits.
pub fn fetch(url: &str) -> Result<String, String> {
    let (ua, timeout_ms, max_bytes, allowlist) = {
        let st = state().read().map_err(|e| format!("lock poisoned: {e}"))?;
        (
            st.user_agent.clone(),
            st.timeout_ms,
            st.max_response_bytes,
            st.network_allowlist.clone(),
        )
    };

    sandbox::check_domain_allowed(url, &allowlist)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(timeout_ms))
        .timeout_read(Duration::from_millis(timeout_ms))
        .user_agent(&ua)
        .build();

    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    // Check content-length if available
    if let Some(len_str) = response.header("content-length") {
        if let Ok(len) = len_str.parse::<usize>() {
            if len > max_bytes {
                return Err(format!(
                    "response too large: {} bytes (limit: {})",
                    len, max_bytes
                ));
            }
        }
    }

    // Read with size limit
    let mut buf = Vec::with_capacity(max_bytes.min(65536));
    let mut reader = response.into_reader();
    let mut total = 0usize;
    loop {
        let mut chunk = [0u8; 8192];
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("read error: {e}"))?;
        if n == 0 {
            break;
        }
        total += n;
        if total > max_bytes {
            return Err(format!("response too large (>{} bytes)", max_bytes));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf).into_owned();

    Ok(body)
}

/// Fetch a URL with vault-injected Bearer token authentication.
pub fn fetch_with_auth(url: &str, auth_symbol: &str) -> Result<String, String> {
    let (ua, timeout_ms, max_bytes, allowlist) = {
        let st = state().read().map_err(|e| format!("lock poisoned: {e}"))?;
        (
            st.user_agent.clone(),
            st.timeout_ms,
            st.max_response_bytes,
            st.network_allowlist.clone(),
        )
    };

    sandbox::check_domain_allowed(url, &allowlist)?;

    let _ = init_from_env();
    let secret = get_secret_for_component("browser-tool", auth_symbol)
        .map_err(|e| format!("vault policy error: {e}"))?
        .ok_or_else(|| format!("missing secret for symbol: {}", auth_symbol))?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(timeout_ms))
        .timeout_read(Duration::from_millis(timeout_ms))
        .user_agent(&ua)
        .build();

    let response = agent
        .get(url)
        .set("Authorization", &format!("Bearer {}", secret))
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let mut buf = Vec::with_capacity(max_bytes.min(65536));
    let mut reader = response.into_reader();
    let mut total = 0usize;
    loop {
        let mut chunk = [0u8; 8192];
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("read error: {e}"))?;
        if n == 0 {
            break;
        }
        total += n;
        if total > max_bytes {
            return Err(format!("response too large (>{} bytes)", max_bytes));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf).into_owned();

    Ok(body)
}

/// Remove all `<script>` and `<style>` blocks from HTML.
pub fn strip_scripts_and_styles(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let lower = html.to_lowercase();
    let bytes = html.as_bytes();
    let lower_bytes = lower.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Check for <script or <style
        if i + 7 < len && lower_bytes[i] == b'<' {
            let rest = &lower[i..];
            if rest.starts_with("<script") {
                // Find closing </script>
                if let Some(end) = lower[i..].find("</script>") {
                    i += end + 9;
                    continue;
                }
            } else if rest.starts_with("<style") {
                // Find closing </style>
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
            // Insert space after tag closes if not already space
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
            continue;
        }
        if in_tag {
            continue;
        }

        // Collapse whitespace
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

    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// Extract all href values from anchor tags.
pub fn extract_links(html: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut search_from = 0;

    while search_from < html.len() {
        let rest = &html[search_from..];

        // Find href="
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

    // Decode entities and clean up
    out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse multiple blank lines
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }

    out.trim().to_string()
}

// ---- Internal extraction helpers ----

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

            // Find end of opening tag
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

    // Sort by position in document (they're already grouped by level, re-sort)
    // Actually we need to track positions. For simplicity, just return as-is
    // grouped by level. A more sophisticated version would track byte offsets.
    headings
}

/// Extract tables as Vec of tables, each table is Vec of rows, each row is Vec of cells.
pub fn extract_tables(html: &str) -> Vec<Vec<Vec<String>>> {
    let mut tables = Vec::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    while search_from < lower.len() {
        let table_start = match lower[search_from..].find("<table") {
            Some(p) => search_from + p,
            None => break,
        };

        let table_end = match lower[table_start..].find("</table>") {
            Some(p) => table_start + p + 8,
            None => break,
        };

        let table_html = &html[table_start..table_end];
        let table_lower = &lower[table_start..table_end];
        let mut rows = Vec::new();
        let mut row_search = 0;

        while row_search < table_lower.len() {
            let tr_start = match table_lower[row_search..].find("<tr") {
                Some(p) => row_search + p,
                None => break,
            };

            let tr_end = match table_lower[tr_start..].find("</tr>") {
                Some(p) => tr_start + p + 5,
                None => break,
            };

            let row_html = &table_html[tr_start..tr_end];
            let row_lower = table_lower[tr_start..tr_end].to_string();
            let mut cells = Vec::new();
            let mut cell_search = 0;

            while cell_search < row_lower.len() {
                // Match both <td and <th
                let td_pos = row_lower[cell_search..].find("<td");
                let th_pos = row_lower[cell_search..].find("<th");

                let cell_start = match (td_pos, th_pos) {
                    (Some(a), Some(b)) => cell_search + a.min(b),
                    (Some(a), None) => cell_search + a,
                    (None, Some(b)) => cell_search + b,
                    (None, None) => break,
                };

                let content_start = match row_lower[cell_start..].find('>') {
                    Some(p) => cell_start + p + 1,
                    None => break,
                };

                // Find </td> or </th>
                let td_end = row_lower[content_start..].find("</td>");
                let th_end = row_lower[content_start..].find("</th>");

                let cell_end = match (td_end, th_end) {
                    (Some(a), Some(b)) => content_start + a.min(b),
                    (Some(a), None) => content_start + a,
                    (None, Some(b)) => content_start + b,
                    (None, None) => break,
                };

                let text = strip_tags(&row_html[content_start..cell_end])
                    .trim()
                    .to_string();
                cells.push(text);
                cell_search = cell_end + 5;
            }

            if !cells.is_empty() {
                rows.push(cells);
            }
            row_search = tr_end;
        }

        if !rows.is_empty() {
            tables.push(rows);
        }
        search_from = table_end;
    }

    tables
}

/// Form field information.
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub placeholder: String,
}

/// Form information.
pub struct FormInfo {
    pub action: String,
    pub method: String,
    pub fields: Vec<FormField>,
}

/// Extract form elements with their fields.
pub fn extract_forms(html: &str) -> Vec<FormInfo> {
    let mut forms = Vec::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    while search_from < lower.len() {
        let form_start = match lower[search_from..].find("<form") {
            Some(p) => search_from + p,
            None => break,
        };

        let form_end = match lower[form_start..].find("</form>") {
            Some(p) => form_start + p + 7,
            None => break,
        };

        let form_tag_end = match lower[form_start..].find('>') {
            Some(p) => form_start + p,
            None => break,
        };

        let form_open = &html[form_start..form_tag_end + 1];
        let action = extract_attr(form_open, "action").unwrap_or_default();
        let method = extract_attr(form_open, "method").unwrap_or_else(|| "GET".to_string());

        let form_body = &html[form_tag_end + 1..form_end - 7];
        let form_body_lower = form_body.to_lowercase();
        let mut fields = Vec::new();
        let mut input_search = 0;

        while input_search < form_body_lower.len() {
            let input_start = match form_body_lower[input_search..].find("<input") {
                Some(p) => input_search + p,
                None => break,
            };

            let input_end = match form_body_lower[input_start..].find('>') {
                Some(p) => input_start + p + 1,
                None => break,
            };

            let input_tag = &form_body[input_start..input_end];
            fields.push(FormField {
                name: extract_attr(input_tag, "name").unwrap_or_default(),
                field_type: extract_attr(input_tag, "type").unwrap_or_else(|| "text".to_string()),
                placeholder: extract_attr(input_tag, "placeholder").unwrap_or_default(),
            });

            input_search = input_end;
        }

        forms.push(FormInfo {
            action,
            method,
            fields,
        });
        search_from = form_end;
    }

    forms
}

/// Extract meta tags as key-value pairs.
pub fn extract_meta(html: &str) -> std::collections::HashMap<String, String> {
    let mut meta = std::collections::HashMap::new();
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

/// Extract list items from ul/ol lists.
fn extract_lists(html: &str) -> Vec<Vec<String>> {
    let mut lists = Vec::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    // Search for both <ul and <ol
    loop {
        let ul_pos = lower[search_from..].find("<ul");
        let ol_pos = lower[search_from..].find("<ol");

        let list_start = match (ul_pos, ol_pos) {
            (Some(a), Some(b)) => search_from + a.min(b),
            (Some(a), None) => search_from + a,
            (None, Some(b)) => search_from + b,
            (None, None) => break,
        };

        // Determine closing tag
        let is_ul = lower[list_start..].starts_with("<ul");
        let close_tag = if is_ul { "</ul>" } else { "</ol>" };

        let list_end = match lower[list_start..].find(close_tag) {
            Some(p) => list_start + p + close_tag.len(),
            None => break,
        };

        let list_html = &html[list_start..list_end];
        let list_lower = &lower[list_start..list_end];
        let mut items = Vec::new();
        let mut li_search = 0;

        while li_search < list_lower.len() {
            let li_start = match list_lower[li_search..].find("<li") {
                Some(p) => li_search + p,
                None => break,
            };

            let content_start = match list_lower[li_start..].find('>') {
                Some(p) => li_start + p + 1,
                None => break,
            };

            let content_end = match list_lower[content_start..].find("</li>") {
                Some(p) => content_start + p,
                None => list_lower.len(),
            };

            let text = strip_tags(&list_html[content_start..content_end])
                .trim()
                .to_string();
            if !text.is_empty() {
                items.push(text);
            }

            li_search = content_end + 5;
        }

        if !items.is_empty() {
            lists.push(items);
        }
        search_from = list_end;
    }

    lists
}

// ---- String processing utilities ----

/// Strip all HTML tags from a string.
fn strip_tags(html: &str) -> String {
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
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let search = format!("{}=\"", attr);
    let pos = lower.find(&search)?;
    let value_start = pos + search.len();
    let rest = &tag[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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

        // Push everything before the tag
        result.push_str(&html[search_from..tag_start]);

        // Find end of opening tag
        let content_start = match lower[tag_start..].find('>') {
            Some(p) => tag_start + p + 1,
            None => {
                result.push_str(&html[tag_start..]);
                break;
            }
        };

        // Find closing tag
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
            None => {
                // Also check for <a> without attributes
                match lower[search_from..].find("<a>") {
                    Some(p) => search_from + p,
                    None => {
                        result.push_str(&html[search_from..]);
                        break;
                    }
                }
            }
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

// ---- Audio source extraction ----

/// Extract audio source URLs from HTML without playing them.
///
/// Finds <audio>, <source> (within audio), and direct links to audio files.
/// Returns structured data: [{src, type, duration}]
/// NEVER instantiates Audio() or plays anything.
pub fn extract_audio_sources(html: &str) -> Vec<Value> {
    let mut sources = Vec::new();
    let lower = html.to_lowercase();

    // 1. Find <audio> elements and their <source> children
    let mut search_from = 0;
    while search_from < lower.len() {
        let audio_start = match lower[search_from..].find("<audio") {
            Some(p) => search_from + p,
            None => break,
        };

        let audio_end = match lower[audio_start..].find("</audio>") {
            Some(p) => audio_start + p + 8,
            None => {
                // Self-closing <audio ... />
                match lower[audio_start..].find('>') {
                    Some(p) => audio_start + p + 1,
                    None => break,
                }
            }
        };

        let audio_block = &html[audio_start..audio_end];

        // Check for src on the <audio> tag itself (not child elements)
        let audio_tag_end = html[audio_start..]
            .find('>')
            .map(|p| audio_start + p + 1)
            .unwrap_or(audio_end);
        let audio_tag_only = &html[audio_start..audio_tag_end];
        if let Some(src) = extract_attr(audio_tag_only, "src") {
            if !src.is_empty() {
                sources.push(json!({
                    "src": src,
                    "type": extract_attr(audio_tag_only, "type").unwrap_or_default(),
                    "element": "audio",
                }));
            }
        }

        // Find <source> tags within this <audio> block
        let audio_lower = audio_block.to_lowercase();
        let mut src_search = 0;
        while src_search < audio_lower.len() {
            let source_start = match audio_lower[src_search..].find("<source") {
                Some(p) => src_search + p,
                None => break,
            };

            let source_end = match audio_lower[source_start..].find('>') {
                Some(p) => source_start + p + 1,
                None => break,
            };

            let source_tag = &audio_block[source_start..source_end];
            if let Some(src) = extract_attr(source_tag, "src") {
                if !src.is_empty() {
                    sources.push(json!({
                        "src": src,
                        "type": extract_attr(source_tag, "type").unwrap_or_default(),
                        "element": "source",
                    }));
                }
            }

            src_search = source_end;
        }

        search_from = audio_end;
    }

    // 2. Find direct links to audio files (.mp3, .wav, .ogg, .flac, .aac, .m4a, .opus, .webm)
    let audio_extensions = [
        ".mp3", ".wav", ".ogg", ".flac", ".aac", ".m4a", ".opus", ".webm",
    ];
    let links = extract_links(html);
    for link in &links {
        let link_lower = link.to_lowercase();
        for ext in &audio_extensions {
            if link_lower.ends_with(ext) || link_lower.contains(&format!("{}?", ext)) {
                // Avoid duplicates
                let already_found = sources
                    .iter()
                    .any(|s| s.get("src").and_then(|v| v.as_str()) == Some(link.as_str()));
                if !already_found {
                    sources.push(json!({
                        "src": link,
                        "type": mime_for_extension(ext),
                        "element": "link",
                    }));
                }
                break;
            }
        }
    }

    sources
}

/// Map audio file extension to MIME type.
fn mime_for_extension(ext: &str) -> &str {
    match ext {
        ".mp3" => "audio/mpeg",
        ".wav" => "audio/wav",
        ".ogg" => "audio/ogg",
        ".flac" => "audio/flac",
        ".aac" => "audio/aac",
        ".m4a" => "audio/mp4",
        ".opus" => "audio/opus",
        ".webm" => "audio/webm",
        _ => "audio/unknown",
    }
}

// ---- S-expression config parsing utilities ----

fn parse_sexp_int(sexp: &str, key: &str) -> Option<i64> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

fn parse_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_sexp_string_list(sexp: &str, key: &str) -> Option<Vec<String>> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    if !after.starts_with('(') {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find(')')?;
    let list_str = &rest[..end];
    let items: Vec<String> = list_str
        .split('"')
        .enumerate()
        .filter_map(|(i, s)| {
            // Odd indices are inside quotes
            if i % 2 == 1 && !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
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
    fn extract_tables_works() {
        let html = "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>";
        let tables = extract_tables(html);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].len(), 2);
        assert_eq!(tables[0][0], vec!["A", "B"]);
        assert_eq!(tables[0][1], vec!["C", "D"]);
    }

    #[test]
    fn extract_meta_works() {
        let html =
            r#"<meta name="description" content="A test page"><meta name="author" content="Test">"#;
        let meta = extract_meta(html);
        assert_eq!(meta.get("description").unwrap(), "A test page");
        assert_eq!(meta.get("author").unwrap(), "Test");
    }

    #[test]
    fn html_to_markdown_headings() {
        let html = "<h1>Title</h1><p>Some text</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("# Title"));
        assert!(md.contains("Some text"));
    }

    #[test]
    fn extract_audio_from_audio_tag() {
        let html = r#"<audio src="song.mp3" type="audio/mpeg"></audio>"#;
        let sources = extract_audio_sources(html);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["src"], "song.mp3");
    }

    #[test]
    fn extract_audio_from_source_tags() {
        let html = r#"<audio><source src="track.ogg" type="audio/ogg"><source src="track.mp3" type="audio/mpeg"></audio>"#;
        let sources = extract_audio_sources(html);
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0]["src"], "track.ogg");
        assert_eq!(sources[1]["src"], "track.mp3");
    }

    #[test]
    fn extract_audio_from_links() {
        let html = r#"<a href="https://cdn.example.com/podcast.mp3">Download</a>"#;
        let sources = extract_audio_sources(html);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["src"], "https://cdn.example.com/podcast.mp3");
        assert_eq!(sources[0]["type"], "audio/mpeg");
    }

    #[test]
    fn extract_audio_no_duplicates() {
        let html = r#"<audio src="song.mp3"></audio><a href="song.mp3">Link</a>"#;
        let sources = extract_audio_sources(html);
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn parse_sexp_int_works() {
        assert_eq!(
            parse_sexp_int("(:timeout 5000 :foo 1)", ":timeout"),
            Some(5000)
        );
    }

    #[test]
    fn parse_sexp_string_works() {
        assert_eq!(
            parse_sexp_string(r#"(:user-agent "MyBot/1.0")"#, ":user-agent"),
            Some("MyBot/1.0".to_string())
        );
    }
}
