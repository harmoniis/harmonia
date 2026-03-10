;;; chronicle.lisp — Port: graph-native knowledge base & observability via chronicle CFFI.
;;;
;;; Stores harmonic snapshots, memory events, delegation decisions, concept graph
;;; decompositions, and lifecycle events in a durable SQLite knowledge base.
;;; All data is queryable via complex SQL from Lisp, returning s-expressions.

(in-package :harmonia)

(defparameter *chronicle-lib* nil)

;;; --- Chronicle CFFI declarations ---

(cffi:defcfun ("harmonia_chronicle_version" %chronicle-version) :string)
(cffi:defcfun ("harmonia_chronicle_healthcheck" %chronicle-healthcheck) :int)
(cffi:defcfun ("harmonia_chronicle_init" %chronicle-init) :int)
(cffi:defcfun ("harmonia_chronicle_last_error" %chronicle-last-error) :pointer)
(cffi:defcfun ("harmonia_chronicle_free_string" %chronicle-free-string) :void
  (ptr :pointer))

;; Recording
(cffi:defcfun ("harmonia_chronicle_record_harmonic" %chronicle-record-harmonic) :int
  (cycle :int64)
  (phase :string)
  (strength :double)
  (utility :double)
  (beauty :double)
  (signal-val :double)
  (noise :double)
  (logistic-x :double)
  (logistic-r :double)
  (chaos-risk :double)
  (rewrite-aggression :double)
  (lorenz-x :double)
  (lorenz-y :double)
  (lorenz-z :double)
  (lorenz-radius :double)
  (lorenz-bounded :double)
  (lambdoma-global :double)
  (lambdoma-local :double)
  (lambdoma-ratio :double)
  (lambdoma-convergent :int)
  (rewrite-ready :int)
  (rewrite-count :int)
  (security-posture :string)
  (security-events :int))

(cffi:defcfun ("harmonia_chronicle_record_memory_event" %chronicle-record-memory-event) :int
  (event-type :string)
  (entries-created :int)
  (entries-source :int)
  (old-size :int64)
  (new-size :int64)
  (node-count :int)
  (edge-count :int)
  (interdisciplinary-edges :int)
  (max-depth :int)
  (detail :string))

(cffi:defcfun ("harmonia_chronicle_record_phoenix_event" %chronicle-record-phoenix-event) :int
  (event-type :string)
  (exit-code :int)
  (attempt :int)
  (max-attempts :int)
  (recovery-ms :int64)
  (detail :string))

(cffi:defcfun ("harmonia_chronicle_record_ouroboros_event" %chronicle-record-ouroboros-event) :int
  (event-type :string)
  (component :string)
  (detail :string)
  (patch-size :int64)
  (success :int))

(cffi:defcfun ("harmonia_chronicle_record_delegation" %chronicle-record-delegation) :int
  (task-hint :string)
  (model-chosen :string)
  (backend :string)
  (reason :string)
  (escalated :int)
  (escalated-from :string)
  (cost-usd :double)
  (latency-ms :int64)
  (success :int)
  (tokens-in :int64)
  (tokens-out :int64))

(cffi:defcfun ("harmonia_chronicle_record_signalograd_event" %chronicle-record-signalograd-event) :int
  (event-type :string)
  (cycle :int64)
  (confidence :double)
  (stability :double)
  (novelty :double)
  (reward :double)
  (accepted :int)
  (recall-hits :int)
  (checkpoint-path :string)
  (checkpoint-digest :string)
  (detail :string))

(cffi:defcfun ("harmonia_chronicle_record_graph" %chronicle-record-graph) :int64
  (source :string)
  (sexp :string)
  (nodes-json :string)
  (edges-json :string))

;; Querying (returns s-expression strings)
(cffi:defcfun ("harmonia_chronicle_harmony_trajectory" %chronicle-harmony-trajectory) :pointer
  (since-ts :int64) (until-ts :int64))
(cffi:defcfun ("harmonia_chronicle_harmonic_history" %chronicle-harmonic-history) :pointer
  (since-ts :int64) (limit :int))
(cffi:defcfun ("harmonia_chronicle_memory_history" %chronicle-memory-history) :pointer
  (since-ts :int64) (limit :int))
(cffi:defcfun ("harmonia_chronicle_phoenix_history" %chronicle-phoenix-history) :pointer
  (since-ts :int64) (limit :int))
(cffi:defcfun ("harmonia_chronicle_ouroboros_history" %chronicle-ouroboros-history) :pointer
  (since-ts :int64) (limit :int))
(cffi:defcfun ("harmonia_chronicle_delegation_history" %chronicle-delegation-history) :pointer
  (since-ts :int64) (limit :int))
(cffi:defcfun ("harmonia_chronicle_harmony_summary" %chronicle-harmony-summary) :pointer)
(cffi:defcfun ("harmonia_chronicle_delegation_report" %chronicle-delegation-report) :pointer)
(cffi:defcfun ("harmonia_chronicle_cost_report" %chronicle-cost-report) :pointer
  (since-ts :int64))
(cffi:defcfun ("harmonia_chronicle_full_digest" %chronicle-full-digest) :pointer)
(cffi:defcfun ("harmonia_chronicle_query_sexp" %chronicle-query-sexp) :pointer
  (sql :string))
(cffi:defcfun ("harmonia_chronicle_dashboard_json" %chronicle-dashboard-json) :pointer)

;; Graph queries
(cffi:defcfun ("harmonia_chronicle_graph_traverse" %chronicle-graph-traverse) :pointer
  (concept :string) (max-hops :int) (snapshot-id :int64))
(cffi:defcfun ("harmonia_chronicle_graph_bridges" %chronicle-graph-bridges) :pointer
  (snapshot-id :int64))
(cffi:defcfun ("harmonia_chronicle_graph_domains" %chronicle-graph-domains) :pointer
  (snapshot-id :int64))
(cffi:defcfun ("harmonia_chronicle_graph_central" %chronicle-graph-central) :pointer
  (snapshot-id :int64) (limit :int))
(cffi:defcfun ("harmonia_chronicle_graph_evolution" %chronicle-graph-evolution) :pointer
  (since-ts :int64))

;; Maintenance
(cffi:defcfun ("harmonia_chronicle_gc" %chronicle-gc) :int)
(cffi:defcfun ("harmonia_chronicle_gc_status" %chronicle-gc-status) :pointer)

;;; --- Helpers ---

(defun %chronicle-read-string (ptr op)
  "Read a C string from chronicle, free it, signal error if null."
  (if (cffi:null-pointer-p ptr)
      (error "chronicle ~A failed: ~A" op (%chronicle-error-string))
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (%chronicle-free-string ptr))))

(defun %chronicle-error-string ()
  (let ((ptr (%chronicle-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%chronicle-free-string ptr)))))

(defun %chronicle-read-sexp (ptr op)
  "Read a C string as s-expression, free pointer, return parsed Lisp data."
  (let ((text (%chronicle-read-string ptr op)))
    (handler-case
        (let ((*read-eval* nil))
          (read-from-string text))
      (error (e)
        (declare (ignore e))
        text))))

;;; --- Port API ---

(defun init-chronicle-port ()
  "Load the chronicle dylib and initialize the database."
  (ensure-cffi)
  (setf *chronicle-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_chronicle.dylib")))
  (let ((rc (%chronicle-init)))
    (unless (zerop rc)
      (error "chronicle init failed: ~A" (%chronicle-error-string))))
  ;; Register chronicle as actor through the unified registry (in parallel-agents dylib)
  (when *runtime*
    (ignore-errors
      (let ((actor-id (actor-register "chronicle")))
        (setf (runtime-state-chronicle-actor-id *runtime*) actor-id)
        (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "chronicle"))))
  (runtime-log *runtime* :chronicle-init
               (list :version (%chronicle-version) :status :ok))
  t)

;;; --- Recording API ---

(defun chronicle-record-harmonic (ctx)
  "Record a full harmonic snapshot from the harmonic context plist."
  (ignore-errors
    (let* ((plan (getf ctx :plan))
           (vitruvian (getf plan :vitruvian))
           (logistic (getf ctx :logistic))
           (lorenz (getf ctx :lorenz))
           (projection (getf ctx :projection))
           (global (getf ctx :global))
           (local (getf ctx :local))
           (security (getf ctx :security)))
      (%chronicle-record-harmonic
       (or (getf ctx :cycle) 0)
       (string-downcase (symbol-name (or (getf plan :state-machine) :observe)))
       (coerce (or (getf vitruvian :strength) 0.0) 'double-float)
       (coerce (or (getf vitruvian :utility) 0.0) 'double-float)
       (coerce (or (getf vitruvian :beauty) 0.0) 'double-float)
       (coerce (or (getf vitruvian :signal) 0.0) 'double-float)
       (coerce (or (getf vitruvian :noise) 0.0) 'double-float)
       (coerce (or (getf logistic :x) 0.5) 'double-float)
       (coerce (or (getf logistic :r) 3.45) 'double-float)
       (coerce (or (getf logistic :chaos-risk) 0.0) 'double-float)
       (coerce (or (getf logistic :rewrite-aggression) 0.0) 'double-float)
       (coerce (or (getf lorenz :x) 0.0) 'double-float)
       (coerce (or (getf lorenz :y) 0.0) 'double-float)
       (coerce (or (getf lorenz :z) 0.0) 'double-float)
       (coerce (or (getf lorenz :radius) 0.0) 'double-float)
       (coerce (or (getf lorenz :bounded-score) 0.0) 'double-float)
       (coerce (or (getf global :global-score) 0.0) 'double-float)
       (coerce (or (getf local :local-score) 0.0) 'double-float)
       (coerce (or (getf projection :ratio) 0.0) 'double-float)
       (if (getf projection :convergent-p) 1 0)
       (if (and plan (getf plan :ready)) 1 0)
       (or (getf plan :rewrite-count) 0)
       (string-downcase (symbol-name (or (getf security :posture) :nominal)))
       (or (getf security :events) 0)))))

(defun chronicle-record-memory-event (event-type &key
                                       (entries-created 0) (entries-source 0)
                                       (old-size 0) (new-size 0)
                                       (node-count 0) (edge-count 0)
                                       (interdisciplinary-edges 0)
                                       (max-depth 0) detail)
  "Record a memory evolution event."
  (ignore-errors
    (%chronicle-record-memory-event
     event-type entries-created entries-source
     old-size new-size node-count edge-count
     interdisciplinary-edges max-depth
     (or detail ""))))

(defun chronicle-record-delegation (&key task-hint model backend reason
                                      escalated escalated-from
                                      cost-usd latency-ms success
                                      tokens-in tokens-out)
  "Record a model delegation decision."
  (ignore-errors
    (%chronicle-record-delegation
     (or task-hint "")
     (or model "unknown")
     (or backend "provider-router")
     (or reason "")
     (if escalated 1 0)
     (or escalated-from "")
     (coerce (or cost-usd 0.0) 'double-float)
     (or latency-ms 0)
     (if success 1 0)
     (or tokens-in 0)
     (or tokens-out 0))))

(defun chronicle-record-graph-snapshot ()
  "Snapshot the current concept graph into the chronicle knowledge base.
   Decomposes the s-expression graph into relational nodes/edges for SQL traversal."
  (ignore-errors
    (let* ((map (memory-map-sexp :entry-limit 200 :edge-limit 300))
           (sexp-str (prin1-to-string map))
           (concept-nodes (getf map :concept-nodes))
           (concept-edges (getf map :concept-edges))
           ;; Build JSON arrays for relational decomposition
           (nodes-json
             (format nil "[~{~A~^,~}]"
                     (mapcar (lambda (n)
                               (format nil "{\"concept\":~S,\"domain\":~S,\"count\":~D,\"depth_min\":~D,\"depth_max\":~D,\"classes\":~S}"
                                       (or (getf n :concept) "")
                                       (string-downcase (symbol-name (or (getf n :domain) :generic)))
                                       (or (getf n :count) 1)
                                       (or (car (getf n :depths)) 0)
                                       (or (car (last (getf n :depths))) 0)
                                       (format nil "~{~A~^,~}" (mapcar #'symbol-name (or (getf n :classes) '())))))
                             concept-nodes)))
           (edges-json
             (format nil "[~{~A~^,~}]"
                     (mapcar (lambda (e)
                               (format nil "{\"a\":~S,\"b\":~S,\"weight\":~D,\"interdisciplinary\":~A,\"reasons\":~S}"
                                       (or (getf e :a) "")
                                       (or (getf e :b) "")
                                       (or (getf e :weight) 1)
                                       (if (getf e :interdisciplinary) "true" "false")
                                       (format nil "~{~A~^,~}" (mapcar #'symbol-name (or (getf e :reasons) '())))))
                             concept-edges))))
      (%chronicle-record-graph "memory" sexp-str nodes-json edges-json))))

(defun chronicle-record-signalograd-event (event-type &key cycle confidence stability novelty
                                                      reward accepted recall-hits checkpoint-path
                                                      checkpoint-digest detail)
  "Record an auditable Signalograd lifecycle event."
  (ignore-errors
    (%chronicle-record-signalograd-event
     (or event-type "unknown")
     (or cycle 0)
     (coerce (or confidence 0.0) 'double-float)
     (coerce (or stability 0.0) 'double-float)
     (coerce (or novelty 0.0) 'double-float)
     (coerce (or reward 0.0) 'double-float)
     (if accepted 1 0)
     (or recall-hits 0)
     (or checkpoint-path "")
     (or checkpoint-digest "")
     (or detail ""))))

;;; --- Query API ---

(defun chronicle-query (sql)
  "Run an arbitrary SELECT query against the chronicle database.
   Returns parsed s-expression results."
  (%chronicle-read-sexp (%chronicle-query-sexp sql) "query"))

(defun chronicle-harmony-summary ()
  (%chronicle-read-sexp (%chronicle-harmony-summary) "harmony-summary"))

(defun chronicle-delegation-report ()
  (%chronicle-read-sexp (%chronicle-delegation-report) "delegation-report"))

(defun chronicle-cost-report (&optional (since-ts 0))
  (%chronicle-read-sexp (%chronicle-cost-report since-ts) "cost-report"))

(defun chronicle-full-digest ()
  (%chronicle-read-sexp (%chronicle-full-digest) "full-digest"))

(defun chronicle-harmonic-history (&key (since-ts 0) (limit 50))
  (%chronicle-read-sexp (%chronicle-harmonic-history since-ts limit) "harmonic-history"))

(defun chronicle-memory-history (&key (since-ts 0) (limit 50))
  (%chronicle-read-sexp (%chronicle-memory-history since-ts limit) "memory-history"))

(defun chronicle-delegation-history (&key (since-ts 0) (limit 50))
  (%chronicle-read-sexp (%chronicle-delegation-history since-ts limit) "delegation-history"))

(defun chronicle-dashboard-json ()
  "Generate an A2UI Composite dashboard as JSON string."
  (%chronicle-read-string (%chronicle-dashboard-json) "dashboard"))

;;; --- Graph Query API ---

(defun chronicle-graph-traverse (concept &key (max-hops 3) (snapshot-id 0))
  "Traverse the knowledge graph from CONCEPT up to MAX-HOPS using recursive CTE."
  (%chronicle-read-sexp (%chronicle-graph-traverse concept max-hops snapshot-id) "graph-traverse"))

(defun chronicle-graph-bridges (&key (snapshot-id 0))
  "Find interdisciplinary bridge edges in the knowledge graph."
  (%chronicle-read-sexp (%chronicle-graph-bridges snapshot-id) "graph-bridges"))

(defun chronicle-graph-domains (&key (snapshot-id 0))
  "Get domain distribution of the knowledge graph."
  (%chronicle-read-sexp (%chronicle-graph-domains snapshot-id) "graph-domains"))

(defun chronicle-graph-central (&key (snapshot-id 0) (limit 20))
  "Find most central (highest-degree) concepts in the knowledge graph."
  (%chronicle-read-sexp (%chronicle-graph-central snapshot-id limit) "graph-central"))

(defun chronicle-graph-evolution (&key (since-ts 0))
  "Track how the knowledge graph has grown/changed over time."
  (%chronicle-read-sexp (%chronicle-graph-evolution since-ts) "graph-evolution"))

;;; --- Batched recording support ---

(defun chronicle-batch-harmonic (ctx)
  "Queue a harmonic snapshot for batched writing during %tick-chronicle-flush.
   Falls back to direct recording if runtime is not available."
  (if (and (boundp '*runtime*) *runtime*)
      (push (list :type "harmonic" :ctx ctx)
            (runtime-state-chronicle-pending *runtime*))
      (chronicle-record-harmonic ctx)))

(defun chronicle-batch-delegation (&rest args)
  "Queue a delegation record for batched writing during %tick-chronicle-flush."
  (if (and (boundp '*runtime*) *runtime*)
      (push (list :type "delegation" :args args)
            (runtime-state-chronicle-pending *runtime*))
      (apply #'chronicle-record-delegation args)))

;;; --- Maintenance ---

(defun chronicle-gc ()
  "Run intelligent pressure-aware garbage collection.
   Preserves high-signal data (chaos events, rewrites, failures, recoveries).
   Thins boring middle data. Downsamples into trajectory before deleting.
   Returns number of rows deleted."
  (let ((deleted (%chronicle-gc)))
    (when (< deleted 0)
      (error "chronicle gc failed: ~A" (%chronicle-error-string)))
    deleted))

(defun chronicle-gc-status ()
  "Query the current GC pressure: DB size, pressure level, row counts per table."
  (%chronicle-read-sexp (%chronicle-gc-status) "gc-status"))
