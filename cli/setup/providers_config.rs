//! Interactive LLM provider configuration (key collection).

use console::style;
use dialoguer::Input;

use super::helpers::{check_command, read_masked};
use super::providers::{llm_provider_defs, provider_has_vault_keys};

pub(crate) fn configure_llm_providers() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let defs = llm_provider_defs();

    let configured_flags: Vec<bool> = defs.iter().map(|d| provider_has_vault_keys(d)).collect();
    let display_names: Vec<String> = defs
        .iter()
        .zip(configured_flags.iter())
        .map(|(d, &has_keys)| {
            if has_keys {
                format!("{} [configured]", d.display)
            } else {
                d.display.to_string()
            }
        })
        .collect();
    let display_refs: Vec<&str> = display_names.iter().map(|s| s.as_str()).collect();

    let defaults: Vec<bool> = defs
        .iter()
        .zip(configured_flags.iter())
        .map(|(d, &has_keys)| has_keys || d.id == "openrouter")
        .collect();

    let selected = dialoguer::MultiSelect::new()
        .with_prompt("  [5/10] Select LLM providers (keys stored in vault only)")
        .items(&display_refs)
        .defaults(&defaults)
        .interact()?;

    if selected.is_empty() {
        return Err("at least one LLM provider must be selected".into());
    }

    let mut configured_count = 0usize;
    let mut configured_provider_ids: Vec<String> = Vec::new();
    for idx in selected {
        let def = &defs[idx];
        let already_configured = configured_flags[idx];

        if let Some(cmd) = def.required_command {
            if !check_command(cmd) {
                println!(
                    "    {} {} CLI not found — provider can still be configured for later",
                    style("!").yellow().bold(),
                    cmd,
                );
            }
        }

        let mut wrote_any = false;
        let mut missing_required = Vec::new();
        for secret in &def.secrets {
            let has_existing = harmonia_vault::has_secret_for_symbol(secret.symbol);
            let prompt = if has_existing {
                format!(
                    "    {} {} (Enter to keep existing)",
                    def.display, secret.prompt
                )
            } else {
                format!("    {} {}", def.display, secret.prompt)
            };

            let value = if secret.is_password {
                read_masked(&prompt)?
            } else {
                let mut input = Input::<String>::new().with_prompt(&prompt);
                if !has_existing {
                    if let Some(default) = secret.default {
                        input = input.default(default.to_string());
                    }
                }
                input.allow_empty(true).interact_text()?
            };
            let value = value.trim().to_string();

            if value.is_empty() {
                if !has_existing && secret.required {
                    missing_required.push(secret.prompt);
                }
                continue;
            }

            harmonia_vault::set_secret_for_symbol(secret.symbol, &value)
                .map_err(|e| format!("vault write failed for {}: {e}", secret.symbol))?;
            wrote_any = true;
        }

        if !missing_required.is_empty() {
            println!(
                "    {} {} skipped (missing required: {})",
                style("!").yellow().bold(),
                def.display,
                missing_required.join(", ")
            );
            continue;
        }

        if already_configured || wrote_any {
            configured_count += 1;
            configured_provider_ids.push(def.id.to_string());
            if wrote_any && already_configured {
                println!("    {} {} — updated", style("✓").green().bold(), def.display);
            } else if wrote_any {
                println!("    {} {} — key stored in vault", style("✓").green().bold(), def.display);
            } else {
                println!("    {} {} — kept existing", style("✓").green().bold(), def.display);
            }
        } else {
            println!(
                "    {} {} skipped (no credentials provided)",
                style("!").yellow().bold(),
                def.display,
            );
        }
    }

    if configured_count == 0 {
        return Err("no LLM provider was fully configured".into());
    }

    println!(
        "    {} Model selection is automatic (pool-based harmonic scoring)",
        style("✓").green().bold()
    );

    Ok(configured_provider_ids)
}
