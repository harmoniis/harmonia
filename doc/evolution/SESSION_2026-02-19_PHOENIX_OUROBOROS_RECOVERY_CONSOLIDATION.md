# Session 2026-02-19: Phoenix/Ouroboros/Recovery Consolidation

## Role split (consolidated)

- `Phoenix` (`lib/core/phoenix`) is the supervisor: detects child failure, restarts, and records restart trauma.
- `Recovery` (`lib/core/recovery`) is the canonical crash ledger and recovery-state substrate.
- `Ouroboros` (`lib/core/ouroboros`) is the repair engine: it records crash context into `Recovery` and consumes that ledger for history/last-crash context.

No duplicate crash ledgers should exist across components.

## Code-level consolidation

- `Ouroboros` crash events now append to recovery log (`HARMONIA_RECOVERY_LOG` / `${HARMONIA_STATE_ROOT}/recovery.log`) with kind `ouroboros/<component>`.
- `Ouroboros` `last_crash` and `history` read from recovery log (single source of truth).
- `Phoenix` trauma append now also mirrors to recovery log with kind `phoenix/restart`.
- Public C ABI names remain stable.

## Intensive test evidence

## `./scripts/grind-harmonia-test.sh`

Pass:
- workspace test/build lanes
- prod-genesis guard
- phoenix supervised restart lane
- CFFI grind:
  - `MEMORY=(cycle . 1)`
  - `MQTT=(event . ok)`
  - `GIT_PUSH=OK`
  - `S3_UPLOAD=OK`
  - `OUROBOROS=<ts>\touroboros/openrouter-backend\tsimulated timeout` (kind stored in recovery ledger)

Result: `Grind test complete: all core test-lane systems validated.`

## `./scripts/grind-harmonia-hardproof.sh`

Pass:
- workspace tests
- live OpenRouter (`ONLINE_PROMPT=... "ONLINE_OK"`)
- AWS identity + live S3 upload and verification
- local PGP+TLS MQTT (`MQTT_TLS_PUB_RC=0`, poll success)
- core live checks (`HTTP_OK`, `RECOVERY_TAIL`, `FS_READ`, `BROWSER_TITLE`, `CRON_DUE`)
- real loop self-push and cleanup:
  - `LOOP_PUSH=... "SELF_PUSH_OK ..."`
  - remote branch visible via `ls-remote`
  - branch deletion executed
- harmonic genesis loop:
  - `GENESIS_LOOP_OK ...`
- communication/search/voice smoke + vault persistence check:
  - `COMMS_HC=(1 1 ... 1)`
  - `VAULT_KEYS_OK`

Result: `Hard proof grind complete.`
