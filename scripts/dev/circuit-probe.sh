#!/usr/bin/env bash
# circuit-probe.sh — End-to-end circuit + observability verification for Harmonia.
#
# Inserts probes at every layer and asserts the agent works AND is observed correctly:
#   P1 MEMORY ROUNDTRIP   store a unique sentinel via TUI → recall it via TUI → assert
#                         it round-trips AND physically persists to chronicle.db.
#   P2 TRACE COMPLETENESS the current process's pipeline-trace contains the full
#                         request→response→memory circuit (every expected stage).
#   P3 SCORING HONESTY    across the whole trace history, no error-form eval-result is
#                         ever scored code-ok (post-fix segments must show 0 violations).
#   P4 SUBSYSTEM OBSERV.  chronicle subsystems are recording (memory/harmonic/
#                         signalograd/delegation). Reports the known in-memory gaps.
#   P5 SCORING MATH       headless: good forms → errored=NIL, bad forms → errored=T,
#                         and fluency drops with errors.
#
# Requires a live dev stack: scripts/dev/harness.sh bringup (runtime + sbcl, persistent).
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
TRACE="$DEV/pipeline-trace.jsonl"
CDB="$DEV/chronicle.db"
SENT="ZQ-$$-$RANDOM"   # unique sentinel per run
PASS=0; FAIL=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
no(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }

drive(){ python3 - "$SOCK" "$1" <<'PY'
import socket,sys,time
sock,prompt=sys.argv[1],sys.argv[2]
s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); s.settimeout(3)
s.connect(sock); s.sendall((prompt+"\n").encode())
buf=b""; t0=time.time(); last=time.time()
while time.time()-t0<50:
    try:
        d=s.recv(8192)
        if not d: break
        buf+=d; last=time.time()
    except socket.timeout:
        if buf and time.time()-last>=3: break
s.close(); print(buf.decode('utf-8','replace').strip())
PY
}

[ -S "$SOCK" ] || { echo "FATAL: no TUI socket ($SOCK). Run: scripts/dev/harness.sh bringup"; exit 2; }
LINE0=$(wc -l < "$TRACE" 2>/dev/null || echo 0)

echo "── P1 MEMORY ROUNDTRIP (sentinel=$SENT) ─────────────────────────────"
drive "Remember this exact fact: the probe code is $SENT. Store it and confirm." >/dev/null
RB="$(drive "What is the probe code? Recall it from memory and state the code.")"
echo "  recall reply: $(echo "$RB" | tr '\n' ' ' | cut -c1-100)"
RB_LEN=$(printf '%s' "$RB" | wc -c | tr -d ' ')
if echo "$RB" | grep -q "$SENT" \
   && ! echo "$RB" | grep -q "MEMORY_RECALL:" \
   && ! echo "$RB" | grep -q -- "- \[d[0-9]" \
   && [ "${RB_LEN:-9999}" -le 240 ]; then
  ok "sentinel recalled concisely via TUI"
else
  no "sentinel recall missing, internally framed, or verbose (${RB_LEN:-?} chars)"
fi
DBN=$(sqlite3 "$CDB" "SELECT count(*) FROM memory_entries WHERE content LIKE '%$SENT%';" 2>/dev/null)
[ "${DBN:-0}" -ge 1 ] && ok "sentinel persisted to chronicle.db memory_entries ($DBN rows)" || no "sentinel NOT in chronicle.db"

echo "── P2 TRACE COMPLETENESS (events after line $LINE0) ─────────────────"
python3 -c "
import json
rows=[]
with open('$TRACE') as f:
    for i,l in enumerate(f, start=1):
        if i <= $LINE0 or not l.strip():
            continue
        rows.append(json.loads(l))
seen={d.get('stage') for d in rows}
need=['gateway-ingestion','signal-constructed','repl-enter','model-selection','repl-llm-prompt','llm-call','repl-sexp-generated','sexp-primitive-call','memory-put','memory-auto-store']
miss=[s for s in need if s not in seen]
print('MISS' if miss else 'OK', miss)
" | { read v rest; [ "$v" = OK ] && ok "full circuit traced (all stages present)" || no "missing stages: $rest"; }

echo "── P3 SCORING HONESTY (no error-form scored code-ok; post-fix) ──────"
python3 -c "
import json
rows=[json.loads(l) for l in open('$TRACE') if l.strip()]
st=[i for i,d in enumerate(rows) if d.get('stage')=='boot-complete']
segs=[rows[s:(st[k+1] if k+1<len(st) else len(rows))] for k,s in enumerate(st)] or [rows]
post=segs[-1]; last=None; bad=0
for d in post:
    if d.get('stage') in ('repl-sexp-eval-ok','repl-sexp-eval-fail'): last=d
    if d.get('stage')=='model-perf-update' and last is not None:
        er=str(last.get('eval-result','')).lstrip()
        if any(er.startswith(p) for p in ('(:error','(:parse-error','(:eval-error','(:unknown')) and d.get('outcome')=='code-ok': bad+=1
        last=None
print(bad)
" | { read bad; [ "${bad:-1}" = 0 ] && ok "post-fix segment: 0 error-forms scored code-ok" || no "$bad inverted scores in post-fix segment"; }

echo "── P4 SUBSYSTEM OBSERVABILITY ───────────────────────────────────────"
for t in memory_entries memory_events harmonic_snapshots signalograd_events delegation_log; do
  n=$(sqlite3 "$CDB" "SELECT count(*) FROM $t;" 2>/dev/null)
  [ "${n:-0}" -ge 1 ] && ok "$t recording ($n rows)" || no "$t empty"
done
# Palace (L3) now persists to its OWN on-disk sexp journal and reconciles against
# the chronicle record on boot (entry-id keyed). Assert the journal — not the
# retired chronicle palace tables (which are correctly no longer written).
MJ="$DEV/mempalace"
if [ -f "$MJ/graph.sexp" ]; then
  nnodes=$(grep -c "(:id " "$MJ/graph.sexp" 2>/dev/null || echo 0)
  ndraw=$(find "$MJ/drawers" -name '*.sexp' 2>/dev/null | wc -l | tr -d ' ')
  [ "${nnodes:-0}" -ge 1 ] && ok "palace journal graph.sexp persisted ($nnodes nodes, $ndraw drawers)" \
                          || no "palace journal graph.sexp present but empty"
else
  no "palace on-disk journal missing ($MJ/graph.sexp)"
fi

echo "── P5 SCORING MATH (headless, injected forms) ───────────────────────"
HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
    --eval '(progn
      (let ((bad 0))
        (flet ((e (code) (nth-value 1 (harmonia::%eval-all-forms code))))
          (when (e "(+ 2 2)") (incf bad))                 ;; good must NOT error
          (unless (e "(recall &key limit 3)") (incf bad)) ;; &key must error
          (unless (e "(frobnicate 1 2)") (incf bad)))     ;; unknown must error
        (harmonia::%record-repl-perf "PB" :code-error)
        (harmonia::%record-repl-perf "PB" :code-error)
        (harmonia::%record-repl-perf "PB" :code-ok)
        (let ((fl (harmonia::%repl-fluency "PB")))
          (format t "PROBE5 bad=~A fluency=~,3F~%" bad fl)))
      (sb-ext:exit :code 0))' 2>&1 | grep "PROBE5" \
  | { read _ b f; echo "  $b  $f";
      [ "${b#bad=}" = 0 ] && ok "error detection correct (good/known-bad/unknown)" || no "${b}";
      python3 -c "import sys;v=float('${f#fluency=}');sys.exit(0 if v<0.5 else 1)" && ok "fluency drops with errors (${f})" || no "fluency wrong (${f})"; }

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
