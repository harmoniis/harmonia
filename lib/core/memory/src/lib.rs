use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-memory/0.2.0\0";

#[derive(Default)]
struct MemoryState {
    file_path: Option<PathBuf>,
    entries: HashMap<String, String>,
}

static MEMORY: OnceLock<RwLock<MemoryState>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn state() -> &'static RwLock<MemoryState> {
    MEMORY.get_or_init(|| RwLock::new(MemoryState::default()))
}

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
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn encode_entry(k: &str, v: &str) -> String {
    let key = k.replace('\\', "\\\\").replace('\n', "\\n");
    let val = v.replace('\\', "\\\\").replace('\n', "\\n");
    format!("{key}\t{val}\n")
}

fn decode_line(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once('\t')?;
    let key = k.replace("\\n", "\n").replace("\\\\", "\\");
    let val = v.replace("\\n", "\n").replace("\\\\", "\\");
    Some((key, val))
}

fn persist_to_disk(path: &Path, map: &HashMap<String, String>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {e}"))?;
    }
    let mut body = String::new();
    for (k, v) in map {
        body.push_str(&encode_entry(k, v));
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, body).map_err(|e| format!("write tmp failed: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename failed: {e}"))?;
    Ok(())
}

#[no_mangle]
pub extern "C" fn harmonia_memory_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_memory_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_memory_init(file_path: *const c_char) -> i32 {
    let path = match cstr_to_string(file_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let path_buf = PathBuf::from(path);
    let mut entries = HashMap::new();
    if path_buf.exists() {
        match fs::read_to_string(&path_buf) {
            Ok(body) => {
                for line in body.lines() {
                    if let Some((k, v)) = decode_line(line) {
                        entries.insert(k, v);
                    }
                }
            }
            Err(e) => {
                set_error(format!("read memory file failed: {e}"));
                return -1;
            }
        }
    }
    match state().write() {
        Ok(mut st) => {
            st.file_path = Some(path_buf);
            st.entries = entries;
            clear_error();
            0
        }
        Err(_) => {
            set_error("memory lock poisoned");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_memory_put(key: *const c_char, value: *const c_char) -> i32 {
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let value = match cstr_to_string(value) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("memory lock poisoned");
            return -1;
        }
    };

    st.entries.insert(key, value);
    if let Some(path) = st.file_path.clone() {
        if let Err(e) = persist_to_disk(&path, &st.entries) {
            set_error(e);
            return -1;
        }
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_memory_get(key: *const c_char) -> *mut c_char {
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("memory lock poisoned");
            return std::ptr::null_mut();
        }
    };
    match st.entries.get(&key) {
        Some(v) => {
            clear_error();
            to_c_string(v.clone())
        }
        None => {
            set_error(format!("missing key: {key}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_memory_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "memory lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_memory_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: ptr must come from CString::into_raw in this crate.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("harmonia-memory-{label}-{}-{ts}.db", process::id()))
    }

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_memory_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_memory_version().is_null());
    }

    #[test]
    fn put_and_get_roundtrip() {
        let path = temp_path("roundtrip");
        let cpath = CString::new(path.to_string_lossy().to_string()).unwrap();
        assert_eq!(harmonia_memory_init(cpath.as_ptr()), 0);

        let key = CString::new("dna/core").unwrap();
        let val = CString::new("(rewrite-count . 3)").unwrap();
        assert_eq!(harmonia_memory_put(key.as_ptr(), val.as_ptr()), 0);

        let ptr = harmonia_memory_get(key.as_ptr());
        assert!(!ptr.is_null());
        let got = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_memory_free_string(ptr);
        assert_eq!(got, "(rewrite-count . 3)");
    }
}
