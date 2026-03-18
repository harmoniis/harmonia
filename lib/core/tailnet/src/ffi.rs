use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

use crate::mesh;
use crate::model::{MeshMessage, MeshMessageType};
use crate::transport;

const VERSION: &[u8] = b"harmonia-tailnet/0.1.0\0";

// ---------------------------------------------------------------------------
// Error state (mirrors vault pattern)
// ---------------------------------------------------------------------------

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn last_error_lock() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_lock().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error_lock().write() {
        slot.clear();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// C-ABI exports
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn harmonia_tailnet_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_tailnet_healthcheck() -> i32 {
    1
}

/// Initialise the mesh from an s-expression config string, then start the
/// TCP listener.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_init(config: *const c_char) -> i32 {
    let config_str = match cstr_to_string(config) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    if let Err(e) = mesh::init(&config_str) {
        set_error(e);
        return -1;
    }

    if let Err(e) = transport::start_listener() {
        set_error(e);
        return -1;
    }

    clear_error();
    0
}

/// Return known peers as an s-expression list.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_discover_peers() -> *mut c_char {
    match mesh::discover_peers() {
        Ok(peers) => {
            let sexps: Vec<String> = peers.iter().map(|p| p.to_sexp()).collect();
            let result = format!("(peers {})", sexps.join(" "));
            clear_error();
            to_c_string(result)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// Send a mesh message to `to_addr` with the given `payload` and `msg_type`.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_send(
    to_addr: *const c_char,
    payload: *const c_char,
    msg_type: *const c_char,
) -> i32 {
    let addr = match cstr_to_string(to_addr) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let payload_str = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let type_str = match cstr_to_string(msg_type) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let mt = match MeshMessageType::from_str(&type_str) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let local_node = mesh::local_node().ok();
    let from = local_node
        .as_ref()
        .map(|node| node.id.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let msg = MeshMessage {
        from,
        to: addr.clone(),
        payload: payload_str,
        msg_type: mt,
        origin: local_node.map(|node| crate::model::MeshOrigin {
            node_id: node.id.0,
            node_label: Some(node.label),
            node_role: Some(node.role),
            channel_class: Some("tailscale-agent".to_string()),
            node_key_id: None,
            transport_security: None,
        }),
        session: None,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        hmac: String::new(),
    };

    match transport::send_message(&addr, &msg) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

/// Poll all received messages and return them as an s-expression list.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_poll() -> *mut c_char {
    let messages = transport::poll_messages();
    let sexps: Vec<String> = messages.iter().map(|m| m.to_sexp()).collect();
    let result = format!("(messages {})", sexps.join(" "));
    clear_error();
    to_c_string(result)
}

/// Return local node info as an s-expression.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_node_info() -> *mut c_char {
    match mesh::local_node_info() {
        Ok(sexp) => {
            clear_error();
            to_c_string(sexp)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// Return the last error message.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_last_error() -> *mut c_char {
    let msg = last_error_lock()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "tailnet lock poisoned".to_string());
    to_c_string(msg)
}

/// Free a string previously returned by this crate.
#[no_mangle]
pub extern "C" fn harmonia_tailnet_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_tailnet_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_tailnet_version().is_null());
    }
}
