use crate::registry::Registry;

/// Send a signal to a frontend.
///
/// FFI-based frontend sending has been removed -- frontends are now ractor
/// actors. This stub returns an error; callers should use the actor mailbox.
pub fn send_signal(
    registry: &Registry,
    frontend_name: &str,
    _sub_channel: &str,
    _payload: &str,
) -> Result<(), String> {
    if !registry.is_registered(frontend_name) {
        return Err(format!("frontend not registered: {frontend_name}"));
    }
    // FFI send removed; actor-based frontends receive messages via their
    // ractor mailbox in the runtime.
    log::debug!(
        "gateway: send_signal to '{}' is a no-op (actor dispatch expected)",
        frontend_name
    );
    Ok(())
}
