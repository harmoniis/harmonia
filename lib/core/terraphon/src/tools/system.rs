use crate::catalog::LodeCatalog;
use crate::platform::Platform;
use crate::tools::{CpuCost, NetCost, Precondition, Domain};
use crate::tools::declare_system_tool;

pub fn register_platform_tools(catalog: &mut LodeCatalog, platform: Platform) {
    let tools = match platform {
        Platform::MacOS => macos_tools(),
        Platform::Linux | Platform::Cloud => linux_tools(),
        Platform::FreeBSD => freebsd_tools(),
        Platform::IOS => ios_tools(),
        Platform::Android => android_tools(),
        Platform::Any => Vec::new(),
    };
    for tool in tools { catalog.register(tool); }
    if platform == Platform::Cloud {
        for tool in cloud_tools() { catalog.register(tool); }
    }
}

pub fn register_universal_tools(catalog: &mut LodeCatalog) {
    for tool in universal_tools() { catalog.register(tool); }
}

pub(crate) fn exec_capture(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("exec failed: {}: {}", cmd, e))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let max_chars = harmonia_config_store::get_own("terraphon", "max-result-chars")
            .ok().flatten()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(2000);
        if stdout.len() > max_chars {
            Ok(format!("{}...(truncated)", crate::truncate_safe(&stdout, max_chars)))
        } else {
            Ok(stdout.into_owned())
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} failed: {}", cmd, stderr.trim()))
    }
}

fn macos_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "spotlight", platform: Platform::MacOS, domain: Domain::Filesystem,
            cost: (300, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("mdfind".into())],
            mine: |args| exec_capture("mdfind", &[args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "file-meta", platform: Platform::MacOS, domain: Domain::Filesystem,
            cost: (100, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("mdls".into())],
            mine: |args| exec_capture("mdls", &[args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "calendar", platform: Platform::MacOS, domain: Domain::Life,
            cost: (500, CpuCost::Medium, NetCost::None), preconditions: [Precondition::BinaryExists("icalBuddy".into())],
            mine: |args| exec_capture("icalBuddy", &["-f", "-nc", "-n", args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "clipboard", platform: Platform::MacOS, domain: Domain::Generic,
            cost: (10, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("pbpaste".into())],
            mine: |_args| exec_capture("pbpaste", &[])),
        declare_system_tool!(id: "process-list", platform: Platform::MacOS, domain: Domain::System,
            cost: (100, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("ps".into())],
            mine: |_args| exec_capture("ps", &["aux"])),
        declare_system_tool!(id: "network-config", platform: Platform::MacOS, domain: Domain::Network,
            cost: (150, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("networksetup".into())],
            mine: |_args| exec_capture("networksetup", &["-listallhardwareports"])),
    ]
}

fn linux_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "locate", platform: Platform::Linux, domain: Domain::Filesystem,
            cost: (100, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("locate".into())],
            mine: |args| exec_capture("locate", &["-i", "-l", "20", args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "journalctl", platform: Platform::Linux, domain: Domain::System,
            cost: (300, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("journalctl".into())],
            mine: |args| exec_capture("journalctl", &["-u", args.first().unwrap_or(&""), "--no-pager", "-n", "50"])),
        declare_system_tool!(id: "systemctl-status", platform: Platform::Linux, domain: Domain::System,
            cost: (100, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("systemctl".into())],
            mine: |args| exec_capture("systemctl", &["status", args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "docker-ps", platform: Platform::Linux, domain: Domain::Engineering,
            cost: (200, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("docker".into())],
            mine: |args| exec_capture("docker", &["ps", "--filter", &format!("name={}", args.first().unwrap_or(&""))])),
        declare_system_tool!(id: "ss-ports", platform: Platform::Linux, domain: Domain::Network,
            cost: (50, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("ss".into())],
            mine: |_args| exec_capture("ss", &["-tlnp"])),
    ]
}

fn freebsd_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "pkg-info", platform: Platform::FreeBSD, domain: Domain::System,
            cost: (100, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("pkg".into())],
            mine: |args| exec_capture("pkg", &["info", args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "jail-list", platform: Platform::FreeBSD, domain: Domain::Engineering,
            cost: (200, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("jls".into())],
            mine: |_args| exec_capture("jls", &[])),
        declare_system_tool!(id: "zfs-list", platform: Platform::FreeBSD, domain: Domain::Filesystem,
            cost: (150, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("zfs".into())],
            mine: |args| exec_capture("zfs", &["list", "-r", args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "bhyve-list", platform: Platform::FreeBSD, domain: Domain::Engineering,
            cost: (200, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("bhyvectl".into())],
            mine: |_args| exec_capture("bhyvectl", &["--get-stats"])),
    ]
}

fn cloud_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "gh-issues", platform: Platform::Cloud, domain: Domain::Engineering,
            cost: (1500, CpuCost::Low, NetCost::Remote), preconditions: [Precondition::BinaryExists("gh".into())],
            mine: |args| exec_capture("gh", &["issue", "list", "-R", args.first().unwrap_or(&""), "--search", args.get(1).unwrap_or(&""), "--limit", "10"])),
        declare_system_tool!(id: "gh-prs", platform: Platform::Cloud, domain: Domain::Engineering,
            cost: (1500, CpuCost::Low, NetCost::Remote), preconditions: [Precondition::BinaryExists("gh".into())],
            mine: |args| exec_capture("gh", &["pr", "list", "-R", args.first().unwrap_or(&""), "--state", args.get(1).unwrap_or(&"open"), "--limit", "10"])),
        declare_system_tool!(id: "nomad-status", platform: Platform::Cloud, domain: Domain::Engineering,
            cost: (800, CpuCost::Low, NetCost::Local), preconditions: [Precondition::BinaryExists("nomad".into())],
            mine: |args| exec_capture("nomad", &["job", "status", args.first().unwrap_or(&"")])),
        declare_system_tool!(id: "consul-services", platform: Platform::Cloud, domain: Domain::Engineering,
            cost: (600, CpuCost::Low, NetCost::Local), preconditions: [Precondition::BinaryExists("consul".into())],
            mine: |_args| exec_capture("consul", &["catalog", "services"])),
    ]
}

fn ios_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "ios-shortcuts", platform: Platform::IOS, domain: Domain::Generic,
            cost: (1000, CpuCost::Medium, NetCost::None), preconditions: [Precondition::ShortcutExists("Harmonia Mine".into())],
            mine: |_args| Err("ios shortcuts not yet bridged".into())),
    ]
}

fn android_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "android-pm-list", platform: Platform::Android, domain: Domain::System,
            cost: (300, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("pm".into())],
            mine: |args| exec_capture("pm", &["list", "packages", "-f", args.first().unwrap_or(&"")])),
    ]
}

fn universal_tools() -> Vec<crate::lode::Lode> {
    vec![
        declare_system_tool!(id: "git-log", platform: Platform::Any, domain: Domain::Engineering,
            cost: (200, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("git".into())],
            mine: |args| exec_capture("git", &["-C", args.first().unwrap_or(&"."), "log", "--oneline", "-20"])),
        declare_system_tool!(id: "git-grep", platform: Platform::Any, domain: Domain::Engineering,
            cost: (300, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("git".into())],
            mine: |args| exec_capture("git", &["-C", args.first().unwrap_or(&"."), "grep", "-n", args.get(1).unwrap_or(&"")])),
        declare_system_tool!(id: "git-diff", platform: Platform::Any, domain: Domain::Engineering,
            cost: (300, CpuCost::Low, NetCost::None), preconditions: [Precondition::BinaryExists("git".into())],
            mine: |args| exec_capture("git", &["-C", args.first().unwrap_or(&"."), "diff", "--stat", args.get(1).unwrap_or(&"HEAD~1")])),
    ]
}
