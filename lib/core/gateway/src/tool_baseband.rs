use crate::tool_registry::ToolRegistry;
use harmonia_tool_channel_protocol::{build_tool_envelope, ToolResult};

pub fn invoke_tool_signal(
    tool_registry: &ToolRegistry,
    tool_name: &str,
    operation: &str,
    params: &str,
) -> Result<String, String> {
    let result = tool_registry.invoke(tool_name, operation, params)?;
    let envelope = build_tool_envelope(&result);

    // Post to actor mailbox if available
    let gw_actor_id = crate::state::actor_id();
    if gw_actor_id > 0 && harmonia_actor_protocol::client::is_available() {
        let envelope_sexp = envelope.to_sexp();
        let _ = harmonia_actor_protocol::client::post(
            gw_actor_id,
            0,
            &format!(
                "(:tool-result :envelope \"{}\")",
                harmonia_actor_protocol::sexp_escape(&envelope_sexp)
            ),
        );
    }

    Ok(result.to_sexp())
}

pub fn invoke_tool_raw(
    tool_registry: &ToolRegistry,
    tool_name: &str,
    operation: &str,
    params: &str,
) -> Result<ToolResult, String> {
    tool_registry.invoke(tool_name, operation, params)
}
