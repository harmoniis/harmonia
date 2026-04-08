use crate::sexp_escape;

pub fn datamine_local(s: &crate::TerraphonState, lode_id: &str, args: &[&str]) -> Result<String, String> {
    let lode = s.catalog.find(lode_id)
        .ok_or_else(|| format!("(:error \"lode not found: {}\")", sexp_escape(lode_id)))?;
    if !lode.available {
        return Err(format!("(:error \"lode unavailable: {}\")", sexp_escape(lode_id)));
    }
    let start = std::time::Instant::now();
    let result = (lode.mine_fn)(args);
    let elapsed_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(data) => Ok(datamine_result_to_sexp(lode_id, &data, elapsed_ms)),
        Err(e) => Err(format!(
            "(:error \"datamine failed: {} — {}\" :lode \"{}\" :elapsed-ms {})",
            sexp_escape(lode_id), sexp_escape(&e), sexp_escape(lode_id), elapsed_ms,
        )),
    }
}

pub fn datamine_result_to_sexp(lode_id: &str, data: &str, elapsed_ms: u64) -> String {
    format!(
        "(:ok :lode \"{}\" :elapsed-ms {} :size {} :data \"{}\")",
        sexp_escape(lode_id), elapsed_ms, data.len(), sexp_escape(data),
    )
}
