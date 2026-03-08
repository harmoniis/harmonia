mod ops;
mod reports;
mod shared;
mod store;

pub(crate) use ops::{
    log_event, observe_route, register_edge, register_node, route_allowed,
    route_allowed_with_context, set_tool_enabled,
};
pub(crate) use reports::{report, route_timeseries, time_report};
pub(crate) use shared::{clear_last_error, last_error_message, set_last_error};
pub(crate) use store::{init, set_store, store_summary};

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::{
        init, log_event, observe_route, register_edge, register_node, report, set_store,
        set_tool_enabled, store_summary,
    };

    fn test_guard() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("matrix test lock poisoned")
    }

    #[test]
    fn sqlite_roundtrip_persists_usage() {
        let _guard = test_guard();
        let db = std::env::temp_dir().join("harmonia-hmatrix-modtest.db");
        let _ = std::fs::remove_file(&db);
        let dbs = db.to_string_lossy().to_string();

        set_store("sqlite", Some(&dbs)).expect("set store sqlite");
        init().expect("init");
        register_node("a", "core").expect("node a");
        register_node("b", "tool").expect("node b");
        register_edge("a", "b", 1.0, 0.1).expect("edge");
        set_tool_enabled("b", true).expect("tool");
        observe_route("a", "b", true, 10, 0.01).expect("observe");
        log_event("a", "output", "test", "payload", true, "").expect("event");

        let r1 = report().expect("report1");
        assert!(r1.contains(":uses 1"));

        init().expect("re-init");
        let r2 = report().expect("report2");
        assert!(r2.contains(":uses 1"));
        assert!(r2.contains(":store-kind \"sqlite\""));

        set_store("memory", None).expect("back to memory");
    }

    #[test]
    fn graph_store_contract_returns_error() {
        let _guard = test_guard();
        set_store("memory", None).expect("start on memory");
        let err = set_store("graph", Some("bolt://127.0.0.1:7687"))
            .expect_err("graph should not be active yet");
        assert!(err.contains("contract exists"));
        let summary = store_summary().expect("summary after graph failure");
        assert!(summary.contains(":store-kind \"memory\""));
    }
}
