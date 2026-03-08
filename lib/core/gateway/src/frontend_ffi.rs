use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[allow(dead_code)]
pub struct FrontendVtable {
    _lib: Library, // keep loaded
    pub name: String,
    // Function pointers resolved from the .so
    fn_version: unsafe extern "C" fn() -> *const c_char,
    fn_healthcheck: unsafe extern "C" fn() -> i32,
    fn_init: unsafe extern "C" fn(*const c_char) -> i32,
    fn_poll: unsafe extern "C" fn(*mut c_char, usize) -> i32,
    fn_send: unsafe extern "C" fn(*const c_char, *const c_char) -> i32,
    fn_last_error: unsafe extern "C" fn() -> *const c_char,
    fn_shutdown: unsafe extern "C" fn() -> i32,
    fn_free_string: unsafe extern "C" fn(*mut c_char),
    // Optional
    fn_list_channels: Option<unsafe extern "C" fn() -> *const c_char>,
    fn_channel_label: Option<unsafe extern "C" fn(*const c_char) -> *const c_char>,
}

impl FrontendVtable {
    pub unsafe fn load(name: &str, so_path: &str) -> Result<Self, String> {
        let lib = Library::new(so_path).map_err(|e| format!("dlopen {} failed: {}", so_path, e))?;

        // Resolve required symbols
        let fn_version: Symbol<unsafe extern "C" fn() -> *const c_char> = lib
            .get(b"harmonia_frontend_version\0")
            .map_err(|e| format!("missing harmonia_frontend_version: {e}"))?;
        let fn_healthcheck: Symbol<unsafe extern "C" fn() -> i32> = lib
            .get(b"harmonia_frontend_healthcheck\0")
            .map_err(|e| format!("missing harmonia_frontend_healthcheck: {e}"))?;
        let fn_init: Symbol<unsafe extern "C" fn(*const c_char) -> i32> = lib
            .get(b"harmonia_frontend_init\0")
            .map_err(|e| format!("missing harmonia_frontend_init: {e}"))?;
        let fn_poll: Symbol<unsafe extern "C" fn(*mut c_char, usize) -> i32> = lib
            .get(b"harmonia_frontend_poll\0")
            .map_err(|e| format!("missing harmonia_frontend_poll: {e}"))?;
        let fn_send: Symbol<unsafe extern "C" fn(*const c_char, *const c_char) -> i32> = lib
            .get(b"harmonia_frontend_send\0")
            .map_err(|e| format!("missing harmonia_frontend_send: {e}"))?;
        let fn_last_error: Symbol<unsafe extern "C" fn() -> *const c_char> = lib
            .get(b"harmonia_frontend_last_error\0")
            .map_err(|e| format!("missing harmonia_frontend_last_error: {e}"))?;
        let fn_shutdown: Symbol<unsafe extern "C" fn() -> i32> = lib
            .get(b"harmonia_frontend_shutdown\0")
            .map_err(|e| format!("missing harmonia_frontend_shutdown: {e}"))?;
        let fn_free_string: Symbol<unsafe extern "C" fn(*mut c_char)> = lib
            .get(b"harmonia_frontend_free_string\0")
            .map_err(|e| format!("missing harmonia_frontend_free_string: {e}"))?;

        // Optional symbols
        let fn_list_channels: Option<Symbol<unsafe extern "C" fn() -> *const c_char>> =
            lib.get(b"harmonia_frontend_list_channels\0").ok();
        let fn_channel_label: Option<Symbol<unsafe extern "C" fn(*const c_char) -> *const c_char>> =
            lib.get(b"harmonia_frontend_channel_label\0").ok();

        Ok(Self {
            fn_version: *fn_version,
            fn_healthcheck: *fn_healthcheck,
            fn_init: *fn_init,
            fn_poll: *fn_poll,
            fn_send: *fn_send,
            fn_last_error: *fn_last_error,
            fn_shutdown: *fn_shutdown,
            fn_free_string: *fn_free_string,
            fn_list_channels: fn_list_channels.map(|s| *s),
            fn_channel_label: fn_channel_label.map(|s| *s),
            _lib: lib,
            name: name.to_string(),
        })
    }

    pub fn version(&self) -> String {
        unsafe {
            let ptr = (self.fn_version)();
            if ptr.is_null() {
                return String::new();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }

    pub fn healthcheck(&self) -> bool {
        unsafe { (self.fn_healthcheck)() == 1 }
    }

    pub fn init(&self, config: &str) -> Result<(), String> {
        let c_config = CString::new(config).map_err(|e| format!("invalid config: {e}"))?;
        let rc = unsafe { (self.fn_init)(c_config.as_ptr()) };
        if rc == 0 {
            Ok(())
        } else {
            Err(self.get_last_error())
        }
    }

    /// Poll for pending inbound signals. Returns raw sexp string from frontend.
    pub fn poll(&self) -> Result<Option<String>, String> {
        let mut buf = vec![0u8; 65536]; // 64KB buffer
        let rc = unsafe { (self.fn_poll)(buf.as_mut_ptr() as *mut c_char, buf.len()) };
        if rc < 0 {
            return Err(self.get_last_error());
        }
        if rc == 0 {
            return Ok(None);
        }
        let len = rc as usize;
        let s = String::from_utf8_lossy(&buf[..len.min(buf.len())]).to_string();
        if s.is_empty() || s == "nil" {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    }

    pub fn send(&self, channel: &str, payload: &str) -> Result<(), String> {
        let c_channel = CString::new(channel).map_err(|e| format!("invalid channel: {e}"))?;
        let c_payload = CString::new(payload).map_err(|e| format!("invalid payload: {e}"))?;
        let rc = unsafe { (self.fn_send)(c_channel.as_ptr(), c_payload.as_ptr()) };
        if rc == 0 {
            Ok(())
        } else {
            Err(self.get_last_error())
        }
    }

    pub fn shutdown(&self) -> Result<(), String> {
        let rc = unsafe { (self.fn_shutdown)() };
        if rc == 0 {
            Ok(())
        } else {
            Err(self.get_last_error())
        }
    }

    pub fn list_channels(&self) -> Option<String> {
        self.fn_list_channels.map(|f| unsafe {
            let ptr = f();
            if ptr.is_null() {
                return String::new();
            }
            let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            // Don't free — it's a static or the frontend manages it
            s
        })
    }

    fn get_last_error(&self) -> String {
        unsafe {
            let ptr = (self.fn_last_error)();
            if ptr.is_null() {
                return "unknown frontend error".to_string();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}
