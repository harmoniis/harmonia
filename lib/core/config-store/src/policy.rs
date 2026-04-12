/// Component-based access control for config-store, mirroring vault's api.rs pattern.
use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

static POLICY_OVERRIDES: OnceLock<HashMap<String, Vec<(String, PolicyMode)>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq)]
enum PolicyMode {
    Read,
    ReadWrite,
}

/// Components with full read/write/delete access to all scopes.
const ADMIN_COMPONENTS: &[&str] = &["conductor", "admin-intent", "harmonia-cli"];

fn is_admin(component: &str) -> bool {
    ADMIN_COMPONENTS.contains(&component)
}

fn default_extra_read_scopes(component: &str) -> &'static [&'static str] {
    match component {
        "parallel-agents-core" => &[
            "openrouter-backend",
            "search-exa-tool",
            "search-brave-tool",
            "prompts",
            "model-capabilities",
        ],
        "openrouter-backend" | "xai-backend" | "provider-protocol" => {
            &["prompts", "model-capabilities"]
        }
        "phoenix-core" | "ouroboros-core" | "recovery" => &["evolution"],
        "evolution" => &["s3-storage"],
        "tailnet-core" => &["node"],
        "mempalace" | "memory-field" => &["node"],
        "gateway" => &["sender-policy"],
        "observability" => &["global"],
        _ => &[],
    }
}

/// Parse `HARMONIA_CONFIG_STORE_POLICY` env var.
/// Format: `comp=scope1,scope2:rw;comp2=scope3:r`
fn parse_policy_env() -> HashMap<String, Vec<(String, PolicyMode)>> {
    let mut out = HashMap::new();
    let raw = match env::var("HARMONIA_CONFIG_STORE_POLICY") {
        Ok(v) => v,
        Err(_) => return out,
    };
    for entry in raw.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (component, rhs) = match entry.split_once('=') {
            Some(v) => v,
            None => continue,
        };
        let component = component.trim().to_ascii_lowercase();
        if component.is_empty() {
            continue;
        }
        let mut grants = Vec::new();
        for grant in rhs.split(',') {
            let grant = grant.trim();
            if grant.is_empty() {
                continue;
            }
            let (scope, mode) = if let Some((s, m)) = grant.rsplit_once(':') {
                let mode = match m {
                    "rw" => PolicyMode::ReadWrite,
                    _ => PolicyMode::Read,
                };
                (s.to_ascii_lowercase(), mode)
            } else {
                (grant.to_ascii_lowercase(), PolicyMode::Read)
            };
            grants.push((scope, mode));
        }
        if !grants.is_empty() {
            out.insert(component, grants);
        }
    }
    out
}

fn policy_overrides() -> &'static HashMap<String, Vec<(String, PolicyMode)>> {
    POLICY_OVERRIDES.get_or_init(parse_policy_env)
}

fn check_override(component: &str, scope: &str, need_write: bool) -> Option<bool> {
    let overrides = policy_overrides();
    for key in [component, "*"] {
        if let Some(grants) = overrides.get(key) {
            for (grant_scope, mode) in grants {
                let scope_match =
                    grant_scope == "*" || grant_scope == scope || grant_scope == "global";
                if scope_match {
                    if need_write {
                        if *mode == PolicyMode::ReadWrite {
                            return Some(true);
                        }
                    } else {
                        return Some(true);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn can_read(component: &str, scope: &str) -> bool {
    let component = component.trim().to_ascii_lowercase();
    let scope = scope.trim().to_ascii_lowercase();

    // Admins read everything
    if is_admin(&component) {
        return true;
    }
    // Every component can read global + own scope
    if scope == "global" || scope == component {
        return true;
    }
    // Check built-in extra read scopes
    if default_extra_read_scopes(&component).contains(&scope.as_str()) {
        return true;
    }
    // Check env overrides
    if let Some(allowed) = check_override(&component, &scope, false) {
        return allowed;
    }
    false
}

pub(crate) fn can_write(component: &str, scope: &str) -> bool {
    let component = component.trim().to_ascii_lowercase();
    let scope = scope.trim().to_ascii_lowercase();

    // Admins write everything
    if is_admin(&component) {
        return true;
    }
    // Each component can write its own scope
    if scope == component {
        return true;
    }
    // Check env overrides
    if let Some(allowed) = check_override(&component, &scope, true) {
        return allowed;
    }
    false
}

pub(crate) fn can_delete(component: &str, _scope: &str) -> bool {
    let component = component.trim().to_ascii_lowercase();
    is_admin(&component)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_read_all() {
        assert!(can_read("conductor", "openai-backend"));
        assert!(can_read("admin-intent", "mqtt-frontend"));
        assert!(can_read("harmonia-cli", "anything"));
    }

    #[test]
    fn component_reads_own_scope() {
        assert!(can_read("openai-backend", "openai-backend"));
        assert!(can_read("mqtt-frontend", "mqtt-frontend"));
    }

    #[test]
    fn component_reads_global() {
        assert!(can_read("openai-backend", "global"));
        assert!(can_read("mqtt-frontend", "global"));
    }

    #[test]
    fn component_denied_other_scope() {
        assert!(!can_read("openai-backend", "anthropic-backend"));
        assert!(!can_read("mqtt-frontend", "openai-backend"));
    }

    #[test]
    fn parallel_agents_extra_reads() {
        assert!(can_read("parallel-agents-core", "openrouter-backend"));
        assert!(can_read("parallel-agents-core", "search-exa-tool"));
        assert!(can_read("parallel-agents-core", "search-brave-tool"));
        assert!(!can_read("parallel-agents-core", "anthropic-backend"));
    }

    #[test]
    fn component_writes_own_scope() {
        assert!(can_write("openai-backend", "openai-backend"));
    }

    #[test]
    fn component_cannot_write_other_scope() {
        assert!(!can_write("openai-backend", "anthropic-backend"));
    }

    #[test]
    fn admin_can_write_all() {
        assert!(can_write("conductor", "openai-backend"));
        assert!(can_write("admin-intent", "anything"));
    }

    #[test]
    fn only_admin_can_delete() {
        assert!(can_delete("conductor", "openai-backend"));
        assert!(can_delete("admin-intent", "anything"));
        assert!(!can_delete("openai-backend", "openai-backend"));
    }
}
