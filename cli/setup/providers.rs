//! LLM provider definitions and vault key checks.

pub(crate) struct LlmSecretDef {
    pub symbol: &'static str,
    pub prompt: &'static str,
    pub is_password: bool,
    pub required: bool,
    pub default: Option<&'static str>,
}

pub(crate) struct LlmProviderDef {
    pub id: &'static str,
    pub display: &'static str,
    pub required_command: Option<&'static str>,
    pub secrets: Vec<LlmSecretDef>,
}

pub(crate) fn llm_provider_defs() -> Vec<LlmProviderDef> {
    vec![
        LlmProviderDef {
            id: "openrouter",
            display: "OpenRouter",
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
            id: "harmoniis",
            display: "Harmoniis",
            required_command: None,
            secrets: vec![LlmSecretDef {
                symbol: "harmoniis-api-key",
                prompt: "Harmoniis Router API key (HARMONIIS_ROUTER_API_KEY)",
                is_password: true,
                required: true,
                default: None,
            }],
        },
        LlmProviderDef {
            id: "openai",
            display: "OpenAI",
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
            display: "Anthropic",
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
            display: "xAI",
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
            display: "Google AI Studio",
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
            display: "Google Vertex AI",
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
            display: "Amazon Bedrock",
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
            display: "Groq",
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
            display: "Alibaba",
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

pub(crate) fn provider_has_vault_keys(def: &LlmProviderDef) -> bool {
    let required: Vec<_> = def.secrets.iter().filter(|s| s.required).collect();
    if required.is_empty() {
        def.secrets
            .iter()
            .any(|s| harmonia_vault::has_secret_for_symbol(s.symbol))
    } else {
        required
            .iter()
            .all(|s| harmonia_vault::has_secret_for_symbol(s.symbol))
    }
}

