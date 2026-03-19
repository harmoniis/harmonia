;;; matrix.lisp — Port: constrained routing mesh for all orchestration paths.
;;;
;;; NOTE: The harmonic matrix is not yet wired as an IPC component.
;;; All wrappers return sensible defaults and log warnings until
;;; the Rust harmonic-matrix actor is connected to the IPC dispatch.

(in-package :harmonia)

(defparameter *harmonic-matrix-topology* nil)
(defparameter *harmonic-matrix-seed-config*
  (merge-pathnames "../../config/matrix-topology.sexp" *boot-file*))
(defparameter *harmonic-route-default-signal*
  (or (ignore-errors (let ((*read-eval* nil))
                       (read-from-string (or (config-get-for "harmonic-matrix" "route-signal-default") ""))))
      1.0d0))
(defparameter *harmonic-route-default-noise*
  (or (ignore-errors (let ((*read-eval* nil))
                       (read-from-string (or (config-get-for "harmonic-matrix" "route-noise-default") ""))))
      0.1d0))

(defun %matrix-state-root ()
  (or (config-get-for "harmonic-matrix" "state-root" "global")
      (%tmpdir-state-root)))

;;; --- IPC-based matrix operations ---
;;; When the matrix IPC component is wired, these will call through IPC.
;;; For now, they call IPC and gracefully degrade on failure.

(defun %hm-ipc-call (op &optional extra)
  "Call the harmonic-matrix IPC component. Returns reply or nil."
  (let ((cmd (if extra
                 (format nil "(:component \"harmonic-matrix\" :op \"~A\" ~A)" op extra)
                 (format nil "(:component \"harmonic-matrix\" :op \"~A\")" op))))
    (ipc-call cmd)))

(defun harmonic-matrix-last-error ()
  (or (ipc-extract-value (%hm-ipc-call "last-error")) ""))

(defun %harmonic-matrix-check (reply op)
  (when (ipc-reply-error-p reply)
    (error "harmonic-matrix ~A failed: ~A" op (or reply "")))
  t)

(defun harmonic-matrix-register-node (node kind)
  (%harmonic-matrix-check
   (%hm-ipc-call "register-node"
     (format nil ":node \"~A\" :kind \"~A\""
             (sexp-escape-lisp node) (sexp-escape-lisp kind)))
   "register-node"))

(defun harmonic-matrix-set-store (kind &optional (path ""))
  (%harmonic-matrix-check
   (%hm-ipc-call "set-store"
     (format nil ":kind \"~A\" :path \"~A\""
             (sexp-escape-lisp kind) (sexp-escape-lisp path)))
   "set-store"))

(defun harmonic-matrix-store-config ()
  (or (ipc-extract-value (%hm-ipc-call "get-store")) ""))

(defun harmonic-matrix-set-tool-enabled (tool-id enabled)
  (%harmonic-matrix-check
   (%hm-ipc-call "set-tool-enabled"
     (format nil ":tool-id \"~A\" :enabled ~D"
             (sexp-escape-lisp tool-id) (if enabled 1 0)))
   "set-tool-enabled"))

(defun harmonic-matrix-register-edge (from to weight min-harmony)
  (%harmonic-matrix-check
   (%hm-ipc-call "register-edge"
     (format nil ":from \"~A\" :to \"~A\" :weight ~F :min-harmony ~F"
             (sexp-escape-lisp from) (sexp-escape-lisp to)
             (coerce weight 'double-float) (coerce min-harmony 'double-float)))
   "register-edge"))

(defun harmonic-matrix-route-defaults ()
  (list :signal *harmonic-route-default-signal* :noise *harmonic-route-default-noise*))

(defun harmonic-matrix-set-route-defaults (&key signal noise)
  (when signal
    (setf *harmonic-route-default-signal* (coerce signal 'double-float)))
  (when noise
    (setf *harmonic-route-default-noise* (coerce noise 'double-float)))
  (harmonic-matrix-route-defaults))

(defun harmonic-matrix-route-allowed-p (from to &key (signal *harmonic-route-default-signal*) (noise *harmonic-route-default-noise*))
  (let ((reply (%hm-ipc-call "route-allowed"
                 (format nil ":from \"~A\" :to \"~A\" :signal ~F :noise ~F"
                         (sexp-escape-lisp from) (sexp-escape-lisp to)
                         (coerce signal 'double-float) (coerce noise 'double-float)))))
    (and reply (ipc-reply-ok-p reply)
         (search ":result 1" reply))))

(defun harmonic-matrix-route-or-error (from to &key (signal *harmonic-route-default-signal*) (noise *harmonic-route-default-noise*))
  (unless (harmonic-matrix-route-allowed-p from to :signal signal :noise noise)
    (error "harmonic-matrix route denied ~A -> ~A: ~A" from to (harmonic-matrix-last-error)))
  t)

(defun harmonic-matrix-route-with-context (from to &key
                                           (signal *harmonic-route-default-signal*)
                                           (noise *harmonic-route-default-noise*)
                                           (security-weight 1.0d0)
                                           (dissonance 0.0d0))
  "Wave 3.2: Security-aware routing with dissonance and security weight."
  (let ((reply (%hm-ipc-call "route-allowed-with-context"
                 (format nil ":from \"~A\" :to \"~A\" :signal ~F :noise ~F :security-weight ~F :dissonance ~F"
                         (sexp-escape-lisp from) (sexp-escape-lisp to)
                         (coerce signal 'double-float) (coerce noise 'double-float)
                         (coerce security-weight 'double-float) (coerce dissonance 'double-float)))))
    (and reply (ipc-reply-ok-p reply)
         (search ":result 1" reply))))

(defun harmonic-matrix-route-with-context-or-error (from to &key
                                                    (signal *harmonic-route-default-signal*)
                                                    (noise *harmonic-route-default-noise*)
                                                    (security-weight 1.0d0)
                                                    (dissonance 0.0d0))
  "Security-aware route check that raises on deny with matrix error context."
  (unless (harmonic-matrix-route-with-context from to
                                              :signal signal
                                              :noise noise
                                              :security-weight security-weight
                                              :dissonance dissonance)
    (error "harmonic-matrix route denied ~A -> ~A: ~A" from to (harmonic-matrix-last-error)))
  t)

(defun harmonic-matrix-observe-route (from to success latency-ms &optional (cost-usd 0.0d0))
  (%harmonic-matrix-check
   (%hm-ipc-call "observe-route"
     (format nil ":from \"~A\" :to \"~A\" :success ~D :latency-ms ~D :cost-usd ~F"
             (sexp-escape-lisp from) (sexp-escape-lisp to)
             (if success 1 0) (max 0 latency-ms)
             (coerce cost-usd 'double-float)))
   "observe-route"))

(defun harmonic-matrix-log-event (component direction channel payload success &optional (error-text ""))
  (%harmonic-matrix-check
   (%hm-ipc-call "log-event"
     (format nil ":component \"~A\" :direction \"~A\" :channel \"~A\" :payload \"~A\" :success ~D :error \"~A\""
             (sexp-escape-lisp component) (sexp-escape-lisp direction)
             (sexp-escape-lisp channel) (sexp-escape-lisp (or payload ""))
             (if success 1 0) (sexp-escape-lisp (or error-text ""))))
   "log-event"))

(defun harmonic-matrix-route-timeseries (from to &optional (limit 100))
  (or (ipc-extract-value
       (%hm-ipc-call "route-timeseries"
         (format nil ":from \"~A\" :to \"~A\" :limit ~D"
                 (sexp-escape-lisp from) (sexp-escape-lisp to) (max 1 limit))))
      ""))

(defun harmonic-matrix-time-report (&optional (since-unix 0))
  (or (ipc-extract-value
       (%hm-ipc-call "time-report"
         (format nil ":since-unix ~D" (max 0 since-unix))))
      ""))

(defun harmonic-matrix-report ()
  (or (ipc-extract-value (%hm-ipc-call "report")) ""))

;;; --- Topology management (pure Lisp, unchanged) ---

(defun %matrix-topology-path ()
  (or (config-get-for "harmonic-matrix" "topology-path")
      (concatenate 'string (%matrix-state-root) "/matrix-topology.sexp")))

(defun %default-matrix-topology ()
  (unless (probe-file *harmonic-matrix-seed-config*)
    (error "matrix seed config missing: ~A" *harmonic-matrix-seed-config*))
  (with-open-file (in *harmonic-matrix-seed-config* :direction :input)
    (let ((*read-eval* nil))
      (let ((value (read in nil :eof)))
        (if (eq value :eof)
            (error "empty matrix seed config: ~A" *harmonic-matrix-seed-config*)
            value)))))

(defun %matrix-copy-topology (topology)
  (let ((*read-eval* nil))
    (read-from-string (with-output-to-string (s) (prin1 topology s)))))

(defun harmonic-matrix-current-topology ()
  (%matrix-copy-topology (or *harmonic-matrix-topology* (%default-matrix-topology))))

(defun harmonic-matrix-save-topology (&optional (topology *harmonic-matrix-topology*))
  (let ((path (%matrix-topology-path)))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 (or topology (%default-matrix-topology)) out)
        (terpri out)))
    path))

(defun harmonic-matrix-load-topology ()
  (let ((path (%matrix-topology-path)))
    (if (probe-file path)
        (with-open-file (in path :direction :input)
          (let ((*read-eval* nil))
            (read in nil (%default-matrix-topology))))
        (%default-matrix-topology))))

(defun harmonic-matrix-apply-topology (topology &key (persist nil))
  (%harmonic-matrix-check (%hm-ipc-call "init") "init")
  (dolist (node (getf topology :nodes))
    (harmonic-matrix-register-node (car node) (cdr node)))
  (dolist (edge (getf topology :edges))
    (destructuring-bind (from to weight min-harmony) edge
      (harmonic-matrix-register-edge from to weight min-harmony)))
  (dolist (tool (getf topology :tools))
    (harmonic-matrix-set-tool-enabled (car tool) (cdr tool)))
  (setf *harmonic-matrix-topology* (%matrix-copy-topology topology))
  (when persist
    (harmonic-matrix-save-topology *harmonic-matrix-topology*))
  t)

(defun harmonic-matrix-reset-defaults (&key (persist t))
  (harmonic-matrix-apply-topology (%default-matrix-topology) :persist persist))

(defun %topology-upsert-node (topology node-id kind)
  (let* ((nodes (remove node-id (copy-list (getf topology :nodes)) :key #'car :test #'string=)))
    (push (cons node-id kind) nodes)
    (setf (getf topology :nodes) nodes)
    topology))

(defun %topology-upsert-edge (topology from to weight min-harmony)
  (let* ((edges (remove-if (lambda (e)
                             (and (string= from (first e))
                                  (string= to (second e))))
                           (copy-list (getf topology :edges)))))
    (push (list from to weight min-harmony) edges)
    (setf (getf topology :edges) edges)
    topology))

(defun %topology-upsert-tool (topology tool-id enabled)
  (let* ((tools (remove tool-id (copy-list (getf topology :tools)) :key #'car :test #'string=)))
    (push (cons tool-id (and enabled t)) tools)
    (setf (getf topology :tools) tools)
    topology))

(defun harmonic-matrix-set-node (node-id kind &key (persist t))
  (let ((next (%topology-upsert-node (harmonic-matrix-current-topology) node-id kind)))
    (harmonic-matrix-apply-topology next :persist persist)))

(defun harmonic-matrix-set-edge (from to weight min-harmony &key (persist t))
  (let ((next (%topology-upsert-edge (harmonic-matrix-current-topology) from to weight min-harmony)))
    (harmonic-matrix-apply-topology next :persist persist)))

(defun harmonic-matrix-set-tool (tool-id enabled &key (persist t))
  (let ((next (%topology-upsert-tool (harmonic-matrix-current-topology) tool-id enabled)))
    (harmonic-matrix-apply-topology next :persist persist)))

(defun harmonic-matrix-route-check (from to &key (signal *harmonic-route-default-signal*) (noise *harmonic-route-default-noise*))
  (list :from from
        :to to
        :signal signal
        :noise noise
        :allowed (harmonic-matrix-route-allowed-p from to :signal signal :noise noise)
        :error (harmonic-matrix-last-error)))

(defun init-matrix-port ()
  (%harmonic-matrix-check (%hm-ipc-call "init") "init")
  (runtime-log *runtime* :harmonic-matrix-init (list :status :ok))
  t)

(defun bootstrap-harmonic-matrix ()
  (init-matrix-port)
  (harmonic-matrix-apply-topology (harmonic-matrix-load-topology) :persist nil)
  t)
