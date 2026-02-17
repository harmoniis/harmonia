use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

#[cfg(test)]
use std::env;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::process;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-git-ops/0.2.0\0";

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

fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git exec failed: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[no_mangle]
pub extern "C" fn harmonia_git_ops_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_git_ops_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_git_ops_commit_all(
    repo_path: *const c_char,
    message: *const c_char,
    author_name: *const c_char,
    author_email: *const c_char,
) -> i32 {
    let repo = match cstr_to_string(repo_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let message = match cstr_to_string(message) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let author_name = cstr_to_string(author_name).unwrap_or_else(|_| "Harmonia".to_string());
    let author_email =
        cstr_to_string(author_email).unwrap_or_else(|_| "harmonia@local.invalid".to_string());

    if let Err(e) = run_git(&repo, &["add", "-A"]) {
        set_error(format!("git add failed: {e}"));
        return -1;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("-c")
        .arg("commit.gpgsign=false")
        .arg("-c")
        .arg(format!("user.name={author_name}"))
        .arg("-c")
        .arg(format!("user.email={author_email}"))
        .arg("commit")
        .arg("-m")
        .arg(&message)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            clear_error();
            0
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if stderr.contains("nothing to commit") || stderr.contains("no changes added") {
                clear_error();
                0
            } else {
                set_error(format!("git commit failed: {stderr}"));
                -1
            }
        }
        Err(e) => {
            set_error(format!("git commit exec failed: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_git_ops_push(
    repo_path: *const c_char,
    remote: *const c_char,
    branch: *const c_char,
) -> i32 {
    let repo = match cstr_to_string(repo_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let remote = cstr_to_string(remote).unwrap_or_else(|_| "origin".to_string());
    let branch = cstr_to_string(branch).unwrap_or_else(|_| "main".to_string());
    match run_git(&repo, &["push", &remote, &format!("HEAD:{branch}")]) {
        Ok(_) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(format!("git push failed: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_git_ops_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "git-ops lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_git_ops_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: ptr comes from this crate CString::into_raw.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn run(cmd: &str, args: &[&str]) {
        let out = Command::new(cmd).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "command failed: {} {:?}\nstdout={}\nstderr={}",
            cmd,
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn temp_dir(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("harmonia-gitops-{label}-{}-{ts}", process::id()))
    }

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_git_ops_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_git_ops_version().is_null());
    }

    #[test]
    fn commit_and_push_to_local_bare_remote() {
        let root = temp_dir("push");
        let repo = root.join("repo");
        let remote = root.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        run("git", &["init", "--bare", remote.to_str().unwrap()]);
        run("git", &["-C", repo.to_str().unwrap(), "init"]);
        run(
            "git",
            &[
                "-C",
                repo.to_str().unwrap(),
                "checkout",
                "-b",
                "main",
            ],
        );
        run(
            "git",
            &[
                "-C",
                repo.to_str().unwrap(),
                "remote",
                "add",
                "origin",
                remote.to_str().unwrap(),
            ],
        );
        fs::write(repo.join("dna.sexp"), "(cycle . 1)\n").unwrap();

        let repo_c = CString::new(repo.to_string_lossy().to_string()).unwrap();
        let msg_c = CString::new("test commit").unwrap();
        let name_c = CString::new("Harmonia Test").unwrap();
        let email_c = CString::new("harmonia@test.local").unwrap();
        assert_eq!(
            harmonia_git_ops_commit_all(
                repo_c.as_ptr(),
                msg_c.as_ptr(),
                name_c.as_ptr(),
                email_c.as_ptr()
            ),
            0
        );

        let origin_c = CString::new("origin").unwrap();
        let main_c = CString::new("main").unwrap();
        assert_eq!(
            harmonia_git_ops_push(repo_c.as_ptr(), origin_c.as_ptr(), main_c.as_ptr()),
            0
        );

        let out = Command::new("git")
            .arg("--git-dir")
            .arg(remote.to_string_lossy().to_string())
            .arg("rev-parse")
            .arg("refs/heads/main")
            .output()
            .unwrap();
        assert!(out.status.success());
    }
}
