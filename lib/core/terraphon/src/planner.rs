use crate::sexp_escape;
use crate::tools::Domain;

crate::define_sexp_enum!(QueryStrategy, Cascade {
    FanOut => "fan-out",
    Cascade => "cascade",
    Nearest => "nearest",
});

pub fn plan_query(
    s: &crate::TerraphonState, domain: &str, _query: &str, strategy: QueryStrategy,
) -> Result<String, String> {
    let domain_enum = Domain::from_str(domain);
    let mut matching: Vec<_> = s.catalog.list(Some(domain_enum))
        .into_iter().filter(|l| l.available).collect();
    if matching.is_empty() {
        return Ok(format!(
            "(:ok :strategy {} :domain {} :local-lodes () :needs-remote t)",
            strategy.to_sexp(), domain_enum.to_sexp(),
        ));
    }
    matching.sort_by_key(|l| l.cost.latency_ms);
    let lode_items: Vec<String> = matching.iter()
        .map(|l| format!("\"{}\"", sexp_escape(&l.id))).collect();
    Ok(format!(
        "(:ok :strategy {} :domain {} :local-lodes ({}) :needs-remote nil)",
        strategy.to_sexp(), domain_enum.to_sexp(), lode_items.join(" "),
    ))
}
