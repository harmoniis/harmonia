// ── Theme constants for the Harmonia TUI ────────────────────────────

pub(crate) const VERSION: &str = "0.2.0";
pub(crate) const MAX_INPUT_LINES: usize = 10;

pub(crate) const LOGO: &str = r#"
  _   _                                  _
 | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _
 | |_| |/ _` | '__| '_ ` _ \ / _ \| '_ \| |/ _` |
 |  _  | (_| | |  | | | | | | (_) | | | | | (_| |
 |_| |_|\__,_|_|  |_| |_| |_|\___/|_| |_|_|\__,_|
"#;

// Harmonia gradient: violet -> dark blue -> cyan -> cyan-green
pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const BOLD_CYAN: &str = "\x1b[1;36m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const BOLD_GREEN: &str = "\x1b[1;32m";
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const BOLD_WHITE: &str = "\x1b[1;37m";
