// Legacy FFI modules — suppress deprecation warnings for singleton access
// until the runtime actor migration is complete.
#[allow(deprecated)]
pub mod ops;
#[allow(deprecated)]
pub mod reports;
pub mod shared;
#[allow(deprecated)]
pub mod store;

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::ops::{advance_epoch, log_event, observe_route, register_edge, register_node, set_tool_enabled};
    use super::reports::report;
    use super::shared::state;
    use super::store::{init, set_store, store_summary};

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
    fn advance_epoch_increments_and_ages_history() {
        let _guard = test_guard();
        set_store("memory", None).expect("memory store");
        init().expect("init");
        register_node("a2", "core").expect("node a2");
        register_node("b2", "tool").expect("node b2");
        register_edge("a2", "b2", 1.0, 0.1).expect("edge");
        set_tool_enabled("b2", true).expect("tool");
        // Seed enough samples that history capping is exercised.
        for i in 0..2000 {
            observe_route("a2", "b2", i % 2 == 0, 10 + i as u64, 0.001).expect("observe");
        }
        let starting_epoch = state().read().unwrap().epoch;
        let summary = advance_epoch().expect("advance epoch");
        assert_eq!(summary.epoch, starting_epoch + 1);
        // 1/4 of the default history_limit (4096) = 1024; we recorded 2000, so
        // post-advance retention should be at most 1024 per edge.
        assert!(summary.samples_retained <= 1024,
            "expected ≤1024 samples after advance, got {}", summary.samples_retained);
        assert!(summary.edges_with_history >= 1);
        let sexp = summary.to_sexp();
        assert!(sexp.contains(":epoch"));
        assert!(sexp.contains(":samples-retained"));
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
