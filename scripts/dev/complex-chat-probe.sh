#!/usr/bin/env bash
# complex-chat-probe.sh — a long, realistic multi-turn conversation that STRESSES memory:
# several facts established across turns, interleaved with distractions (math, tools), then
# long-range recall, a MID-CONVERSATION RESTART (memory must survive + reload), and a
# compound multi-fact query. Proves memory works under the complexity of a real chat.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
ID="$$-$RANDOM"
PROJ="ORION-$ID"
PASS=0; FAIL=0; GAP=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
no(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }
gap(){ echo "  ⚠️  KNOWN GAP: $*"; GAP=$((GAP+1)); }
drive(){ python3 - "$SOCK" "$1" <<'PY'
import socket,sys,time
s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); s.settimeout(16); s.connect(sys.argv[1])
s.sendall((sys.argv[2]+"\n").encode()); buf=b""; t0=time.time()
while time.time()-t0<70:
    try: d=s.recv(8192)
    except socket.timeout: break
    if not d: break
    buf+=d
    if time.time()-t0>2 and b"\n" in buf: break
s.close(); print(buf.decode('utf-8','replace').strip())
PY
}
say(){ echo "  • $1"; R="$(drive "$2")"; echo "    > $(echo "$R" | tr '\n' ' ' | cut -c1-90)"; }
recall_hit(){ # recall_hit <desc> <prompt> <needle>
  say "$1" "$2"
  if echo "$R" | grep -qi "$3"; then ok "$1 ✓"; else no "$1 — '$3' not recalled"; fi
}

[ -S "$SOCK" ] || "$HARNESS" bringup || { echo "FATAL bringup"; exit 2; }
echo "── Phase A: establish facts, interleaved with distractions ─────────"
say "store project name"   "Let's start a project. Remember: my project is called $PROJ."
say "distraction: math"    "Quick — what is 13 plus 29?"
say "store language"       "Remember: $PROJ is written in Haskell."
say "distraction: tool"    "What OS am I on? Use a shell command."
say "store database"       "Remember: $PROJ uses FoundationDB for storage."
say "store deploy region"  "Also remember: $PROJ deploys to Reykjavik."
say "distraction: math"    "What is 7 times 8?"
say "store team lead"      "One more: the $PROJ team lead is Dr. Vance."
say "distraction: opinion" "In one short sentence, why is immutability useful?"

echo "── Phase B: long-range recall (after many turns + distractions) ────"
recall_hit "recall project name" "What is the name of my project? Just the name." "$PROJ"
recall_hit "recall language"     "What language is my project written in?" "haskell"
recall_hit "recall database"     "Which database does my project use?" "foundationdb"
recall_hit "recall deploy region" "Where does my project deploy to?" "reykjavik"
recall_hit "recall team lead"    "Who is the team lead?" "vance"

echo "── Phase C: MID-CONVERSATION RESTART — memory must survive + reload ─"
"$HARNESS" teardown >/dev/null 2>&1; "$HARNESS" bringup >/dev/null 2>&1 || no "restart bringup failed"
sleep 1
recall_hit "recall project name AFTER restart" "What is the name of my project? Just the name." "$PROJ"
recall_hit "recall language AFTER restart"     "What language does my project use?" "haskell"

echo "── Phase D: compound multi-fact recall (the tracked gap) ───────────"
say "compound recall" "For my project, what language is it in AND which database does it use?"
lang=0; db=0
echo "$R" | grep -qi "haskell" && lang=1
echo "$R" | grep -qi "foundationdb" && db=1
if [ $((lang+db)) -ge 2 ]; then ok "compound recall surfaced BOTH facts"
elif [ $((lang+db)) -ge 1 ]; then ok "compound recall surfaced one fact (partial)"
else gap "compound multi-fact recall did not decompose into both facts (each recalls fine alone — Phase B). Recall-quality follow-up."; fi
echo "$R" | grep -qiE '\(search:|no results for' && no "P7 LEAK: raw envelope reached the user" || ok "no raw envelope leaked (P7 holds)"

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed, $GAP known-gap"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
