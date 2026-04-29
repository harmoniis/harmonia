use crate::sexp_escape;

pub fn datamine_local(s: &mut crate::TerraphonState, lode_id: &str, args: &[&str]) -> Result<String, String> {
    // Snapshot the lode before mutating state so the borrow ends before record_datamine.
    let lode_snapshot = s.catalog.find(lode_id).map(|lode| (lode.available, lode.mine_fn));
    let (available, mine_fn) = match lode_snapshot {
        Some(pair) => pair,
        None => {
            // Lookup miss is itself a sample so the rolling window reflects
            // misconfigured invocations the same as failed ones.
            s.record_datamine(false, 0);
            return Err(format!("(:error \"lode not found: {}\")", sexp_escape(lode_id)));
        }
    };
    if !available {
        s.record_datamine(false, 0);
        return Err(format!("(:error \"lode unavailable: {}\")", sexp_escape(lode_id)));
    }
    let start = std::time::Instant::now();
    let result = (mine_fn)(args);
    let elapsed_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(data) => {
            s.record_datamine(true, elapsed_ms);
            Ok(datamine_result_to_sexp(lode_id, &data, elapsed_ms))
        }
        Err(e) => {
            s.record_datamine(false, elapsed_ms);
            Err(format!(
                "(:error \"datamine failed: {} — {}\" :lode \"{}\" :elapsed-ms {})",
                sexp_escape(lode_id), sexp_escape(&e), sexp_escape(lode_id), elapsed_ms,
            ))
        }
    }
}

pub fn datamine_result_to_sexp(lode_id: &str, data: &str, elapsed_ms: u64) -> String {
    format!(
        "(:ok :lode \"{}\" :elapsed-ms {} :size {} :data \"{}\")",
        sexp_escape(lode_id), elapsed_ms, data.len(), sexp_escape(data),
    )
}
