//! AgentBrowser SDK macros — high-level extraction operations.
//!
//! These are server-side "macros" (named extraction strategies) that the agent
//! calls by name. Each macro encapsulates a specific data extraction pattern
//! from raw HTML content.

use crate::engine;
use serde_json::{json, Value};

/// All available browser extraction macros.
pub enum BrowserMacro {
    ExtractTitle,
    ExtractText,
    ExtractLinks,
    ExtractHeadings,
    ExtractTables,
    ExtractForms,
    ExtractMeta,
    ExtractAudioSources,
    PageToMarkdown,
    ExtractStructured { hint: String },
    SmartExtract { query: String },
}

impl BrowserMacro {
    /// Parse a macro name and optional argument into a BrowserMacro variant.
    pub fn from_name(name: &str, arg: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "title" => Some(Self::ExtractTitle),
            "text" => Some(Self::ExtractText),
            "links" => Some(Self::ExtractLinks),
            "headings" => Some(Self::ExtractHeadings),
            "tables" => Some(Self::ExtractTables),
            "forms" => Some(Self::ExtractForms),
            "meta" => Some(Self::ExtractMeta),
            "audio" | "audio_sources" => Some(Self::ExtractAudioSources),
            "markdown" => Some(Self::PageToMarkdown),
            "structured" => Some(Self::ExtractStructured {
                hint: arg.to_string(),
            }),
            "smart" => Some(Self::SmartExtract {
                query: arg.to_string(),
            }),
            _ => None,
        }
    }

    /// Execute the macro against raw HTML content, returning structured JSON.
    pub fn execute(&self, html: &str) -> Value {
        let cleaned = engine::strip_scripts_and_styles(html);
        match self {
            Self::ExtractTitle => json!(engine::extract_title(&cleaned)),
            Self::ExtractText => json!(engine::extract_text(&cleaned)),
            Self::ExtractLinks => json!(engine::extract_links(&cleaned)),
            Self::ExtractHeadings => {
                let headings = engine::extract_headings(&cleaned);
                json!(headings
                    .iter()
                    .map(|(level, text)| json!({"level": level, "text": text}))
                    .collect::<Vec<_>>())
            }
            Self::ExtractTables => json!(engine::extract_tables(&cleaned)),
            Self::ExtractForms => {
                let forms = engine::extract_forms(&cleaned);
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
            Self::ExtractMeta => json!(engine::extract_meta(&cleaned)),
            Self::ExtractAudioSources => json!(engine::extract_audio_sources(&cleaned)),
            Self::PageToMarkdown => json!(engine::html_to_markdown(&cleaned)),
            Self::ExtractStructured { hint } => engine::extract_structured(&cleaned, hint),
            Self::SmartExtract { query } => smart_extract(&cleaned, query),
        }
    }

    /// Return the name of this macro for logging/reporting.
    pub fn name(&self) -> &str {
        match self {
            Self::ExtractTitle => "title",
            Self::ExtractText => "text",
            Self::ExtractLinks => "links",
            Self::ExtractHeadings => "headings",
            Self::ExtractTables => "tables",
            Self::ExtractForms => "forms",
            Self::ExtractMeta => "meta",
            Self::ExtractAudioSources => "audio_sources",
            Self::PageToMarkdown => "markdown",
            Self::ExtractStructured { .. } => "structured",
            Self::SmartExtract { .. } => "smart",
        }
    }
}

/// Smart extraction: analyze the query and pick the best extraction strategy.
///
/// Heuristic-based: looks for keywords in the query to determine what
/// data the agent is looking for, then combines relevant extractions.
fn smart_extract(html: &str, query: &str) -> Value {
    let q = query.to_lowercase();
    let mut results = serde_json::Map::new();

    // Always include title for context
    if let Some(title) = engine::extract_title(html) {
        results.insert("title".to_string(), json!(title));
    }

    // Keyword-based strategy selection
    if q.contains("link") || q.contains("url") || q.contains("href") || q.contains("navigate") {
        results.insert("links".to_string(), json!(engine::extract_links(html)));
    }

    if q.contains("table") || q.contains("data") || q.contains("row") || q.contains("column") {
        results.insert("tables".to_string(), json!(engine::extract_tables(html)));
    }

    if q.contains("heading")
        || q.contains("section")
        || q.contains("outline")
        || q.contains("structure")
        || q.contains("toc")
    {
        let headings = engine::extract_headings(html);
        results.insert(
            "headings".to_string(),
            json!(headings
                .iter()
                .map(|(l, t)| json!({"level": l, "text": t}))
                .collect::<Vec<_>>()),
        );
    }

    if q.contains("form") || q.contains("input") || q.contains("field") || q.contains("submit") {
        let forms = engine::extract_forms(html);
        results.insert(
            "forms".to_string(),
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
                .collect::<Vec<_>>()),
        );
    }

    if q.contains("meta")
        || q.contains("description")
        || q.contains("keyword")
        || q.contains("author")
    {
        results.insert("meta".to_string(), json!(engine::extract_meta(html)));
    }

    if q.contains("audio")
        || q.contains("music")
        || q.contains("podcast")
        || q.contains("sound")
        || q.contains("song")
        || q.contains("mp3")
    {
        results.insert(
            "audio_sources".to_string(),
            json!(engine::extract_audio_sources(html)),
        );
    }

    // If no specific strategy matched, return text + links as default
    if results.len() <= 1 {
        results.insert("text".to_string(), json!(engine::extract_text(html)));
        results.insert("links".to_string(), json!(engine::extract_links(html)));
    }

    results.insert("query".to_string(), json!(query));
    Value::Object(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_resolves_all_macros() {
        assert!(BrowserMacro::from_name("title", "").is_some());
        assert!(BrowserMacro::from_name("text", "").is_some());
        assert!(BrowserMacro::from_name("links", "").is_some());
        assert!(BrowserMacro::from_name("headings", "").is_some());
        assert!(BrowserMacro::from_name("tables", "").is_some());
        assert!(BrowserMacro::from_name("forms", "").is_some());
        assert!(BrowserMacro::from_name("meta", "").is_some());
        assert!(BrowserMacro::from_name("markdown", "").is_some());
        assert!(BrowserMacro::from_name("audio", "").is_some());
        assert!(BrowserMacro::from_name("audio_sources", "").is_some());
        assert!(BrowserMacro::from_name("structured", "tables").is_some());
        assert!(BrowserMacro::from_name("smart", "find links").is_some());
        assert!(BrowserMacro::from_name("nonexistent", "").is_none());
    }

    #[test]
    fn case_insensitive_name() {
        assert!(BrowserMacro::from_name("TITLE", "").is_some());
        assert!(BrowserMacro::from_name("Links", "").is_some());
    }

    #[test]
    fn execute_title() {
        let html = "<html><head><title>Test Page</title></head><body></body></html>";
        let m = BrowserMacro::from_name("title", "").unwrap();
        let result = m.execute(html);
        assert_eq!(result, json!("Test Page"));
    }

    #[test]
    fn execute_links() {
        let html = r#"<a href="https://a.com">A</a><a href="https://b.com">B</a>"#;
        let m = BrowserMacro::from_name("links", "").unwrap();
        let result = m.execute(html);
        assert_eq!(result, json!(["https://a.com", "https://b.com"]));
    }

    #[test]
    fn smart_extract_with_link_query() {
        let html = r#"<title>Test</title><a href="https://a.com">A</a>"#;
        let result = smart_extract(html, "find all links on the page");
        assert!(result.get("links").is_some());
    }
}
