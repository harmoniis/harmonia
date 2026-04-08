//! Model catalogue: sync from OpenRouter API and upsert hardcoded offerings.

use rusqlite::params;

use super::db::{db, now_secs};

/// Sync the model catalogue from the OpenRouter /api/v1/models endpoint.
/// Fetches all models with pricing and upserts into the `models` table.
/// Returns count of models synced, or error.
pub fn sync_models_from_openrouter(api_key: &str) -> Result<usize, String> {
    let output = std::process::Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg("10")
        .arg("--max-time")
        .arg("30")
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("https://openrouter.ai/api/v1/models")
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl failed: {}", &stderr[..stderr.len().min(200)]));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("json parse failed: {e}"))?;

    let data = parsed
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "missing data array in response".to_string())?;

    let conn = db()
        .lock()
        .map_err(|_| "metrics db lock poisoned".to_string())?;

    let ts = now_secs();
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("transaction failed: {e}"))?;

    let mut count = 0usize;
    for model in data {
        let id = match model.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };
        let name = model.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let ctx = model
            .get("context_length")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let max_comp = model
            .get("max_completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let pricing = model.get("pricing");
        let usd_in = pricing
            .and_then(|p| p.get("prompt"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let usd_out = pricing
            .and_then(|p| p.get("completion"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let modality = model
            .get("architecture")
            .and_then(|a| a.get("modality"))
            .and_then(|v| v.as_str())
            .unwrap_or("text->text");

        // Extract provider from id (e.g. "openai/gpt-5" -> "openai")
        let provider = id.split_once('/').map(|(p, _)| p).unwrap_or("");

        let _ = tx.execute(
            "INSERT INTO models (id, name, provider, context_length, max_completion, usd_per_tok_in, usd_per_tok_out, modality, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'openrouter-api', ?9)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, context_length=excluded.context_length,
               max_completion=excluded.max_completion, usd_per_tok_in=excluded.usd_per_tok_in,
               usd_per_tok_out=excluded.usd_per_tok_out, modality=excluded.modality,
               source='openrouter-api', updated_at=excluded.updated_at",
            params![id, name, provider, ctx, max_comp, usd_in, usd_out, modality, ts],
        );
        count += 1;
    }

    tx.commit().map_err(|e| format!("commit failed: {e}"))?;
    eprintln!("[INFO] [metrics] Synced {count} models from OpenRouter API");
    Ok(count)
}

/// Insert hardcoded model offerings into the catalogue (lower priority than API data).
/// Only inserts if model not already present from API sync.
pub fn upsert_hardcoded_offerings(offerings: &[crate::ModelOffering], backend: &str) {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return,
    };
    let ts = now_secs();
    for o in offerings {
        // Convert per-1k-token price to per-token price
        let usd_in = o.usd_in_1k / 1000.0;
        let usd_out = o.usd_out_1k / 1000.0;
        let provider = o.id.split_once('/').map(|(p, _)| p).unwrap_or(backend);
        // Only insert if not already present (don't overwrite API data)
        let _ = conn.execute(
            "INSERT OR IGNORE INTO models (id, name, provider, usd_per_tok_in, usd_per_tok_out, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'hardcoded', ?6)",
            params![o.id, o.id, provider, usd_in, usd_out, ts],
        );
    }
}
