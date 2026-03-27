//! Process isolation and runtime self-protection.
//!
//! - **Anti-ptrace**: Prevents debuggers and other processes from attaching
//! - **Core dump suppression**: Prevents sensitive memory from being dumped to disk
//! - **Environment sanitization**: Strips dangerous env vars before spawning children
//!
//! All hardening is best-effort: failures are logged but never fatal.

/// Apply all available hardening measures for the current platform.
pub fn harden() {
    #[cfg(target_os = "linux")]
    linux::harden_linux();

    #[cfg(target_os = "macos")]
    macos::harden_macos();

    #[cfg(target_os = "freebsd")]
    freebsd::harden_freebsd();

    sanitize_env();
    eprintln!("[INFO] [runtime] Process hardening applied");
}

/// Remove dangerous environment variables that could hijack child processes.
fn sanitize_env() {
    const DANGEROUS_VARS: &[&str] = &[
        "LD_PRELOAD", "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH", "DYLD_FRAMEWORK_PATH",
        "PYTHONPATH", "RUBYOPT", "PERL5OPT", "PERL5LIB",
        "LOCALDOMAIN", "HOSTALIASES",
        "MALLOC_CHECK_", "MALLOC_PERTURB_",
    ];
    for var in DANGEROUS_VARS {
        if std::env::var_os(var).is_some() {
            std::env::remove_var(var);
            eprintln!("[INFO] [hardening] Removed dangerous env var: {var}");
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    pub fn harden_linux() {
        let ret = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] PR_SET_DUMPABLE=0 (ptrace protection)");
        }
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] RLIMIT_CORE=0 (core dumps disabled)");
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    pub fn harden_macos() {
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] RLIMIT_CORE=0 (core dumps disabled)");
        }
    }
}

#[cfg(target_os = "freebsd")]
mod freebsd {
    pub fn harden_freebsd() {
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] RLIMIT_CORE=0 (core dumps disabled)");
        }
        const PROC_TRACE_CTL: i32 = 7;
        const PROC_TRACE_CTL_DISABLE: i32 = 1;
        let val: i32 = PROC_TRACE_CTL_DISABLE;
        let ret = unsafe {
            libc::procctl(libc::P_PID as u32, 0, PROC_TRACE_CTL,
                &val as *const i32 as *mut libc::c_void)
        };
        if ret == 0 {
            eprintln!("[INFO] [hardening] PROC_TRACE_CTL disabled (ptrace protection)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_env_removes_dangerous_vars() {
        std::env::set_var("LD_PRELOAD", "/tmp/evil.so");
        sanitize_env();
        assert!(std::env::var_os("LD_PRELOAD").is_none());
    }

    #[test]
    fn sanitize_env_preserves_safe_vars() {
        std::env::set_var("HARMONIA_TEST_SAFE", "keep-me");
        sanitize_env();
        assert_eq!(std::env::var("HARMONIA_TEST_SAFE").unwrap(), "keep-me");
        std::env::remove_var("HARMONIA_TEST_SAFE");
    }

    #[test]
    fn harden_does_not_panic() {
        harden();
    }
}
