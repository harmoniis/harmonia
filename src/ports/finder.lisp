;;; finder.lisp — Port: fff-search fuzzy file + content retrieval via IPC.
;;;
;;; The agent's DEFAULT local search substrate — "looking into files" is the
;;; most important thing an agent does. One fast, persistent, frecency-ranked
;;; index (mmap + filesystem watcher, in the Rust finder actor) serves both
;;; project files and the on-disk memory files. Results are always BOUNDED
;;; (top-N) — never an unbounded list — the retrieval half of "always choose
;;; from a finite matrix of possibilities".
;;;
;;; HOMOICONIC: IPC commands are Lisp LISTS serialized with %sexp-to-ipc-string.

(in-package :harmonia)

(defparameter *finder-ready* nil)

(defun finder-port-ready-p () *finder-ready*)

(defun init-finder-port ()
  "Handshake with the Rust finder actor. The index scans in the background;
readiness here just confirms the actor answers."
  (let ((reply (ipc-call (%sexp-to-ipc-string '(:component "finder" :op "ready")))))
    (setf *finder-ready* (and reply (ipc-reply-ok-p reply)))
    *finder-ready*))

;;; ─── Raw result plists (for memory fusion + rendering) ───────────────

(defun finder-find-entries (query &key (limit 12) (scope "all"))
  "Fuzzy file-path search → list of (:path \"...\") plists, bounded top-N.
SCOPE is \"all\" (project+memory), \"project\", or \"memory\"."
  (when (and (finder-port-ready-p) (stringp query) (plusp (length query)))
    (getf (%parse-port-reply
           (ipc-call (%sexp-to-ipc-string
                      `(:component "finder" :op "find-files"
                        :query ,query :limit ,limit :scope ,scope))))
          :results)))

(defun finder-grep-entries (query &key (limit 20) (scope "all"))
  "Content search → list of (:path :line :def :text) plists, bounded top-N.
SCOPE is \"all\" (project+memory), \"project\", or \"memory\"."
  (when (and (finder-port-ready-p) (stringp query) (plusp (length query)))
    (getf (%parse-port-reply
           (ipc-call (%sexp-to-ipc-string
                      `(:component "finder" :op "grep"
                        :query ,query :limit ,limit :scope ,scope))))
          :results)))

;;; ─── Rendered text (for the REPL primitives the model reads) ─────────

(defun finder-find (query &key (limit 12))
  "Fuzzy file-path search rendered as newline-joined paths."
  (let ((results (finder-find-entries query :limit limit)))
    (if results
        (with-output-to-string (out)
          (dolist (r results)
            (let ((p (getf r :path)))
              (when (and p (stringp p) (plusp (length p)))
                (format out "~A~%" p)))))
        "(no matching files)")))

(defun finder-grep (query &key (limit 20))
  "Content search rendered as path:line: text lines."
  (let ((results (finder-grep-entries query :limit limit)))
    (if results
        (with-output-to-string (out)
          (dolist (r results)
            (format out "~A:~A: ~A~%"
                    (or (getf r :path) "?")
                    (or (getf r :line) "?")
                    (string-trim '(#\Space #\Tab) (or (getf r :text) "")))))
        "(no matches)")))
