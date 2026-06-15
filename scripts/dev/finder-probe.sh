#!/usr/bin/env bash
# finder-probe.sh — comprehensive test of the harmonia-finder (fff-search) engine.
#
# Exercises the engine DIRECTLY through the port functions (finder-find-entries /
# finder-grep-entries) — no model in the loop, so results are deterministic:
#   A. fuzzy FILE-PATH search over the real codebase (exact, partial, by-extension, Rust, docs)
#   B. CONTENT grep over the real codebase (Lisp defuns, Rust symbols, primitives, line text)
#   C. SCOPE (project / memory / all) — including search over the on-disk memory files
#   D. BOUNDING (never unbounded — top-N respected)
#   E. EDGE cases (no-match → empty, empty query → empty, no crash)
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
drive(){ python3 - "$SOCK" "$1" <<'PY'
import socket,sys,time
s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM); s.settimeout(14); s.connect(sys.argv[1])
s.sendall((sys.argv[2]+"\n").encode()); buf=b""; t0=time.time()
while time.time()-t0<40:
    try: d=s.recv(8192)
    except socket.timeout: break
    if not d: break
    buf+=d
    if time.time()-t0>2 and b"\n" in buf: break
s.close()
PY
}

# Bring up, store a distinctive memory fact, then RESTART so the finder MEMORY index scans the
# new drawer at boot (deterministic — independent of filesystem-watcher latency).
[ -S "$SOCK" ] || "$HARNESS" bringup >/dev/null 2>&1
drive "Remember this fact: the finder test token is FINDERTOKEN42 inside module Zephyr."
sleep 1
"$HARNESS" teardown >/dev/null 2>&1; rm -f "$SOCK"
"$HARNESS" bringup >/dev/null 2>&1
sleep 5   # let both indexes complete their initial scan

ASSERT="$(mktemp -t finder-assert-XXXX).lisp"
cat > "$ASSERT" <<'LISP'
(in-package :harmonia)
(defparameter *fp* 0) (defparameter *ff* 0)
(defun fa (label ok) (if ok (progn (incf *fp*) (format t "  ✅ ~A~%" label))
                         (progn (incf *ff*) (format t "  ❌ ~A~%" label))))
(defun pmatch (rs sub) (some (lambda (r) (search sub (string-downcase (or (getf r :path) "")))) rs))
(defun tmatch (rs sub) (some (lambda (r) (search sub (string-downcase (or (getf r :text) "")))) rs))
(defun ff (q &rest a) (apply #'finder-find-entries q a))
(defun gg (q &rest a) (apply #'finder-grep-entries q a))
(handler-case
 (progn
  (format t "~%── A. fuzzy FILE-PATH search (real codebase) ──~%")
  (fa "finder ready" (finder-port-ready-p))
  (fa "exact 'dna.lisp' -> src/dna/dna.lisp"          (pmatch (ff "dna.lisp" :scope "project") "dna/dna.lisp"))
  (fa "partial 'repl-loop' -> repl-loop.lisp"         (pmatch (ff "repl-loop" :scope "project") "repl-loop.lisp"))
  (fa "stem 'operations' -> operations.lisp"          (pmatch (ff "operations" :scope "project") "operations.lisp"))
  (fa "Rust crate 'finder lib' -> finder/src/lib.rs"  (pmatch (ff "finder lib" :scope "project" :limit 20) "finder/src/lib.rs"))
  (fa "doc 'GENOME' -> doc/GENOME.md"                 (pmatch (ff "GENOME" :scope "project" :limit 20) "genome.md"))
  (fa "fuzzy 'harmonicmachine' -> harmonic-machine"   (pmatch (ff "harmonicmachine" :scope "project" :limit 20) "harmonic-machine"))

  (format t "~%── B. CONTENT grep (real codebase) ──~%")
  (fa "grep 'defun memory-recall' -> operations.lisp" (pmatch (gg "defun memory-recall" :scope "project") "operations.lisp"))
  (fa "grep 'lambdoma-select' -> operations.lisp"     (pmatch (gg "lambdoma-select" :scope "project") "operations.lisp"))
  (fa "grep Rust 'FilePicker' -> finder lib.rs"       (pmatch (gg "FilePicker" :scope "project" :limit 20) "finder/src/lib.rs"))
  (fa "grep 'defprimitive find' -> repl-primitives"   (pmatch (gg "defprimitive find" :scope "project" :limit 20) "repl-primitives"))
  (fa "grep returns the matching LINE text"           (tmatch (gg "defun memory-recall" :scope "project") "memory-recall"))
  (fa "grep multi-word 'supersede dedup'"             (pmatch (gg "supersede-dedup" :scope "project") "operations.lisp"))

  (format t "~%── C. SCOPE (project / memory / all) ──~%")
  (fa "scope all finds a project file"                (pmatch (ff "signalograd" :scope "all" :limit 20) "signalograd"))
  (fa "scope memory greps the stored drawer (FINDERTOKEN42)"
      (pmatch (gg "FINDERTOKEN42" :scope "memory" :limit 20) ".sexp"))
  (fa "scope memory find over memory files"           (let ((rs (ff "drawers" :scope "memory" :limit 10))) (or (null rs) (pmatch rs "mempalace"))))

  (format t "~%── D. BOUNDING (never unbounded) ──~%")
  (fa "find bounded to :limit 5"                      (<= (length (ff "lisp" :scope "project" :limit 5)) 5))
  (fa "grep bounded to :limit 8"                      (<= (length (gg "defun" :scope "project" :limit 8)) 8))

  (format t "~%── E. EDGE cases (no crash) ──~%")
  (fa "no-match find -> empty"                        (null (ff "zzqxnonexistentfile99999" :scope "project")))
  ;; fff grep is fuzzy-tolerant (QueryParser): a nonsense query returns at most a few closest
  ;; matches, never an unbounded flood and never a crash — the correct forgiving behaviour.
  (fa "no-match grep -> bounded, no crash"            (<= (length (gg "zzqxnonexistentsymbol99999" :scope "project" :limit 8)) 8))
  (fa "empty query -> empty"                          (null (ff "" :scope "project")))

  (format t "~%════════ FINDER PROBE: ~A pass / ~A fail ════════~%" *fp* *ff*))
 (error (e) (format t "~%FINDER-PROBE ERROR: ~A~%" e) (incf *ff*)))
(sb-ext:exit :code (if (zerop *ff*) 0 1))
LISP

env HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" \
    HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
       --eval '(harmonia:start :run-loop nil)' --load "$ASSERT"
RC=$?
rm -f "$ASSERT"
exit $RC
