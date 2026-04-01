//! Backend status reporting — sexp plist for Lisp introspection.

use crate::registry::{provider_is_active, PROVIDERS};

pub fn all_backends_sexp() -> String {
    let mut entries = Vec::new();
    entries.push(
        "(:id \"openrouter\" :healthy t :default t :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))".to_string()
    );
    for p in PROVIDERS {
        if provider_is_active(p.id) {
            entries.push(format!(
                "(:id \"{}\" :healthy t :default nil :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))",
                p.id
            ));
        }
    }
    format!("({})", entries.join(" "))
}

pub fn backend_status_sexp(name: &str) -> Option<String> {
    if name.is_empty() || name == "openrouter" {
        return Some(
            "(:id \"openrouter\" :healthy t :default t :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))".to_string()
        );
    }
    let provider = PROVIDERS.iter().find(|p| p.id == name)?;
    let active = provider_is_active(provider.id);
    Some(format!(
        "(:id \"{}\" :healthy {} :default nil :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))",
        provider.id,
        if active { "t" } else { "nil" }
    ))
}
