;;; chronicle.lisp — Port: graph-native knowledge base & observability via IPC.
;;;
;;; Stores harmonic snapshots, memory events, delegation decisions, concept graph
;;; decompositions, and lifecycle events in a durable SQLite knowledge base.
;;; All data is queryable via complex SQL from Lisp, returning s-expressions.

(in-package :harmonia)

;;; --- Init ---

(defun init-chronicle-port ()
  "Initialize the chronicle database via IPC."
  (let ((reply (ipc-chronicle-init)))
    ;; Register chronicle as actor through the unified registry
    (when *runtime*
      (ignore-errors
        (let ((actor-id (actor-register "chronicle")))
          (setf (runtime-state-chronicle-actor-id *runtime*) actor-id)
          (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "chronicle"))))
    (runtime-log *runtime* :chronicle-init
                 (list :status (if (ipc-reply-ok-p reply) :ok :failed)))
    (ipc-reply-ok-p reply)))

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
      (ipc-call
       (format nil "(:component \"chronicle\" :op \"record-harmonic\" :cycle ~D :phase \"~A\" :strength ~F :utility ~F :beauty ~F :signal ~F :noise ~F :logistic-x ~F :logistic-r ~F :chaos-risk ~F :rewrite-aggression ~F :lorenz-x ~F :lorenz-y ~F :lorenz-z ~F :lorenz-radius ~F :lorenz-bounded ~F :lambdoma-global ~F :lambdoma-local ~F :lambdoma-ratio ~F :lambdoma-convergent ~D :rewrite-ready ~D :rewrite-count ~D :security-posture \"~A\" :security-events ~D)"
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
               (or (getf security :events) 0))))))

(defun chronicle-record-memory-event (event-type &key
                                       (entries-created 0) (entries-source 0)
                                       (old-size 0) (new-size 0)
                                       (node-count 0) (edge-count 0)
                                       (interdisciplinary-edges 0)
                                       (max-depth 0) detail)
  "Record a memory evolution event."
  (ignore-errors
    (ipc-call
     (format nil "(:component \"chronicle\" :op \"record-memory-event\" :event-type \"~A\" :entries-created ~D :entries-source ~D :old-size ~D :new-size ~D :node-count ~D :edge-count ~D :interdisciplinary-edges ~D :max-depth ~D :detail \"~A\")"
             (sexp-escape-lisp event-type) entries-created entries-source
             old-size new-size node-count edge-count
             interdisciplinary-edges max-depth
             (sexp-escape-lisp (or detail ""))))))

(defun chronicle-record-delegation (&key task-hint model backend reason
                                      escalated escalated-from
                                      cost-usd latency-ms success
                                      tokens-in tokens-out)
  "Record a model delegation decision."
  (ignore-errors
    (ipc-call
     (format nil "(:component \"chronicle\" :op \"record-delegation\" :task-hint \"~A\" :model \"~A\" :backend \"~A\" :reason \"~A\" :escalated ~D :escalated-from \"~A\" :cost-usd ~F :latency-ms ~D :success ~D :tokens-in ~D :tokens-out ~D)"
             (sexp-escape-lisp (or task-hint ""))
             (sexp-escape-lisp (or model "unknown"))
             (sexp-escape-lisp (or backend "provider-router"))
             (sexp-escape-lisp (or reason ""))
             (if escalated 1 0)
             (sexp-escape-lisp (or escalated-from ""))
             (coerce (or cost-usd 0.0) 'double-float)
             (or latency-ms 0)
             (if success 1 0)
             (or tokens-in 0)
             (or tokens-out 0)))))

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
      (ipc-call
       (format nil "(:component \"chronicle\" :op \"record-graph\" :source \"memory\" :sexp \"~A\" :nodes-json \"~A\" :edges-json \"~A\")"
               (sexp-escape-lisp sexp-str)
               (sexp-escape-lisp nodes-json)
               (sexp-escape-lisp edges-json))))))

(defun chronicle-record-signalograd-event (event-type &key cycle confidence stability novelty
                                                      reward accepted recall-hits checkpoint-path
                                                      checkpoint-digest detail)
  "Record an auditable Signalograd lifecycle event."
  (ignore-errors
    (ipc-call
     (format nil "(:component \"chronicle\" :op \"record-signalograd-event\" :event-type \"~A\" :cycle ~D :confidence ~F :stability ~F :novelty ~F :reward ~F :accepted ~D :recall-hits ~D :checkpoint-path \"~A\" :checkpoint-digest \"~A\" :detail \"~A\")"
             (sexp-escape-lisp (or event-type "unknown"))
             (or cycle 0)
             (coerce (or confidence 0.0) 'double-float)
             (coerce (or stability 0.0) 'double-float)
             (coerce (or novelty 0.0) 'double-float)
             (coerce (or reward 0.0) 'double-float)
             (if accepted 1 0)
             (or recall-hits 0)
             (sexp-escape-lisp (or checkpoint-path ""))
             (sexp-escape-lisp (or checkpoint-digest ""))
             (sexp-escape-lisp (or detail ""))))))

(defun chronicle-record-phoenix-event (event-type &key exit-code attempt max-attempts
                                                   recovery-ms detail)
  "Record a phoenix lifecycle event."
  (ignore-errors
    (ipc-call
     (format nil "(:component \"chronicle\" :op \"record-phoenix-event\" :event-type \"~A\" :exit-code ~D :attempt ~D :max-attempts ~D :recovery-ms ~D :detail \"~A\")"
             (sexp-escape-lisp (or event-type "unknown"))
             (or exit-code 0)
             (or attempt 0)
             (or max-attempts 0)
             (or recovery-ms 0)
             (sexp-escape-lisp (or detail ""))))))

(defun chronicle-record-ouroboros-event (event-type &key component detail patch-size success)
  "Record an ouroboros lifecycle event."
  (ignore-errors
    (ipc-call
     (format nil "(:component \"chronicle\" :op \"record-ouroboros-event\" :event-type \"~A\" :component \"~A\" :detail \"~A\" :patch-size ~D :success ~D)"
             (sexp-escape-lisp (or event-type "unknown"))
             (sexp-escape-lisp (or component ""))
             (sexp-escape-lisp (or detail ""))
             (or patch-size 0)
             (if success 1 0)))))

;;; --- Query API ---

(defun %chronicle-ipc-query-sexp (op &optional extra-params)
  "Send a chronicle query via IPC and parse the result as sexp."
  (let* ((cmd (if extra-params
                  (format nil "(:component \"chronicle\" :op \"~A\" ~A)" op extra-params)
                  (format nil "(:component \"chronicle\" :op \"~A\")" op)))
         (reply (ipc-call cmd))
         (val (ipc-extract-value reply)))
    (when val
      (handler-case
          (let ((*read-eval* nil))
            (read-from-string val))
        (error () val)))))

(defun chronicle-query (sql)
  "Run an arbitrary SELECT query against the chronicle database.
   Returns parsed s-expression results."
  (let ((reply (ipc-chronicle-query sql)))
    (when reply
      (handler-case
          (let ((*read-eval* nil))
            (read-from-string reply))
        (error () reply)))))

(defun chronicle-harmony-summary ()
  (%chronicle-ipc-query-sexp "harmony-summary"))

(defun chronicle-delegation-report ()
  (%chronicle-ipc-query-sexp "delegation-report"))

(defun chronicle-cost-report (&optional (since-ts 0))
  (%chronicle-ipc-query-sexp "cost-report"
    (format nil ":since-ts ~D" since-ts)))

(defun chronicle-full-digest ()
  (%chronicle-ipc-query-sexp "full-digest"))

(defun chronicle-harmonic-history (&key (since-ts 0) (limit 50))
  (%chronicle-ipc-query-sexp "harmonic-history"
    (format nil ":since-ts ~D :limit ~D" since-ts limit)))

(defun chronicle-memory-history (&key (since-ts 0) (limit 50))
  (%chronicle-ipc-query-sexp "memory-history"
    (format nil ":since-ts ~D :limit ~D" since-ts limit)))

(defun chronicle-delegation-history (&key (since-ts 0) (limit 50))
  (%chronicle-ipc-query-sexp "delegation-history"
    (format nil ":since-ts ~D :limit ~D" since-ts limit)))

(defun chronicle-dashboard-json ()
  "Generate an A2UI Composite dashboard as JSON string."
  (or (ipc-extract-value
       (ipc-call "(:component \"chronicle\" :op \"dashboard\")"))
      "{}"))

;;; --- Graph Query API ---

(defun chronicle-graph-traverse (concept &key (max-hops 3) (snapshot-id 0))
  "Traverse the knowledge graph from CONCEPT up to MAX-HOPS using recursive CTE."
  (%chronicle-ipc-query-sexp "graph-traverse"
    (format nil ":concept \"~A\" :max-hops ~D :snapshot-id ~D"
            (sexp-escape-lisp concept) max-hops snapshot-id)))

(defun chronicle-graph-bridges (&key (snapshot-id 0))
  "Find interdisciplinary bridge edges in the knowledge graph."
  (%chronicle-ipc-query-sexp "graph-bridges"
    (format nil ":snapshot-id ~D" snapshot-id)))

(defun chronicle-graph-domains (&key (snapshot-id 0))
  "Get domain distribution of the knowledge graph."
  (%chronicle-ipc-query-sexp "graph-domains"
    (format nil ":snapshot-id ~D" snapshot-id)))

(defun chronicle-graph-central (&key (snapshot-id 0) (limit 20))
  "Find most central (highest-degree) concepts in the knowledge graph."
  (%chronicle-ipc-query-sexp "graph-central"
    (format nil ":snapshot-id ~D :limit ~D" snapshot-id limit)))

(defun chronicle-graph-evolution (&key (since-ts 0))
  "Track how the knowledge graph has grown/changed over time."
  (%chronicle-ipc-query-sexp "graph-evolution"
    (format nil ":since-ts ~D" since-ts)))

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
   Returns number of rows deleted."
  (let ((reply (ipc-chronicle-gc)))
    (if (ipc-reply-ok-p reply)
        (or (ipc-extract-u64 reply ":result") 0)
        (error "chronicle gc failed: ~A" reply))))

(defun chronicle-gc-status ()
  "Query the current GC pressure: DB size, pressure level, row counts per table."
  (%chronicle-ipc-query-sexp "gc-status"))
