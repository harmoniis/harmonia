use std::os::raw::c_char;

const VERSION: &[u8] = b"harmonia-push-sns/0.1.0\0";

#[no_mangle]
pub extern "C" fn harmonia_push_sns_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_push_sns_healthcheck() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_push_sns_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_push_sns_version().is_null());
    }
}
