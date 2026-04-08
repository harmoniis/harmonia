//! Audio source extraction from HTML.

use serde_json::{json, Value};

use super::extract::extract_links;
use super::html::extract_attr;

/// Extract audio source URLs from HTML without playing them.
///
/// Finds <audio>, <source> (within audio), and direct links to audio files.
/// Returns structured data: [{src, type, element}]
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
            None => match lower[audio_start..].find('>') {
                Some(p) => audio_start + p + 1,
                None => break,
            },
        };

        let audio_block = &html[audio_start..audio_end];

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

    // 2. Find direct links to audio files
    let audio_extensions = [
        ".mp3", ".wav", ".ogg", ".flac", ".aac", ".m4a", ".opus", ".webm",
    ];
    let links = extract_links(html);
    for link in &links {
        let link_lower = link.to_lowercase();
        for ext in &audio_extensions {
            if link_lower.ends_with(ext) || link_lower.contains(&format!("{}?", ext)) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
