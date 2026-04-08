use std::collections::HashMap;

use ractor::RpcReplyPort;
use serde::Serialize;

// ── Supervisor messages ──────────────────────────────────────────────

pub enum SupervisorMsg {
    /// Synchronous health query from the HTTP server.
    GetHealth(RpcReplyPort<HealthSnapshot>),
    /// A subsystem actor reports its state has changed.
    SubsystemStateChanged { name: String, state: SubsystemState },
    /// Initiate graceful shutdown of all subsystems.
    Shutdown,
}

// ── Subsystem messages ───────────────────────────────────────────────

pub enum SubsystemMsg {
    /// (Re)start the managed OS process.
    Start,
    /// Graceful stop: SIGTERM → timeout → SIGKILL → Stopped.
    Stop { timeout_secs: u64 },
    /// Watcher task reports the OS process exited.
    ProcessExited { exit_code: Option<i32> },
    /// Query current state synchronously. Part of OTP gen_server contract.
    /// Currently the supervisor uses reactive push (SubsystemStateChanged) instead,
    /// but this variant completes the protocol for direct actor queries.
    #[allow(dead_code)] // OTP protocol completeness — handler exists in subsystem.rs
    GetState(RpcReplyPort<SubsystemState>),
}

// ── State enums ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubsystemState {
    Starting,
    Running { pid: u32 },
    Backoff { attempt: u32, next_retry_ms: u64 },
    Stopped,
    Failed { reason: String, attempts: u32 },
}

impl SubsystemState {
    pub fn is_failed(&self) -> bool {
        matches!(self, SubsystemState::Failed { .. })
    }

    pub fn is_starting_or_backoff(&self) -> bool {
        matches!(
            self,
            SubsystemState::Starting | SubsystemState::Backoff { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum DaemonMode {
    Starting,
    Full,
    Degraded { failed: Vec<String> },
    CoreOnly,
}

// ── Restart policy ───────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

// ── Health snapshot (returned as JSON) ───────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct HealthSnapshot {
    pub mode: DaemonMode,
    pub uptime_secs: u64,
    pub subsystems: HashMap<String, SubsystemState>,
}
