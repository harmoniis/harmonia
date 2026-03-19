use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct Task {
    pub(crate) id: u64,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) status: String,
    pub(crate) response: String,
    pub(crate) error: String,
    pub(crate) latency_ms: u64,
    pub(crate) cost_usd: f64,
    pub(crate) success: bool,
    pub(crate) verified: bool,
    pub(crate) verification_source: String,
    pub(crate) verification_detail: String,
    #[allow(dead_code)]
    pub(crate) created_at: u64,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ModelPrice {
    pub(crate) usd_per_1k_input: f64,
    pub(crate) usd_per_1k_output: f64,
}

// ---------------------------------------------------------------------------
// Tmux CLI Agent types — distinct operational tier from OpenRouter agents
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CliType {
    ClaudeCode,
    Codex,
    Custom {
        command: String,
        shell_args: Vec<String>,
    },
}

#[allow(dead_code)]
impl CliType {
    pub(crate) fn estimated_cost_per_interaction(&self) -> f64 {
        match self {
            CliType::ClaudeCode => 0.01,
            CliType::Codex => 0.005,
            CliType::Custom { .. } => 0.0,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            CliType::ClaudeCode => "claude-code",
            CliType::Codex => "codex",
            CliType::Custom { .. } => "custom",
        }
    }

    /// Returns the command and arguments for launching this CLI.
    ///
    /// By default, agents launch in maximum-autonomy mode to minimize
    /// the number of permission prompts the conductor must handle:
    /// - Claude Code: `--dangerously-skip-permissions` bypasses all tool approvals
    /// - Codex: `--full-auto` enables automatic approval with workspace-write sandbox
    ///
    /// The tmux session environment is pre-sanitized (CLAUDECODE unset)
    /// by session::create_session, so nesting protection is disabled.
    pub(crate) fn launch_command(&self) -> (&str, Vec<String>) {
        match self {
            CliType::ClaudeCode => ("claude", vec!["--dangerously-skip-permissions".to_string()]),
            CliType::Codex => ("codex", vec!["--full-auto".to_string()]),
            CliType::Custom {
                command,
                shell_args,
            } => (command.as_str(), shell_args.clone()),
        }
    }

    /// Returns launch args for non-interactive (print-and-exit) mode.
    /// Used when the prompt is known upfront and no interactive session is needed.
    pub(crate) fn launch_command_noninteractive(&self, prompt: &str) -> (String, Vec<String>) {
        match self {
            CliType::ClaudeCode => (
                "claude".to_string(),
                vec![
                    "--dangerously-skip-permissions".to_string(),
                    "-p".to_string(),
                    prompt.to_string(),
                ],
            ),
            CliType::Codex => (
                "codex".to_string(),
                vec![
                    "exec".to_string(),
                    "--full-auto".to_string(),
                    prompt.to_string(),
                ],
            ),
            CliType::Custom {
                command,
                shell_args,
            } => {
                let mut args = shell_args.clone();
                args.push(prompt.to_string());
                (command.clone(), args)
            }
        }
    }

    pub(crate) fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "claude-code" | "claude" | "cc" => Ok(CliType::ClaudeCode),
            "codex" => Ok(CliType::Codex),
            _ => Err(format!(
                "unknown cli type: {s} (use claude-code, codex, or spawn_custom)"
            )),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CliState {
    /// Session created, CLI launching
    Launching,
    /// CLI is idle, waiting for user to type a prompt
    WaitingForInput,
    /// CLI is actively processing a request
    Processing,
    /// CLI is asking for permission (tool approval, file write, etc.)
    WaitingForPermission {
        tool_name: String,
        description: String,
    },
    /// CLI is asking a yes/no confirmation
    WaitingForConfirmation { question: String },
    /// CLI is presenting a selection menu
    WaitingForSelection { options: Vec<String> },
    /// CLI is showing an onboarding/survey/first-run prompt
    Onboarding,
    /// CLI is in plan mode — showing a plan for accept/reject
    PlanMode,
    /// CLI has finished its task
    Completed,
    /// CLI has encountered an error
    Error(String),
    /// Session was terminated
    Terminated,
}

#[allow(dead_code)]
impl CliState {
    pub(crate) fn to_sexp(&self) -> String {
        match self {
            CliState::Launching => ":launching".to_string(),
            CliState::WaitingForInput => ":waiting-for-input".to_string(),
            CliState::Processing => ":processing".to_string(),
            CliState::WaitingForPermission {
                tool_name,
                description,
            } => {
                format!(
                    "(:waiting-for-permission :tool \"{}\" :description \"{}\")",
                    sexp_escape(tool_name),
                    sexp_escape(description)
                )
            }
            CliState::WaitingForConfirmation { question } => {
                format!(
                    "(:waiting-for-confirmation :question \"{}\")",
                    sexp_escape(question)
                )
            }
            CliState::WaitingForSelection { options } => {
                let opts: Vec<String> = options
                    .iter()
                    .map(|o| format!("\"{}\"", sexp_escape(o)))
                    .collect();
                format!("(:waiting-for-selection :options ({}))", opts.join(" "))
            }
            CliState::Onboarding => ":onboarding".to_string(),
            CliState::PlanMode => ":plan-mode".to_string(),
            CliState::Completed => ":completed".to_string(),
            CliState::Error(e) => format!("(:error \"{}\")", sexp_escape(e)),
            CliState::Terminated => ":terminated".to_string(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn needs_input(&self) -> bool {
        matches!(
            self,
            CliState::WaitingForInput
                | CliState::WaitingForPermission { .. }
                | CliState::WaitingForConfirmation { .. }
                | CliState::WaitingForSelection { .. }
                | CliState::Onboarding
                | CliState::PlanMode
        )
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct TmuxAgent {
    pub(crate) id: u64,
    pub(crate) cli_type: CliType,
    pub(crate) session_name: String,
    pub(crate) workdir: String,
    #[allow(dead_code)]
    pub(crate) initial_prompt: String,
    pub(crate) state: CliState,
    pub(crate) created_at: u64,
    pub(crate) last_output: String,
    pub(crate) last_poll_at: u64,
    pub(crate) interaction_count: u64,
    pub(crate) total_inputs_sent: u64,
    pub(crate) permissions_approved: u64,
    pub(crate) permissions_denied: u64,
    pub(crate) estimated_cost_usd: f64,
    pub(crate) duration_ms: u64,
}

#[allow(dead_code)]
impl TmuxAgent {
    pub(crate) fn to_sexp(&self) -> String {
        format!(
            concat!(
                "(:id {} :cli-type \"{}\" :session \"{}\" :workdir \"{}\"",
                " :state {} :created-at {} :interactions {}",
                " :inputs-sent {} :approved {} :denied {}",
                " :cost-usd {:.6} :duration-ms {})"
            ),
            self.id,
            self.cli_type.as_str(),
            sexp_escape(&self.session_name),
            sexp_escape(&self.workdir),
            self.state.to_sexp(),
            self.created_at,
            self.interaction_count,
            self.total_inputs_sent,
            self.permissions_approved,
            self.permissions_denied,
            self.estimated_cost_usd,
            self.duration_ms,
        )
    }
}

// ---------------------------------------------------------------------------
// Global state — unified for both OpenRouter tasks and Tmux agents
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Default)]
pub(crate) struct State {
    pub(crate) next_id: u64,
    pub(crate) tasks: HashMap<u64, Task>,
    pub(crate) prices: HashMap<String, ModelPrice>,
    pub(crate) tmux_agents: HashMap<u64, TmuxAgent>,
}

#[allow(dead_code)]
static STATE: OnceLock<RwLock<State>> = OnceLock::new();

#[allow(dead_code)]
pub(crate) fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| {
        RwLock::new(State {
            next_id: 1,
            ..State::default()
        })
    })
}

#[allow(dead_code)]
pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[allow(dead_code)]
pub(crate) fn json_escape(input: &str) -> String {
    // Used for actual JSON contexts (HTTP payloads).
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[allow(dead_code)]
pub(crate) fn sexp_escape(input: &str) -> String {
    // CL's reader handles literal newlines inside strings natively — do NOT
    // escape them. Only backslash and double-quote need escaping.
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[allow(dead_code)]
pub(crate) fn append_tmux_metric_line(agent: &TmuxAgent, event: &str) {
    harmonia_provider_protocol::record_tmux_event(
        agent.id,
        agent.cli_type.as_str(),
        &agent.session_name,
        &agent.workdir,
        event,
        agent.interaction_count,
        agent.total_inputs_sent,
        agent.estimated_cost_usd,
        agent.duration_ms,
    );
}

#[allow(dead_code)]
pub(crate) fn append_metric_line(task: &Task) {
    harmonia_provider_protocol::record_parallel_task(
        task.id,
        &task.model,
        task.latency_ms,
        task.cost_usd,
        task.success,
        task.verified,
        &task.verification_source,
        &task.verification_detail,
    );
}
