use console::style;
use dialoguer::{Input, MultiSelect, Password};
use std::fs;
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

fn prompt_llm_secret(
    provider: &LlmProviderDef,
    secret: &LlmSecretDef,
) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = format!("    {} {}", provider.display, secret.prompt);
    let value = if secret.is_password {
        Password::new()
            .with_prompt(prompt)
            .allow_empty_password(true)
            .interact()?
    } else {
        let mut input = Input::<String>::new().with_prompt(prompt);
        if let Some(default) = secret.default {
            input = input.default(default.to_string());
        }
        input.allow_empty(true).interact_text()?
    };
    Ok(value.trim().to_string())
}

fn configure_llm_providers() -> Result<(), Box<dyn std::error::Error>> {
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
    println!(
        "  {} System workspace: {}",
        style("[1/4]").bold().dim(),
        style(system_dir.display()).green()
    );
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;
    fs::create_dir_all(system_dir.join("genesis"))?;
    fs::create_dir_all(system_dir.join("frontends"))?;

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

    // Detect and store source dir
    if let Ok(source_dir) = find_source_dir() {
        cs("global", "source-dir", &source_dir.to_string_lossy())?;
        let lib_dir = source_dir.join("target").join("release");
        cs("global", "lib-dir", &lib_dir.to_string_lossy())?;
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
    configure_llm_providers()?;

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
    if !selected.is_empty() {
        println!("  Frontend credentials:");
        for &idx in &selected {
            let def = &defs[idx];
            enabled_frontends.push(def.name);

            for (symbol, prompt, is_password) in &def.vault_keys {
                let value = if *is_password {
                    Password::new()
                        .with_prompt(format!("    {} {}", def.display, prompt))
                        .allow_empty_password(true)
                        .interact()?
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

    // Optional tool API keys
    println!("\n  Optional tool API keys (Enter to skip):");
    let optional_keys = [
        ("exa-api-key", "Exa search API key"),
        ("brave-api-key", "Brave search API key"),
        ("elevenlabs-api-key", "ElevenLabs API key"),
    ];
    for (symbol, prompt) in &optional_keys {
        let value: String = Input::new()
            .with_prompt(format!("    {}", prompt))
            .allow_empty(true)
            .interact_text()?;
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

        let github_token: String = Password::new()
            .with_prompt("    GitHub PAT (for git push to fork, Enter to skip)")
            .allow_empty_password(true)
            .interact()?;
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

        let s3_secret_key: String = Password::new()
            .with_prompt("    AWS secret access key")
            .allow_empty_password(true)
            .interact()?;
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

    // Copy genesis docs to system workspace
    if let Ok(source_dir) = find_source_dir() {
        let genesis_src = source_dir.join("doc").join("genesis");
        if genesis_src.exists() {
            copy_dir_recursive(&genesis_src, &system_dir.join("genesis"))?;
            println!(
                "    {} Evolution knowledge installed",
                style("✓").green().bold()
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
        }
    }

    // Done
    println!();
    println!("  {} Setup complete!", style("✓").green().bold());
    println!();
    println!(
        "  System workspace: {}",
        style(system_dir.display()).green()
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

    // Priority 3: Standard install location (~/.harmoniis/harmonia)
    if let Some(home) = dirs::home_dir() {
        let installed_root = home.join(".harmoniis").join("harmonia");
        if is_runtime_root(&installed_root) {
            return Ok(installed_root);
        }
        let installed_src = installed_root.join("src");
        if is_runtime_root(&installed_src) {
            return Ok(installed_src);
        }
    }

    // Priority 4: Walk up from binary location
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
