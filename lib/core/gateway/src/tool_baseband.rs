use crate::tool_registry::ToolRegistry;
use harmonia_tool_channel_protocol::ToolResult;

pub fn invoke_tool_signal(
    tool_registry: &ToolRegistry,
    tool_name: &str,
    operation: &str,
    params: &str,
) -> Result<String, String> {
    let result = tool_registry.invoke(tool_name, operation, params)?;

    // Actor mailbox posting is now handled by the runtime IPC system.

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
