use harmonia_ouroboros::OuroborosState;

fn test_state() -> OuroborosState {
    let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("ouroboros-test-{}", id));
    let _ = std::fs::create_dir_all(&dir);
    OuroborosState::with_paths(
        dir.join("recovery.log").to_string_lossy().into(),
        dir.join("patches").to_string_lossy().into(),
    )
}

#[test]
fn test_healthcheck() {
    let mut state = test_state();
    let result = harmonia_ouroboros::dispatch(&mut state, "(:op \"healthcheck\")");
    assert!(result.contains(":ok"));
    assert!(result.contains("repair-engine"));
}

#[test]
fn test_record_and_last_crash() {
    let mut state = test_state();
    let record = harmonia_ouroboros::dispatch(&mut state,
        "(:op \"record-crash\" :component-name \"test-component\" :detail \"test failure\")");
    assert!(record.contains(":ok"), "record failed: {}", record);
    let last = harmonia_ouroboros::dispatch(&mut state, "(:op \"last-crash\")");
    assert!(last.contains(":ok"), "last-crash failed: {}", last);
    assert!(last.contains("test-component") || last.contains("test failure"),
        "last-crash missing content: {}", last);
}

#[test]
fn test_history() {
    let mut state = test_state();
    harmonia_ouroboros::dispatch(&mut state,
        "(:op \"record-crash\" :component-name \"a\" :detail \"first\")");
    harmonia_ouroboros::dispatch(&mut state,
        "(:op \"record-crash\" :component-name \"b\" :detail \"second\")");
    let history = harmonia_ouroboros::dispatch(&mut state, "(:op \"history\" :limit 10)");
    assert!(history.contains(":ok"), "history failed: {}", history);
}

#[test]
fn test_write_patch() {
    let mut state = test_state();
    let result = harmonia_ouroboros::dispatch(&mut state,
        "(:op \"write-patch\" :component-name \"test\" :patch-body \"--- a/file\\n+++ b/file\")");
    assert!(result.contains(":ok") || result.contains(":path"), "write-patch failed: {}", result);
}

#[test]
fn test_write_patch_empty_body_rejected() {
    let mut state = test_state();
    let result = harmonia_ouroboros::dispatch(&mut state,
        "(:op \"write-patch\" :component-name \"test\" :patch-body \"\")");
    assert!(result.contains(":error"));
}

#[test]
fn test_unknown_op() {
    let mut state = test_state();
    let result = harmonia_ouroboros::dispatch(&mut state, "(:op \"nonexistent\")");
    assert!(result.contains(":error"));
}
