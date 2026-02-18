use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-cron-scheduler/0.2.0\0";

#[derive(Clone)]
struct Job {
    name: String,
    interval_secs: u64,
    next_due: u64,
}

#[derive(Default)]
struct SchedulerState {
    jobs: Vec<Job>,
}

static STATE: OnceLock<RwLock<SchedulerState>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn state() -> &'static RwLock<SchedulerState> {
    STATE.get_or_init(|| RwLock::new(SchedulerState::default()))
}

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_add_job(name: *const c_char, interval_secs: i32) -> i32 {
    let name = match cstr_to_string(name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    if interval_secs <= 0 {
        set_error("interval must be > 0");
        return -1;
    }
    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("scheduler lock poisoned");
            return -1;
        }
    };
    let iv = interval_secs as u64;
    st.jobs.push(Job {
        name,
        interval_secs: iv,
        next_due: now_secs() + iv,
    });
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_due_jobs(now: i64) -> *mut c_char {
    let now = if now <= 0 { now_secs() } else { now as u64 };
    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("scheduler lock poisoned");
            return std::ptr::null_mut();
        }
    };
    let mut due = Vec::new();
    for job in &mut st.jobs {
        if now >= job.next_due {
            due.push(job.name.clone());
            while now >= job.next_due {
                job.next_due += job.interval_secs;
            }
        }
    }
    clear_error();
    to_c_string(due.join(","))
}

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_reset() -> i32 {
    match state().write() {
        Ok(mut st) => {
            st.jobs.clear();
            clear_error();
            0
        }
        Err(_) => {
            set_error("scheduler lock poisoned");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "scheduler lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_cron_scheduler_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer comes from CString::into_raw in this crate.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_cron_scheduler_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_cron_scheduler_version().is_null());
    }
}
