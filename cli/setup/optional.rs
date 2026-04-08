//! Optional setup steps: LangSmith observability and evolution profile.

use console::style;
use dialoguer::Input;
use std::path::Path;

use super::helpers::read_masked;

pub(crate) fn configure_langsmith_observability() -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("  LangSmith observability (optional — Enter to skip):");
    println!(
        "    {}",
        style("Enables distributed tracing for debugging agent pipelines").dim()
    );

    let has_key = harmonia_vault::has_secret_for_symbol("langsmith-api-key");
    let key_prompt = if has_key {
        "    LangSmith API key [configured] (Enter to keep)"
    } else {
        "    LangSmith API key"
    };
    let api_key = read_masked(key_prompt)?;
    if !api_key.is_empty() {
        harmonia_vault::set_secret_for_symbol("langsmith-api-key", &api_key)
            .map_err(|e| format!("vault write failed for langsmith-api-key: {e}"))?;

        let project_name: String = Input::new()
            .with_prompt("    LangSmith project name")
            .default("harmonia".to_string())
            .interact_text()?;

        let cs = |scope: &str, key: &str, val: &str| -> Result<(), Box<dyn std::error::Error>> {
            harmonia_config_store::set_config("harmonia-cli", scope, key, val)
                .map_err(|e| e.into())
        };

        cs("observability", "enabled", "1")?;
        cs("observability", "trace-level", "standard")?;
        cs("observability", "sample-rate", "1.0")?;
        cs("observability", "project-name", &project_name)?;

        println!(
            "    {} LangSmith observability configured (project={})",
            style("✓").green().bold(),
            project_name
        );
    } else if has_key {
        println!(
            "    {} LangSmith — keeping existing configuration",
            style("✓").green().bold()
        );
    }

    Ok(())
}

pub(crate) fn configure_evolution_profile(_home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!();
    let options = vec!["Local evolution", "Distributed evolution"];

    let distributed = harmonia_config_store::get_config(
        "harmonia-cli",
        "evolution",
        "distributed-enabled",
    )
    .ok()
    .flatten();
    let default_idx = if distributed.as_deref() == Some("1") {
        1
    } else {
        0
    };

    let selection = dialoguer::Select::new()
        .with_prompt("  Evolution mode")
        .items(&options)
        .default(default_idx)
        .interact()?;

    let cs = |scope: &str, key: &str, val: &str| {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val)
    };

    cs("evolution", "mode", "binary")?;

    match selection {
        0 => {
            cs("evolution", "distributed-enabled", "0")?;
        }
        _ => {
            cs("evolution", "distributed-enabled", "1")?;

            let storage_options = vec!["S3", "Other"];
            let existing_kind = harmonia_config_store::get_config(
                "harmonia-cli",
                "evolution",
                "distributed-store-kind",
            )
            .ok()
            .flatten()
            .unwrap_or_default();
            let storage_default = if existing_kind == "other" { 1 } else { 0 };
            let storage_sel = dialoguer::Select::new()
                .with_prompt("    Storage backend")
                .items(&storage_options)
                .default(storage_default)
                .interact()?;

            let store_kind = if storage_sel == 0 { "s3" } else { "other" };
            cs("evolution", "distributed-store-kind", store_kind)?;

            if storage_sel == 0 {
                let existing_bucket = harmonia_config_store::get_config(
                    "harmonia-cli",
                    "evolution",
                    "distributed-store-bucket",
                )
                .ok()
                .flatten()
                .unwrap_or_default();
                let mut bucket_input = Input::<String>::new().with_prompt("    S3 bucket");
                if !existing_bucket.is_empty() {
                    bucket_input = bucket_input.default(existing_bucket);
                }
                let bucket: String = bucket_input.allow_empty(true).interact_text()?;
                if !bucket.trim().is_empty() {
                    cs("evolution", "distributed-store-bucket", bucket.trim())?;
                }

                let existing_prefix = harmonia_config_store::get_config(
                    "harmonia-cli",
                    "evolution",
                    "distributed-store-prefix",
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| "harmonia/evolution".to_string());
                let prefix: String = Input::new()
                    .with_prompt("    S3 prefix")
                    .default(existing_prefix)
                    .interact_text()?;
                cs("evolution", "distributed-store-prefix", prefix.trim())?;

                let has_s3_key = harmonia_vault::has_secret_for_symbol("s3-access-key-id");
                let key_prompt = if has_s3_key {
                    "    AWS access key ID [configured] (Enter to keep)"
                } else {
                    "    AWS access key ID"
                };
                let access_key: String = Input::new()
                    .with_prompt(key_prompt)
                    .allow_empty(true)
                    .interact_text()?;
                if !access_key.is_empty() {
                    harmonia_vault::set_secret_for_symbol("s3-access-key-id", &access_key)
                        .map_err(|e| format!("vault: {e}"))?;
                }

                let has_s3_secret =
                    harmonia_vault::has_secret_for_symbol("s3-secret-access-key");
                let secret_prompt = if has_s3_secret {
                    "    AWS secret key [configured] (Enter to keep)"
                } else {
                    "    AWS secret key"
                };
                let secret_key = read_masked(secret_prompt)?;
                if !secret_key.is_empty() {
                    harmonia_vault::set_secret_for_symbol("s3-secret-access-key", &secret_key)
                        .map_err(|e| format!("vault: {e}"))?;
                }
            }
        }
    }

    Ok(())
}
