# harmonia-phoenix

## Purpose

Supervisor process (PID 1) that keeps the Harmonia agent alive. Runs a child command with automatic restart on crash, writes trauma logs, and emits heartbeats. This is a binary crate, not a library.

## Binary

`phoenix` -- standalone supervisor executable.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_ENV` | `test` | Environment mode (`test`/`prod`) |
| `HARMONIA_ALLOW_PROD_GENESIS` | `false` | Must be `1` to start in prod mode |
| `HARMONIA_RECOVERY_LOG` | `$STATE_ROOT/recovery.log` | Recovery event log |
| `PHOENIX_CHILD_CMD` | -- | Shell command to supervise |
| `PHOENIX_MAX_RESTARTS` | `3` | Max restart attempts before giving up |
| `PHOENIX_TRAUMA_LOG` | `$STATE_ROOT/trauma.log` | Crash/restart event log |

## CLI Usage

```bash
# Start with 10-second heartbeat, supervising the Lisp agent
PHOENIX_CHILD_CMD="sbcl --script agent.lisp" phoenix 10
```

## Behavior

1. Refuses to start in `prod` mode without explicit `HARMONIA_ALLOW_PROD_GENESIS=1`.
2. If `PHOENIX_CHILD_CMD` is set, runs it up to `PHOENIX_MAX_RESTARTS+1` times.
3. On child failure, logs to both trauma.log and recovery.log with timestamps.
4. If no child command, enters infinite heartbeat loop (used as keep-alive).

## Self-Improvement Notes

- Phoenix is the outermost process; it must be the simplest and most reliable component.
- No dependencies on other Harmonia crates (zero `[dependencies]`).
- Trauma log format: free-form text. Recovery log format: `<unix_ts>\tphoenix/restart\t<detail>`.
- To add health-check integration: poll `harmonia_gateway_healthcheck()` via a small helper binary.
- To add graceful shutdown: trap SIGTERM and forward to child process.
