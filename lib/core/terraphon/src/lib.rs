/// Terraphon — platform-specific datamining tool substrate.
///
/// Stores HOW to extract data, not the data itself.
/// Pure functional. Actor-owned state. No singletons.

macro_rules! define_sexp_enum {
    ($name:ident, $default:ident { $($variant:ident => $kw:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($variant),* }
        impl $name {
            pub fn to_sexp(&self) -> &'static str {
                match self { $(Self::$variant => concat!(":", $kw)),* }
            }
            pub fn try_from_sexp(s: &str) -> Option<Self> {
                let s = s.strip_prefix(':').unwrap_or(s);
                match s { $($kw => Some(Self::$variant),)* _ => None, }
            }
            pub fn from_str(s: &str) -> Self {
                Self::try_from_sexp(s).unwrap_or(Self::$default)
            }
        }
    };
}
pub(crate) use define_sexp_enum;

mod catalog;
mod executor;
pub mod lode;
pub mod platform;
mod planner;
mod sexp;
pub mod tools;

pub use catalog::{LodeCatalog, catalog_list, lode_status, register_lode};
pub use lode::{Lode, LodeSummary};
pub use platform::Platform;
pub use tools::{MineCost, CpuCost, NetCost, Precondition, ToolKind};
pub use executor::{datamine_local, datamine_result_to_sexp};
pub use planner::{plan_query, QueryStrategy};

pub(crate) fn sexp_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn truncate_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

pub struct TerraphonState {
    pub(crate) catalog: LodeCatalog,
    pub(crate) platform: Platform,
}

impl TerraphonState {
    pub fn new() -> Self {
        let platform = Platform::detect();
        let mut catalog = LodeCatalog::new();
        tools::system::register_platform_tools(&mut catalog, platform);
        tools::system::register_universal_tools(&mut catalog);
        Self { catalog, platform }
    }
    pub fn lode_count(&self) -> usize { self.catalog.len() }
    pub fn platform(&self) -> Platform { self.platform }
}

pub fn init(s: &mut TerraphonState) -> Result<String, String> {
    let total = s.catalog.len();
    let available = s.catalog.check_all_preconditions();
    if let Ok(Some(json)) = harmonia_config_store::get_own("terraphon", "user-tools") {
        tools::userspace::load_user_tools(&mut s.catalog, &json);
    }
    Ok(format!(
        "(:ok :platform {} :total-lodes {} :available {} :user-tools {})",
        s.platform.to_sexp(), total, available, s.catalog.user_tool_count(),
    ))
}

pub fn health_check(s: &TerraphonState) -> Result<String, String> {
    Ok(format!(
        "(:ok :healthy t :platform {} :lodes {} :available {})",
        s.platform.to_sexp(), s.catalog.len(), s.catalog.available_count(),
    ))
}
