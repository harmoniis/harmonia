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

struct FrontendDef {
    name: &'static str,
    display: &'static str,
    vault_keys: Vec<(&'static str, &'static str, bool)>, // (symbol, prompt, is_password)
}

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

fn frontend_defs() -> Vec<FrontendDef> {
    vec![
        FrontendDef {
            name: "mqtt",
            display: "MQTT",
            vault_keys: vec![("mqtt-broker-url", "MQTT broker URL", false)],
        },
        FrontendDef {
            name: "telegram",
            display: "Telegram",
            vault_keys: vec![("telegram-bot-token", "Telegram bot token", true)],
        },
        FrontendDef {
            name: "slack",
            display: "Slack",
            vault_keys: vec![
                ("slack-app-token", "Slack app token (xapp-...)", true),
                ("slack-bot-token", "Slack bot token (xoxb-...)", true),
            ],
        },
        FrontendDef {
            name: "discord",
            display: "Discord",
            vault_keys: vec![("discord-bot-token", "Discord bot token", true)],
        },
        FrontendDef {
            name: "signal",
            display: "Signal (signal-cli bridge)",
            vault_keys: vec![
                ("signal-account", "Signal account/number", false),
                ("signal-rpc-url", "Signal bridge URL", false),
                (
                    "signal-auth-token",
                    "Signal bridge auth token (optional)",
                    true,
                ),
            ],
        },
        FrontendDef {
            name: "whatsapp",
            display: "WhatsApp",
            vault_keys: vec![
                ("whatsapp-bridge-url", "WhatsApp bridge API URL", false),
                ("whatsapp-session", "WhatsApp bridge API token", true),
            ],
        },
        FrontendDef {
            name: "imessage",
            display: "iMessage (BlueBubbles)",
            vault_keys: vec![
                ("bluebubbles-server-url", "BlueBubbles server URL", false),
                ("bluebubbles-password", "BlueBubbles password", true),
            ],
        },
        FrontendDef {
            name: "tailscale",
            display: "Tailscale mesh",
            vault_keys: vec![("tailscale-auth-key", "Tailscale auth key", true)],
        },
        FrontendDef {
            name: "email",
            display: "Email (IMAP/SMTP)",
            vault_keys: vec![],
        },
        FrontendDef {
            name: "mattermost",
            display: "Mattermost",
            vault_keys: vec![],
        },
        FrontendDef {
            name: "nostr",
            display: "Nostr",
            vault_keys: vec![],
        },
    ]
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
            vec!["google/gemini-3.1-flash-lite-preview", "google/gemini-2.5-pro"]
        }
        "bedrock" => vec!["amazon/nova-micro-v1", "amazon/nova-lite-v1", "amazon/nova-pro-v1"],
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
        ("google-vertex", default_seed_models_for_provider("google-vertex")),
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

fn prompt_llm_secret(
    provider: &LlmProviderDef,
    secret: &LlmSecretDef,
) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = format!("    {} {}", provider.display, secret.prompt);
    let value = if secret.is_password {
        read_masked(&prompt)?
    } else {
        let mut input = Input::<String>::new().with_prompt(prompt);
        if let Some(default) = secret.default {
            input = input.default(default.to_string());
        }
        input.allow_empty(true).interact_text()?
    };
    Ok(value.trim().to_string())
}

fn configure_llm_providers() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Setup only collects API keys and stores them in the vault.
    // Model selection is automatic — the backend owns a built-in model pool
    // with pricing, and the harmonic-matrix evolves selection over time.
    let defs = llm_provider_defs();
    let display_names: Vec<&str> = defs.iter().map(|d| d.display).collect();
    let defaults: Vec<bool> = defs.iter().map(|d| d.id == "openrouter").collect();

    let selected = MultiSelect::new()
        .with_prompt("  [5/10] Select LLM providers (keys stored in vault only)")
        .items(&display_names)
        .defaults(&defaults)
        .interact()?;

    if selected.is_empty() {
        return Err("at least one LLM provider must be selected".into());
    }

    println!(
        "    {} Storing provider credentials in vault...",
        style("→").cyan().bold()
    );

    let mut configured_count = 0usize;
    let mut configured_provider_ids: Vec<String> = Vec::new();
    for idx in selected {
        let def = &defs[idx];
        if let Some(cmd) = def.required_command {
            if !check_command(cmd) {
                println!(
                    "    {} {} CLI not found (provider can still be configured for later): {}",
                    style("!").yellow().bold(),
                    cmd,
                    def.display
                );
            }
        }

        let mut staged: Vec<(&str, String)> = Vec::new();
        let mut missing_required = Vec::new();
        for secret in &def.secrets {
            let value = prompt_llm_secret(def, secret)?;
            if value.is_empty() {
                if secret.required {
                    missing_required.push(secret.prompt);
                }
                continue;
            }
            staged.push((secret.symbol, value));
        }

        if !missing_required.is_empty() {
            println!(
                "    {} {} skipped (missing required fields: {})",
                style("!").yellow().bold(),
                def.display,
                missing_required.join(", ")
            );
            continue;
        }

        for (symbol, value) in staged {
            harmonia_vault::set_secret_for_symbol(symbol, &value)
                .map_err(|e| format!("vault write failed for {}: {e}", symbol))?;
        }
        configured_count += 1;
        configured_provider_ids.push(def.id.to_string());
        println!(
            "    {} {} — key stored in vault",
            style("✓").green().bold(),
            def.display
        );
    }

    if configured_count == 0 {
        return Err("no LLM provider was fully configured".into());
    }

    println!(
        "    {} Model selection is automatic (pool-based harmonic scoring)",
        style("✓").green().bold()
    );

    // No model env vars — the backend owns model selection via its built-in pool.
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
    let provider_seed_csv = harmonia_config_store::get_config("harmonia-cli", "model-policy", &provider_key)
        .ok()
        .flatten()
        .map(|raw| normalize_model_csv(&raw))
        .filter(|csv| !csv.is_empty());
    if provider_seed_csv.is_some() {
        return provider_seed_csv;
    }

    let active_provider = harmonia_config_store::get_config("harmonia-cli", "model-policy", "provider")
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

fn configure_evolution_profile(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!();
    let options = vec![
        "Binary-only evolution (artifact rollout, no source rewrite)",
        "Local source rewrite (ouroboros + git lineage)",
        "Distributed evolution participant (organization harmonization)",
    ];

    let selection = dialoguer::Select::new()
        .with_prompt("  Evolution profile")
        .items(&options)
        .default(0)
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

            let bucket: String = Input::new()
                .with_prompt("    Distributed evolution bucket")
                .allow_empty(true)
                .interact_text()?;
            if !bucket.trim().is_empty() {
                cs("evolution", "distributed-store-bucket", bucket.trim())?;
            }

            let prefix: String = Input::new()
                .with_prompt("    Distributed evolution prefix")
                .default("harmonia/evolution".to_string())
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
    println!(
        "  {} User data:     {}",
        style("[1/4]").bold().dim(),
        style(system_dir.display()).green()
    );
    println!(
        "       Libraries:   {}",
        style(lib_dir.display()).green()
    );
    println!(
        "       App data:    {}",
        style(share_dir.display()).green()
    );
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;
    fs::create_dir_all(system_dir.join("frontends"))?;
    fs::create_dir_all(share_dir.join("genesis"))?;

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

    // Store platform-standard paths
    cs("global", "lib-dir", &lib_dir.to_string_lossy())?;
    cs("global", "share-dir", &share_dir.to_string_lossy())?;

    // Detect source dir for dev builds
    if let Ok(source_dir) = find_source_dir() {
        cs("global", "source-dir", &source_dir.to_string_lossy())?;
    }

    // User workspace
    let default_workspace = home.join("workspace");
    let workspace: String = Input::new()
        .with_prompt(format!(
            "  {} User workspace directory",
            style("[3/4]").bold().dim()
        ))
        .default(default_workspace.to_string_lossy().to_string())
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

    // Frontend selection
    println!();
    let defs = frontend_defs();
    let display_names: Vec<&str> = defs.iter().map(|d| d.display).collect();
    let selected = MultiSelect::new()
        .with_prompt("  Select frontends to enable (TUI always on, Enter to skip all)")
        .items(&display_names)
        .interact()?;

    let mut enabled_frontends: Vec<&str> = vec!["tui"];
    let mut mqtt_selected = false;
    if !selected.is_empty() {
        println!("  Frontend credentials:");
        for &idx in &selected {
            let def = &defs[idx];
            enabled_frontends.push(def.name);
            if def.name == "mqtt" {
                mqtt_selected = true;
            }

            for (symbol, prompt, is_password) in &def.vault_keys {
                let value = if *is_password {
                    read_masked(&format!("    {} {}", def.display, prompt))?
                } else {
                    Input::<String>::new()
                        .with_prompt(format!("    {} {}", def.display, prompt))
                        .allow_empty(true)
                        .interact_text()?
                };
                if !value.is_empty() {
                    harmonia_vault::set_secret_for_symbol(symbol, &value)
                        .map_err(|e| format!("vault write failed for {}: {e}", symbol))?;
                }
            }
        }
    }

    if mqtt_selected {
        configure_mqtt_defaults(&wallet_path, &cs)?;
    }

    // Optional tool API keys
    println!("\n  Optional tool API keys (Enter to skip):");
    let optional_keys = [
        ("exa-api-key", "Exa search API key"),
        ("brave-api-key", "Brave search API key"),
        ("elevenlabs-api-key", "ElevenLabs API key"),
    ];
    for (symbol, prompt) in &optional_keys {
        let value = read_masked(&format!("    {}", prompt))?;
        if !value.is_empty() {
            harmonia_vault::set_secret_for_symbol(symbol, &value)
                .map_err(|e| format!("vault write failed for {}: {e}", symbol))?;
        }
    }

    // Git fork + GitHub token (optional)
    println!();
    let default_fork = "https://github.com/harmoniis/harmonia.git".to_string();
    let fork_url: String = Input::new()
        .with_prompt("  Git fork URL (Enter to skip)")
        .default(default_fork)
        .interact_text()?;
    if !fork_url.is_empty() {
        harmonia_vault::set_secret_for_symbol("github-fork-url", &fork_url)
            .map_err(|e| format!("vault write failed for github-fork-url: {e}"))?;

        let github_token = read_masked("    GitHub PAT (for git push to fork, Enter to skip)")?;
        if !github_token.is_empty() {
            harmonia_vault::set_secret_for_symbol("github-token", &github_token)
                .map_err(|e| format!("vault write failed for github-token: {e}"))?;
        }
    }

    // S3 credentials (optional)
    println!();
    let s3_bucket: String = Input::new()
        .with_prompt("  S3 bucket for binary backups (Enter to skip)")
        .allow_empty(true)
        .interact_text()?;
    if !s3_bucket.is_empty() {
        harmonia_vault::set_secret_for_symbol("s3-bucket", &s3_bucket)
            .map_err(|e| format!("vault write failed for s3-bucket: {e}"))?;

        let s3_access_key: String = Input::new()
            .with_prompt("    AWS access key ID")
            .allow_empty(true)
            .interact_text()?;
        if !s3_access_key.is_empty() {
            harmonia_vault::set_secret_for_symbol("s3-access-key-id", &s3_access_key)
                .map_err(|e| format!("vault write failed for s3-access-key-id: {e}"))?;
            let _ = harmonia_vault::set_secret_for_symbol("aws-access-key-id", &s3_access_key);
        }

        let s3_secret_key = read_masked("    AWS secret access key")?;
        if !s3_secret_key.is_empty() {
            harmonia_vault::set_secret_for_symbol("s3-secret-access-key", &s3_secret_key)
                .map_err(|e| format!("vault write failed for s3-secret-access-key: {e}"))?;
            let _ = harmonia_vault::set_secret_for_symbol("aws-secret-access-key", &s3_secret_key);
        }
        println!(
            "    {} S3 credentials stored in vault",
            style("✓").green().bold()
        );
    }

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
        // Copy genesis docs to share dir
        let genesis_src = source_dir.join("doc").join("genesis");
        if genesis_src.exists() {
            copy_dir_recursive(&genesis_src, &share_dir.join("genesis"))?;
            println!(
                "    {} Evolution knowledge installed",
                style("✓").green().bold()
            );
        }

        // Copy Lisp source to share dir
        let lisp_src = source_dir.join("src");
        if lisp_src.exists() {
            let share_src = crate::paths::source_dir()?;
            copy_dir_recursive(&lisp_src, &share_src)?;
            // Copy config/ for baseband.sexp fallback
            let config_src = source_dir.join("config");
            if config_src.exists() {
                copy_dir_recursive(&config_src, &share_dir.join("config"))?;
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
            let bin_name = if cfg!(target_os = "windows") { "harmonia.exe" } else { "harmonia" };
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
    println!(
        "  Libraries:        {}",
        style(lib_dir.display()).green()
    );
    println!(
        "  App data:         {}",
        style(share_dir.display()).green()
    );
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
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let wallet_path = home.join(".harmoniis").join("master.db");
    let legacy_wallet_path = home.join(".harmoniis").join("rgb.db");
    if wallet_path.exists() {
        return Ok(wallet_path);
    }
    if legacy_wallet_path.exists() {
        return Ok(legacy_wallet_path);
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
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
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

fn configure_mqtt_defaults<F>(
    wallet_path: &Path,
    set_config: &F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&str, &str, &str) -> Result<(), Box<dyn std::error::Error>>,
{
    if !check_command("openssl") {
        return Err("openssl is required to generate wallet-bound MQTT TLS certificates".into());
    }

    let mqtt_dir = crate::paths::data_dir()?.join("mqtt");
    fs::create_dir_all(&mqtt_dir)?;
    let label = "mqtt-client-alice";
    let vault_new = Command::new("hrmw")
        .args([
            "key",
            "vault-new",
            "--wallet",
            wallet_path.to_string_lossy().as_ref(),
            "--label",
            label,
        ])
        .output()?;
    if !vault_new.status.success() {
        return Err(format!(
            "failed to create MQTT vault identity: {}",
            String::from_utf8_lossy(&vault_new.stderr)
        )
        .into());
    }
    let public_key = parse_hrmw_output_field(&String::from_utf8_lossy(&vault_new.stdout), "Vault public key:")?;

    let key_path = mqtt_dir.join("broker.key.pem");
    let export = Command::new("hrmw")
        .args([
            "key",
            "vault-export",
            "--wallet",
            wallet_path.to_string_lossy().as_ref(),
            "--label",
            label,
            "--output",
            key_path.to_string_lossy().as_ref(),
        ])
        .output()?;
    if !export.status.success() {
        return Err(format!(
            "failed to export MQTT vault private key: {}",
            String::from_utf8_lossy(&export.stderr)
        )
        .into());
    }

    let ca_key_path = mqtt_dir.join("broker-ca.key.pem");
    let ca_path = mqtt_dir.join("broker-ca.crt");
    let csr_path = mqtt_dir.join("broker.csr");
    let cert_path = mqtt_dir.join("broker.crt");
    let chain_path = mqtt_dir.join("broker.chain.crt");
    let ext_path = mqtt_dir.join("broker.ext");
    fs::write(
        &ext_path,
        "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1\n",
    )?;

    let ca_key_status = Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "Ed25519",
            "-out",
            ca_key_path.to_string_lossy().as_ref(),
        ])
        .output()?;
    if !ca_key_status.status.success() {
        return Err(format!(
            "failed to generate MQTT CA private key: {}",
            String::from_utf8_lossy(&ca_key_status.stderr)
        )
        .into());
    }

    let ca_status = Command::new("openssl")
        .args([
            "req",
            "-new",
            "-x509",
            "-key",
            ca_key_path.to_string_lossy().as_ref(),
            "-out",
            ca_path.to_string_lossy().as_ref(),
            "-days",
            "365",
            "-subj",
            "/CN=harmonia-mqtt-ca",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,digitalSignature,keyCertSign",
        ])
        .output()?;
    if !ca_status.status.success() {
        return Err(format!(
            "failed to generate MQTT CA certificate: {}",
            String::from_utf8_lossy(&ca_status.stderr)
        )
        .into());
    }

    let csr_status = Command::new("openssl")
        .args([
            "req",
            "-new",
            "-key",
            key_path.to_string_lossy().as_ref(),
            "-out",
            csr_path.to_string_lossy().as_ref(),
            "-subj",
            &format!("/CN={public_key}"),
        ])
        .output()?;
    if !csr_status.status.success() {
        return Err(format!(
            "failed to generate MQTT broker CSR: {}",
            String::from_utf8_lossy(&csr_status.stderr)
        )
        .into());
    }

    let cert_status = Command::new("openssl")
        .args([
            "x509",
            "-req",
            "-in",
            csr_path.to_string_lossy().as_ref(),
            "-CA",
            ca_path.to_string_lossy().as_ref(),
            "-CAkey",
            ca_key_path.to_string_lossy().as_ref(),
            "-CAcreateserial",
            "-out",
            cert_path.to_string_lossy().as_ref(),
            "-days",
            "365",
            "-extfile",
            ext_path.to_string_lossy().as_ref(),
        ])
        .output()?;
    if !cert_status.status.success() {
        return Err(format!(
            "failed to generate MQTT broker certificate: {}",
            String::from_utf8_lossy(&cert_status.stderr)
        )
        .into());
    }
    let chain_pem = format!(
        "{}\n{}",
        fs::read_to_string(&cert_path)?,
        fs::read_to_string(&ca_path)?
    );
    fs::write(&chain_path, chain_pem)?;

    set_config("mqtt-broker", "mode", "embedded")?;
    set_config("mqtt-broker", "bind", "127.0.0.1:8883")?;
    set_config("mqtt-broker", "tls", "1")?;
    set_config("mqtt-broker", "ca-cert", &ca_path.to_string_lossy())?;
    set_config("mqtt-broker", "server-cert", &chain_path.to_string_lossy())?;
    set_config("mqtt-broker", "server-key", &key_path.to_string_lossy())?;
    set_config("mqtt-broker", "remote-config-url", "https://harmoniis.com/api/agent")?;
    set_config("mqtt-broker", "remote-config-identity-label", label)?;
    set_config("mqtt-broker", "remote-config-refresh-seconds", "60")?;
    set_config("mqtt-broker", "identity-public-key", &public_key)?;

    set_config("mqtt-frontend", "broker", "127.0.0.1:8883")?;
    set_config("mqtt-frontend", "tls", "1")?;
    set_config("mqtt-frontend", "ca-cert", &ca_path.to_string_lossy())?;
    set_config("mqtt-frontend", "client-cert", &cert_path.to_string_lossy())?;
    set_config("mqtt-frontend", "client-key", &key_path.to_string_lossy())?;
    set_config("mqtt-frontend", "push-webhook-url", "https://harmoniis.com/api/webhooks/push")?;
    set_config("mqtt-frontend", "trusted-client-fingerprints-json", "[]")?;
    set_config("mqtt-frontend", "trusted-device-registry-json", "[]")?;
    Ok(())
}

fn parse_hrmw_output_field(output: &str, prefix: &str) -> Result<String, Box<dyn std::error::Error>> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Ok(rest.trim().to_string());
        }
    }
    Err(format!("missing hrmw output field: {prefix}").into())
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
            "(:signal-account :signal-rpc-url)",
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
        let auto_load = if enabled.contains(name) { "t" } else { "nil" };
        entries.push(format!(
            "   (:name \"{name}\"\n    :so-path \"{path}.{so_ext}\"\n    :security-label {label}\n    :auto-load {auto_load}\n    :vault-keys {keys})",
        ));
    }

    format!("(:frontends\n  ({}\n  ))\n", entries.join("\n"))
}
