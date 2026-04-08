pub mod system;
pub mod userspace;

crate::define_sexp_enum!(ToolKind, System {
    System => "system",
    UserSpace => "user-space",
});

crate::define_sexp_enum!(Domain, Generic {
    Music => "music",
    Math => "math",
    Engineering => "engineering",
    Cognitive => "cognitive",
    Life => "life",
    Filesystem => "filesystem",
    System => "system",
    Network => "network",
    Generic => "generic",
});

#[derive(Clone, Copy, Debug)]
pub struct MineCost {
    pub latency_ms: u32,
    pub cpu: CpuCost,
    pub network: NetCost,
}

crate::define_sexp_enum!(CpuCost, Low {
    Low => "low",
    Medium => "medium",
    High => "high",
});

crate::define_sexp_enum!(NetCost, None {
    None => "none",
    Local => "local",
    Remote => "remote",
});

#[derive(Clone, Debug)]
pub enum Precondition {
    BinaryExists(String),
    FileExists(String),
    EnvSet(String),
    Permission(String),
    ShortcutExists(String),
}

impl Precondition {
    pub fn is_met(&self) -> bool {
        match self {
            Self::BinaryExists(name) => which_exists(name),
            Self::FileExists(path) => std::path::Path::new(&expand_tilde(path)).exists(),
            Self::EnvSet(var) => std::env::var(var).is_ok(),
            Self::Permission(_) | Self::ShortcutExists(_) => true,
        }
    }
}

fn which_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(name).exists()))
        .unwrap_or(false)
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") { return format!("{}{}", home, &path[1..]); }
    }
    path.to_string()
}

macro_rules! declare_system_tool {
    (
        id: $id:expr, platform: $platform:expr, domain: $domain:expr,
        cost: ($lat:expr, $cpu:expr, $net:expr), preconditions: [$($pre:expr),* $(,)?],
        mine: |$args:ident| $body:expr
    ) => {
        crate::lode::Lode {
            id: $id.to_string(),
            kind: crate::tools::ToolKind::System,
            platform: $platform,
            domain: $domain,
            cost: crate::tools::MineCost { latency_ms: $lat, cpu: $cpu, network: $net },
            preconditions: vec![$($pre),*],
            available: true,
            mine_fn: |$args: &[&str]| -> Result<String, String> { $body },
        }
    };
}
pub(crate) use declare_system_tool;
