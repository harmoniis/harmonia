;;; harmonic-matrix.lisp — Core constrained routing mesh for all orchestration paths.

(in-package :harmonia)

(defparameter *harmonic-matrix-lib* nil)
(defparameter *harmonic-matrix-topology* nil)
(defparameter *harmonic-matrix-seed-config*
  (merge-pathnames "../../config/matrix-topology.sexp" *boot-file*))

(defun %matrix-state-root ()
  (or (sb-ext:posix-getenv "HARMONIA_STATE_ROOT")
      "/tmp/harmonia"))

(defun %config-number (key env-key fallback)
  (or (ignore-errors
        (let ((v (and (fboundp 'config-get) (config-get key))))
          (when (and v (plusp (length v)))
            (coerce (read-from-string v) 'double-float))))
      (ignore-errors
        (let ((v (sb-ext:posix-getenv env-key)))
          (when (and v (plusp (length v)))
            (coerce (read-from-string v) 'double-float))))
      fallback))

(defun %route-default-signal ()
  (%config-number "matrix.route.signal_default" "HARMONIA_ROUTE_SIGNAL_DEFAULT" 1.0d0))

(defun %route-default-noise ()
  (%config-number "matrix.route.noise_default" "HARMONIA_ROUTE_NOISE_DEFAULT" 0.1d0))

(cffi:defcfun ("harmonia_harmonic_matrix_init" %hm-init) :int)
(cffi:defcfun ("harmonia_harmonic_matrix_set_store" %hm-set-store) :int
  (kind :string) (path :string))
(cffi:defcfun ("harmonia_harmonic_matrix_get_store" %hm-get-store) :pointer)
(cffi:defcfun ("harmonia_harmonic_matrix_register_node" %hm-register-node) :int
  (node-id :string) (kind :string))
(cffi:defcfun ("harmonia_harmonic_matrix_set_tool_enabled" %hm-set-tool-enabled) :int
  (tool-id :string) (enabled :int))
(cffi:defcfun ("harmonia_harmonic_matrix_register_edge" %hm-register-edge) :int
  (from :string) (to :string) (weight :double) (min-harmony :double))
(cffi:defcfun ("harmonia_harmonic_matrix_route_allowed" %hm-route-allowed) :int
  (from :string) (to :string) (signal :double) (noise :double))
(cffi:defcfun ("harmonia_harmonic_matrix_observe_route" %hm-observe-route) :int
  (from :string) (to :string) (success :int) (latency-ms :unsigned-long-long) (cost-usd :double))
(cffi:defcfun ("harmonia_harmonic_matrix_log_event" %hm-log-event) :int
  (component :string) (direction :string) (channel :string) (payload :string) (success :int) (error :string))
(cffi:defcfun ("harmonia_harmonic_matrix_route_timeseries" %hm-route-timeseries) :pointer
  (from :string) (to :string) (limit :int))
(cffi:defcfun ("harmonia_harmonic_matrix_time_report" %hm-time-report) :pointer
  (since-unix :unsigned-long-long))
(cffi:defcfun ("harmonia_harmonic_matrix_report" %hm-report) :pointer)
(cffi:defcfun ("harmonia_harmonic_matrix_last_error" %hm-last-error) :pointer)
(cffi:defcfun ("harmonia_harmonic_matrix_free_string" %hm-free-string) :void (ptr :pointer))

(defun harmonic-matrix-last-error ()
  (let ((ptr (%hm-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%hm-free-string ptr)))))

(defun %harmonic-matrix-check (rc op)
  (unless (zerop rc)
    (error "harmonic-matrix ~A failed: ~A" op (harmonic-matrix-last-error)))
  t)

(defun %hm-read-string (ptr op)
  (if (cffi:null-pointer-p ptr)
      (error "harmonic-matrix ~A failed: ~A" op (harmonic-matrix-last-error))
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (%hm-free-string ptr))))

(defun harmonic-matrix-register-node (node kind)
  (%harmonic-matrix-check (%hm-register-node node kind) "register-node"))

(defun harmonic-matrix-set-store (kind &optional (path ""))
  (%harmonic-matrix-check (%hm-set-store kind path) "set-store"))

(defun harmonic-matrix-store-config ()
  (%hm-read-string (%hm-get-store) "get-store"))

(defun harmonic-matrix-set-tool-enabled (tool-id enabled)
  (%harmonic-matrix-check (%hm-set-tool-enabled tool-id (if enabled 1 0)) "set-tool-enabled"))

(defun harmonic-matrix-register-edge (from to weight min-harmony)
  (%harmonic-matrix-check (%hm-register-edge from to (coerce weight 'double-float) (coerce min-harmony 'double-float))
                          "register-edge"))

(defun harmonic-matrix-route-defaults ()
  (list :signal (%route-default-signal) :noise (%route-default-noise)))

(defun harmonic-matrix-set-route-defaults (&key signal noise)
  (when signal
    (when (fboundp 'config-set)
      (config-set "matrix.route.signal_default" (format nil "~,6f" (coerce signal 'double-float)))))
  (when noise
    (when (fboundp 'config-set)
      (config-set "matrix.route.noise_default" (format nil "~,6f" (coerce noise 'double-float)))))
  (harmonic-matrix-route-defaults))

(defun harmonic-matrix-route-allowed-p (from to &key (signal (%route-default-signal)) (noise (%route-default-noise)))
  (plusp (%hm-route-allowed from to (coerce signal 'double-float) (coerce noise 'double-float))))

(defun harmonic-matrix-route-or-error (from to &key (signal (%route-default-signal)) (noise (%route-default-noise)))
  (unless (harmonic-matrix-route-allowed-p from to :signal signal :noise noise)
    (error "harmonic-matrix route denied ~A -> ~A: ~A" from to (harmonic-matrix-last-error)))
  t)

(defun harmonic-matrix-observe-route (from to success latency-ms &optional (cost-usd 0.0d0))
  (%harmonic-matrix-check (%hm-observe-route from to (if success 1 0)
                                             (max 0 latency-ms)
                                             (coerce cost-usd 'double-float))
                          "observe-route"))

(defun harmonic-matrix-log-event (component direction channel payload success &optional (error ""))
  (%harmonic-matrix-check (%hm-log-event component direction channel (or payload "") (if success 1 0) (or error ""))
                          "log-event"))

(defun harmonic-matrix-route-timeseries (from to &optional (limit 100))
  (%hm-read-string (%hm-route-timeseries from to (max 1 limit)) "route-timeseries"))

(defun harmonic-matrix-time-report (&optional (since-unix 0))
  (%hm-read-string (%hm-time-report (max 0 since-unix)) "time-report"))

(defun harmonic-matrix-report ()
  (%hm-read-string (%hm-report) "report"))

(defun %matrix-topology-path ()
  (or (and (fboundp 'config-get) (config-get "matrix.topology.path"))
      (sb-ext:posix-getenv "HARMONIA_MATRIX_TOPOLOGY_PATH")
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
  (read-from-string (with-output-to-string (s) (prin1 topology s))))

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
  (%harmonic-matrix-check (%hm-init) "init")
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

(defun harmonic-matrix-route-check (from to &key (signal (%route-default-signal)) (noise (%route-default-noise)))
  (list :from from
        :to to
        :signal signal
        :noise noise
        :allowed (harmonic-matrix-route-allowed-p from to :signal signal :noise noise)
        :error (harmonic-matrix-last-error)))

(defun init-harmonic-matrix-backend ()
  (ensure-cffi)
  (setf *harmonic-matrix-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_harmonic_matrix.dylib")))
  (%harmonic-matrix-check (%hm-init) "init")
  (runtime-log *runtime* :harmonic-matrix-init (list :status :ok))
  t)

(defun bootstrap-harmonic-matrix ()
  (init-harmonic-matrix-backend)
  (harmonic-matrix-apply-topology (harmonic-matrix-load-topology) :persist nil)
  t)
