use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

const COMPONENT: &str = "mattermost-frontend";
const MATTERMOST_TOKEN_SYMBOLS: &[&str] = &["mattermost-bot-token", "mattermost-token"];

pub struct MattermostState {
    pub server_url: String,
    pub bot_token: String,
    pub user_id: String,
    pub channels: Vec<String>,
    pub last_post_ids: HashMap<String, String>,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<MattermostState>> = OnceLock::new();

fn state() -> &'static RwLock<MattermostState> {
    STATE.get_or_init(|| {
        RwLock::new(MattermostState {
            server_url: String::new(),
            bot_token: String::new(),
            user_id: String::new(),
            channels: Vec::new(),
            last_post_ids: HashMap::new(),
            initialized: false,
        })
    })
}

#[derive(Deserialize)]
struct MmUser {
    id: String,
}

#[derive(Deserialize)]
struct MmPostsResponse {
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    posts: HashMap<String, MmPost>,
}

#[derive(Deserialize)]
struct MmPost {
    #[serde(default)]
    _id: String,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    message: String,
    #[serde(default, rename = "type")]
    post_type: String,
}

fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let pattern = format!("({key} \"");
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn sexp_value(config: &str, key: &str) -> Option<String> {
    let idx = config.find(key)?;
    let rest = &config[idx + key.len()..];
    let rest = rest.trim_start();
    if rest.starts_with('"') {
        let inner = &rest[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn extract_sexp_string_list(sexp: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("({key} ");
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find(')')?;
    let segment = &rest[..end];
    let items: Vec<String> = segment
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn parse_channels_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
    harmonia_vault::init_from_env()?;
    for symbol in symbols {
        let maybe = harmonia_vault::get_secret_for_component(COMPONENT, symbol)
            .map_err(|e| format!("vault policy error: {e}"))?;
        if let Some(value) = maybe {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }
    }
    Ok(None)
}

fn format_request_error(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            if body.is_empty() {
                format!("mattermost api status {code}")
            } else {
                format!("mattermost api status {code}: {body}")
            }
        }
        ureq::Error::Transport(t) => format!("mattermost transport error: {t}"),
    }
}

pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;
    if s.initialized {
        return Err("mattermost already initialized".into());
    }

    // Ingest token from s-expr config into vault
    if let Some(token) =
        sexp_value(config, ":bot-token").or_else(|| extract_sexp_string(config, "bot-token"))
    {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("mattermost-bot-token", trimmed)?;
        }
    }

    // Ingest server URL into config-store
    if let Some(url) =
        sexp_value(config, ":api-url").or_else(|| extract_sexp_string(config, "api-url"))
    {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, "api-url", trimmed);
        }
    }

    // Ingest channels
    if let Some(channels) = extract_sexp_string_list(config, "channels") {
        let csv = channels.join(",");
        let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, "channels", &csv);
    }

    // Read back from vault/config-store
    s.bot_token = read_vault_secret(MATTERMOST_TOKEN_SYMBOLS)?.unwrap_or_default();

    s.server_url = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .unwrap_or_default();
    if s.server_url.ends_with('/') {
        s.server_url.pop();
    }

    s.channels = harmonia_config_store::get_own(COMPONENT, "channels")
        .ok()
        .flatten()
        .map(|v| parse_channels_csv(&v))
        .unwrap_or_default();

    if s.bot_token.is_empty() {
        return Err("missing bot token in vault (symbol: mattermost-bot-token)".into());
    }
    if s.server_url.is_empty() {
        return Err("missing api-url: set mattermost-frontend/api-url in config-store".into());
    }
    if s.channels.is_empty() {
        return Err(
            "no channels configured: set mattermost-frontend/channels in config-store".into(),
        );
    }

    // Discover bot user ID
    let me_url = format!("{}/api/v4/users/me", s.server_url);
    let me: MmUser = ureq::get(&me_url)
        .set("Authorization", &format!("Bearer {}", s.bot_token))
        .call()
        .map_err(format_request_error)?
        .into_json()
        .map_err(|e| format!("mattermost json decode: {e}"))?;
    s.user_id = me.id;

    // Establish cursors: fetch latest post per channel to skip history
    let channels = s.channels.clone();
    for channel in &channels {
        let url = format!(
            "{}/api/v4/channels/{}/posts?per_page=1",
            s.server_url, channel
        );
        match ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", s.bot_token))
            .call()
        {
            Ok(resp) => {
                if let Ok(posts) = resp.into_json::<MmPostsResponse>() {
                    if let Some(first_id) = posts.order.first() {
                        s.last_post_ids.insert(channel.clone(), first_id.clone());
                    }
                }
            }
            Err(_) => {
                // Channel might not be accessible yet — start with empty cursor
            }
        }
    }

    s.initialized = true;
    Ok(())
}

pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (server_url, token, channels, cursors, self_id) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("mattermost not initialized".into());
        }
        (
            s.server_url.clone(),
            s.bot_token.clone(),
            s.channels.clone(),
            s.last_post_ids.clone(),
            s.user_id.clone(),
        )
    };

    let mut next_cursors = cursors;
    let mut outbound = Vec::new();

    for channel in &channels {
        let cursor = next_cursors.get(channel).cloned().unwrap_or_default();
        let url = if cursor.is_empty() {
            format!(
                "{}/api/v4/channels/{}/posts?per_page=50",
                server_url, channel
            )
        } else {
            format!(
                "{}/api/v4/channels/{}/posts?after={}&per_page=50",
                server_url, channel, cursor
            )
        };

        let resp = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .call()
            .map_err(format_request_error)?;

        let posts: MmPostsResponse = resp
            .into_json()
            .map_err(|e| format!("mattermost json decode: {e}"))?;

        if posts.order.is_empty() {
            continue;
        }

        let initial_sync = cursor.is_empty();
        let mut max_id = cursor.clone();

        for post_id in &posts.order {
            // Track highest ID
            if max_id.is_empty() || post_id > &max_id {
                max_id = post_id.clone();
            }

            if initial_sync {
                continue; // Just establish cursor
            }

            if let Some(post) = posts.posts.get(post_id) {
                // Skip self-messages and system posts
                if post.user_id == self_id || !post.post_type.is_empty() {
                    continue;
                }
                if post.message.trim().is_empty() {
                    continue;
                }
                let metadata = format!(
                    "(:channel-class \"mattermost\" :node-id \"{}\" :remote t)",
                    escape_metadata(&post.user_id)
                );
                outbound.push((channel.clone(), post.message.clone(), Some(metadata)));
            }
        }

        if !max_id.is_empty() && max_id != cursor.as_str() {
            next_cursors.insert(channel.clone(), max_id);
        }
    }

    if let Ok(mut s) = state().write() {
        s.last_post_ids = next_cursors;
    }

    Ok(outbound)
}

pub fn send(channel_id: &str, text: &str) -> Result<(), String> {
    let (server_url, token) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("mattermost not initialized".into());
        }
        (s.server_url.clone(), s.bot_token.clone())
    };

    let url = format!("{}/api/v4/posts", server_url);
    let payload = serde_json::json!({
        "channel_id": channel_id,
        "message": text,
    });

    ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(format_request_error)?;

    Ok(())
}

pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.server_url.clear();
        s.bot_token.clear();
        s.user_id.clear();
        s.channels.clear();
        s.last_post_ids.clear();
        s.initialized = false;
    }
}

fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
