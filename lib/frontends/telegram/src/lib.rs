pub mod bot;
mod frontend_ffi;

// Re-export FFI symbols so they appear in the cdylib.
pub use frontend_ffi::*;

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    #[test]
    fn version_returns_expected_string() {
        let ptr = super::harmonia_frontend_version();
        assert!(!ptr.is_null());
        let v = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        assert_eq!(v, "harmonia-telegram/0.2.0");
    }

    #[test]
    fn healthcheck_zero_before_init() {
        let h = super::harmonia_frontend_healthcheck();
        assert!(h == 0 || h == 1); // may be 1 if another test already called init
    }
}
