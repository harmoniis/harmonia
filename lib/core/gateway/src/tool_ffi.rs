use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{self, AssertUnwindSafe};

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic in tool cdylib".to_string()
    }
}

#[allow(dead_code)]
pub struct ToolVtable {
    _lib: Library,
    pub name: String,
    so_path: String,
    config: String,
    // Required symbols
    fn_version: unsafe extern "C" fn() -> *const c_char,
    fn_healthcheck: unsafe extern "C" fn() -> i32,
    fn_init: unsafe extern "C" fn(*const c_char) -> i32,
    fn_invoke: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char,
    fn_capabilities: unsafe extern "C" fn() -> *const c_char,
    fn_last_error: unsafe extern "C" fn() -> *const c_char,
    fn_shutdown: unsafe extern "C" fn() -> i32,
    fn_free_string: unsafe extern "C" fn(*mut c_char),
}

impl ToolVtable {
    pub unsafe fn load(name: &str, so_path: &str) -> Result<Self, String> {
        let lib = Library::new(so_path).map_err(|e| format!("dlopen {} failed: {}", so_path, e))?;

        let fn_version: Symbol<unsafe extern "C" fn() -> *const c_char> = lib
            .get(b"harmonia_tool_version\0")
            .map_err(|e| format!("missing harmonia_tool_version: {e}"))?;
        let fn_healthcheck: Symbol<unsafe extern "C" fn() -> i32> = lib
            .get(b"harmonia_tool_healthcheck\0")
            .map_err(|e| format!("missing harmonia_tool_healthcheck: {e}"))?;
        let fn_init: Symbol<unsafe extern "C" fn(*const c_char) -> i32> = lib
            .get(b"harmonia_tool_init\0")
            .map_err(|e| format!("missing harmonia_tool_init: {e}"))?;
        let fn_invoke: Symbol<unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char> =
            lib.get(b"harmonia_tool_invoke\0")
                .map_err(|e| format!("missing harmonia_tool_invoke: {e}"))?;
        let fn_capabilities: Symbol<unsafe extern "C" fn() -> *const c_char> = lib
            .get(b"harmonia_tool_capabilities\0")
            .map_err(|e| format!("missing harmonia_tool_capabilities: {e}"))?;
        let fn_last_error: Symbol<unsafe extern "C" fn() -> *const c_char> = lib
            .get(b"harmonia_tool_last_error\0")
            .map_err(|e| format!("missing harmonia_tool_last_error: {e}"))?;
        let fn_shutdown: Symbol<unsafe extern "C" fn() -> i32> = lib
            .get(b"harmonia_tool_shutdown\0")
            .map_err(|e| format!("missing harmonia_tool_shutdown: {e}"))?;
        let fn_free_string: Symbol<unsafe extern "C" fn(*mut c_char)> = lib
            .get(b"harmonia_tool_free_string\0")
            .map_err(|e| format!("missing harmonia_tool_free_string: {e}"))?;

        Ok(Self {
            fn_version: *fn_version,
            fn_healthcheck: *fn_healthcheck,
            fn_init: *fn_init,
            fn_invoke: *fn_invoke,
            fn_capabilities: *fn_capabilities,
            fn_last_error: *fn_last_error,
            fn_shutdown: *fn_shutdown,
            fn_free_string: *fn_free_string,
            _lib: lib,
            name: name.to_string(),
            so_path: so_path.to_string(),
            config: String::new(),
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

    pub fn healthcheck(&self) -> Result<bool, String> {
        let f = self.fn_healthcheck;
        panic::catch_unwind(AssertUnwindSafe(|| unsafe { f() }))
            .map(|rc| rc == 1)
            .map_err(panic_message)
    }

    pub fn init(&mut self, config: &str) -> Result<(), String> {
        self.config = config.to_string();
        let c_config = CString::new(config).map_err(|e| format!("invalid config: {e}"))?;
        let f = self.fn_init;
        let ptr = c_config.as_ptr();
        let rc =
            panic::catch_unwind(AssertUnwindSafe(|| unsafe { f(ptr) })).map_err(panic_message)?;
        if rc == 0 {
            Ok(())
        } else {
            Err(self.get_last_error())
        }
    }

    pub fn invoke(&self, operation: &str, params: &str) -> Result<String, String> {
        let c_op = CString::new(operation).map_err(|e| format!("invalid operation: {e}"))?;
        let c_params = CString::new(params).map_err(|e| format!("invalid params: {e}"))?;
        let f = self.fn_invoke;
        let op_ptr = c_op.as_ptr();
        let params_ptr = c_params.as_ptr();
        let ptr = panic::catch_unwind(AssertUnwindSafe(|| unsafe { f(op_ptr, params_ptr) }))
            .map_err(panic_message)?;
        if ptr.is_null() {
            return Err(self.get_last_error());
        }
        let result = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { (self.fn_free_string)(ptr) };
        Ok(result)
    }

    pub fn capabilities(&self) -> String {
        unsafe {
            let ptr = (self.fn_capabilities)();
            if ptr.is_null() {
                return "nil".to_string();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }

    pub fn shutdown(&self) -> Result<(), String> {
        let f = self.fn_shutdown;
        let rc = panic::catch_unwind(AssertUnwindSafe(|| unsafe { f() })).map_err(panic_message)?;
        if rc == 0 {
            Ok(())
        } else {
            Err(self.get_last_error())
        }
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let _ = self.shutdown();
        let so_path = self.so_path.clone();
        let name = self.name.clone();
        let config = self.config.clone();
        let mut fresh = unsafe { Self::load(&name, &so_path)? };
        fresh.config = config.clone();
        if !config.is_empty() {
            fresh.init(&config)?;
        }
        *self = fresh;
        Ok(())
    }

    fn get_last_error(&self) -> String {
        unsafe {
            let ptr = (self.fn_last_error)();
            if ptr.is_null() {
                return "unknown tool error".to_string();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}
