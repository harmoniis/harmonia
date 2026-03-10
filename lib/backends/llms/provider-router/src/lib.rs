use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use harmonia_openrouter::client as openrouter;
use harmonia_provider_protocol::{clear_error, last_error_message, set_error};

const VERSION: &[u8] = b"harmonia-provider-router/0.1.0\0";

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

fn backend_status_sexp() -> String {
    "(:id \"openrouter\" :healthy t :default t :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))".to_string()
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_init() -> i32 {
    match openrouter::init_backend() {
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
pub extern "C" fn harmonia_provider_router_complete(
    prompt: *const c_char,
    model: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let model = cstr_to_string(model).unwrap_or_default();
    match openrouter::complete(&prompt, &model) {
        Ok(text) => {
            clear_error();
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_complete_for_task(
    prompt: *const c_char,
    task_hint: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let task_hint = cstr_to_string(task_hint).unwrap_or_default();
    match openrouter::complete_for_task(&prompt, &task_hint) {
        Ok(text) => {
            clear_error();
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_list_models() -> *mut c_char {
    clear_error();
    to_c_string(openrouter::list_offerings())
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_select_model(task_hint: *const c_char) -> *mut c_char {
    let task_hint = cstr_to_string(task_hint).unwrap_or_default();
    clear_error();
    to_c_string(openrouter::select_model_for_task(&task_hint))
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_list_backends() -> *mut c_char {
    clear_error();
    to_c_string(format!("({})", backend_status_sexp()))
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_backend_status(name: *const c_char) -> *mut c_char {
    let name = cstr_to_string(name).unwrap_or_default();
    if !name.is_empty() && name != "openrouter" {
        set_error(format!("unknown backend adapter: {name}"));
        return std::ptr::null_mut();
    }
    clear_error();
    to_c_string(backend_status_sexp())
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
