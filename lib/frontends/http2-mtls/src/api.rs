use bytes::Bytes;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::atomic::Ordering;

use crate::model::{clear_error, cstr_to_string, now_ms, set_error, state_slot, VERSION};
use crate::session::{enqueue_outbound, shutdown_state, start_server, take_state};

pub fn harmonia_frontend_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

pub fn harmonia_frontend_healthcheck() -> i32 {
    1
}

pub fn harmonia_frontend_init(config: *const c_char) -> i32 {
    let _ = cstr_to_string(config);
    if let Some(previous) = take_state() {
        shutdown_state(previous);
    }
    match start_server() {
        Ok(state) => {
            if let Ok(mut guard) = state_slot().lock() {
                *guard = Some(state);
            }
            clear_error();
            0
        }
        Err(error) => {
            set_error(error);
            -1
        }
    }
}

pub fn harmonia_frontend_poll(buf: *mut c_char, buf_len: usize) -> i32 {
    if buf.is_null() || buf_len == 0 {
        set_error("poll: null buffer or zero length");
        return -1;
    }
    let Some(inbound) = state_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|state| state.inbound.clone()))
    else {
        return 0;
    };

    let mut output = String::new();
    if let Ok(mut queue) = inbound.lock() {
        while let Some(signal) = queue.pop_front() {
            let line = format!(
                "{}\t{}\t{}\n",
                signal.sub_channel, signal.payload, signal.metadata
            );
            if output.len() + line.len() >= buf_len.saturating_sub(1) {
                queue.push_front(signal);
                break;
            }
            output.push_str(&line);
        }
    }

    if output.is_empty() {
        return 0;
    }

    let bytes = output.as_bytes();
    let count = bytes.len().min(buf_len.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, count);
        *((buf as *mut u8).add(count)) = 0;
    }
    clear_error();
    count as i32
}

pub fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    let route = match cstr_to_string(channel) {
        Ok(route) => route,
        Err(error) => {
            set_error(error);
            return -1;
        }
    };
    let payload = match cstr_to_string(payload) {
        Ok(payload) => payload,
        Err(error) => {
            set_error(error);
            return -1;
        }
    };

    let Some((outbound, last_activity_ms)) = state_slot().lock().ok().and_then(|guard| {
        guard.as_ref().and_then(|state| {
            state.sessions.read().ok().and_then(|sessions| {
                sessions
                    .get(&route)
                    .map(|handle| (handle.outbound.clone(), handle.last_activity_ms.clone()))
            })
        })
    }) else {
        set_error(format!("no active HTTP/2 stream for route {route}"));
        return -1;
    };

    let json = match serde_json::to_string(&serde_json::json!({ "payload": payload })) {
        Ok(json) => json + "\n",
        Err(error) => {
            set_error(format!("serialize outbound payload failed: {error}"));
            return -1;
        }
    };
    if let Err(error) = enqueue_outbound(&outbound, Bytes::from(json)) {
        set_error(format!("failed sending to HTTP/2 stream {route}: {error}"));
        return -1;
    }
    last_activity_ms.store(now_ms(), Ordering::Relaxed);
    clear_error();
    0
}

pub fn harmonia_frontend_last_error() -> *const c_char {
    let message = crate::model::last_error()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| "http2 frontend lock poisoned".to_string());
    CString::new(message)
        .map(|value| value.into_raw() as *const c_char)
        .unwrap_or(std::ptr::null())
}

pub fn harmonia_frontend_shutdown() -> i32 {
    if let Some(state) = take_state() {
        shutdown_state(state);
    }
    clear_error();
    0
}

pub fn harmonia_frontend_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

pub fn harmonia_frontend_list_channels() -> *const c_char {
    let channels = state_slot()
        .lock()
        .ok()
        .and_then(|guard| {
            guard.as_ref().map(|state| {
                state
                    .sessions
                    .read()
                    .map(|sessions| sessions.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default();
    let sexp = if channels.is_empty() {
        "nil".to_string()
    } else {
        format!(
            "({})",
            channels
                .iter()
                .map(|channel| format!("\"{}\"", channel))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    CString::new(sexp)
        .map(|value| value.into_raw() as *const c_char)
        .unwrap_or(std::ptr::null())
}
