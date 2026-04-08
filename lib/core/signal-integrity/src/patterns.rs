//! Injection pattern definitions: severity tiers, attack categories, and pattern table.

/// Severity tier for injection patterns. Determines dissonance weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Category of injection attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    SocialEngineering,
    ToolInjection,
    SystemOverride,
    LispAttack,
    StructuralInjection,
}

impl Category {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SocialEngineering => "social_engineering",
            Self::ToolInjection => "tool_injection",
            Self::SystemOverride => "system_override",
            Self::LispAttack => "lisp_attack",
            Self::StructuralInjection => "structural_injection",
        }
    }
}

pub(crate) struct InjectionPattern {
    pub pattern: &'static str,
    pub severity: Severity,
    pub category: Category,
}

pub(crate) const PATTERNS: &[InjectionPattern] = &[
    // -- Social engineering --------------------------------------------------
    InjectionPattern { pattern: "ignore previous",         severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "ignore above",            severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "ignore all prior",        severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "ignore the above",        severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "ignore everything above", severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "disregard previous",      severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "disregard above",         severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "forget previous",         severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "forget your instructions",severity: Severity::High,     category: Category::SocialEngineering },
    InjectionPattern { pattern: "you are now",             severity: Severity::High,     category: Category::SocialEngineering },
    InjectionPattern { pattern: "new instructions",        severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "act as",                  severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "pretend you are",         severity: Severity::High,     category: Category::SocialEngineering },
    InjectionPattern { pattern: "simulate a system",       severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "role play as",            severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "hypothetical scenario",   severity: Severity::Low,      category: Category::SocialEngineering },
    InjectionPattern { pattern: "for testing purposes",    severity: Severity::Low,      category: Category::SocialEngineering },
    InjectionPattern { pattern: "do not follow",           severity: Severity::Medium,   category: Category::SocialEngineering },
    InjectionPattern { pattern: "jailbreak",               severity: Severity::High,     category: Category::SocialEngineering },
    InjectionPattern { pattern: "bypass",                  severity: Severity::Medium,   category: Category::SocialEngineering },

    // -- System override -----------------------------------------------------
    InjectionPattern { pattern: "system prompt",           severity: Severity::High,     category: Category::SystemOverride },
    InjectionPattern { pattern: "override instructions",   severity: Severity::High,     category: Category::SystemOverride },
    InjectionPattern { pattern: "override",                severity: Severity::Medium,   category: Category::SystemOverride },
    InjectionPattern { pattern: "[system]",                severity: Severity::High,     category: Category::SystemOverride },
    InjectionPattern { pattern: "[/system]",               severity: Severity::High,     category: Category::SystemOverride },
    InjectionPattern { pattern: "<system>",                severity: Severity::High,     category: Category::SystemOverride },
    InjectionPattern { pattern: "</system>",               severity: Severity::High,     category: Category::SystemOverride },
    InjectionPattern { pattern: "assistant:",              severity: Severity::Medium,   category: Category::SystemOverride },
    InjectionPattern { pattern: "human:",                  severity: Severity::Medium,   category: Category::SystemOverride },

    // -- Tool injection (Harmonia-specific) -----------------------------------
    InjectionPattern { pattern: "tool op=",                severity: Severity::Critical, category: Category::ToolInjection },
    InjectionPattern { pattern: "vault-set",               severity: Severity::Critical, category: Category::ToolInjection },
    InjectionPattern { pattern: "vault-delete",            severity: Severity::Critical, category: Category::ToolInjection },
    InjectionPattern { pattern: "config-set",              severity: Severity::Critical, category: Category::ToolInjection },
    InjectionPattern { pattern: "harmony-policy",          severity: Severity::High,     category: Category::ToolInjection },
    InjectionPattern { pattern: "matrix-set-edge",         severity: Severity::High,     category: Category::ToolInjection },
    InjectionPattern { pattern: "matrix-reset",            severity: Severity::High,     category: Category::ToolInjection },
    InjectionPattern { pattern: "codemode-run",            severity: Severity::Critical, category: Category::ToolInjection },
    InjectionPattern { pattern: "self-push",               severity: Severity::Critical, category: Category::ToolInjection },
    InjectionPattern { pattern: "datamine",                severity: Severity::Medium,   category: Category::ToolInjection },
    InjectionPattern { pattern: "palace-file",             severity: Severity::Medium,   category: Category::ToolInjection },

    // -- Lisp reader macro attacks -------------------------------------------
    InjectionPattern { pattern: "#.",                      severity: Severity::Critical, category: Category::LispAttack },
    InjectionPattern { pattern: "(eval",                   severity: Severity::Critical, category: Category::LispAttack },
    InjectionPattern { pattern: "(load",                   severity: Severity::Critical, category: Category::LispAttack },
    InjectionPattern { pattern: "(compile",                severity: Severity::Critical, category: Category::LispAttack },
    InjectionPattern { pattern: "(setf",                   severity: Severity::High,     category: Category::LispAttack },
    InjectionPattern { pattern: "(funcall",                severity: Severity::High,     category: Category::LispAttack },

    // -- Structural injection ------------------------------------------------
    InjectionPattern { pattern: "```system",               severity: Severity::High,     category: Category::StructuralInjection },
    InjectionPattern { pattern: "---\nsystem",             severity: Severity::High,     category: Category::StructuralInjection },
    InjectionPattern { pattern: "\n\n[inst",               severity: Severity::Medium,   category: Category::StructuralInjection },
];
