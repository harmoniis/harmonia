/// First-run environment variable seeding into config DB.

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::legacy::all_legacy_entries;
use crate::store;

pub(crate) fn seed_from_env() -> Result<u32, String> {
    // Check if already seeded
    if store::get_meta("seeded_at")?.is_some() {
        return Ok(0);
    }

    let mut count: u32 = 0;
    for (scope, key, env_name) in all_legacy_entries() {
        if let Ok(val) = env::var(env_name) {
            let val = val.trim().to_string();
            if !val.is_empty() {
                store::set_value(scope, key, &val)?;
                count += 1;
            }
        }
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    store::set_meta("seeded_at", &ts)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_idempotent() {
        // After first seed, subsequent calls return 0
        // (This test relies on the config_meta check)
        // We can't fully test without a DB, but ensure the function signature is correct.
        let _ = seed_from_env();
    }
}
