#!/usr/bin/env bash
# persistence-probe.sh — P3 guard: palace survives a real restart with IDENTICAL
# structure (no loss) and NO duplication, and reconciles to the chronicle record.
#
# The architecture under test: chronicle (L2) is the durable record; the palace (L3)
# actor warm-starts from its own on-disk sexp journal, then reconciles missing
# chronicle entries by entry-id (idempotent). This probe proves a teardown→bring-up
# cycle neither loses nor doubles palace state, and that a stored fact is still
# recallable afterward.
#
# Drives its own lifecycle via harness.sh. Assumes the release runtime is built.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
MJ="$DEV/mempalace"
SENT="PERSIST-$$-$RANDOM"
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

nodes(){ grep -c "(:id " "$MJ/graph.sexp" 2>/dev/null || echo 0; }
drawers(){ find "$MJ/drawers" -name '*.sexp' 2>/dev/null | wc -l | tr -d ' '; }
sentinel_filed(){ grep -lr "$SENT" "$MJ/drawers" 2>/dev/null | head -1; }

echo "── ensure stack is up ───────────────────────────────────────────────"
[ -S "$SOCK" ] || "$HARNESS" bringup || { echo "FATAL: bringup failed"; exit 2; }

echo "── store sentinel ($SENT) → palace journal ──────────────────────────"
drive "Remember this exact fact: the durable probe code is $SENT. Store it." >/dev/null
sleep 1
N1=$(nodes); D1=$(drawers)
echo "  pre-restart: nodes=$N1 drawers=$D1"
[ -f "$MJ/graph.sexp" ] && ok "palace journal graph.sexp exists" || no "no palace journal"
[ -n "$(sentinel_filed)" ] && ok "sentinel persisted to a drawer (entry-id keyed)" || no "sentinel not in any drawer file"

echo "── RESTART 1: teardown → warm-start ─────────────────────────────────"
"$HARNESS" teardown >/dev/null 2>&1
"$HARNESS" bringup  >/dev/null 2>&1 || { no "bringup after teardown failed"; }
sleep 1
N2=$(nodes); D2=$(drawers)
echo "  post-restart: nodes=$N2 drawers=$D2"
[ "$N2" = "$N1" ] && ok "node count IDENTICAL across restart ($N2)"   || no "node count changed $N1 -> $N2 (loss or duplication)"
[ "$D2" = "$D1" ] && ok "drawer count IDENTICAL across restart ($D2)" || no "drawer count changed $D1 -> $D2 (loss or duplication)"
[ -n "$(sentinel_filed)" ] && ok "sentinel drawer survived restart" || no "sentinel drawer lost on restart"
RB="$(drive "What is the durable probe code? Recall it.")"
echo "  recall: $(echo "$RB" | tr '\n' ' ' | cut -c1-90)"
echo "$RB" | grep -q "$SENT" && ok "sentinel recallable after restart" || no "sentinel NOT recalled after restart"

echo "── RESTART 2: reconcile must be idempotent (file 0, no NEW writes) ───"
# Baseline AFTER the recall's own auto-stored interaction has settled, so this
# isolates reconcile idempotency from new memory writes.
sleep 1
DB=$(drawers); NB=$(nodes)
"$HARNESS" teardown >/dev/null 2>&1
"$HARNESS" bringup  >/dev/null 2>&1 || true
sleep 1
N3=$(nodes); D3=$(drawers)
echo "  pre-2nd-restart: nodes=$NB drawers=$DB ; post: nodes=$N3 drawers=$D3"
[ "$D3" = "$DB" ] && ok "reconcile idempotent: drawer count stable ($D3)" || no "reconcile not idempotent $DB -> $D3"
[ "$N3" = "$NB" ] && ok "reconcile idempotent: node count stable ($N3)"   || no "node count drifted $NB -> $N3"

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
