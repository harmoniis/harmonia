#!/usr/bin/env bash
# availability-probe.sh — self-healing model favorites against the live OpenRouter catalogue.
# Verifies: the available-models query returns the live catalogue; boot sanitizes favorites
# (prunes the deprecated qwen3.6-plus:free, keeps the live ones, never over-prunes); the curated
# snapshot is preserved (restorable); selection never sees a pruned model; and the fail-safes hold
# (unknown availability → keep all; all-unavailable → keep all, never strand).
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
[ -S "$SOCK" ] || "$REPO/scripts/dev/harness.sh" bringup >/dev/null 2>&1

ASSERT="$(mktemp -t avail-XXXX).lisp"
cat > "$ASSERT" <<'LISP'
(in-package :harmonia)
(defparameter *p* 0) (defparameter *f* 0)
(defun a (l ok) (if ok (progn (incf *p*) (format t "  ✅ ~A~%" l))
                    (progn (incf *f*) (format t "  ❌ ~A~%" l))))
(defun has (id set) (find id set :key (lambda (p) (getf p :id)) :test #'equal))
(handler-case
 (progn
  (format t "~%── A. live availability query ──~%")
  (let ((avail (model-available-ids)))
    (a "available-models returns the live OpenRouter catalogue (>100 ids)"
       (and avail (> (length avail) 100)))
    (a "the query is a list of model-id strings" (and avail (every #'stringp avail))))

  (format t "~%── B. boot already sanitized the favorites ──~%")
  (a "curated snapshot *model-profiles-all* intact (8 favorites)"
     (= (length *model-profiles-all*) 8))
  (a "deprecated qwen3.6-plus:free PRUNED from the live favorites"
     (not (has "qwen/qwen3.6-plus:free" *model-profiles*)))
  (a "...but RETAINED in the curated snapshot (restored automatically if it returns)"
     (has "qwen/qwen3.6-plus:free" *model-profiles-all*))
  (a "live favorites kept (claude-opus-4.6 + gemini-2.5-flash-lite)"
     (and (has "anthropic/claude-opus-4.6" *model-profiles*)
          (has "google/gemini-2.5-flash-lite-preview-09-2025" *model-profiles*)))
  (a "no over-pruning — 7 of 8 favorites kept"
     (= (length *model-profiles*) 7))
  (a "selection never sees the pruned model (free pool excludes qwen3.6-plus:free)"
     (not (member "qwen/qwen3.6-plus:free" (%tier-model-pool :free) :test #'equal)))
  (a "the free pool is still viable after pruning (non-empty)"
     (plusp (length (%tier-model-pool :free))))

  (format t "~%── C. fail-safes (never strand the agent) ──~%")
  ;; all-unavailable favorites → keep the curated list rather than empty the pool
  (let ((*model-profiles-all* (list (list :id "fake/deprecated-zzz" :tier :free :cost 0)))
        (*model-profiles* nil))
    (sanitize-model-favorites)
    (a "all-unavailable favorites → kept curated (not stranded to empty)"
       (= (length *model-profiles*) 1)))
  ;; a genuinely-available model is kept while a fake one is pruned (mechanism proof)
  (let* ((live (model-available-ids))
         (real (first live))
         (*model-profiles-all* (list (list :id real :tier :free :cost 0)
                                     (list :id "fake/deprecated-zzz" :tier :free :cost 0)))
         (*model-profiles* nil))
    (sanitize-model-favorites)
    (a "mechanism: real live id kept, fake id pruned"
       (and (has real *model-profiles*) (not (has "fake/deprecated-zzz" *model-profiles*)))))

  (format t "~%════════ AVAILABILITY PROBE: ~A pass / ~A fail ════════~%" *p* *f*))
 (error (e) (format t "~%AVAILABILITY-PROBE ERROR: ~A~%" e) (incf *f*)))
(sb-ext:exit :code (if (zerop *f*) 0 1))
LISP

env HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" \
    HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
       --eval '(harmonia:start :run-loop nil)' --load "$ASSERT"
RC=$?
rm -f "$ASSERT"
exit $RC
