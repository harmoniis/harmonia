use std::collections::HashMap;

use crate::model::{state, ModelPrice};

fn estimate_tokens(s: &str) -> f64 {
    (s.chars().count() as f64 / 4.0).max(1.0)
}

pub(super) fn estimate_cost(model: &str, prompt: &str, response: &str) -> f64 {
    let st = match state().read() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };
    let p: ModelPrice = match st.prices.get(model) {
        Some(v) => *v,
        None => return 0.0,
    };
    let in_k = estimate_tokens(prompt) / 1000.0;
    let out_k = estimate_tokens(response) / 1000.0;
    p.usd_per_1k_input * in_k + p.usd_per_1k_output * out_k
}

pub(super) fn render_report(content: &str) -> String {
    let mut total = 0u64;
    let mut success = 0u64;
    let mut verified = 0u64;
    let mut total_cost = 0.0f64;
    let mut total_latency = 0u64;
    let mut by_model: HashMap<String, (u64, u64, u64, f64, u64)> = HashMap::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 6 {
            continue;
        }
        let model = parts[2].to_string();
        let latency = parts[3].parse::<u64>().unwrap_or(0);
        let cost = parts[4].parse::<f64>().unwrap_or(0.0);
        let ok = parts[5].trim() == "1";
        let ver = parts.get(6).map(|v| v.trim() == "1").unwrap_or(false);

        total += 1;
        if ok {
            success += 1;
        }
        if ver {
            verified += 1;
        }
        total_cost += cost;
        total_latency += latency;

        let e = by_model.entry(model).or_insert((0, 0, 0, 0.0, 0));
        e.0 += 1;
        if ok {
            e.1 += 1;
        }
        if ver {
            e.2 += 1;
        }
        e.3 += cost;
        e.4 += latency;
    }

    let success_rate = if total == 0 {
        0.0
    } else {
        success as f64 / total as f64
    };
    let avg_latency = if total == 0 {
        0.0
    } else {
        total_latency as f64 / total as f64
    };
    let verified_rate = if total == 0 {
        0.0
    } else {
        verified as f64 / total as f64
    };

    let mut model_bits = Vec::new();
    for (m, (cnt, ok, ver, cost, lat)) in by_model {
        let sr = if cnt == 0 {
            0.0
        } else {
            ok as f64 / cnt as f64
        };
        let vr = if cnt == 0 {
            0.0
        } else {
            ver as f64 / cnt as f64
        };
        let al = if cnt == 0 {
            0.0
        } else {
            lat as f64 / cnt as f64
        };
        model_bits.push(format!(
            "(:model \"{}\" :count {} :success-rate {:.4} :verified-rate {:.4} :cost-usd {:.8} :avg-latency-ms {:.2})",
            m, cnt, sr, vr, cost, al
        ));
    }
    model_bits.sort();

    format!(
        "(:total {} :success-rate {:.4} :verified-rate {:.4} :total-cost-usd {:.8} :avg-latency-ms {:.2} :models ({}))",
        total,
        success_rate,
        verified_rate,
        total_cost,
        avg_latency,
        model_bits.join(" ")
    )
}
