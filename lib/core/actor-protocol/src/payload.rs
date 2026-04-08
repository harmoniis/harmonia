//! Message payload variants — the body of every actor message.

#[derive(Clone, Debug)]
pub enum MessagePayload {
    InboundSignal {
        envelope_sexp: String,
    },
    OutboundSignal {
        frontend: String,
        sub_channel: String,
        payload: String,
    },
    TaskCompleted {
        output: String,
        exit_code: i32,
        duration_ms: u64,
    },
    TaskFailed {
        error: String,
        duration_ms: u64,
    },
    ProgressHeartbeat {
        bytes_delta: u64,
    },
    StateChanged {
        to: String,
    },
    MeshInbound {
        from_node: String,
        msg_type: String,
        payload: String,
    },
    RecordAck {
        table: String,
        count: u64,
    },
    ToolInvoked {
        tool_name: String,
        operation: String,
        request_id: u64,
    },
    ToolCompleted {
        tool_name: String,
        operation: String,
        request_id: u64,
        envelope_sexp: String,
        duration_ms: u64,
    },
    ToolFailed {
        tool_name: String,
        operation: String,
        request_id: u64,
        error: String,
        duration_ms: u64,
    },
    Shutdown,
    SupervisionReady {
        task: u64,
        spec: u64,
        taxonomy: String,
        assertions: u32,
    },
    SupervisionVerdict {
        task: u64,
        spec: u64,
        passed: u32,
        failed: u32,
        skipped: u32,
        confidence: f64,
        grade: String,
        summary: String,
    },
    /// User changed routing tier via /auto /eco /premium /free.
    TierChanged {
        tier: String,
    },
    /// Feedback from a completed LLM route for experience tracking.
    RouteFeedback {
        request_id: u64,
        model_id: String,
        task_kind: String,
        tier: String,
        success: bool,
        latency_ms: u64,
        cost_usd_estimate: f64,
        complexity_score: f64,
    },
    /// Cascade escalation: a model failed, try next in chain.
    CascadeEscalate {
        request_id: u64,
        failed_model: String,
        reason: String,
    },
}
