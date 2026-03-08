# harmonia-parallel-agents

## Purpose

Two-tier parallel execution engine. Tier 1: OpenRouter-based LLM sub-agents dispatched via HTTP. Tier 2: tmux-based CLI agent sessions with full interactive control (spawn, poll, send keys, approve/deny).

## FFI Surface — Tier 1 (OpenRouter Sub-Agents)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_parallel_agents_version` | `() -> *const c_char` | Version string |
| `harmonia_parallel_agents_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_parallel_agents_init` | `() -> i32` | Initialize engine |
| `harmonia_parallel_agents_set_model_price` | `(model: *const c_char, input_cost: f64, output_cost: f64) -> i32` | Set model pricing |
| `harmonia_parallel_agents_submit` | `(prompt: *const c_char, model: *const c_char, priority: i32) -> i64` | Submit task, returns task_id |
| `harmonia_parallel_agents_run_pending` | `(max_parallel: i32) -> i32` | Execute pending tasks |
| `harmonia_parallel_agents_task_result` | `(task_id: i64) -> *mut c_char` | Get result for task |
| `harmonia_parallel_agents_report` | `() -> *mut c_char` | Cost/usage report |
| `harmonia_parallel_agents_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_parallel_agents_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## FFI Surface — Tier 2 (tmux CLI Sessions)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_tmux_spawn` | `(prompt: *const c_char, model: *const c_char) -> i64` | Spawn Claude Code in tmux |
| `harmonia_tmux_spawn_custom` | `(command: *const c_char, working_dir: *const c_char) -> i64` | Spawn custom command in tmux |
| `harmonia_tmux_poll` | `(id: i64) -> *mut c_char` | Poll session output |
| `harmonia_tmux_send` | `(id: i64, input: *const c_char) -> i32` | Send text input |
| `harmonia_tmux_send_key` | `(id: i64, key: *const c_char) -> i32` | Send tmux key (Enter, Tab, etc.) |
| `harmonia_tmux_approve` | `(id: i64) -> i32` | Approve pending action (sends "y") |
| `harmonia_tmux_deny` | `(id: i64) -> i32` | Deny pending action (sends "n") |
| `harmonia_tmux_confirm_yes` | `(id: i64) -> i32` | Confirm yes |
| `harmonia_tmux_confirm_no` | `(id: i64) -> i32` | Confirm no |
| `harmonia_tmux_select` | `(id: i64, index: i32) -> i32` | Select option by index |
| `harmonia_tmux_capture` | `(id: i64, history_lines: i32) -> *mut c_char` | Capture pane output |
| `harmonia_tmux_status` | `(id: i64) -> *mut c_char` | Session status |
| `harmonia_tmux_kill` | `(id: i64) -> i32` | Kill session |
| `harmonia_tmux_interrupt` | `(id: i64) -> i32` | Send Ctrl-C |
| `harmonia_tmux_list` | `() -> *mut c_char` | List active sessions |
| `harmonia_tmux_swarm_poll` | `() -> *mut c_char` | Poll all sessions at once |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_PARALLEL_METRICS_LOG` | `$STATE_ROOT/parallel-metrics.log` | Tier 1 metrics log |
| `HARMONIA_TMUX_METRICS_LOG` | `$STATE_ROOT/tmux-metrics.log` | Tier 2 metrics log |
| `HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS` | `10` | HTTP connect timeout |
| `HARMONIA_OPENROUTER_MAX_TIME_SECS` | `45` | HTTP max time |

## Vault Symbols

- `openrouter` — API key for Tier 1 sub-agent dispatch
- `exa_api_key` — Used by built-in search client
- `brave_api_key` — Used by built-in search client

## Usage from Lisp

```lisp
;; Tier 1: fire-and-forget LLM tasks
(let ((id (cffi:foreign-funcall "harmonia_parallel_agents_submit"
            :string "summarize this" :string "anthropic/claude-sonnet-4" :int 1 :int64)))
  (cffi:foreign-funcall "harmonia_parallel_agents_run_pending" :int 4 :int)
  (cffi:foreign-funcall "harmonia_parallel_agents_task_result" :int64 id :pointer))

;; Tier 2: interactive CLI session
(let ((id (cffi:foreign-funcall "harmonia_tmux_spawn"
            :string "fix the tests" :string "" :int64)))
  (cffi:foreign-funcall "harmonia_tmux_poll" :int64 id :pointer))
```

## Self-Improvement Notes

- Tier 1 uses `engine/clients.rs` for HTTP dispatch with automatic fallback across models.
- Tier 2 tmux sessions are real terminal sessions; the agent can drive any CLI tool interactively.
- `swarm_poll` is key for the orchestrator: it returns all session outputs in one call.
- Cost tracking is per-model with configurable pricing via `set_model_price`.
- To add new execution tiers: follow the pattern in `engine/mod.rs` and add FFI wrappers.
