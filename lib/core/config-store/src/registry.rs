/// Declarative configuration registry — single source of truth for all known
/// (scope, key) pairs and their environment variable names.
///
/// Env var names are derived by three rules applied in order:
///   1. Global scope drops its name:  ("global", "state-root") → HARMONIA_STATE_ROOT
///   2. Strip known scope suffixes:   ("openai-backend", "base-url") → HARMONIA_OPENAI_BASE_URL
///   3. Apply stem aliases:           ("harmonic-matrix", "db") → HARMONIA_MATRIX_DB
///
/// The few entries that break all three rules carry an explicit `env_override`.

/// A known configuration entry.
pub(crate) struct Entry {
    pub scope: &'static str,
    pub key: &'static str,
    /// Explicit env var name. If `None`, derived by [`derive_env_name`].
    pub env_override: Option<&'static str>,
}

/// Suffixes stripped from scope names to produce concise env var names.
const SCOPE_SUFFIXES: &[&str] = &["-backend", "-frontend", "-tool", "-core", "-storage"];

/// Scope stems that get aliased after suffix stripping.
const STEM_ALIASES: &[(&str, &str)] = &[
    ("harmonic-matrix", "matrix"),
    ("search-exa", "exa"),
    ("search-brave", "brave"),
    ("amazon-bedrock", "bedrock"),
];

/// Derive the env var name for a (scope, key) pair using the three-rule system.
pub(crate) fn derive_env_name(scope: &str, key: &str) -> String {
    let key_upper = key.to_ascii_uppercase().replace('-', "_");

    // Rule 1: global scope drops its name entirely.
    if scope == "global" {
        return format!("HARMONIA_{key_upper}");
    }

    // Rule 2: strip the first matching suffix.
    let mut stem = scope.to_string();
    for suffix in SCOPE_SUFFIXES {
        if let Some(stripped) = scope.strip_suffix(suffix) {
            stem = stripped.to_string();
            break;
        }
    }

    // Rule 3: apply stem alias if one exists.
    for &(from, to) in STEM_ALIASES {
        if stem == from {
            stem = to.to_string();
            break;
        }
    }

    let stem_upper = stem.to_ascii_uppercase().replace('-', "_");
    format!("HARMONIA_{stem_upper}_{key_upper}")
}

/// Get the env var name for a (scope, key) pair.
/// Checks registry overrides first, then falls back to derivation.
pub(crate) fn env_name(scope: &str, key: &str) -> String {
    for entry in registry_entries() {
        if entry.scope.eq_ignore_ascii_case(scope) && entry.key.eq_ignore_ascii_case(key) {
            if let Some(name) = entry.env_override {
                return name.to_string();
            }
            return derive_env_name(entry.scope, entry.key);
        }
    }
    // Unknown entry — still derive a reasonable name.
    derive_env_name(scope, key)
}

/// All known (scope, key, env_name) triples, derived from the single registry.
pub(crate) fn all_entries() -> Vec<(&'static str, &'static str, String)> {
    registry_entries()
        .map(|e| {
            let env = match e.env_override {
                Some(name) => name.to_string(),
                None => derive_env_name(e.scope, e.key),
            };
            (e.scope, e.key, env)
        })
        .collect()
}

// ─── Registry ───────────────────────────────────────────────────────

mod backends;
#[cfg(test)]
mod compat_table;
mod frontends;
mod global_node;
mod runtime;

const REGISTRY_SECTIONS: &[&[Entry]] = &[
    global_node::ENTRIES,
    backends::ENTRIES,
    frontends::ENTRIES,
    runtime::ENTRIES,
];

fn registry_entries() -> impl Iterator<Item = &'static Entry> {
    REGISTRY_SECTIONS.iter().flat_map(|entries| entries.iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_scope_drops_name() {
        assert_eq!(
            derive_env_name("global", "state-root"),
            "HARMONIA_STATE_ROOT"
        );
        assert_eq!(derive_env_name("global", "env"), "HARMONIA_ENV");
        assert_eq!(
            derive_env_name("global", "system-dir"),
            "HARMONIA_SYSTEM_DIR"
        );
    }

    #[test]
    fn suffix_stripping() {
        assert_eq!(
            derive_env_name("openai-backend", "base-url"),
            "HARMONIA_OPENAI_BASE_URL"
        );
        assert_eq!(
            derive_env_name("mqtt-frontend", "broker"),
            "HARMONIA_MQTT_BROKER"
        );
        assert_eq!(
            derive_env_name("whisper-backend", "groq-api-url"),
            "HARMONIA_WHISPER_GROQ_API_URL"
        );
        assert_eq!(
            derive_env_name("elevenlabs-backend", "base-url"),
            "HARMONIA_ELEVENLABS_BASE_URL"
        );
        assert_eq!(
            derive_env_name("tailnet-core", "port"),
            "HARMONIA_TAILNET_PORT"
        );
        assert_eq!(derive_env_name("s3-storage", "mode"), "HARMONIA_S3_MODE");
    }

    #[test]
    fn stem_aliases() {
        assert_eq!(
            derive_env_name("harmonic-matrix", "db"),
            "HARMONIA_MATRIX_DB"
        );
        assert_eq!(
            derive_env_name("harmonic-matrix", "store-kind"),
            "HARMONIA_MATRIX_STORE_KIND"
        );
        assert_eq!(
            derive_env_name("search-exa-tool", "api-url"),
            "HARMONIA_EXA_API_URL"
        );
        assert_eq!(
            derive_env_name("search-brave-tool", "api-url"),
            "HARMONIA_BRAVE_API_URL"
        );
        assert_eq!(
            derive_env_name("amazon-bedrock-backend", "region"),
            "HARMONIA_BEDROCK_REGION"
        );
    }

    #[test]
    fn explicit_overrides() {
        assert_eq!(
            env_name("anthropic-backend", "api-version"),
            "HARMONIA_ANTHROPIC_VERSION"
        );
        assert_eq!(
            env_name("evolution", "source-rewrite-enabled"),
            "HARMONIA_SOURCE_REWRITE_ENABLED"
        );
        assert_eq!(env_name("phoenix-core", "trauma-log"), "PHOENIX_TRAUMA_LOG");
        assert_eq!(
            env_name("phoenix-core", "allow-prod-genesis"),
            "HARMONIA_ALLOW_PROD_GENESIS"
        );
    }

    #[test]
    fn unknown_entries_still_derive() {
        // Entries not in the registry still get a reasonable env name.
        assert_eq!(env_name("custom-backend", "foo"), "HARMONIA_CUSTOM_FOO");
        assert_eq!(env_name("global", "new-key"), "HARMONIA_NEW_KEY");
    }

    /// Exhaustive check: every registry entry must produce the expected env var
    /// name. This is the authoritative backward-compatibility test -- if this
    /// passes, all existing env vars continue to work.
    #[test]
    fn all_entries_match_historic_names() {
        let expected = compat_table::EXPECTED;
        let actual = all_entries();
        assert_eq!(
            actual.len(),
            expected.len(),
            "registry has {} entries but expected {}",
            actual.len(),
            expected.len()
        );

        for (scope, key, want) in expected {
            let got = env_name(scope, key);
            assert_eq!(
                got, *want,
                "mismatch for ({scope}, {key}): got {got}, want {want}"
            );
        }
    }

    #[test]
    fn no_duplicate_entries() {
        let mut seen = std::collections::HashSet::new();
        for entry in registry_entries() {
            assert!(
                seen.insert((entry.scope, entry.key)),
                "duplicate registry entry: ({}, {})",
                entry.scope,
                entry.key
            );
        }
    }

    #[test]
    fn route_entries_use_derivation_correctly() {
        // These are the trickiest: harmonic-matrix entries where the stem alias
        // is applied but the env name drops "matrix" for route-* keys.
        // They use the stem alias "matrix" which produces HARMONIA_MATRIX_ROUTE_*
        // but historically they were HARMONIA_ROUTE_*. Verify overrides are set
        // where needed or derivation matches.
        assert_eq!(
            env_name("harmonic-matrix", "route-signal-default"),
            "HARMONIA_ROUTE_SIGNAL_DEFAULT"
        );
        assert_eq!(
            env_name("harmonic-matrix", "route-noise-default"),
            "HARMONIA_ROUTE_NOISE_DEFAULT"
        );
    }
}
