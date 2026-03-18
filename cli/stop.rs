use console::style;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let pid_path = crate::paths::pid_path()?;
    stop_broker_if_running()?;
    let node_service_stopped = stop_node_service_if_running()?;

    if !pid_path.exists() {
        if node_service_stopped {
            eprintln!("{} Node service stopped.", style("✓").green().bold());
        } else {
            eprintln!(
                "{} Harmonia is not running (no PID file).",
                style("!").yellow().bold()
            );
        }
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str.trim().parse().map_err(|_| "invalid PID file")?;

    // Check if process is alive
    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            eprintln!(
                "{} Stale PID file (process {} not running). Cleaning up.",
                style("!").yellow().bold(),
                pid
            );
            let _ = std::fs::remove_file(&pid_path);
            if let Ok(sock) = crate::paths::socket_path() {
                let _ = std::fs::remove_file(&sock);
            }
            return Ok(());
        }

        // Send SIGTERM
        eprintln!(
            "{} Stopping Harmonia (PID {})...",
            style("→").cyan().bold(),
            pid
        );
        let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
        if rc != 0 {
            return Err(format!("failed to send SIGTERM to PID {}", pid).into());
        }

        // Wait up to 10 seconds for shutdown
        for i in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let still_alive = unsafe { libc::kill(pid, 0) } == 0;
            if !still_alive {
                let _ = std::fs::remove_file(&pid_path);
                if let Ok(sock) = crate::paths::socket_path() {
                    let _ = std::fs::remove_file(&sock);
                }
                eprintln!("{} Harmonia stopped.", style("✓").green().bold());
                return Ok(());
            }
            if i == 10 {
                eprintln!("  waiting for graceful shutdown...");
            }
        }

        // Force kill
        eprintln!(
            "{} Graceful shutdown timed out, sending SIGKILL...",
            style("!").yellow().bold()
        );
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = std::fs::remove_file(&pid_path);
        if let Ok(sock) = crate::paths::socket_path() {
            let _ = std::fs::remove_file(&sock);
        }
        eprintln!("{} Harmonia killed.", style("✓").green().bold());
    }

    Ok(())
}

fn stop_node_service_if_running() -> Result<bool, Box<dyn std::error::Error>> {
    let pid_path = crate::paths::node_service_pid_path()?;
    if !pid_path.exists() {
        return Ok(false);
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|_| "invalid node-service PID file")?;

    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if alive {
            let _ = unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if unsafe { libc::kill(pid, 0) } != 0 {
                    break;
                }
            }
            if unsafe { libc::kill(pid, 0) } == 0 {
                let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
    }

    let _ = std::fs::remove_file(&pid_path);
    if let Ok(sock) = crate::paths::socket_path() {
        let _ = std::fs::remove_file(&sock);
    }
    Ok(true)
}

fn stop_broker_if_running() -> Result<(), Box<dyn std::error::Error>> {
    let pid_path = crate::paths::broker_pid_path()?;
    if !pid_path.exists() {
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|_| "invalid broker PID file")?;

    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if alive {
            let _ = unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if unsafe { libc::kill(pid, 0) } != 0 {
                    break;
                }
            }
            if unsafe { libc::kill(pid, 0) } == 0 {
                let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
    }

    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}
