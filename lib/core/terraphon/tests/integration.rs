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
    assert!(datamine_local(&TerraphonState::new(), "nonexistent", &[]).is_err());
}
#[test]
fn test_git_log_lode_exists() {
    let s = TerraphonState::new();
    assert!(lode_status(&s, "git-log").unwrap().contains("git-log"));
}
