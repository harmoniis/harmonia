pub const PING: &str = "node.ping";
pub const CAPABILITIES: &str = "node.capabilities";
pub const FS_LIST: &str = "fs.list";
pub const FS_READ_TEXT: &str = "fs.read-text";
pub const SHELL_EXEC: &str = "shell.exec";
pub const TMUX_LIST: &str = "tmux.list";
pub const TMUX_SPAWN: &str = "tmux.spawn";
pub const TMUX_CAPTURE: &str = "tmux.capture";
pub const TMUX_SEND_LINE: &str = "tmux.send-line";
pub const TMUX_SEND_KEY: &str = "tmux.send-key";
pub const WALLET_STATUS: &str = "wallet.status";
pub const WALLET_LIST_SYMBOLS: &str = "wallet.list-symbols";
pub const WALLET_HAS_SYMBOL: &str = "wallet.has-symbol";
pub const WALLET_SET_SECRET: &str = "wallet.set-secret";
pub const FRONTEND_PAIR_LIST: &str = "frontend.pair-list";
pub const FRONTEND_CONFIGURE: &str = "frontend.configure";
pub const FRONTEND_PAIR_INIT: &str = "frontend.pair-init";
pub const FRONTEND_PAIR_STATUS: &str = "frontend.pair-status";
pub const DATAMINE_QUERY: &str = "datamine.query";
pub const DATAMINE_CATALOG: &str = "datamine.catalog";
pub const DATAMINE_PROBE: &str = "datamine.probe";

pub fn default_capabilities() -> Vec<String> {
    vec![
        PING.to_string(),
        CAPABILITIES.to_string(),
        FS_LIST.to_string(),
        FS_READ_TEXT.to_string(),
        SHELL_EXEC.to_string(),
        TMUX_LIST.to_string(),
        TMUX_SPAWN.to_string(),
        TMUX_CAPTURE.to_string(),
        TMUX_SEND_LINE.to_string(),
        TMUX_SEND_KEY.to_string(),
        WALLET_STATUS.to_string(),
        WALLET_LIST_SYMBOLS.to_string(),
        WALLET_HAS_SYMBOL.to_string(),
        WALLET_SET_SECRET.to_string(),
        FRONTEND_PAIR_LIST.to_string(),
        FRONTEND_CONFIGURE.to_string(),
        FRONTEND_PAIR_INIT.to_string(),
        FRONTEND_PAIR_STATUS.to_string(),
        DATAMINE_QUERY.to_string(),
        DATAMINE_CATALOG.to_string(),
        DATAMINE_PROBE.to_string(),
    ]
}
