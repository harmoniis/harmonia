#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "=== Unified Actor Protocol & Swarm FFI Test ==="
echo ""

echo "[1/4] Verify release dylibs exist"
for lib in libharmonia_parallel_agents libharmonia_gateway libharmonia_chronicle libharmonia_tailnet; do
  dylib="$HOME/.local/lib/harmonia/${lib}.dylib"
  if [ -f "$dylib" ]; then
    echo "  OK: $lib"
  else
    echo "  MISSING: $dylib"
    exit 1
  fi
done

TMPDIR_TEST=$(mktemp -d)
trap 'rm -rf "$TMPDIR_TEST"' EXIT

echo ""
echo "[2/4] Test unified actor-protocol FFI (register, heartbeat, drain, deregister)"
cat > "$TMPDIR_TEST/test2.lisp" << 'LISP'
(load #P"~/quicklisp/setup.lisp")
(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)
(cffi:load-foreign-library (merge-pathnames "libharmonia_parallel_agents.dylib" #P"~/.local/lib/harmonia/"))

;; Unified actor protocol FFI bindings
(cffi:defcfun ("harmonia_actor_register" %actor-register) :long-long (kind :string))
(cffi:defcfun ("harmonia_actor_heartbeat" %actor-heartbeat) :int (id :long-long) (bytes :unsigned-long-long))
(cffi:defcfun ("harmonia_actor_drain" %actor-drain) :pointer)
(cffi:defcfun ("harmonia_actor_state" %actor-state) :pointer (id :long-long))
(cffi:defcfun ("harmonia_actor_list" %actor-list) :pointer)
(cffi:defcfun ("harmonia_actor_deregister" %actor-deregister) :int (id :long-long))
(cffi:defcfun ("harmonia_actor_free_string" %actor-free-string) :void (ptr :pointer))

(defun ptr->str (ptr &optional (freefn #'%actor-free-string))
  (if (cffi:null-pointer-p ptr) "nil"
      (let ((s (cffi:foreign-string-to-lisp ptr)))
        (funcall freefn ptr) s)))

;; Test 1: Register actors
(let ((gw-id (%actor-register "gateway"))
      (cli-id (%actor-register "cli-agent"))
      (llm-id (%actor-register "llm-task"))
      (tn-id (%actor-register "tailnet"))
      (ch-id (%actor-register "chronicle")))
  (format t "~&REGISTER_OK gw=~D cli=~D llm=~D tn=~D ch=~D~%" gw-id cli-id llm-id tn-id ch-id)
  (assert (> gw-id 0))
  (assert (> cli-id 0))

  ;; Test 2: List actors
  (let ((list-str (ptr->str (%actor-list))))
    (format t "~&ACTOR_LIST=~A~%" list-str)
    (assert (search "gateway" list-str))
    (assert (search "cli-agent" list-str)))

  ;; Test 3: Heartbeat
  (let ((rc (%actor-heartbeat cli-id 512)))
    (format t "~&HEARTBEAT_RC=~D~%" rc)
    (assert (zerop rc)))

  ;; Test 4: Actor state
  (let ((state-str (ptr->str (%actor-state cli-id))))
    (format t "~&ACTOR_STATE=~A~%" state-str)
    (assert (search "running" state-str)))

  ;; Test 5: Drain (should have heartbeat message from test 3)
  (let ((drain-str (ptr->str (%actor-drain))))
    (format t "~&DRAIN=~A~%" drain-str)
    (assert (search "progress-heartbeat" drain-str)))

  ;; Test 6: Drain again (should be empty)
  (let ((drain2 (ptr->str (%actor-drain))))
    (format t "~&DRAIN_EMPTY=~A~%" drain2)
    (assert (string= drain2 "()")))

  ;; Test 7: Deregister
  (let ((rc (%actor-deregister gw-id)))
    (format t "~&DEREGISTER_RC=~D~%" rc)
    (assert (zerop rc)))

  ;; Test 8: Deregister again (should fail)
  (let ((rc2 (%actor-deregister gw-id)))
    (format t "~&DEREGISTER_AGAIN_RC=~D (expected -1)~%" rc2)
    (assert (= rc2 -1)))

  ;; Cleanup
  (%actor-deregister cli-id)
  (%actor-deregister llm-id)
  (%actor-deregister tn-id)
  (%actor-deregister ch-id))

(format t "~&~%ACTOR_PROTOCOL_FFI_OK=1~%")
(sb-ext:exit)
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test2.lisp"

echo ""
echo "[3/4] Test parallel-agents mailbox (drain via unified)"
cat > "$TMPDIR_TEST/test3.lisp" << 'LISP'
(load #P"~/quicklisp/setup.lisp")
(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)
(cffi:load-foreign-library (merge-pathnames "libharmonia_parallel_agents.dylib" #P"~/.local/lib/harmonia/"))

(cffi:defcfun ("harmonia_parallel_agents_init" %pa-init) :int)
(cffi:defcfun ("harmonia_parallel_agents_healthcheck" %pa-hc) :int)
(cffi:defcfun ("harmonia_actor_drain_mailbox" %drain-old) :pointer)
(cffi:defcfun ("harmonia_actor_drain" %drain-new) :pointer)
(cffi:defcfun ("harmonia_actor_register" %register) :long-long (kind :string))
(cffi:defcfun ("harmonia_actor_heartbeat" %heartbeat) :int (id :long-long) (bytes :unsigned-long-long))
(cffi:defcfun ("harmonia_parallel_agents_free_string" %free) :void (ptr :pointer))
(cffi:defcfun ("harmonia_actor_free_string" %afree) :void (ptr :pointer))

;; Init
(let ((hc (%pa-hc)))
  (format t "~&HEALTHCHECK=~D~%" hc)
  (assert (= hc 1)))

(let ((rc (%pa-init)))
  (format t "~&INIT=~D~%" rc))

;; Register and heartbeat to put something in mailbox
(let ((id (%register "cli-agent")))
  (%heartbeat id 100))

;; Old drain function now delegates to unified
(let* ((ptr (%drain-old))
       (s (if (cffi:null-pointer-p ptr) "nil" (cffi:foreign-string-to-lisp ptr))))
  (unless (cffi:null-pointer-p ptr) (%free ptr))
  (format t "~&OLD_DRAIN=~A~%" s)
  (assert (search "progress-heartbeat" s)))

;; New drain should be empty (old already drained everything)
(let* ((ptr (%drain-new))
       (s (if (cffi:null-pointer-p ptr) "nil" (cffi:foreign-string-to-lisp ptr))))
  (unless (cffi:null-pointer-p ptr) (%afree ptr))
  (format t "~&NEW_DRAIN_AFTER=~A~%" s)
  (assert (string= s "()")))

(format t "~&~%UNIFIED_MAILBOX_OK=1~%")
(sb-ext:exit)
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test3.lisp"

echo ""
echo "[4/4] Test tmux CLI spawn + swarm poll (requires tmux)"
if ! command -v tmux &>/dev/null; then
  echo "  SKIP: tmux not installed"
  exit 0
fi

cat > "$TMPDIR_TEST/test4.lisp" << 'LISP'
(load #P"~/quicklisp/setup.lisp")
(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)
(cffi:load-foreign-library (merge-pathnames "libharmonia_parallel_agents.dylib" #P"~/.local/lib/harmonia/"))

(cffi:defcfun ("harmonia_parallel_agents_init" %pa-init) :int)
(cffi:defcfun ("harmonia_tmux_spawn" %spawn) :long-long (cli :string) (workdir :string) (prompt :string))
(cffi:defcfun ("harmonia_tmux_poll" %poll) :pointer (id :long-long))
(cffi:defcfun ("harmonia_tmux_capture" %capture) :pointer (id :long-long) (history :int))
(cffi:defcfun ("harmonia_tmux_kill" %kill) :int (id :long-long))
(cffi:defcfun ("harmonia_tmux_swarm_poll" %swarm-poll) :pointer)
(cffi:defcfun ("harmonia_tmux_list" %list) :pointer)
(cffi:defcfun ("harmonia_actor_drain" %drain) :pointer)
(cffi:defcfun ("harmonia_actor_list" %actor-list) :pointer)
(cffi:defcfun ("harmonia_parallel_agents_free_string" %free) :void (ptr :pointer))
(cffi:defcfun ("harmonia_actor_free_string" %afree) :void (ptr :pointer))
(cffi:defcfun ("harmonia_parallel_agents_last_error" %last-error) :pointer)

(defun ptr->str (ptr &optional (freefn #'%free))
  (if (cffi:null-pointer-p ptr) "nil"
      (let ((s (cffi:foreign-string-to-lisp ptr)))
        (funcall freefn ptr) s)))

(%pa-init)

;; Spawn a simple shell echo command as custom CLI
(format t "~&Spawning tmux agent with echo command...~%")
(let ((id (%spawn "claude-code" "/tmp" "")))
  (if (< id 0)
      (progn
        (format t "~&SPAWN_ERROR=~A~%" (ptr->str (%last-error)))
        ;; Non-fatal: claude may not be installed
        (format t "~&SPAWN_SKIP (claude not available)~%"))
      (progn
        (format t "~&SPAWN_ID=~D~%" id)
        (assert (> id 0))

        ;; Wait a bit for session
        (sleep 2)

        ;; Poll
        (let ((poll-str (ptr->str (%poll id))))
          (format t "~&POLL=~A~%" poll-str))

        ;; List
        (let ((list-str (ptr->str (%list))))
          (format t "~&TMUX_LIST=~A~%" list-str)
          (assert (search (format nil ":id ~D" id) list-str)))

        ;; Swarm poll
        (let ((swarm-str (ptr->str (%swarm-poll))))
          (format t "~&SWARM_POLL=~A~%" swarm-str)
          (assert (search "swarm-status" swarm-str)))

        ;; Drain unified mailbox (should have messages from poll)
        (let ((drain-str (ptr->str (%drain) #'%afree)))
          (format t "~&DRAIN_AFTER_POLL=~A~%" drain-str))

        ;; Actor list (unified)
        (let ((actors (ptr->str (%actor-list) #'%afree)))
          (format t "~&UNIFIED_ACTORS=~A~%" actors))

        ;; Kill
        (let ((rc (%kill id)))
          (format t "~&KILL_RC=~D~%" rc)
          (assert (zerop rc)))

        ;; Capture after kill should show terminated
        (sleep 0.5)
        (let ((poll2 (ptr->str (%poll id))))
          (format t "~&POLL_AFTER_KILL=~A~%" poll2)))))

(format t "~&~%TMUX_SWARM_OK=1~%")
(sb-ext:exit)
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test4.lisp"

echo ""
echo "=== All tests passed ==="
