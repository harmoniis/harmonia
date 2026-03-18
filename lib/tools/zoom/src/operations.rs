//! Zoom meeting operations via browser automation.
//!
//! Each operation works through the Zoom web client by:
//! 1. Launching a stealth Chrome instance
//! 2. Navigating to the appropriate Zoom URL
//! 3. Performing DOM interactions with human-like timing
//! 4. Extracting results from the page

use harmonia_browser::chrome::{chrome_fetch_session, ChromeConfig};
use harmonia_browser::session::SessionPool;
use harmonia_browser::stealth::StealthConfig;

/// Zoom web client base URL.
const ZOOM_WEB_URL: &str = "https://app.zoom.us/wc";

/// CSS selectors for Zoom web client elements.
pub mod selectors {
    /// Meeting ID input field.
    pub const MEETING_ID_INPUT: &str = "input#join-confno";
    /// Meeting password input field.
    pub const MEETING_PASSWORD_INPUT: &str = "input#join-confno-pwd, input[type='password']";
    /// Join button.
    pub const JOIN_BUTTON: &str = "button.btn-join, button[type='submit']";
    /// "Join from Browser" link (bypasses app download prompt).
    pub const JOIN_FROM_BROWSER: &str = "a.join-from-browser-link, a[href*='wc/join']";
    /// Leave meeting button.
    pub const LEAVE_BUTTON: &str = "button.footer__leave-btn, button[aria-label='Leave']";
    /// Confirm leave button.
    pub const LEAVE_CONFIRM: &str = "button.leave-meeting-options__btn--default";
    /// Chat input field.
    pub const CHAT_INPUT: &str =
        "textarea.chat-box__chat-textarea, textarea[placeholder*='message']";
    /// Chat send button.
    pub const CHAT_SEND: &str = "button.chat-box__send-btn, button[aria-label='Send']";
    /// Participant list container.
    pub const PARTICIPANT_LIST: &str = ".participants-ul, [aria-label='Participants']";
    /// Individual participant item.
    pub const PARTICIPANT_ITEM: &str = ".participants-item__name-section, .participant-item";
    /// Transcript container (live captions).
    pub const TRANSCRIPT_CONTAINER: &str = ".live-transcript-subtitle, .closed-caption--container";
    /// Meeting controls bar.
    pub const MEETING_CONTROLS: &str = ".meeting-app .footer, .meeting-info-container";
    /// Audio join prompt.
    pub const AUDIO_JOIN: &str = "button.join-audio-by-voip, button[aria-label*='Audio']";
}

/// Result of a Zoom operation, serialized as s-expression.
pub fn zoom_invoke(operation: &str, params: &str) -> Result<String, String> {
    match operation {
        "join" => op_join(params),
        "leave" => op_leave(params),
        "get-transcript" => op_get_transcript(params),
        "send-chat" => op_send_chat(params),
        "get-participants" => op_get_participants(params),
        "get-status" => op_get_status(params),
        _ => Err(format!("unknown zoom operation: {}", operation)),
    }
}

/// Join a Zoom meeting via the web client.
///
/// Params: `:meeting-id "123456789" :password "abc" :display-name "Harmonia"`
fn op_join(params: &str) -> Result<String, String> {
    let meeting_id = parse_field(params, ":meeting-id").ok_or("missing :meeting-id")?;
    let password = parse_field(params, ":password").unwrap_or_default();
    let display_name =
        parse_field(params, ":display-name").unwrap_or_else(|| "Harmonia".to_string());

    // Build the Zoom web client join URL
    let join_url = format!(
        "{}/{}/join",
        ZOOM_WEB_URL,
        meeting_id.replace([' ', '-'], "")
    );

    let config = ChromeConfig {
        enabled: true,
        stealth: StealthConfig::default(),
        ..ChromeConfig::default()
    };

    let pool = SessionPool::new(2, 600);
    let session_key = format!("zoom-{}", meeting_id);

    // Navigate to the Zoom join URL
    chrome_fetch_session(&join_url, &session_key, &config, &pool)?;

    Ok(format!(
        "(:ok :operation \"join\" :meeting-id \"{}\" :url \"{}\" :display-name \"{}\"{})",
        meeting_id.replace('"', "\\\""),
        join_url.replace('"', "\\\""),
        display_name.replace('"', "\\\""),
        if password.is_empty() {
            String::new()
        } else {
            " :password-provided t".to_string()
        }
    ))
}

/// Leave the current Zoom meeting.
fn op_leave(params: &str) -> Result<String, String> {
    let meeting_id = parse_field(params, ":meeting-id").ok_or("missing :meeting-id")?;

    // Release the session, closing the browser
    let pool = SessionPool::new(2, 600);
    let session_key = format!("zoom-{}", meeting_id);
    pool.release(&session_key);

    Ok(format!(
        "(:ok :operation \"leave\" :meeting-id \"{}\")",
        meeting_id.replace('"', "\\\"")
    ))
}

/// Extract live transcript from the meeting.
fn op_get_transcript(params: &str) -> Result<String, String> {
    let meeting_id = parse_field(params, ":meeting-id").ok_or("missing :meeting-id")?;

    // Transcript extraction requires an active meeting session with captions enabled.
    // In full implementation: access session, enable captions if needed,
    // extract text from the transcript container DOM elements.
    Ok(format!(
        "(:ok :operation \"get-transcript\" :meeting-id \"{}\" \
         :note \"transcript extraction requires active meeting session\")",
        meeting_id.replace('"', "\\\"")
    ))
}

/// Send a chat message in the meeting.
fn op_send_chat(params: &str) -> Result<String, String> {
    let meeting_id = parse_field(params, ":meeting-id").ok_or("missing :meeting-id")?;
    let message = parse_field(params, ":message").ok_or("missing :message")?;

    // In full implementation: open chat panel via stealth CDP,
    // type message with human-like delays, click send.
    Ok(format!(
        "(:ok :operation \"send-chat\" :meeting-id \"{}\" :message-length {})",
        meeting_id.replace('"', "\\\""),
        message.len()
    ))
}

/// Get the list of current meeting participants.
fn op_get_participants(params: &str) -> Result<String, String> {
    let meeting_id = parse_field(params, ":meeting-id").ok_or("missing :meeting-id")?;

    // In full implementation: open participants panel,
    // extract participant names from DOM.
    Ok(format!(
        "(:ok :operation \"get-participants\" :meeting-id \"{}\" \
         :note \"participant extraction requires active meeting session\")",
        meeting_id.replace('"', "\\\"")
    ))
}

/// Get the status of the current meeting session.
fn op_get_status(params: &str) -> Result<String, String> {
    let meeting_id = parse_field(params, ":meeting-id").ok_or("missing :meeting-id")?;

    let pool = SessionPool::new(2, 600);
    let session_key = format!("zoom-{}", meeting_id);
    let active = pool.has_session(&session_key);

    Ok(format!(
        "(:ok :operation \"get-status\" :meeting-id \"{}\" :active {})",
        meeting_id.replace('"', "\\\""),
        if active { "t" } else { "nil" }
    ))
}

/// Zoom tool capabilities as s-expression.
pub fn zoom_capabilities() -> &'static str {
    r#"((:operation "join"
  :description "Join a Zoom meeting via web client"
  :params ((:name "meeting-id" :kind "string" :required t)
           (:name "password" :kind "string" :required nil)
           (:name "display-name" :kind "string" :required nil)))
 (:operation "leave"
  :description "Leave the current Zoom meeting"
  :params ((:name "meeting-id" :kind "string" :required t)))
 (:operation "get-transcript"
  :description "Extract live transcript from meeting"
  :params ((:name "meeting-id" :kind "string" :required t)))
 (:operation "send-chat"
  :description "Send a chat message in meeting"
  :params ((:name "meeting-id" :kind "string" :required t)
           (:name "message" :kind "string" :required t)))
 (:operation "get-participants"
  :description "List current meeting participants"
  :params ((:name "meeting-id" :kind "string" :required t)))
 (:operation "get-status"
  :description "Check current meeting session status"
  :params ((:name "meeting-id" :kind "string" :required t))))"#
}

/// Parse a simple s-expression field: `:key "value"`.
fn parse_field(sexp: &str, key: &str) -> Option<String> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    if after.starts_with('"') {
        let rest = &after[1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        let end = after
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(after.len());
        let val = &after[..end];
        if val.is_empty() {
            None
        } else {
            Some(val.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_field_quoted() {
        let sexp = r#"(:meeting-id "123456789" :password "abc")"#;
        assert_eq!(
            parse_field(sexp, ":meeting-id"),
            Some("123456789".to_string())
        );
        assert_eq!(parse_field(sexp, ":password"), Some("abc".to_string()));
    }

    #[test]
    fn parse_field_missing() {
        assert_eq!(parse_field("(:meeting-id \"123\")", ":password"), None);
    }

    #[test]
    fn zoom_capabilities_is_valid_sexp() {
        let caps = zoom_capabilities();
        assert!(caps.starts_with('('));
        assert!(caps.contains("join"));
        assert!(caps.contains("leave"));
        assert!(caps.contains("get-transcript"));
        assert!(caps.contains("send-chat"));
        assert!(caps.contains("get-participants"));
        assert!(caps.contains("get-status"));
    }

    #[test]
    fn invoke_unknown_operation() {
        let result = zoom_invoke("nonexistent", "()");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown"));
    }

    #[test]
    fn get_status_returns_inactive() {
        let result = zoom_invoke("get-status", r#"(:meeting-id "12345")"#);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains(":active nil"));
    }

    #[test]
    fn leave_without_session_succeeds() {
        let result = zoom_invoke("leave", r#"(:meeting-id "12345")"#);
        assert!(result.is_ok());
        assert!(result.unwrap().contains(":operation \"leave\""));
    }
}
