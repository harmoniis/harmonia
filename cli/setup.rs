use console::{style, Term};
use dialoguer::{Input, MultiSelect, Select};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const BANNER: &str = r#"
  _   _                                  _
 | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _
 | |_| |/ _` | '__| '_ ` _ \ / _ \| '_ \| |/ _` |
 |  _  | (_| | |  | | | | | | (_) | | | | | (_| |
 |_| |_|\__,_|_|  |_| |_| |_|\___/|_| |_|_|\__,_|
"#;

struct LlmSecretDef {
    symbol: &'static str,
    prompt: &'static str,
    is_password: bool,
    required: bool,
    default: Option<&'static str>,
}

struct LlmProviderDef {
    id: &'static str,
    display: &'static str,
    required_command: Option<&'static str>,
    secrets: Vec<LlmSecretDef>,
}

fn llm_provider_defs() -> Vec<LlmProviderDef> {
    // Model selection is NOT configured here — the backend owns its model pool
    // with built-in pricing, and the harmonic-matrix evolves selection over time.
    // Setup only collects API keys and stores them securely in the vault.
    vec![
        LlmProviderDef {
            id: "openrouter",
            display: "OpenRouter (recommended — routes to all providers)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "openrouter-api-key",
                prompt: "OpenRouter API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "openai",
            display: "OpenAI (direct)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "openai-api-key",
                prompt: "OpenAI API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "anthropic",
            display: "Anthropic (direct)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "anthropic-api-key",
                prompt: "Anthropic API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "xai",
            display: "xAI (direct)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "xai-api-key",
                prompt: "xAI API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "google-ai-studio",
            display: "Google AI Studio (direct)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "google-ai-studio-api-key",
                prompt: "Google AI Studio API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "google-vertex",
            display: "Google Vertex AI (direct)",
            required_command: None,
            secrets: vec![
                LlmSecretDef {
                    symbol: "google-vertex-access-token",
                    prompt: "Google Vertex access token (Bearer)",
                    is_password: true,
                    required: true,
                    default: None,
                },
                LlmSecretDef {
                    symbol: "google-vertex-project-id",
                    prompt: "Google Vertex project ID",
                    is_password: false,
                    required: true,
                    default: None,
                },
                LlmSecretDef {
                    symbol: "google-vertex-location",
                    prompt: "Google Vertex location",
                    is_password: false,
                    required: false,
                    default: Some("us-central1"),
                },
            ],
        },
        LlmProviderDef {
            id: "bedrock",
            display: "Amazon Bedrock / Nova (direct)",
            required_command: Some("aws"),
            secrets: vec![
                LlmSecretDef {
                    symbol: "aws-region",
                    prompt: "AWS region",
                    is_password: false,
                    required: false,
                    default: Some("us-east-1"),
                },
                LlmSecretDef {
                    symbol: "aws-access-key-id",
                    prompt: "AWS access key ID (optional, Enter to use ambient IAM)",
                    is_password: false,
                    required: false,
                    default: None,
                },
                LlmSecretDef {
                    symbol: "aws-secret-access-key",
                    prompt: "AWS secret access key (optional)",
                    is_password: true,
                    required: false,
                    default: None,
                },
                LlmSecretDef {
                    symbol: "aws-session-token",
                    prompt: "AWS session token (optional)",
                    is_password: true,
                    required: false,
                    default: None,
                },
            ],
        },
        LlmProviderDef {
            id: "groq",
            display: "Groq (direct)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "groq-api-key",
                prompt: "Groq API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "alibaba",
            display: "Alibaba / DashScope / Qwen (direct)",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "alibaba-api-key",
                prompt: "Alibaba API key",
                is_password: true,
                required: true,
                default: None,
            }],
        },
    ]
}

fn default_seed_models_for_provider(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "openrouter" => vec![
            "inception/mercury-2",
            "qwen/qwen3.5-flash-02-23",
            "minimax/minimax-m2.5",
            "google/gemini-3.1-flash-lite-preview",
        ],
        "openai" => vec!["openai/gpt-5"],
        "anthropic" => vec!["anthropic/claude-sonnet-4.6", "anthropic/claude-opus-4.6"],
        "xai" => vec!["x-ai/grok-4-fast:online"],
        "google-ai-studio" | "google-vertex" => {
            vec![
                "google/gemini-3.1-flash-lite-preview",
                "google/gemini-2.5-pro",
            ]
        }
        "bedrock" => vec![
            "amazon/nova-micro-v1",
            "amazon/nova-lite-v1",
            "amazon/nova-pro-v1",
        ],
        "groq" => vec!["qwen/qwen3-coder:free"],
        "alibaba" => vec!["qwen/qwen3-coder:free"],
        _ => vec![],
    }
}

fn all_provider_seed_defaults() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("openrouter", default_seed_models_for_provider("openrouter")),
        ("openai", default_seed_models_for_provider("openai")),
        ("anthropic", default_seed_models_for_provider("anthropic")),
        ("xai", default_seed_models_for_provider("xai")),
        (
            "google-ai-studio",
            default_seed_models_for_provider("google-ai-studio"),
        ),
        (
            "google-vertex",
            default_seed_models_for_provider("google-vertex"),
        ),
        ("bedrock", default_seed_models_for_provider("bedrock")),
        ("groq", default_seed_models_for_provider("groq")),
        ("alibaba", default_seed_models_for_provider("alibaba")),
    ]
}

fn normalize_model_csv(raw: &str) -> String {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// Read a secret with per-character `*` feedback (handles typing, paste, and backspace).
fn read_masked(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let term = Term::stderr();
    eprint!("{}: ", prompt);
    std::io::stderr().flush()?;
    let mut buf = String::new();
    loop {
        let key = term.read_key()?;
        match key {
            console::Key::Char(c) if !c.is_control() => {
                buf.push(c);
                eprint!("*");
                std::io::stderr().flush()?;
            }
            console::Key::Backspace => {
                if buf.pop().is_some() {
                    // Move cursor back, overwrite with space, move back again
                    eprint!("\x08 \x08");
                    std::io::stderr().flush()?;
                }
            }
            console::Key::Enter => {
                eprintln!();
                break;
            }
            _ => {}
        }
    }
    Ok(buf)
}

fn provider_has_vault_keys(def: &LlmProviderDef) -> bool {
    let required: Vec<_> = def.secrets.iter().filter(|s| s.required).collect();
    if required.is_empty() {
        // No required secrets (e.g. Bedrock) — only "configured" if at least one secret exists
        def.secrets
            .iter()
            .any(|s| harmonia_vault::has_secret_for_symbol(s.symbol))
    } else {
        required
            .iter()
            .all(|s| harmonia_vault::has_secret_for_symbol(s.symbol))
    }
}

fn configure_llm_providers() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Setup only collects API keys and stores them in the vault.
    // Model selection is automatic — the backend owns a built-in model pool
    // with pricing, and the harmonic-matrix evolves selection over time.
    let defs = llm_provider_defs();

    // Build display names showing which providers already have vault keys
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

    // Pre-select: already-configured providers stay selected, openrouter as default for fresh
    let defaults: Vec<bool> = defs
        .iter()
        .zip(configured_flags.iter())
        .map(|(d, &has_keys)| has_keys || d.id == "openrouter")
        .collect();

    let selected = MultiSelect::new()
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

        // Prompt for each secret, showing existing status per-key
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
                if has_existing {
                    // Keeping existing value — counts toward being configured
                } else if secret.required {
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

        // A provider is configured if it had existing keys OR new keys were written
        if already_configured || wrote_any {
            configured_count += 1;
            configured_provider_ids.push(def.id.to_string());
            if wrote_any && already_configured {
                println!(
                    "    {} {} — updated",
                    style("✓").green().bold(),
                    def.display
                );
            } else if wrote_any {
                println!(
                    "    {} {} — key stored in vault",
                    style("✓").green().bold(),
                    def.display
                );
            } else {
                println!(
                    "    {} {} — kept existing",
                    style("✓").green().bold(),
                    def.display
                );
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

fn seed_provider_ids(configured_provider_ids: &[String]) -> Vec<String> {
    let defs = llm_provider_defs();
    let mut provider_ids: Vec<String> = Vec::new();

    let mut push_known = |id: &str| {
        let known = defs.iter().any(|d| d.id == id);
        if known && !provider_ids.iter().any(|existing| existing == id) {
            provider_ids.push(id.to_string());
        }
    };

    for id in configured_provider_ids {
        push_known(id);
    }

    if let Ok(Some(active_provider)) =
        harmonia_config_store::get_config("harmonia-cli", "model-policy", "provider")
    {
        push_known(&active_provider);
    }

    for (provider, _) in all_provider_seed_defaults() {
        push_known(provider);
    }

    if provider_ids.is_empty() {
        for def in defs {
            provider_ids.push(def.id.to_string());
        }
    }

    provider_ids
}

fn stored_seed_models_for_provider(provider_id: &str) -> Option<String> {
    let provider_key = format!("seed-models-{}", provider_id);
    let provider_seed_csv =
        harmonia_config_store::get_config("harmonia-cli", "model-policy", &provider_key)
            .ok()
            .flatten()
            .map(|raw| normalize_model_csv(&raw))
            .filter(|csv| !csv.is_empty());
    if provider_seed_csv.is_some() {
        return provider_seed_csv;
    }

    let active_provider =
        harmonia_config_store::get_config("harmonia-cli", "model-policy", "provider")
            .ok()
            .flatten();
    if active_provider.as_deref() != Some(provider_id) {
        return None;
    }

    harmonia_config_store::get_config("harmonia-cli", "model-policy", "seed-models")
        .ok()
        .flatten()
        .map(|raw| normalize_model_csv(&raw))
        .filter(|csv| !csv.is_empty())
}

fn seed_prompt_default_for_provider(provider_id: &str) -> String {
    stored_seed_models_for_provider(provider_id).unwrap_or_else(|| {
        normalize_model_csv(&default_seed_models_for_provider(provider_id).join(","))
    })
}

fn configure_model_seed_policy(
    configured_provider_ids: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let provider_ids = seed_provider_ids(configured_provider_ids);
    if provider_ids.is_empty() {
        return Err("at least one provider must be available for seed policy".into());
    }

    let defs = llm_provider_defs();
    let provider_labels: Vec<String> = provider_ids
        .iter()
        .map(|id| {
            defs.iter()
                .find(|d| d.id == id)
                .map(|d| d.display.to_string())
                .unwrap_or_else(|| id.clone())
        })
        .collect();

    let stored_primary_provider =
        harmonia_config_store::get_config("harmonia-cli", "model-policy", "provider")
            .ok()
            .flatten();
    let default_primary = stored_primary_provider
        .as_deref()
        .and_then(|provider| provider_ids.iter().position(|id| id == provider))
        .or_else(|| provider_ids.iter().position(|id| id == "openrouter"))
        .unwrap_or(0);

    let selected_idx = Select::new()
        .with_prompt("  Primary provider for seed models")
        .items(&provider_labels)
        .default(default_primary)
        .interact()?;

    let active_provider = provider_ids[selected_idx].clone();
    let default_seed_csv = seed_prompt_default_for_provider(&active_provider);
    let entered_seed_csv: String = Input::new()
        .with_prompt("    Seed models for primary provider (comma-separated)")
        .default(default_seed_csv.clone())
        .interact_text()?;
    let normalized_seed_csv = {
        let n = normalize_model_csv(&entered_seed_csv);
        if n.is_empty() {
            default_seed_csv
        } else {
            n
        }
    };

    let cs = |scope: &str, key: &str, val: &str| -> Result<(), Box<dyn std::error::Error>> {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val).map_err(|e| e.into())
    };

    cs("model-policy", "provider", &active_provider)?;
    cs("model-policy", "seed-models", &normalized_seed_csv)?;

    for (provider, defaults) in all_provider_seed_defaults() {
        let key = format!("seed-models-{}", provider);
        let csv = defaults.join(",");
        cs("model-policy", &key, &csv)?;
    }

    let active_key = format!("seed-models-{}", active_provider);
    cs("model-policy", &active_key, &normalized_seed_csv)?;

    println!(
        "    {} Seed policy stored (provider={}, models={})",
        style("✓").green().bold(),
        active_provider,
        normalized_seed_csv
    );

    Ok(())
}

pub fn run_seeds_only() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", style(BANNER).cyan().bold());
    println!("  {}", style("Seed model policy setup").dim());
    println!();

    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());

    harmonia_config_store::init_v2().map_err(|e| format!("config-store init failed: {e}"))?;

    println!(
        "  {} Updating model seeds in {}",
        style("→").cyan().bold(),
        style(system_dir.join("config.db").display()).green()
    );

    configure_model_seed_policy(&[])?;

    println!();
    println!("  {} Seed setup complete.", style("✓").green().bold());
    println!(
        "  Re-run anytime with {}",
        style("harmonia setup --seeds").cyan().bold()
    );
    println!();

    Ok(())
}

fn configure_langsmith_observability() -> Result<(), Box<dyn std::error::Error>> {
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
            harmonia_config_store::set_config("harmonia-cli", scope, key, val).map_err(|e| e.into())
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

fn configure_evolution_profile(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!();
    let options = vec![
        "Binary-only evolution (artifact rollout, no source rewrite)",
        "Local source rewrite (ouroboros + git lineage)",
        "Distributed evolution participant (organization harmonization)",
    ];

    // Detect existing evolution mode to set default selection
    let existing_mode = harmonia_config_store::get_config("harmonia-cli", "evolution", "mode")
        .ok()
        .flatten();
    let default_idx = match existing_mode.as_deref() {
        Some("source-rewrite") => 1,
        Some("artifact-rollout") => {
            // Check if distributed is enabled
            let distributed = harmonia_config_store::get_config(
                "harmonia-cli",
                "evolution",
                "distributed-enabled",
            )
            .ok()
            .flatten();
            if distributed.as_deref() == Some("1") {
                2
            } else {
                0
            }
        }
        _ => 0,
    };

    let selection = dialoguer::Select::new()
        .with_prompt("  Evolution profile")
        .items(&options)
        .default(default_idx)
        .interact()?;

    let cs = |scope: &str, key: &str, val: &str| {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val)
    };

    match selection {
        0 => {
            cs("evolution", "mode", "artifact-rollout")?;
            cs("evolution", "source-rewrite-enabled", "0")?;
            cs("evolution", "distributed-enabled", "0")?;
        }
        1 => {
            cs("evolution", "mode", "source-rewrite")?;
            cs("evolution", "source-rewrite-enabled", "1")?;
            cs("evolution", "distributed-enabled", "0")?;

            if let Some(rewrite_root) = detect_source_rewrite_root(home) {
                cs("global", "source-dir", &rewrite_root.to_string_lossy())?;
                println!(
                    "    {} Source rewrite root: {}",
                    style("✓").green().bold(),
                    rewrite_root.display()
                );
            } else {
                println!(
                    "    {} Source rewrite git checkout not found — can configure later with `harmonia setup`.",
                    style("!").yellow().bold()
                );
            }
        }
        _ => {
            cs("evolution", "mode", "artifact-rollout")?;
            cs("evolution", "source-rewrite-enabled", "0")?;
            cs("evolution", "distributed-enabled", "1")?;
            cs("evolution", "distributed-store-kind", "s3")?;

            let existing_bucket = harmonia_config_store::get_config(
                "harmonia-cli",
                "evolution",
                "distributed-store-bucket",
            )
            .ok()
            .flatten()
            .unwrap_or_default();
            let mut bucket_input = Input::<String>::new()
                .with_prompt("    Distributed evolution bucket")
                .allow_empty(true);
            if !existing_bucket.is_empty() {
                bucket_input = bucket_input.default(existing_bucket);
            }
            let bucket: String = bucket_input.interact_text()?;
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
                .with_prompt("    Distributed evolution prefix")
                .default(existing_prefix)
                .interact_text()?;
            cs("evolution", "distributed-store-prefix", prefix.trim())?;
        }
    }

    Ok(())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", style(BANNER).cyan().bold());
    println!(
        "  {}",
        style("Distributed evolutionary homoiconic self-improving agent").dim()
    );
    println!();

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  REQUIRED — system workspace, runtime deps, LLM provider
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    println!(
        "  {}",
        style("─── Required ───────────────────────────────────────").dim()
    );

    // Step 1: System workspace
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");
    let lib_dir = crate::paths::lib_dir()?;
    let share_dir = crate::paths::share_dir()?;
    let node_identity = crate::paths::current_node_identity()?;
    println!(
        "  {} User data:     {}",
        style("[1/4]").bold().dim(),
        style(system_dir.display()).green()
    );
    println!("       Libraries:   {}", style(lib_dir.display()).green());
    println!("       App data:    {}", style(share_dir.display()).green());
    println!(
        "       Node:        {} ({}, {})",
        style(&node_identity.label).green(),
        node_identity.role.as_str(),
        node_identity.install_profile.as_str()
    );
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;
    fs::create_dir_all(system_dir.join("frontends"))?;
    fs::create_dir_all(share_dir.join("genesis"))?;
    crate::paths::ensure_node_layout(&node_identity)?;

    // Step 2: Check SBCL + Quicklisp
    println!(
        "  {} Checking runtime dependencies...",
        style("[2/4]").bold().dim()
    );
    if !check_command("sbcl") {
        println!(
            "    {} SBCL not found. Install it:",
            style("!").red().bold()
        );
        println!("      macOS:        brew install sbcl");
        println!("      Ubuntu/Debian: sudo apt install sbcl");
        println!("      Fedora:       sudo dnf install sbcl");
        println!("      FreeBSD:      sudo pkg install sbcl");
        println!("      Arch:         sudo pacman -S sbcl");
        println!();
        println!("    Install SBCL and re-run: harmonia setup");
        return Err("SBCL is required".into());
    }
    println!("    {} SBCL found", style("✓").green().bold());

    let quicklisp_path = home.join("quicklisp").join("setup.lisp");
    if !quicklisp_path.exists() {
        println!(
            "    {} Quicklisp not found. Installing...",
            style("→").yellow().bold()
        );
        install_quicklisp()?;
    }
    println!("    {} Quicklisp found", style("✓").green().bold());

    // Step 3: Wallet + vault + config-store bootstrap
    println!();
    let wallet_path = ensure_harmoniis_wallet()?;
    let wallet_root = wallet_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    std::env::set_var(
        "HARMONIA_WALLET_ROOT",
        wallet_root.to_string_lossy().to_string(),
    );
    std::env::set_var(
        "HARMONIA_VAULT_WALLET_DB",
        wallet_path.to_string_lossy().to_string(),
    );
    println!(
        "    {} Wallet identity root: {}",
        style("✓").green().bold(),
        wallet_path.display()
    );

    let vault_path = system_dir.join("vault.db");
    std::env::set_var("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref());
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());
    harmonia_vault::init_from_env().map_err(|e| format!("vault init failed: {e}"))?;

    // Initialize config-store and seed bootstrap values
    harmonia_config_store::init_v2().map_err(|e| format!("config-store init failed: {e}"))?;

    let cs = |scope: &str, key: &str, val: &str| -> Result<(), Box<dyn std::error::Error>> {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val).map_err(|e| e.into())
    };

    // Write system paths to config-store
    cs("global", "state-root", &system_dir.to_string_lossy())?;
    cs("global", "system-dir", &system_dir.to_string_lossy())?;
    cs("global", "wallet-root", &wallet_root.to_string_lossy())?;
    cs("global", "wallet-db", &wallet_path.to_string_lossy())?;

    // Store platform-standard paths
    cs("global", "lib-dir", &lib_dir.to_string_lossy())?;
    cs("global", "share-dir", &share_dir.to_string_lossy())?;
    cs("node", "label", &node_identity.label)?;
    cs("node", "hostname", &node_identity.hostname)?;
    cs("node", "role", node_identity.role.as_str())?;
    cs(
        "node",
        "install-profile",
        node_identity.install_profile.as_str(),
    )?;
    cs(
        "node",
        "sessions-root",
        &crate::paths::node_sessions_dir(&node_identity.label)?.to_string_lossy(),
    )?;
    cs(
        "node",
        "pairings-root",
        &crate::paths::node_pairings_dir(&node_identity.label)?.to_string_lossy(),
    )?;
    cs(
        "node",
        "memory-root",
        &crate::paths::node_memory_dir(&node_identity.label)?.to_string_lossy(),
    )?;

    // Detect source dir for dev builds
    if let Ok(source_dir) = find_source_dir() {
        cs("global", "source-dir", &source_dir.to_string_lossy())?;
    }

    // User workspace — read existing workspace.sexp for default
    let default_workspace = read_existing_workspace(&system_dir)
        .unwrap_or_else(|| home.join("workspace").to_string_lossy().to_string());
    let workspace: String = Input::new()
        .with_prompt(format!(
            "  {} User workspace directory",
            style("[3/4]").bold().dim()
        ))
        .default(default_workspace)
        .interact_text()?;
    let workspace_path = PathBuf::from(&workspace);
    fs::create_dir_all(&workspace_path)?;

    let workspace_config = format!(
        "(:workspace\n  (:system-dir \"{}\")\n  (:user-workspace \"{}\"))\n",
        system_dir.display(),
        workspace_path.display()
    );
    fs::write(
        system_dir.join("config").join("workspace.sexp"),
        &workspace_config,
    )?;

    // Step 4: LLM provider (the only truly required config)
    println!(
        "\n  {} LLM provider credentials (at least one required)",
        style("[4/4]").bold().dim()
    );
    let configured_providers = configure_llm_providers()?;
    configure_model_seed_policy(&configured_providers)?;

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  OPTIONAL — frontends, tools, evolution, git, S3
    //  All can be skipped now and configured later via `harmonia setup`
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    println!();
    println!(
        "  {}",
        style("─── Optional (Enter to skip, configure later with `harmonia setup`) ──").dim()
    );

    let enabled_frontends: Vec<&str> = vec!["tui"];
    println!();
    println!(
        "  {} Frontends are configured from the interactive CLI only.",
        style("→").cyan().bold()
    );
    println!(
        "    {}",
        style("Finish setup, start Harmonia, then use /menu -> Frontends").dim()
    );

    // Optional tool API keys — detect existing
    println!("\n  Optional tool API keys (Enter to skip):");
    let optional_keys = [
        ("exa-api-key", "Exa search API key"),
        ("brave-api-key", "Brave search API key"),
        ("elevenlabs-api-key", "ElevenLabs API key"),
    ];
    for (symbol, prompt) in &optional_keys {
        let existing = harmonia_vault::has_secret_for_symbol(symbol);
        let label = if existing {
            format!("    {} [configured] (Enter to keep)", prompt)
        } else {
            format!("    {}", prompt)
        };
        let value = read_masked(&label)?;
        if !value.is_empty() {
            harmonia_vault::set_secret_for_symbol(symbol, &value)
                .map_err(|e| format!("vault write failed for {}: {e}", symbol))?;
        }
    }

    // Git fork + GitHub token (optional) — detect existing
    println!();
    let has_fork = harmonia_vault::has_secret_for_symbol("github-fork-url");
    let has_gh_token = harmonia_vault::has_secret_for_symbol("github-token");
    let fork_prompt = if has_fork {
        "  Git fork URL [configured] (Enter to keep)"
    } else {
        "  Git fork URL (Enter to skip)"
    };
    let default_fork = "https://github.com/harmoniis/harmonia.git".to_string();
    let fork_url: String = Input::new()
        .with_prompt(fork_prompt)
        .default(default_fork)
        .interact_text()?;
    if !fork_url.is_empty() {
        harmonia_vault::set_secret_for_symbol("github-fork-url", &fork_url)
            .map_err(|e| format!("vault write failed for github-fork-url: {e}"))?;

        let gh_prompt = if has_gh_token {
            "    GitHub PAT [configured] (Enter to keep)"
        } else {
            "    GitHub PAT (for git push to fork, Enter to skip)"
        };
        let github_token = read_masked(gh_prompt)?;
        if !github_token.is_empty() {
            harmonia_vault::set_secret_for_symbol("github-token", &github_token)
                .map_err(|e| format!("vault write failed for github-token: {e}"))?;
        }
    }

    // S3 credentials (optional) — detect existing
    println!();
    let has_s3 = harmonia_vault::has_secret_for_symbol("s3-bucket");
    let s3_prompt = if has_s3 {
        "  S3 bucket [configured] (Enter to keep)"
    } else {
        "  S3 bucket for binary backups (Enter to skip)"
    };
    let s3_bucket: String = Input::new()
        .with_prompt(s3_prompt)
        .allow_empty(true)
        .interact_text()?;
    if !s3_bucket.is_empty() {
        harmonia_vault::set_secret_for_symbol("s3-bucket", &s3_bucket)
            .map_err(|e| format!("vault write failed for s3-bucket: {e}"))?;

        let has_s3_key = harmonia_vault::has_secret_for_symbol("s3-access-key-id");
        let s3_key_prompt = if has_s3_key {
            "    AWS access key ID [configured] (Enter to keep)"
        } else {
            "    AWS access key ID"
        };
        let s3_access_key: String = Input::new()
            .with_prompt(s3_key_prompt)
            .allow_empty(true)
            .interact_text()?;
        if !s3_access_key.is_empty() {
            harmonia_vault::set_secret_for_symbol("s3-access-key-id", &s3_access_key)
                .map_err(|e| format!("vault write failed for s3-access-key-id: {e}"))?;
            let _ = harmonia_vault::set_secret_for_symbol("aws-access-key-id", &s3_access_key);
        }

        let has_s3_secret = harmonia_vault::has_secret_for_symbol("s3-secret-access-key");
        let s3_secret_prompt = if has_s3_secret {
            "    AWS secret access key [configured] (Enter to keep)"
        } else {
            "    AWS secret access key"
        };
        let s3_secret_key = read_masked(s3_secret_prompt)?;
        if !s3_secret_key.is_empty() {
            harmonia_vault::set_secret_for_symbol("s3-secret-access-key", &s3_secret_key)
                .map_err(|e| format!("vault write failed for s3-secret-access-key: {e}"))?;
            let _ = harmonia_vault::set_secret_for_symbol("aws-secret-access-key", &s3_secret_key);
        }
        println!(
            "    {} S3 credentials stored in vault",
            style("✓").green().bold()
        );
    } else if has_s3 {
        println!(
            "    {} S3 credentials — keeping existing",
            style("✓").green().bold()
        );
    }

    // LangSmith observability (optional)
    configure_langsmith_observability()?;

    // Evolution profile (optional — defaults to artifact-rollout)
    configure_evolution_profile(&home)?;

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Finalize
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    println!("\n  {} Finalizing...", style("→").cyan().bold());

    // Write baseband frontend config (system overlay).
    let gateway_config = generate_gateway_config(&enabled_frontends);
    fs::write(
        system_dir.join("config").join("baseband.sexp"),
        &gateway_config,
    )?;
    fs::write(
        system_dir.join("config").join("gateway-frontends.sexp"),
        &gateway_config,
    )?;

    // Build and install to platform-standard locations
    if let Ok(source_dir) = find_source_dir() {
        // Copy genesis docs to share dir (skip self-copy)
        let genesis_src = source_dir.join("doc").join("genesis");
        let genesis_dst = share_dir.join("genesis");
        if genesis_src.exists() && genesis_src != genesis_dst {
            copy_dir_recursive(&genesis_src, &genesis_dst)?;
            println!(
                "    {} Evolution knowledge installed",
                style("✓").green().bold()
            );
        }

        // Copy Lisp source to share dir (skip if source IS the share dir to avoid self-copy)
        let lisp_src = source_dir.join("src");
        if lisp_src.exists() {
            let share_src = crate::paths::source_dir()?;
            if lisp_src != share_src {
                copy_dir_recursive(&lisp_src, &share_src)?;
            }
            // Copy config/ for baseband.sexp fallback (skip self-copy)
            let config_src = source_dir.join("config");
            let config_dst = share_dir.join("config");
            if config_src.exists() && config_src != config_dst {
                copy_dir_recursive(&config_src, &config_dst)?;
            }
            cs("global", "source-dir", &share_dir.to_string_lossy())?;
            println!(
                "    {} Lisp source installed to {}",
                style("✓").green().bold(),
                share_dir.display()
            );
        }

        // Build full runtime artifacts so start works immediately.
        if source_dir.join("Cargo.toml").exists() {
            println!(
                "    {} Building runtime artifacts...",
                style("→").cyan().bold()
            );
            let build_status = Command::new("cargo")
                .args(["build", "--workspace", "--release"])
                .current_dir(&source_dir)
                .status()?;
            if !build_status.success() {
                return Err("failed to build runtime artifacts".into());
            }
            println!("    {} Runtime artifacts built", style("✓").green().bold());

            // Install cdylibs to platform lib dir
            let target_release = source_dir.join("target").join("release");
            install_cdylibs(&target_release, &lib_dir)?;
            println!(
                "    {} Libraries installed to {}",
                style("✓").green().bold(),
                lib_dir.display()
            );

            // Install binary
            let bin_name = if cfg!(target_os = "windows") {
                "harmonia.exe"
            } else {
                "harmonia"
            };
            let built_bin = target_release.join(bin_name);
            if built_bin.exists() {
                let dest_bin = home.join(".local").join("bin").join(bin_name);
                fs::create_dir_all(dest_bin.parent().unwrap())?;
                fs::copy(&built_bin, &dest_bin)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&dest_bin, fs::Permissions::from_mode(0o755));
                }
                println!(
                    "    {} Binary installed to {}",
                    style("✓").green().bold(),
                    dest_bin.display()
                );
            }
        }
    }

    // Done
    println!();
    println!("  {} Setup complete!", style("✓").green().bold());
    println!();
    println!(
        "  User data:        {}",
        style(system_dir.display()).green()
    );
    println!("  Libraries:        {}", style(lib_dir.display()).green());
    println!("  App data:         {}", style(share_dir.display()).green());
    println!(
        "  User workspace:   {}",
        style(workspace_path.display()).green()
    );
    println!(
        "  Vault:            {}",
        style(system_dir.join("vault.db").display()).green()
    );
    println!(
        "  Config DB:        {}",
        style(system_dir.join("config.db").display()).green()
    );
    println!();
    println!("  Start the agent:");
    println!("    {}", style("harmonia start").cyan().bold());
    println!();
    println!(
        "  {}",
        style("  To add frontends/tools later: harmonia setup").dim()
    );
    println!();

    Ok(())
}

fn check_command(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ensure_harmoniis_wallet() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let wallet_root = crate::paths::wallet_root_path()?;
    let wallet_path = wallet_root.join("master.db");
    if wallet_path.exists() || wallet_root.join("rgb.db").exists() {
        return Ok(wallet_path);
    }

    ensure_wallet_cli()?;

    let wallet_path_string = wallet_path.to_string_lossy().to_string();

    let mut output = Command::new("hrmw")
        .args([
            "setup",
            "--wallet",
            &wallet_path_string,
            "--password-manager",
            "best-effort",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("unexpected argument '--password-manager'") {
            output = Command::new("hrmw")
                .args(["setup", "--wallet", &wallet_path_string])
                .output()?;
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("failed to initialize harmoniis-wallet: {stderr}").into());
    }

    if !wallet_path.exists() {
        return Err(format!(
            "harmoniis-wallet setup completed but wallet file not found at {}",
            wallet_path.display()
        )
        .into());
    }
    Ok(wallet_path)
}

fn ensure_wallet_cli() -> Result<(), Box<dyn std::error::Error>> {
    const REQUIRED: (u64, u64, u64) = (0, 1, 26);

    let needs_install = match installed_hrmw_version() {
        Some(v) => v < REQUIRED,
        None => true,
    };
    if !needs_install {
        return Ok(());
    }

    if !check_command("cargo") {
        return Err(
            "cargo is required to install harmoniis-wallet (hrmw) for vault bootstrap".into(),
        );
    }

    println!(
        "    {} Installing harmoniis-wallet CLI (hrmw >= {}.{}.{})...",
        style("→").cyan().bold(),
        REQUIRED.0,
        REQUIRED.1,
        REQUIRED.2
    );
    let status = Command::new("cargo")
        .args([
            "install",
            "harmoniis-wallet",
            "--version",
            "0.1.26",
            "--force",
        ])
        .status()?;
    if !status.success() {
        return Err("failed to install harmoniis-wallet CLI".into());
    }
    Ok(())
}

fn installed_hrmw_version() -> Option<(u64, u64, u64)> {
    let output = Command::new("hrmw").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.split_whitespace().nth(1)?;
    parse_semver_tuple(version)
}

fn is_git_runtime_root(path: &Path) -> bool {
    path.join(".git").exists() && path.join("src").join("core").join("boot.lisp").exists()
}

fn detect_source_rewrite_root(home: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(env_dir) = std::env::var("HARMONIA_SOURCE_DIR") {
        candidates.push(PathBuf::from(env_dir));
    }

    if let Ok(found) = find_source_dir() {
        candidates.push(found);
    }

    candidates.push(
        home.join(".harmoniis")
            .join("harmonia")
            .join("source-rewrite"),
    );
    candidates.push(home.join(".harmoniis").join("harmonia").join("src"));

    candidates.into_iter().find(|p| is_git_runtime_root(p))
}

fn parse_semver_tuple(input: &str) -> Option<(u64, u64, u64)> {
    let clean = input
        .trim()
        .split('-')
        .next()
        .unwrap_or(input)
        .trim_start_matches('v');
    let mut parts = clean.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor, patch))
}

fn install_quicklisp() -> Result<(), Box<dyn std::error::Error>> {
    // Download quicklisp.lisp
    let tmp = std::env::temp_dir().join("quicklisp.lisp");
    let output = Command::new("curl")
        .args([
            "-sS",
            "-o",
            &tmp.to_string_lossy(),
            "https://beta.quicklisp.org/quicklisp.lisp",
        ])
        .output()?;

    if !output.status.success() {
        return Err("failed to download quicklisp.lisp".into());
    }

    // Install via SBCL
    let output = Command::new("sbcl")
        .arg("--non-interactive")
        .arg("--load")
        .arg(&tmp)
        .arg("--eval")
        .arg("(quicklisp-quickstart:install)")
        .arg("--eval")
        .arg("(ql:add-to-init-file)")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("quicklisp install failed: {}", stderr).into());
    }

    Ok(())
}

fn read_existing_workspace(system_dir: &Path) -> Option<String> {
    let ws_path = system_dir.join("config").join("workspace.sexp");
    let content = fs::read_to_string(ws_path).ok()?;
    // Parse (:user-workspace "...") from workspace.sexp
    let marker = ":user-workspace \"";
    let start = content.find(marker)? + marker.len();
    let end = content[start..].find('"')? + start;
    let path = &content[start..end];
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn find_source_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    fn is_runtime_root(path: &Path) -> bool {
        path.join("src").join("core").join("boot.lisp").exists()
    }

    // Priority 1: HARMONIA_SOURCE_DIR env var
    if let Ok(env_dir) = std::env::var("HARMONIA_SOURCE_DIR") {
        let p = PathBuf::from(&env_dir);
        if is_runtime_root(&p) {
            return Ok(p);
        }
    }

    // Priority 2: Current directory (developer-local workflow)
    let cwd = std::env::current_dir()?;
    if is_runtime_root(&cwd) {
        return Ok(cwd);
    }

    // Priority 3: Platform-standard share dir (~/.local/share/harmonia/)
    if let Ok(share) = crate::paths::share_dir() {
        if is_runtime_root(&share) {
            return Ok(share);
        }
    }

    // Priority 4: Legacy location (~/.harmoniis/harmonia) — migration compat
    if let Some(home) = dirs::home_dir() {
        let installed_root = home.join(".harmoniis").join("harmonia");
        if is_runtime_root(&installed_root) {
            return Ok(installed_root);
        }
    }

    // Priority 5: Walk up from binary location
    let exe = std::env::current_exe()?;
    let mut dir = exe.parent().unwrap().to_path_buf();

    for _ in 0..10 {
        if is_runtime_root(&dir) {
            return Ok(dir);
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    Err("cannot find Harmonia source directory — set HARMONIA_SOURCE_DIR or run from the project root".into())
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

fn install_cdylibs(target_dir: &Path, lib_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    let prefix = if cfg!(target_os = "windows") {
        ""
    } else {
        "lib"
    };
    fs::create_dir_all(lib_dir)?;
    for entry in fs::read_dir(target_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && name.contains("harmonia") && name.ends_with(ext) {
            let dest = lib_dir.join(&name);
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn generate_gateway_config(enabled: &[&str]) -> String {
    let so_ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };

    let mut entries = Vec::new();

    let all_frontends = [
        ("tui", "target/release/libharmonia_tui", ":owner", "nil"),
        (
            "mqtt",
            "target/release/libharmonia_mqtt_client",
            ":authenticated",
            "(:mqtt-broker-url :mqtt-cert)",
        ),
        (
            "http2",
            "target/release/libharmonia_http2_mtls",
            ":authenticated",
            "nil",
        ),
        (
            "imessage",
            "target/release/libharmonia_imessage",
            ":authenticated",
            "(:bluebubbles-server-url :bluebubbles-password)",
        ),
        (
            "whatsapp",
            "target/release/libharmonia_whatsapp",
            ":authenticated",
            "(:whatsapp-session)",
        ),
        (
            "telegram",
            "target/release/libharmonia_telegram",
            ":authenticated",
            "(:telegram-bot-token)",
        ),
        (
            "slack",
            "target/release/libharmonia_slack",
            ":authenticated",
            "(:slack-app-token :slack-bot-token)",
        ),
        (
            "discord",
            "target/release/libharmonia_discord",
            ":authenticated",
            "(:discord-bot-token)",
        ),
        (
            "signal",
            "target/release/libharmonia_signal",
            ":authenticated",
            "nil",
        ),
        (
            "tailscale",
            "target/release/libharmonia_tailscale_frontend",
            ":authenticated",
            "(:tailscale-auth-key)",
        ),
        (
            "email",
            "target/release/libharmonia_email_client",
            ":authenticated",
            "nil",
        ),
        (
            "mattermost",
            "target/release/libharmonia_mattermost",
            ":authenticated",
            "nil",
        ),
        (
            "nostr",
            "target/release/libharmonia_nostr",
            ":authenticated",
            "nil",
        ),
    ];

    for (name, path, label, keys) in &all_frontends {
        let auto_load = if *name == "signal" && enabled.contains(name) {
            ":if-ready"
        } else if enabled.contains(name) {
            "t"
        } else {
            "nil"
        };
        let mut extra = String::new();
        if *name == "signal" {
            extra.push_str("\n    :config-keys ((\"signal-frontend\" \"account\"))");
        }
        if *name == "http2" {
            extra.push_str(
                "\n    :config-keys ((\"http2-frontend\" \"bind\") (\"http2-frontend\" \"ca-cert\") (\"http2-frontend\" \"server-cert\") (\"http2-frontend\" \"server-key\") (\"http2-frontend\" \"trusted-client-fingerprints-json\"))",
            );
        }
        if *name == "imessage" {
            extra.push_str("\n    :platforms (:macos)");
        }
        entries.push(format!(
            "   (:name \"{name}\"\n    :so-path \"{path}.{so_ext}\"\n    :security-label {label}\n    :auto-load {auto_load}{extra}\n    :vault-keys {keys})",
        ));
    }

    format!("(:frontends\n  ({}\n  ))\n", entries.join("\n"))
}
