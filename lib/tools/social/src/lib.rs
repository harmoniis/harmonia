use std::os::raw::c_char;

const VERSION: &[u8] = b"harmonia-social/0.1.0\0";

#[no_mangle]
pub extern "C" fn harmonia_social_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_social_healthcheck() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_social_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_social_version().is_null());
    }
}
