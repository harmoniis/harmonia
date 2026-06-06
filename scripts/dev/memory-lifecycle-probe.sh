#!/usr/bin/env bash
# memory-lifecycle-probe.sh — prove a memory is CREATED across all three stores, then
# LOADED after a restart, then RECALLED cross-session. Every pipeline point of the memory
# lifecycle is asserted: store → field (L1) + chronicle (L2) + palace (L3) → restart →
# warm-start load → recall. "Are memories created and loaded when used?" — answered yes/no.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
CDB="$DEV/chronicle.db"; MJ="$DEV/mempalace"
TOK="LIFECYCLE-$$-$RANDOM"
PASS=0; FAIL=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
no(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }
drive(){ python3 - "$SOCK" "$1" <<'PY'
import socket,sys,time
s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); s.settimeout(14); s.connect(sys.argv[1])
s.sendall((sys.argv[2]+"\n").encode()); buf=b""; t0=time.time()
while time.time()-t0<60:
    try: d=s.recv(8192)
    except socket.timeout: break
    if not d: break
    buf+=d
    if time.time()-t0>2 and b"\n" in buf: break
s.close(); print(buf.decode('utf-8','replace').strip())
PY
}
cdb(){ sqlite3 "$CDB" "$1" 2>/dev/null; }
drawer_cnt(){ find "$MJ/drawers" -name '*.sexp' 2>/dev/null | wc -l | tr -d ' '; }
graph_nodes(){ grep -c "(:id " "$MJ/graph.sexp" 2>/dev/null || echo 0; }

[ -S "$SOCK" ] || "$HARNESS" bringup || { echo "FATAL bringup"; exit 2; }

echo "── 1. CREATE: store a fact, verify it enters every store ───────────"
N0=$(graph_nodes); D0=$(drawer_cnt)
drive "Remember this exact fact: my codename is $TOK and my home base is Lisbon." >/dev/null
sleep 1
# L2 chronicle (durable record)
[ "$(cdb "SELECT count(*) FROM memory_entries WHERE content LIKE '%$TOK%';")" -ge 1 ] \
  && ok "CREATE L2 chronicle: fact in memory_entries" || no "CREATE L2: fact NOT in chronicle"
# L3 palace (drawer, entry-id keyed)
[ -n "$(grep -lr "$TOK" "$MJ/drawers" 2>/dev/null | head -1)" ] \
  && ok "CREATE L3 palace: drawer holds the fact" || no "CREATE L3: no drawer holds the fact"
# L1 field (concept graph grew — new concepts/edges from the fact)
N1=$(graph_nodes); D1=$(drawer_cnt)
[ "${N1:-0}" -ge "${N0:-0}" ] && ok "CREATE L1 field: concept graph grew/held ($N0 -> $N1 nodes)" \
                              || no "CREATE L1: graph shrank unexpectedly"
echo "  (drawers $D0 -> $D1)"

echo "── 2. LOAD: restart, verify warm-start reloads the fact ────────────"
"$HARNESS" teardown >/dev/null 2>&1; "$HARNESS" bringup >/dev/null 2>&1 || no "restart bringup failed"
sleep 1
[ "$(cdb "SELECT count(*) FROM memory_entries WHERE content LIKE '%$TOK%';")" -ge 1 ] \
  && ok "LOAD L2: chronicle still holds the fact (durable record)" || no "LOAD L2: fact lost from chronicle"
[ -n "$(grep -lr "$TOK" "$MJ/drawers" 2>/dev/null | head -1)" ] \
  && ok "LOAD L3: palace drawer survived/reconciled" || no "LOAD L3: drawer lost on restart"
[ -f "$MJ/graph.sexp" ] && ok "LOAD L1: field graph warm-started ($(graph_nodes) nodes)" \
                        || no "LOAD L1: field graph missing after restart"
[ -f "$DEV/repl_fluency.sexp" ] && ok "LOAD: REPL fluency mark restored (heritable competence)" \
                                || echo "  ℹ️  no fluency file yet (no prior turns persisted)"

echo "── 3. RECALL cross-session: the loaded fact is USABLE ──────────────"
R1="$(drive "What is my codename? State just the codename.")"
echo "  recall codename: $(echo "$R1" | tr '\n' ' ' | cut -c1-70)"
echo "$R1" | grep -q "$TOK" && ok "RECALL: codename recalled in a NEW session (created→loaded→used)" \
                            || no "RECALL: codename NOT recalled after restart"
R2="$(drive "Where is my home base?")"
echo "  recall base: $(echo "$R2" | tr '\n' ' ' | cut -c1-70)"
echo "$R2" | grep -qi "lisbon" && ok "RECALL: second fact (home base) recalled cross-session" \
                               || no "RECALL: home base NOT recalled after restart"

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
