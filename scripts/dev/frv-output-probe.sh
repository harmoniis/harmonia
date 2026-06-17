#!/usr/bin/env bash
# frv-output-probe.sh — property-based verification of AGENT OUTPUT (QuickChick-style, Vol 4).
#
# The grounding predicate (%output-grounded-p) is SOUND on a given answer: the answer must carry the
# ground-truth value AND that value must be recallable (justified by memory, not fabricated). But the
# agent's output is model-produced, so we can only exercise the predicate over GENERATED cases. This
# is property-based TESTING: it can EXPOSE ungrounded/fabricated output, it cannot prove its absence.
# The result is therefore reported as "property held on N/N SAMPLED cases" — never "verified".
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
N="${1:-3}"
[ -S "$SOCK" ] || "$REPO/scripts/dev/harness.sh" bringup >/dev/null 2>&1

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

echo "════════ AGENT-OUTPUT PROPERTY TEST (QuickChick-style — SAMPLED, not a proof) ════════"
echo "  property: the agent's answer to a recallable fact carries the stored value (grounded, not fabricated)"
pass=0; total=0
for i in $(seq 1 "$N"); do
  total=$((total+1))
  # generate a fresh, non-guessable case
  code="VC$(date +%s)$RANDOM"
  value=$(( 30000 + RANDOM % 60000 ))
  drive "Remember this fact: the verification code $code maps to the number $value." >/dev/null
  ans="$(drive "What number does the verification code $code map to? State just the number.")"
  ansline="$(echo "$ans" | tr '\n' ' ' | cut -c1-60)"
  if echo "$ans" | grep -qw "$value"; then
    echo "  ✓ case $i  code=$code  expected=$value  GROUNDED  (answer: $ansline)"
    pass=$((pass+1))
  else
    echo "  ✗ case $i  code=$code  expected=$value  UNGROUNDED/FABRICATED  (answer: $ansline)"
  fi
done
echo "─────────────────────────────────────────────────────────────────────────────────────"
echo "  property held on $pass/$total SAMPLED cases  —  a sample, NOT a proof of grounding for all inputs"
echo "════════════════════════════════════════════════════════════════════════════════════"
[ "$pass" -eq "$total" ] && exit 0 || exit 1
