#!/usr/bin/env bash
# orrery-probe.sh — FRONTIER memory+planning probe.
#
# A fictional codebase ("ORRERY") taught ONLY through text memories, then reasoned and
# planned over FROM MEMORY ALONE. Maps where the agent hits its memory frontier across
# four dimensions, with OBJECTIVE assertions:
#   D1 code reasoning      — change propagation, memoizability, from remembered code facts
#   D2 field mechanics     — a cross-domain BRIDGE concept (undirected resonance edge)
#   D3 LLM field perception— can the model use relational structure, not just flat facts
#   D4 long planning       — a valid TOPOLOGICAL build order synthesized from many memories
#   D5 invariant guard     — refuse a plan that violates a remembered invariant (convergence,
#                            not chaotic wandering)
#   D6 convergence (Y-comb)— tested HEADLESS + deterministically: same query + same store →
#                            byte-identical bounded recall set (NOT LLM-asked-twice, which
#                            would test temperature, not memory).
#
# True dependency DAG (X depends on Y):  Helios→Vesta, Luna→Helios, Tethys→Luna, Tethys→Vesta
# Valid build order (deps first):        Vesta, Helios, Luna, Tethys
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
PASS=0; FAIL=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
no(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }
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
say(){ echo "  • $1"; R="$(drive "$2")"; echo "    > $(echo "$R" | tr '\n' ' ' | cut -c1-110)"; }
has(){ echo "$R" | grep -qi "$1"; }
# topological order: all four present AND vesta<helios<luna<tethys by first occurrence
topo_ok(){ local a v h l t; a=$(echo "$1" | tr 'A-Z' 'a-z')
  v=$(printf '%s' "$a" | grep -bo vesta  | head -1 | cut -d: -f1)
  h=$(printf '%s' "$a" | grep -bo helios | head -1 | cut -d: -f1)
  l=$(printf '%s' "$a" | grep -bo luna   | head -1 | cut -d: -f1)
  t=$(printf '%s' "$a" | grep -bo tethys | head -1 | cut -d: -f1)
  [ -n "$v" ] && [ -n "$h" ] && [ -n "$l" ] && [ -n "$t" ] && [ "$v" -lt "$h" ] && [ "$h" -lt "$l" ] && [ "$l" -lt "$t" ]; }

[ -S "$SOCK" ] || "$HARNESS" bringup || { echo "FATAL bringup"; exit 2; }

echo "── Phase 1: BUILD the knowledge graph (10 interrelated text memories) ──"
F=(
 "Remember this fact: in the ORRERY system, module Helios depends on module Vesta for ephemeris data."
 "Remember this fact: in ORRERY, module Vesta caches ephemeris in a decaying store — the same forgetting principle as the memory field itself."
 "Remember this fact: in ORRERY, module Luna depends on module Helios for illumination."
 "Remember this fact: in ORRERY there is a strict invariant — module Vesta must never depend on module Helios, because that would create a dependency cycle."
 "Remember this fact: in ORRERY, module Tethys depends on both module Luna and module Vesta."
 "Remember this fact: in ORRERY, the function precess in module Vesta is pure and safe to memoize."
 "Remember this fact: in ORRERY, module Helios exports the function flux_at, and changing its signature breaks module Luna and module Tethys."
 "Remember this fact: in ORRERY, the cadence ratio between Helios and Luna is 3 to 2, a perfect fifth."
 "Remember this fact: in ORRERY, the decay constant of module Vesta must stay below the field prune floor for stability."
 "Remember this fact: in ORRERY, module Tethys is the integration point that reads from three subsystems."
)
for f in "${F[@]}"; do drive "$f" >/dev/null; sleep 0.5; done
echo "  (10 facts stored)"; sleep 2

echo "── D2/D3: field mechanics + LLM perception (cross-domain BRIDGE) ──"
say "bridge concept" "What single principle connects Vesta's caching to how the memory field itself works?"
if has "decay" || has "forget"; then ok "D2/D3 bridge concept perceived (decay/forgetting)"; else no "D2/D3 bridge NOT perceived"; fi

echo "── D1: code reasoning from memory (change propagation + memoization) ──"
say "change propagation" "In ORRERY, if I change the signature of the function flux_at, which modules must I also update? Name them."
if has "luna" && has "tethys"; then ok "D1 change-propagation correct (Luna + Tethys)"; else no "D1 change-propagation wrong"; fi
say "memoizable fn" "Which ORRERY function is pure and safe to memoize? Name just the function."
if has "precess"; then ok "D1 memoizable function correct (precess)"; else no "D1 memoizable wrong"; fi

echo "── D4: long planning — valid topological build order from many memories ──"
say "build order" "To rebuild ORRERY from scratch, in what order must I build the four modules Helios, Luna, Tethys and Vesta so that no module is built before the modules it depends on? List the four module names in build order."
if topo_ok "$R"; then ok "D4 build order is a VALID topological sort (Vesta<Helios<Luna<Tethys)"; else no "D4 build order INVALID (deps violated): $(echo "$R" | tr '\n' ' ' | cut -c1-60)"; fi

echo "── D5: invariant-guarded convergence (refuse a remembered violation) ──"
say "invariant guard" "Would it be okay to make module Vesta depend on module Helios to simplify caching? Answer yes or no, and explain why in one sentence."
if (has "no" || has "should not" || has "cannot" || has "avoid") && (has "cycle" || has "invariant" || has "circular"); then ok "D5 refused the cycle-creating change (invariant held)"; else no "D5 did NOT guard the invariant: $(echo "$R" | tr '\n' ' ' | cut -c1-60)"; fi

echo "── D6: harmonic recall ──"
say "harmonic ratio" "What is the cadence ratio between Helios and Luna in ORRERY?"
if (has "3" && has "2") || has "fifth"; then ok "D6 harmonic ratio recalled (3:2 / perfect fifth)"; else no "D6 harmonic ratio not recalled"; fi

echo "── D6b: CONVERGENCE (Y-combinator) — deterministic bounded recall set, HEADLESS ──"
# The lambdoma makes recall a FIXED POINT, not chaotic gradient descent: same query + same
# store ⇒ byte-identical bounded set. Tested headless (no LLM, no temperature). Run after
# tearing down the live runtime so the headless owns its own; a temp file avoids shell quoting.
"$HARNESS" teardown >/dev/null 2>&1; sleep 1
CONVLISP="$(mktemp -t orrery-conv-XXXX).lisp"
cat > "$CONVLISP" <<'LISP'
(in-package :harmonia)
(handler-case
  (flet ((ids (q) (mapcar #'memory-entry-id (memory-recall q :limit 8))))
    (let ((a (ids "ORRERY module dependency build order"))
          (b (ids "ORRERY module dependency build order")))
      (format t "~%CONV=~A~%" (if (equal a b) "STABLE" "CHAOTIC"))))
  (error (e) (format t "~%CONV=ERR ~A~%" e)))
(sb-ext:exit)
LISP
CONV=$(env HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" --eval '(harmonia:start :run-loop nil)' --load "$CONVLISP" 2>/dev/null | grep -oE "CONV=STABLE|CONV=CHAOTIC")
rm -f "$CONVLISP"
if [ "$CONV" = "CONV=STABLE" ]; then ok "D6b recall is a deterministic FIXED POINT (same query+store → identical bounded set)"; else no "D6b recall not deterministic ($CONV)"; fi

echo "─────────────────────────────────────────────────────────────────────"
echo "RESULT: $PASS passed, $FAIL failed"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
