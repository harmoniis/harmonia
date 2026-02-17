use std::os::raw::c_char;

const VERSION: &[u8] = b"harmonia-rust-forge/0.1.0\0";

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_healthcheck() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_rust_forge_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_rust_forge_version().is_null());
    }
}
