#!/usr/bin/env bash
# memory-reasoning-probe.sh — MEMORY-REAL multi-step reasoning + self-correction.
# Forces the agent to BUILD memory and RELY on it, and proves the reliance is real (not the
# model doing mental math around dead plumbing):
#   * a NON-DERIVABLE seed (a random token) that can ONLY be recalled, never computed;
#   * the stored INTERMEDIATES (SQ, DIFF) are asserted to actually exist in chronicle;
#   * green requires the chain AND the verified stored intermediates AND the seed recall.
#
#   B=6  -> SQ=B*B=36 -> DIFF=SQ-B=30 -> answer DIFF+SQ = 66
#   CORRECT B=10 -> SQ=100 -> DIFF=90 -> answer DIFF+SQ = 190
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
CDB="$DEV/chronicle.db"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
SEED="7K2Q9X"
PASS=0; FAIL=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
no(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }
# Per-turn window raised to 140s so premium multi-round turns aren't cut off (de-confound).
drive(){ python3 - "$SOCK" "$1" <<'PY'
import socket,sys,time
s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); s.settimeout(25); s.connect(sys.argv[1])
s.sendall((sys.argv[2]+"\n").encode()); buf=b""; t0=time.time()
while time.time()-t0<140:
    try: d=s.recv(8192)
    except socket.timeout: break
    if not d: break
    buf+=d
    if time.time()-t0>2 and b"\n" in buf: break
s.close(); print(buf.decode('utf-8','replace').strip())
PY
}
say(){ echo "  • $1"; R="$(drive "$2")"; echo "    > $(echo "$R" | tr '\n' ' ' | cut -c1-95)"; }
needs_num(){ say "$1" "$2"; if echo "$R" | grep -qw "$3"; then ok "$1 = $3 ✓"; else no "$1 — expected $3, got: $(echo "$R" | tr '\n' ' ' | cut -c1-50)"; fi; }
# chronicle has an entry mentioning NAME and VALUE -> the intermediate was actually STORED.
stored(){ local n="$1" v="$2"; [ "$(sqlite3 "$CDB" "SELECT count(*) FROM memory_entries WHERE content LIKE '%$n%' AND content LIKE '%$v%';" 2>/dev/null)" -ge 1 ]; }

[ -S "$SOCK" ] || "$HARNESS" bringup || { echo "FATAL bringup"; exit 2; }

echo "── Phase 1: BUILD memory (a non-derivable seed + a derived chain) ──"
say "T1 store seed (non-derivable)" "Remember this fact: the secret build seed is $SEED."
say "T2 store base"  "Remember this fact: the value of B is the number 6."
say "T3 compute+store SQ"  "Recall the value of B, multiply it by itself, and store the result as the value of SQ."
say "T4 compute+store DIFF" "Recall the values of SQ and B, subtract B from SQ, and store the result as the value of DIFF."

echo "── Phase 2: RELY on memory (chain) + prove it is MEMORY, not mental math ──"
needs_num "T5 add DIFF+SQ from memory" "Recall the values of DIFF and SQ and add them together. Answer with just the number." 66
say "T6 recall non-derivable seed" "What is the secret build seed? State just the seed."
if echo "$R" | grep -q "$SEED"; then ok "T6 non-derivable seed recalled ($SEED) — recall is REAL, not computed"; else no "T6 seed NOT recalled (memory recall not real)"; fi

echo "── Phase 3: SELF-CORRECTION ──"
say "T7 correction" "Correction: the value of B is now the number 10. Recompute SQ and DIFF and store the new values."
needs_num "T8 add DIFF+SQ after correction" "Recall the new values of DIFF and SQ and add them together. Answer with just the number." 190

echo "── Verify the INTERMEDIATES were actually STORED (anti mental-math) ──"
( stored SQ 36 || stored SQ 100 ) && ok "intermediate SQ was stored to chronicle (36 or 100)" || no "SQ never stored — model did not use memory for SQ"
( stored DIFF 30 || stored DIFF 90 ) && ok "intermediate DIFF was stored to chronicle (30 or 90)" || no "DIFF never stored — model did not use memory for DIFF"

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
