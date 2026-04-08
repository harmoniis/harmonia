//! Browser engine: HTTP fetch, HTML processing, JS-free extraction.
//!
//! Uses `ureq` for HTTP fetching. Structured so Chrome CDP can be added
//! later via feature flag without changing the public API.

mod config;
mod extract;
mod extract_media;
mod extract_tables;
mod fetch;
mod html;
mod markdown;

// Re-export: config / init
pub use config::{init, BrowserState};

// Re-export: fetch
pub use fetch::{fetch, fetch_with_auth};

// Re-export: HTML processing
pub use html::{extract_text, strip_scripts_and_styles};

// Re-export: markdown
pub use markdown::html_to_markdown;

// Re-export: structured extraction
pub use extract::{
    extract_headings, extract_links, extract_meta, extract_structured, extract_title,
};
pub use extract_media::extract_audio_sources;
pub use extract_tables::{extract_forms, extract_lists, extract_tables, FormField, FormInfo};
