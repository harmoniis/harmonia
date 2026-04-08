//! `harmonia status` — daemon health, subsystems, modules, paths.

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    let pid_path = super::paths::pid_path()?;
    let node_service_pid_path = super::paths::node_service_pid_path()?;
    let sock_path = super::paths::socket_path()?;
    let log_path = super::paths::log_path()?;

    if !pid_path.exists() {
        if node_service_pid_path.exists() {
            println!("Harmonia daemon is not running.");
            if let Ok(pid_str) = std::fs::read_to_string(&node_service_pid_path) {
                let log = super::paths::node_service_log_path()?;
                println!("Node service is running (PID {})", pid_str.trim());
                println!("  log: {}", log.display());
            }
            return Ok(());
        }
        println!("Harmonia is not running.");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str.trim().parse().map_err(|_| "invalid PID file")?;

    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            println!("Harmonia is not running (stale PID file for {}).", pid);
            let _ = std::fs::remove_file(&pid_path);
            return Ok(());
        }

        println!();
        println!("  {} Harmonia is {} (PID {})", console::style("*").green().bold(), console::style("running").green().bold(), pid);
        println!();

        // Subsystems
        println!("  {}", console::style("Subsystems").cyan().bold());
        println!("  {}", console::style("----------------------------------------").dim());
        match query_phoenix_health() {
            Ok(json) => print_health(&json),
            Err(_) => println!("  health:           {}", console::style("unavailable").red()),
        }

        // Modules
        #[cfg(unix)]
        if let Ok(module_output) = super::status_modules::query_runtime_modules() {
            println!();
            println!("  {}", console::style("Modules").cyan().bold());
            println!("  {}", console::style("----------------------------------------").dim());
            print_modules(&module_output);
        }

        // Paths
        println!();
        println!("  {}", console::style("Paths").cyan().bold());
        println!("  {}", console::style("----------------------------------------").dim());
        if sock_path.exists() { println!("  socket:           {}", console::style(sock_path.display()).dim()); }
        println!("  log:              {}", console::style(log_path.display()).dim());
        if node_service_pid_path.exists() {
            if let Ok(ns_pid) = std::fs::read_to_string(&node_service_pid_path) { println!("  node-service:     PID {}", ns_pid.trim()); }
        }

        println!();
        println!("  {}       to open session", console::style("harmonia").cyan().bold());
        println!("  {}  to stop", console::style("harmonia stop").cyan().bold());
        println!("  {} to manage modules", console::style("harmonia modules").cyan().bold());
        println!();
    }

    Ok(())
}

#[cfg(unix)]
fn print_health(json_str: &str) {
    if let Ok(health) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(uptime) = health.get("uptime_secs").and_then(|v| v.as_u64()) {
            println!("  uptime:           {}h {}m {}s", uptime / 3600, (uptime % 3600) / 60, uptime % 60);
        }
        if let Some(mode) = health.pointer("/mode/mode").and_then(|v| v.as_str()) {
            let styled = match mode {
                "full" => console::style(mode).green().to_string(),
                "starting" => console::style(mode).yellow().to_string(),
                _ => mode.to_string(),
            };
            println!("  mode:             {}", styled);
        }
        if let Some(subs) = health.get("subsystems").and_then(|v| v.as_object()) {
            for (name, info) in subs {
                let status = info.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                let styled = match status {
                    "running" => console::style(status).green().to_string(),
                    "backoff" | "starting" => console::style(status).yellow().to_string(),
                    "stopped" | "crashed" => console::style(status).red().to_string(),
                    _ => status.to_string(),
                };
                let mut detail = String::new();
                if let Some(attempt) = info.get("attempt").and_then(|v| v.as_u64()) { detail = format!(" (attempt {}/10)", attempt); }
                println!("  {:<18} {}{}", name, styled, detail);
            }
        }
    } else {
        println!("  health:           {}", json_str);
    }
}

#[cfg(unix)]
fn print_modules(module_output: &[(String, String, String)]) {
    let mut loaded = Vec::new();
    let mut errors = Vec::new();
    for m in module_output {
        match m.1.as_str() {
            "loaded" => loaded.push(&m.0),
            "unloaded" => {}
            _ => errors.push(m),
        }
    }
    if !loaded.is_empty() {
        let names: Vec<&str> = loaded.iter().map(|s| s.as_str()).collect();
        println!("  {} loaded:      {}", console::style(loaded.len()).green().bold(), console::style(names.join(", ")).dim());
    }
    if !errors.is_empty() {
        println!("  {} unconfigured:", console::style(errors.len()).yellow().bold());
        for (name, _, needs) in &errors {
            if !needs.is_empty() {
                let clean = needs.replace("\\\"", "\"").replace("\\\\", "\\");
                println!("    {:<18} needs: {}", name, console::style(&clean).dim());
            } else {
                println!("    {}", name);
            }
        }
    }
}

fn query_phoenix_health() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("http://127.0.0.1:{}/health", 9100u16);
    let resp = ureq::get(&url).timeout(std::time::Duration::from_secs(3)).call()?;
    Ok(resp.into_string()?)
}
