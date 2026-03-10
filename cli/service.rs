use console::style;
use std::path::PathBuf;

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let harmonia_bin = resolve_bin_path()?;

    #[cfg(target_os = "macos")]
    install_launchd(&home, &harmonia_bin)?;

    #[cfg(target_os = "linux")]
    install_systemd(&home, &harmonia_bin)?;

    #[cfg(target_os = "freebsd")]
    install_freebsd(&home, &harmonia_bin)?;

    #[cfg(target_os = "windows")]
    install_windows(&home, &harmonia_bin)?;

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    uninstall_launchd()?;

    #[cfg(target_os = "linux")]
    uninstall_systemd()?;

    #[cfg(target_os = "freebsd")]
    {
        eprintln!("Remove /usr/local/etc/rc.d/harmonia and set harmonia_enable=NO in /etc/rc.conf");
    }

    #[cfg(target_os = "windows")]
    uninstall_windows()?;

    Ok(())
}

fn resolve_bin_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    // Resolve symlinks
    let canonical = std::fs::canonicalize(&exe).unwrap_or(exe);
    Ok(canonical)
}

#[cfg(target_os = "macos")]
fn install_launchd(
    home: &std::path::Path,
    harmonia_bin: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let plist_dir = home.join("Library").join("LaunchAgents");
    std::fs::create_dir_all(&plist_dir)?;
    let plist_path = plist_dir.join("com.harmoniis.harmonia.plist");

    let template = include_str!("../service/com.harmoniis.harmonia.plist");
    let home_str = home.to_string_lossy();
    let content = template
        .replace("__HARMONIA_BIN__", &harmonia_bin.to_string_lossy())
        .replace("__HOME__", &home_str);

    std::fs::write(&plist_path, &content)?;

    println!(
        "{} Installed launchd service at {}",
        style("✓").green().bold(),
        plist_path.display()
    );

    // Load the service
    let status = std::process::Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist_path)
        .status()?;

    if status.success() {
        println!(
            "{} Service loaded. Harmonia will start automatically on login.",
            style("✓").green().bold()
        );
    } else {
        eprintln!(
            "{} Failed to load service. Try manually: launchctl load -w {}",
            style("!").yellow().bold(),
            plist_path.display()
        );
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall_launchd() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join("com.harmoniis.harmonia.plist");

    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", "-w"])
            .arg(&plist_path)
            .status();
        std::fs::remove_file(&plist_path)?;
        println!("{} Removed launchd service.", style("✓").green().bold());
    } else {
        println!("No launchd service found.");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_systemd(
    home: &std::path::Path,
    harmonia_bin: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let user_service_dir = home.join(".config").join("systemd").join("user");
    std::fs::create_dir_all(&user_service_dir)?;
    let service_path = user_service_dir.join("harmonia.service");

    let template = include_str!("../service/harmonia.service");
    let home_str = home.to_string_lossy();
    let content = template
        .replace("__HARMONIA_BIN__", &harmonia_bin.to_string_lossy())
        .replace("__HOME__", &home_str);

    std::fs::write(&service_path, &content)?;

    println!(
        "{} Installed systemd user service at {}",
        style("✓").green().bold(),
        service_path.display()
    );

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    let status = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "harmonia"])
        .status()?;

    if status.success() {
        println!(
            "{} Service enabled. Harmonia will start automatically on login.",
            style("✓").green().bold()
        );
    } else {
        eprintln!(
            "{} Failed to enable service. Try: systemctl --user enable --now harmonia",
            style("!").yellow().bold(),
        );
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall_systemd() -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "--now", "harmonia"])
        .status();

    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let service_path = home
        .join(".config")
        .join("systemd")
        .join("user")
        .join("harmonia.service");

    if service_path.exists() {
        std::fs::remove_file(&service_path)?;
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        println!("{} Removed systemd service.", style("✓").green().bold());
    } else {
        println!("No systemd service found.");
    }
    Ok(())
}

#[cfg(target_os = "freebsd")]
fn install_freebsd(
    home: &std::path::Path,
    harmonia_bin: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let template = include_str!("../service/harmonia-freebsd.sh");
    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
    let home_str = home.to_string_lossy();
    let content = template
        .replace("__HARMONIA_BIN__", &harmonia_bin.to_string_lossy())
        .replace("__HOME__", &home_str)
        .replace("__USER__", &user);

    let rc_path = std::path::Path::new("/usr/local/etc/rc.d/harmonia");
    println!(
        "{} To install the FreeBSD rc.d service, run:",
        style("→").cyan().bold()
    );
    println!("  sudo install -m 755 /dev/stdin {}", rc_path.display());
    println!("  sudo sysrc harmonia_enable=YES");
    println!("  sudo service harmonia start");
    println!();
    println!("Service script content saved to: service/harmonia-freebsd.sh");
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows(
    home: &std::path::Path,
    harmonia_bin: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bin_str = harmonia_bin.to_string_lossy();
    let state_root = home.join(".harmoniis").join("harmonia");
    let log_path = state_root.join("harmonia.log");

    // Create a scheduled task that runs at logon
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Harmonia — self-improving agent daemon</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions>
    <Exec>
      <Command>{}</Command>
      <Arguments>start --foreground</Arguments>
      <WorkingDirectory>{}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>"#,
        bin_str,
        state_root.to_string_lossy()
    );

    let xml_path = state_root.join("harmonia-task.xml");
    std::fs::write(&xml_path, &xml)?;

    let status = std::process::Command::new("schtasks")
        .args([
            "/Create",
            "/TN",
            "Harmonia",
            "/XML",
            &xml_path.to_string_lossy(),
            "/F",
        ])
        .status()?;

    let _ = std::fs::remove_file(&xml_path);

    if status.success() {
        println!(
            "{} Installed Windows scheduled task 'Harmonia'.",
            style("✓").green().bold()
        );
        println!("  Harmonia will start automatically on logon.");
        println!("  log: {}", log_path.display());
    } else {
        eprintln!(
            "{} Failed to create scheduled task. Try running as Administrator.",
            style("!").yellow().bold()
        );
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn uninstall_windows() -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", "Harmonia", "/F"])
        .status()?;

    if status.success() {
        println!(
            "{} Removed Windows scheduled task 'Harmonia'.",
            style("✓").green().bold()
        );
    } else {
        println!("No scheduled task 'Harmonia' found.");
    }
    Ok(())
}
