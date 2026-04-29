use harmonia_terraphon::*;

#[test]
fn test_platform_detection() {
    assert_ne!(Platform::detect().to_sexp(), "");
}
#[test]
fn test_state_creation() {
    let s = TerraphonState::new();
    assert!(s.lode_count() > 0);
    assert_ne!(s.platform(), Platform::Any);
}
#[test]
fn test_catalog_list() {
    let s = TerraphonState::new();
    assert!(catalog_list(&s).unwrap().contains(":count"));
}
#[test]
fn test_lode_status_unknown() {
    assert!(lode_status(&TerraphonState::new(), "nonexistent-tool").is_err());
}
#[test]
fn test_plan_query() {
    let s = TerraphonState::new();
    assert!(plan_query(&s, "engineering", "test", QueryStrategy::Cascade).unwrap().contains(":strategy"));
}
#[test]
fn test_health_check() {
    assert!(health_check(&TerraphonState::new()).unwrap().contains(":healthy t"));
}
#[test]
fn test_datamine_nonexistent() {
    let mut s = TerraphonState::new();
    assert!(datamine_local(&mut s, "nonexistent", &[]).is_err());
    // Stats record the failed lookup so the rolling window stays accurate.
    assert_eq!(s.stats().samples(), 1);
    assert_eq!(s.stats().success_rate(), 0.0);
}

#[test]
fn test_stats_rolling_window() {
    let mut s = TerraphonState::new();
    for _ in 0..5 {
        let _ = datamine_local(&mut s, "nonexistent", &[]);
    }
    assert_eq!(s.stats().samples(), 5);
    assert_eq!(s.stats().success_rate(), 0.0);
    let stats_sexp = harmonia_terraphon::stats(&s).unwrap();
    assert!(stats_sexp.contains(":samples 5"));
}
#[test]
fn test_git_log_lode_exists() {
    let s = TerraphonState::new();
    assert!(lode_status(&s, "git-log").unwrap().contains("git-log"));
}
