use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{OnceLock, RwLock};

#[cfg(test)]
use std::process;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-s3-sync/0.2.0\0";

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    // Safety: caller provides valid null-terminated string.
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn local_copy(src: &Path, bucket: &str, prefix: &str, key: &str) -> Result<PathBuf, String> {
    let root = env::var("HARMONIA_S3_LOCAL_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::temp_dir().join("harmonia-s3-local"));
    let clean_prefix = prefix.trim_matches('/');
    let mut dest = root.join(bucket);
    if !clean_prefix.is_empty() {
        dest = dest.join(clean_prefix);
    }
    dest = dest.join(key);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {e}"))?;
    }
    fs::copy(src, &dest).map_err(|e| format!("copy failed: {e}"))?;
    Ok(dest)
}

#[no_mangle]
pub extern "C" fn harmonia_s3_sync_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_s3_sync_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_s3_sync_upload_file(
    source_path: *const c_char,
    bucket: *const c_char,
    prefix: *const c_char,
    key: *const c_char,
) -> i32 {
    let source = match cstr_to_string(source_path) {
        Ok(v) => PathBuf::from(v),
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let bucket = match cstr_to_string(bucket) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let prefix = cstr_to_string(prefix).unwrap_or_default();
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    if !source.exists() {
        set_error(format!("source missing: {}", source.display()));
        return -1;
    }

    let mode = env::var("HARMONIA_S3_MODE").unwrap_or_else(|_| "local".to_string());
    if mode.eq_ignore_ascii_case("local") {
        match local_copy(&source, &bucket, &prefix, &key) {
            Ok(_) => {
                clear_error();
                0
            }
            Err(e) => {
                set_error(e);
                -1
            }
        }
    } else {
        let target = if prefix.trim().is_empty() {
            format!("s3://{bucket}/{key}")
        } else {
            format!("s3://{bucket}/{}/{}", prefix.trim_matches('/'), key)
        };
        let output = Command::new("aws")
            .arg("s3")
            .arg("cp")
            .arg(source.to_string_lossy().to_string())
            .arg(target)
            .output();
        match output {
            Ok(out) if out.status.success() => {
                clear_error();
                0
            }
            Ok(out) => {
                set_error(format!("aws s3 cp failed: {}", String::from_utf8_lossy(&out.stderr)));
                -1
            }
            Err(e) => {
                set_error(format!("aws command failed: {e}"));
                -1
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_s3_sync_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "s3 lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_s3_sync_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer comes from CString::into_raw in this crate.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("harmonia-s3-{label}-{}-{ts}", process::id()))
    }

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_s3_sync_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_s3_sync_version().is_null());
    }

    #[test]
    fn local_upload_roundtrip() {
        let root = temp_dir("local");
        fs::create_dir_all(&root).unwrap();
        let src = root.join("artifact.bin");
        fs::write(&src, b"harmonia-artifact").unwrap();
        env::set_var("HARMONIA_S3_MODE", "local");
        env::set_var("HARMONIA_S3_LOCAL_ROOT", root.join("bucket-root"));

        let csrc = CString::new(src.to_string_lossy().to_string()).unwrap();
        let bucket = CString::new("test-bucket").unwrap();
        let prefix = CString::new("v-test").unwrap();
        let key = CString::new("artifact.bin").unwrap();
        assert_eq!(
            harmonia_s3_sync_upload_file(csrc.as_ptr(), bucket.as_ptr(), prefix.as_ptr(), key.as_ptr()),
            0
        );
        let dest = root
            .join("bucket-root")
            .join("test-bucket")
            .join("v-test")
            .join("artifact.bin");
        assert!(dest.exists());
    }
}
