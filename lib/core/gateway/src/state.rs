use crate::registry::Registry;
use parking_lot::RwLock;
use std::sync::OnceLock;

static GATEWAY: OnceLock<GatewayState> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

pub struct GatewayState {
    pub registry: Registry,
    pub initialized: RwLock<bool>,
}

pub fn gateway() -> &'static GatewayState {
    GATEWAY.get_or_init(|| GatewayState {
        registry: Registry::new(),
        initialized: RwLock::new(false),
    })
}

pub fn init() -> Result<(), String> {
    let gw = gateway();
    let mut init = gw.initialized.write();
    if *init {
        return Ok(());
    }
    *init = true;
    Ok(())
}

pub fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_error(msg: impl Into<String>) {
    *last_error().write() = msg.into();
}

pub fn clear_error() {
    last_error().write().clear();
}
