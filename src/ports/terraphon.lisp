;;; terraphon.lisp — Port: Platform-specific datamining tools via IPC.
;;;
;;; Terraphon is the agent's sensory system — actors that know how to
;;; extract data from each environment. Datamining is a skill, not a stockpile.
;;;
;;; All datamining goes through policy-gate. Results are ephemeral.
;;;
;;; HOMOICONIC: all IPC commands are built as Lisp LISTS, then serialized
;;; with %sexp-to-ipc-string. S-expressions are law — no format strings.

(in-package :harmonia)

(defparameter *terraphon-ready* nil)

;;; ─── Port lifecycle ─────────────────────────────────────────────────
;;; Reply parsing uses the shared %parse-port-reply from mempalace.lisp.

(defun terraphon-port-ready-p ()
  *terraphon-ready*)

(defun init-terraphon-port ()
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           '(:component "terraphon" :op "init")))))
    (setf *terraphon-ready* (and reply (ipc-reply-ok-p reply)))
    *terraphon-ready*))

;;; ─── Datamining operations ──────────────────────────────────────────

(defun terraphon-datamine (lode-id &rest args)
  "Datamine locally using a specific lode. Goes through policy-gate."
  (when (terraphon-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "terraphon" :op "datamine"
                  :lode-id ,lode-id
                  :args ,(format nil "~{~A~^ ~}" args)))))))

(defun terraphon-datamine-for (&key domain query (prefer "cascade"))
  "Declarative datamining: Terraphon picks the best lode and node."
  (when (terraphon-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "terraphon" :op "plan"
                  :domain ,(or domain "generic")
                  :query ,(or query "")
                  :prefer ,prefer))))))

(defun terraphon-lodes ()
  "List all available datamining tools across all reachable nodes."
  (when (terraphon-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                '(:component "terraphon" :op "catalog"))))))

(defun terraphon-lode-status (lode-id)
  "Check status of a specific datamining lode."
  (when (terraphon-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "terraphon" :op "lode-status"
                  :lode-id ,lode-id))))))

(defun terraphon-stats ()
  "Return rolling-window datamining stats: :samples, :success-rate, :avg-latency-ms."
  (when (terraphon-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                '(:component "terraphon" :op "stats"))))))
