//! Actor identity types: ActorId, ActorKind, ActorState.

/// Unique numeric identifier for an actor within the runtime.
pub type ActorId = u64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ActorKind {
    Gateway,
    CliAgent,
    LlmTask,
    Chronicle,
    Tailnet,
    Signalograd,
    Tool,
    Supervisor,
    Observability,
    Router,
    MemoryField,
}

impl ActorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorKind::Gateway => "gateway",
            ActorKind::CliAgent => "cli-agent",
            ActorKind::LlmTask => "llm-task",
            ActorKind::Chronicle => "chronicle",
            ActorKind::Tailnet => "tailnet",
            ActorKind::Signalograd => "signalograd",
            ActorKind::Tool => "tool",
            ActorKind::Supervisor => "supervisor",
            ActorKind::Observability => "observability",
            ActorKind::Router => "router",
            ActorKind::MemoryField => "memory-field",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "gateway" => Ok(ActorKind::Gateway),
            "cli-agent" => Ok(ActorKind::CliAgent),
            "llm-task" => Ok(ActorKind::LlmTask),
            "chronicle" => Ok(ActorKind::Chronicle),
            "tailnet" => Ok(ActorKind::Tailnet),
            "signalograd" => Ok(ActorKind::Signalograd),
            "tool" => Ok(ActorKind::Tool),
            "supervisor" => Ok(ActorKind::Supervisor),
            "observability" => Ok(ActorKind::Observability),
            "router" => Ok(ActorKind::Router),
            "memory-field" => Ok(ActorKind::MemoryField),
            _ => Err(format!("unknown actor kind: {}", s)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActorState {
    Starting,
    Running,
    Idle,
    Completed,
    Failed(String),
    Terminated,
}

impl ActorState {
    pub fn as_str(&self) -> &str {
        match self {
            ActorState::Starting => "starting",
            ActorState::Running => "running",
            ActorState::Idle => "idle",
            ActorState::Completed => "completed",
            ActorState::Failed(_) => "failed",
            ActorState::Terminated => "terminated",
        }
    }
}
