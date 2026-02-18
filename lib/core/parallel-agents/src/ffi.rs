use std::os::raw::c_char;

use crate::engine;
use crate::model::{clear_error, cstr_to_string, last_error_message, set_error, to_c_string};

const VERSION: &[u8] = b"harmonia-parallel-agents/0.1.0\0";

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_healthcheck() -> i32 {
    engine::healthcheck()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_init() -> i32 {
    engine::init_ffi()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_set_model_price(
    model: *const c_char,
    usd_per_1k_input: f64,
    usd_per_1k_output: f64,
) -> i32 {
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match engine::set_model_price(&model, usd_per_1k_input, usd_per_1k_output) {
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

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_submit(
    prompt: *const c_char,
    model: *const c_char,
) -> i64 {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match engine::submit(&prompt, &model) {
        Ok(id) => {
            clear_error();
            id
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_run_pending(max_parallel: i32) -> i32 {
    match engine::run_pending(max_parallel) {
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

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_task_result(task_id: i64) -> *mut c_char {
    match engine::task_result(task_id) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_report() -> *mut c_char {
    match engine::report() {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(std::ffi::CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_parallel_agents_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_non_null() {
        assert!(!harmonia_parallel_agents_version().is_null());
    }
}
