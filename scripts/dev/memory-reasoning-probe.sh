#!/usr/bin/env bash
# memory-reasoning-probe.sh — a SUPER-COMPLEX task that forces the agent to BUILD memory and
# HEAVILY RELY on it. A reasoning chain where each step depends on RECALLING prior stored
# values (the value is NOT in the prompt), so the agent must construct the solution from its
# own memory, step by step (tree of thought over memory). A mid-chain CORRECTION forces it to
# update memory and recompute — self-error-correction. Asserts it reaches the right answers.
#
#   BASE=6  -> SQ=BASE^2=36 -> DIFF=SQ-BASE=30 -> answer DIFF+SQ = 66
#   CORRECT BASE=10 -> SQ=100 -> DIFF=90 -> answer DIFF+SQ = 190
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
PASS=0; FAIL=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
no(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }
drive(){ python3 - "$SOCK" "$1" <<'PY'
import socket,sys,time
s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); s.settimeout(18); s.connect(sys.argv[1])
s.sendall((sys.argv[2]+"\n").encode()); buf=b""; t0=time.time()
while time.time()-t0<80:
    try: d=s.recv(8192)
    except socket.timeout: break
    if not d: break
    buf+=d
    if time.time()-t0>2 and b"\n" in buf: break
s.close(); print(buf.decode('utf-8','replace').strip())
PY
}
say(){ echo "  • $1"; R="$(drive "$2")"; echo "    > $(echo "$R" | tr '\n' ' ' | cut -c1-95)"; }
needs_num(){ # needs_num <desc> <prompt> <expected-number>
  say "$1" "$2"
  if echo "$R" | grep -qw "$3"; then ok "$1 = $3 ✓"; else no "$1 — expected $3, got: $(echo "$R" | tr '\n' ' ' | cut -c1-50)"; fi
}
[ -S "$SOCK" ] || "$HARNESS" bringup || { echo "FATAL bringup"; exit 2; }

echo "── Phase 1: BUILD memory (each step relies on recalling the last) ──"
say "T1 store base"   "Remember this fact: the value of the variable BASE is the number 6."
say "T2 derive SQ"    "Recall the value of BASE from your memory, multiply it by itself, and remember the result as the value of SQ. Tell me the value of SQ."
say "T3 derive DIFF"  "Recall the values of SQ and BASE from your memory, subtract BASE from SQ, and remember the result as the value of DIFF. Tell me the value of DIFF."

echo "── Phase 2: RELY on the built memory to answer ────────────────────"
needs_num "T4 combine from memory" "Recall the values of DIFF and SQ from your memory and add them together. Answer with just the number." 66

echo "── Phase 3: SELF-CORRECTION (update memory, recompute) ────────────"
say "T5 correction"   "Correction: the value of BASE is now the number 10 instead of 6. Recompute the values of SQ and DIFF and remember the new values."
needs_num "T6 recompute after correction" "Recall the new values of DIFF and SQ and add them together. Answer with just the number." 190

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
