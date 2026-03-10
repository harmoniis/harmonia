//! Response content extractors for each provider API format.

use serde_json::Value;

/// Extract content from an OpenAI-compatible response (OpenAI, xAI, Groq, Alibaba).
pub fn extract_openai_like_content(v: &Value) -> Option<String> {
    let content = v
        .get("choices")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("message"))
        .and_then(|x| x.get("content"))?;
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = content.as_array() {
        let mut out = String::new();
        for item in arr {
            if let Some(s) = item.get("text").and_then(|x| x.as_str()) {
                out.push_str(s);
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Extract content from an Anthropic Messages API response.
pub fn extract_anthropic_content(v: &Value) -> Option<String> {
    let arr = v.get("content")?.as_array()?;
    let mut out = String::new();
    for item in arr {
        if let Some(s) = item.get("text").and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Extract content from a Google (AI Studio / Vertex) response.
pub fn extract_google_content(v: &Value) -> Option<String> {
    let parts = v
        .get("candidates")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("content"))
        .and_then(|x| x.get("parts"))
        .and_then(|x| x.as_array())?;
    let mut out = String::new();
    for p in parts {
        if let Some(s) = p.get("text").and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Extract content from an Amazon Bedrock Converse response.
pub fn extract_bedrock_content(v: &Value) -> Option<String> {
    let parts = v
        .get("output")
        .and_then(|x| x.get("message"))
        .and_then(|x| x.get("content"))
        .and_then(|x| x.as_array())?;
    let mut out = String::new();
    for p in parts {
        if let Some(s) = p.get("text").and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openai_text() {
        let v = json!({"choices":[{"message":{"content":"hello"}}]});
        assert_eq!(extract_openai_like_content(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn anthropic_text() {
        let v = json!({"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]});
        assert_eq!(extract_anthropic_content(&v).as_deref(), Some("ab"));
    }

    #[test]
    fn google_text() {
        let v = json!({"candidates":[{"content":{"parts":[{"text":"ok"}]}}]});
        assert_eq!(extract_google_content(&v).as_deref(), Some("ok"));
    }

    #[test]
    fn bedrock_text() {
        let v = json!({"output":{"message":{"content":[{"text":"yes"}]}}});
        assert_eq!(extract_bedrock_content(&v).as_deref(), Some("yes"));
    }
}
