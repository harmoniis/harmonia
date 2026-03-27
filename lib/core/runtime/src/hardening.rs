//! Process isolation and runtime self-protection.
//!
//! Hardens the runtime process against external introspection and tampering:
//!
//! - **Anti-ptrace**: Prevents debuggers and other processes from attaching
//! - **Core dump suppression**: Prevents sensitive memory from being dumped to disk
//! - **Environment sanitization**: Strips dangerous env vars before spawning children
//! - **File descriptor hygiene**: Closes inherited fds that shouldn't be open
//!
//! All hardening is best-effort: failures are logged but never fatal.
//! The runtime must start even if hardening cannot be applied (e.g., in containers).

/// Apply all available hardening measures for the current platform.
pub fn harden() {
    #[cfg(target_os = "linux")]
    linux::harden_linux();

    #[cfg(target_os = "macos")]
    macos::harden_macos();

    #[cfg(target_os = "freebsd")]
    freebsd::harden_freebsd();

    // Cross-platform: sanitize environment
    sanitize_env();

    eprintln!("[INFO] [runtime] Process hardening applied");
}

/// Remove dangerous environment variables that could be used for injection
/// or to hijack child process behavior.
fn sanitize_env() {
    // Variables that can hijack dynamic linking
    const DANGEROUS_VARS: &[&str] = &[
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
        // Python/Ruby/Perl injection vectors
        "PYTHONPATH",
        "RUBYOPT",
        "PERL5OPT",
        "PERL5LIB",
        // Locale/encoding manipulation
        "LOCALDOMAIN",
        "HOSTALIASES",
        // Debug/profiling tools that could leak data
        "MALLOC_CHECK_",
        "MALLOC_PERTURB_",
    ];

    for var in DANGEROUS_VARS {
        if std::env::var_os(var).is_some() {
            std::env::remove_var(var);
            eprintln!("[INFO] [hardening] Removed dangerous env var: {var}");
        }
    }
}

// ── Linux-specific hardening ────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux {
    pub fn harden_linux() {
        disable_ptrace();
        disable_core_dumps();
        close_leaked_fds();
    }

    /// PR_SET_DUMPABLE(0) — prevent ptrace attach from non-root processes
    /// and suppress core dumps containing sensitive memory.
    fn disable_ptrace() {
        // PR_SET_DUMPABLE = 4, value 0 = not dumpable
        let ret = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] PR_SET_DUMPABLE=0 (ptrace protection active)");
        } else {
            eprintln!("[WARN] [hardening] PR_SET_DUMPABLE failed (errno={})", errno());
        }
    }

    /// RLIMIT_CORE = 0 — belt-and-suspenders core dump prevention.
    fn disable_core_dumps() {
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] RLIMIT_CORE=0 (core dumps disabled)");
        } else {
            eprintln!("[WARN] [hardening] RLIMIT_CORE failed (errno={})", errno());
        }
    }

    /// Close file descriptors above stderr that may have been inherited.
    /// Prevents fd leaks from parent processes reaching our child processes.
    fn close_leaked_fds() {
        // Only close fds 3..64 — higher fds are unlikely to be leaked
        // and iterating to MAX_FD is expensive.
        let mut closed = 0u32;
        for fd in 3..64 {
            // fcntl F_GETFD to check if fd is open without side effects
            let ret = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if ret >= 0 {
                unsafe { libc::close(fd); }
                closed += 1;
            }
        }
        if closed > 0 {
            eprintln!("[INFO] [hardening] Closed {closed} inherited file descriptors");
        }
    }

    fn errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(-1)
    }
}

// ── macOS-specific hardening ────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    pub fn harden_macos() {
        disable_core_dumps();
        close_leaked_fds();
        // Note: macOS has PT_DENY_ATTACH but it kills the process if a debugger
        // is already attached. We skip it to avoid breaking development workflows.
        // The IPC nonce token provides the primary security boundary.
    }

    fn disable_core_dumps() {
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] RLIMIT_CORE=0 (core dumps disabled)");
        } else {
            eprintln!("[WARN] [hardening] RLIMIT_CORE failed");
        }
    }

    fn close_leaked_fds() {
        let mut closed = 0u32;
        for fd in 3..64 {
            let ret = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if ret >= 0 {
                unsafe { libc::close(fd); }
                closed += 1;
            }
        }
        if closed > 0 {
            eprintln!("[INFO] [hardening] Closed {closed} inherited file descriptors");
        }
    }
}

// ── FreeBSD-specific hardening ──────────────────────────────────────────

#[cfg(target_os = "freebsd")]
mod freebsd {
    pub fn harden_freebsd() {
        disable_core_dumps();
        disable_ptrace();
        close_leaked_fds();
    }

    fn disable_core_dumps() {
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
        if ret == 0 {
            eprintln!("[INFO] [hardening] RLIMIT_CORE=0 (core dumps disabled)");
        } else {
            eprintln!("[WARN] [hardening] RLIMIT_CORE failed");
        }
    }

    /// PROC_TRACE_CTL_DISABLE — FreeBSD equivalent of PR_SET_DUMPABLE.
    fn disable_ptrace() {
        // procctl(P_PID, 0, PROC_TRACE_CTL, &PROC_TRACE_CTL_DISABLE)
        const PROC_TRACE_CTL: i32 = 7;
        const PROC_TRACE_CTL_DISABLE: i32 = 1;
        let val: i32 = PROC_TRACE_CTL_DISABLE;
        let ret = unsafe {
            libc::procctl(
                libc::P_PID as u32,
                0, // current process
                PROC_TRACE_CTL,
                &val as *const i32 as *mut libc::c_void,
            )
        };
        if ret == 0 {
            eprintln!("[INFO] [hardening] PROC_TRACE_CTL disabled (ptrace protection active)");
        } else {
            eprintln!("[WARN] [hardening] PROC_TRACE_CTL failed");
        }
    }

    fn close_leaked_fds() {
        let mut closed = 0u32;
        for fd in 3..64 {
            let ret = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if ret >= 0 {
                unsafe { libc::close(fd); }
                closed += 1;
            }
        }
        if closed > 0 {
            eprintln!("[INFO] [hardening] Closed {closed} inherited file descriptors");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_env_removes_dangerous_vars() {
        // Set a dangerous var, sanitize, verify removed
        std::env::set_var("LD_PRELOAD", "/tmp/evil.so");
        sanitize_env();
        assert!(std::env::var_os("LD_PRELOAD").is_none());
    }

    #[test]
    fn sanitize_env_preserves_safe_vars() {
        std::env::set_var("HARMONIA_TEST_SAFE", "keep-me");
        sanitize_env();
        assert_eq!(
            std::env::var("HARMONIA_TEST_SAFE").unwrap(),
            "keep-me"
        );
        std::env::remove_var("HARMONIA_TEST_SAFE");
    }

    #[test]
    fn harden_does_not_panic() {
        // Hardening must never crash, even in restricted environments
        harden();
    }
}
