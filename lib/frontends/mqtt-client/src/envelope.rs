use chrono::Utc;
use std::os::raw::c_char;
use uuid::Uuid;

use crate::ffi::{cstr_to_string, to_c_string};
use crate::model::{clear_error, set_error, MessageEnvelope};

pub fn harmonia_mqtt_client_make_envelope(
    kind: *const c_char,
    type_name: *const c_char,
    agent_fp: *const c_char,
    client_fp: *const c_char,
    body_json: *const c_char,
) -> *mut c_char {
    let kind = match cstr_to_string(kind) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let type_name = match cstr_to_string(type_name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let agent_fp = match cstr_to_string(agent_fp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let client_fp = match cstr_to_string(client_fp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let body_json = match cstr_to_string(body_json) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let body: serde_json::Value = match serde_json::from_str(&body_json) {
        Ok(v) => v,
        Err(e) => {
            set_error(format!("invalid envelope body json: {e}"));
            return std::ptr::null_mut();
        }
    };
    let env = MessageEnvelope {
        v: 1,
        kind,
        type_name,
        id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        agent_fp,
        client_fp,
        body,
    };
    match serde_json::to_string(&env) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(format!("envelope serialize failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

pub fn harmonia_mqtt_client_parse_envelope(payload: *const c_char) -> *mut c_char {
    let payload = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let env: MessageEnvelope = match serde_json::from_str(&payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(format!("invalid envelope json: {e}"));
            return std::ptr::null_mut();
        }
    };
    if env.v != 1 {
        set_error(format!("unsupported envelope version {}", env.v));
        return std::ptr::null_mut();
    }
    match serde_json::to_string(&env) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(format!("envelope normalize failed: {e}"));
            std::ptr::null_mut()
        }
    }
}
