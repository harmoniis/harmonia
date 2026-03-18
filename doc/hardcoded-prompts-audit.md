# Hardcoded Prompts Audit Report

## Migration Status

All tunable (EVOLUTION tier) prompts have been migrated to `config/prompts.sexp` via `load-prompt`. Injection detection patterns are in `config/security-patterns.sexp`. Code retains hardcoded fallback defaults as safety nets.

GENESIS tier prompts (DNA identity, security boundaries) remain correctly hardcoded in source.

## Summary: What Goes Where

### GENESIS (immutable, never changes)

These define identity and security. Must live in `dna.lisp` or hardcoded in security modules:

| Prompt | Current Location | Action |
|--------|-----------------|--------|
| DNA Constitution (rules 1-13) | dna.lisp | **Keep** |
| Mode lines (planner/rewrite/orchestrate) | dna.lisp | **Keep** |
| Internal Runtime Orientation | dna.lisp | **Keep** |
| `*dna*` plist (creator, ethic, vitruvian...) | dna.lisp | **Keep** |
| External Data Boundary Wrapper | conductor.lisp + signal-integrity | **Keep** |
| Browser Security Instruction | browser/security.rs | **Keep** |
| VISIBLE_REPLY_POLICY header | state.lisp | **Keep** |

### EVOLUTION (tunable between versions)

These are operational and may need adjustment. Should live in config files:

| Prompt | Config Location | Status |
|--------|-----------------|--------|
| Personality Anchor | `config/prompts.sexp :personality-anchor` | Done |
| Task Classifier | `config/prompts.sexp :task-classifier` | Done |
| Grok Live Search | `config/prompts.sexp :grok-live-search` | Done |
| Grok Verification | `config/prompts.sexp :grok-verification` | Done |
| Context Summarizer | `config/prompts.sexp :context-summarizer` | Done |
| A2UI Device Instruction | `config/prompts.sexp :a2ui-device-instruction` | Done |
| Presentation Guidance | `config/prompts.sexp :presentation-guidance` | Done |
| Injection Detection Patterns | `config/security-patterns.sexp` | Done |
| System Capabilities | `config/prompts.sexp :system-capabilities` | Done |
| Subagent Context | `config/prompts.sexp :subagent-context` | Done |
| DAG Implementer/Auditor | `config/prompts.sexp :dag-implementer-suffix/:dag-auditor-prefix` | Done |
| Orchestrator Direct Answer | `config/prompts.sexp :orchestrator-direct-answer` | Done |

### MIGRATION STATUS

`config/prompts.sexp` exists and is the canonical source for all EVOLUTION prompts. Code-side fallback defaults remain as safety nets. Subagent prompts (context, DAG implementer/auditor, orchestrator direct answer, system capabilities) are all in `config/prompts.sexp`.

### KEEP IN CODE (structural plumbing)

| Prompt | Reason |
|--------|--------|
| Baseband channel envelope | Serialization format, not behavioral |
| Condensed context marker | Structural marker |
| USER_TASK: marker | Structural separator |
| System context block (dynamic parts) | Reads live runtime state |
