use crate::catalog::LodeCatalog;
use crate::lode::Lode;
use crate::platform::Platform;
use crate::tools::{CpuCost, Domain, MineCost, NetCost, Precondition, ToolKind};

pub fn load_user_tools(catalog: &mut LodeCatalog, json: &str) {
    for tool in parse_user_tools(json) { catalog.register(tool); }
}

fn parse_user_tools(json: &str) -> Vec<Lode> {
    serde_json::from_str::<Vec<UserToolDef>>(json)
        .map(|defs| defs.into_iter().filter_map(|d| d.into_lode()).collect())
        .unwrap_or_default()
}

#[derive(serde::Deserialize)]
struct UserToolDef {
    id: String,
    cmd: String,
    #[serde(default, rename = "args")]
    _args: Vec<String>,
    #[serde(default = "default_domain")]
    domain: String,
    #[serde(default = "default_latency")]
    latency_ms: u32,
}
fn default_domain() -> String { "generic".into() }
fn default_latency() -> u32 { 500 }

impl UserToolDef {
    fn into_lode(self) -> Option<Lode> {
        if self.id.is_empty() || self.cmd.is_empty() { return None; }
        let domain = Domain::from_str(&self.domain);
        let cmd = self.cmd.clone();
        Some(Lode {
            id: self.id,
            kind: ToolKind::UserSpace,
            platform: Platform::Any,
            domain,
            cost: MineCost { latency_ms: self.latency_ms, cpu: CpuCost::Low, network: NetCost::None },
            preconditions: vec![Precondition::BinaryExists(cmd)],
            available: true,
            mine_fn: user_tool_exec,
        })
    }
}

/// User tools dispatch: args[0] = cmd, args[1..] = cmd args.
fn user_tool_exec(args: &[&str]) -> Result<String, String> {
    if args.is_empty() { return Err("user-space tool requires args: cmd [arg1 arg2 ...]".into()); }
    super::system::exec_capture(args[0], &args[1..])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_empty() {
        assert!(parse_user_tools("[]").is_empty());
    }
    #[test]
    fn test_parse_valid() {
        let tools = parse_user_tools(r#"[{"id":"my-tool","cmd":"echo","args":["hello"],"domain":"engineering"}]"#);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].id, "my-tool");
        assert_eq!(tools[0].kind, ToolKind::UserSpace);
    }
    #[test]
    fn test_parse_malformed() {
        assert!(parse_user_tools("not valid json").is_empty());
    }
    #[test]
    fn test_empty_id_rejected() {
        assert!(parse_user_tools(r#"[{"id":"","cmd":"echo"}]"#).is_empty());
    }
}
