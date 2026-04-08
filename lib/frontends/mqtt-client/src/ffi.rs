use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub(crate) fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

pub(crate) fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

pub fn harmonia_mqtt_client_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(CString::from_raw(ptr)) };
}
