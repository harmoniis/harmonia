//! IPC token generation and persistence.

use rand::Rng;

/// Generate a 32-byte random token as 64 hex characters.
pub fn generate() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Write the IPC token to disk for CLI clients in separate shells.
pub fn persist_token(state_root: &str, token: &str) {
    let token_path = format!("{}/ipc.token", state_root);
    if let Some(parent) = std::path::Path::new(&token_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&token_path, token);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600));
    }
}

/// Write the IPC name to disk for SBCL and CLI clients.
pub fn persist_ipc_name(state_root: &str, name: &str) {
    let name_path = format!("{}/ipc.name", state_root);
    let _ = std::fs::write(&name_path, name);
}
