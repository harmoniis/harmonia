//! Filesystem RPC operations (list, read).

use harmonia_node_rpc::{NodeFsEntry, NodePathRef, NodeRpcResult};
use std::fs;

use super::helpers::resolve_path;

pub(crate) fn fs_list(
    node: &crate::paths::NodeIdentity,
    path: &NodePathRef,
    include_hidden: bool,
    max_entries: u32,
) -> Result<NodeRpcResult, String> {
    let dir = resolve_path(node, path)?;
    let mut entries = Vec::new();
    let mut dir_entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("read_dir {} failed: {e}", dir.display()))?
        .filter_map(Result::ok)
        .collect();
    dir_entries.sort_by_key(|entry| entry.file_name());
    for entry in dir_entries.into_iter().take(max_entries.max(1) as usize) {
        let name = entry.file_name().to_string_lossy().to_string();
        if !include_hidden && name.starts_with('.') {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|e| format!("metadata {} failed: {e}", name))?;
        entries.push(NodeFsEntry {
            name: name.clone(),
            path: entry.path().display().to_string(),
            is_dir: metadata.is_dir(),
            size_bytes: metadata.len(),
        });
    }
    Ok(NodeRpcResult::FsList { entries })
}

pub(crate) fn fs_read_text(
    node: &crate::paths::NodeIdentity,
    path: &NodePathRef,
    max_bytes: u64,
) -> Result<NodeRpcResult, String> {
    let full = resolve_path(node, path)?;
    let max_bytes = max_bytes.clamp(1, 256 * 1024) as usize;
    let bytes = fs::read(&full).map_err(|e| format!("read {} failed: {e}", full.display()))?;
    let truncated = bytes.len() > max_bytes;
    let slice = if truncated {
        &bytes[..max_bytes]
    } else {
        &bytes[..]
    };
    Ok(NodeRpcResult::FsReadText {
        path: full.display().to_string(),
        text: String::from_utf8_lossy(slice).into_owned(),
        truncated,
    })
}
