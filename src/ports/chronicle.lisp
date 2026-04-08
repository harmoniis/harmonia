;;; chronicle.lisp — Port: graph-native knowledge base & observability via IPC.
;;;
;;; Stores harmonic snapshots, memory events, delegation decisions, concept graph
;;; decompositions, and lifecycle events in a durable SQLite knowledge base.
;;; All data is queryable via complex SQL from Lisp, returning s-expressions.
;;;
;;; HOMOICONIC: all IPC commands are built as Lisp LISTS, then serialized
;;; with %sexp-to-ipc-string. S-expressions are law — no format strings.

(in-package :harmonia)

;;; --- Init ---

(defun init-chronicle-port ()
  "Initialize the chronicle database via IPC."
  (let ((reply (ipc-chronicle-init)))
    ;; Register chronicle as actor through the unified registry
    (when *runtime*
      (handler-case

          (let ((actor-id (actor-register "chronicle")

        (error () nil)))
          (setf (runtime-state-chronicle-actor-id *runtime*) actor-id)
          (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "chronicle"))))
    (runtime-log *runtime* :chronicle-init
                 (list :status (if (ipc-reply-ok-p reply) :ok :failed)))
    (ipc-reply-ok-p reply)))

;;; --- Recording API ---

(defun chronicle-record-harmonic (ctx)
  "Record a full harmonic snapshot from the harmonic context plist."
  (handler-case

      (let* ((plan (getf ctx :plan)

    (error () nil))
           (vitruvian (getf plan :vitruvian))
           (logistic (getf ctx :logistic))
           (lorenz (getf ctx :lorenz))
           (projection (getf ctx :projection))
           (global (getf ctx :global))
           (local (getf ctx :local))
           (security (getf ctx :security)))
      (ipc-call
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-harmonic"
          :cycle ,(or (getf ctx :cycle) 0)
          :phase ,(string-downcase (symbol-name (or (getf plan :state-machine) :observe)))
          :strength ,(coerce (or (getf vitruvian :strength) 0.0) 'double-float)
          :utility ,(coerce (or (getf vitruvian :utility) 0.0) 'double-float)
          :beauty ,(coerce (or (getf vitruvian :beauty) 0.0) 'double-float)
          :signal ,(coerce (or (getf vitruvian :signal) 0.0) 'double-float)
          :noise ,(coerce (or (getf vitruvian :noise) 0.0) 'double-float)
          :logistic-x ,(coerce (or (getf logistic :x) 0.5) 'double-float)
          :logistic-r ,(coerce (or (getf logistic :r) 3.45) 'double-float)
          :chaos-risk ,(coerce (or (getf logistic :chaos-risk) 0.0) 'double-float)
          :rewrite-aggression ,(coerce (or (getf logistic :rewrite-aggression) 0.0) 'double-float)
          :lorenz-x ,(coerce (or (getf lorenz :x) 0.0) 'double-float)
          :lorenz-y ,(coerce (or (getf lorenz :y) 0.0) 'double-float)
          :lorenz-z ,(coerce (or (getf lorenz :z) 0.0) 'double-float)
          :lorenz-radius ,(coerce (or (getf lorenz :radius) 0.0) 'double-float)
          :lorenz-bounded ,(coerce (or (getf lorenz :bounded-score) 0.0) 'double-float)
          :lambdoma-global ,(coerce (or (getf global :global-score) 0.0) 'double-float)
          :lambdoma-local ,(coerce (or (getf local :local-score) 0.0) 'double-float)
          :lambdoma-ratio ,(coerce (or (getf projection :ratio) 0.0) 'double-float)
          :lambdoma-convergent ,(if (getf projection :convergent-p) 1 0)
          :rewrite-ready ,(if (and plan (getf plan :ready)) 1 0)
          :rewrite-count ,(or (getf plan :rewrite-count) 0)
          :security-posture ,(string-downcase (symbol-name (or (getf security :posture) :nominal)))
          :security-events ,(or (getf security :events) 0)))))))

(defun chronicle-record-memory-event (event-type &key
                                       (entries-created 0) (entries-source 0)
                                       (old-size 0) (new-size 0)
                                       (node-count 0) (edge-count 0)
                                       (interdisciplinary-edges 0)
                                       (max-depth 0) detail)
  "Record a memory evolution event."
  (handler-case
      (ipc-call
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-memory-event"
          :event-type ,event-type
          :entries-created ,entries-created :entries-source ,entries-source
          :old-size ,old-size :new-size ,new-size
          :node-count ,node-count :edge-count ,edge-count
          :interdisciplinary-edges ,interdisciplinary-edges
          :max-depth ,max-depth :detail ,(or detail ""))))
    (error (e) (%log :warn "chronicle" "record-memory-event failed: ~A" e))))

(defun chronicle-record-delegation (&key task-hint model backend reason
                                      escalated escalated-from
                                      cost-usd latency-ms success
                                      tokens-in tokens-out)
  "Record a model delegation decision."
  (handler-case
      (ipc-call
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-delegation"
          :task-hint ,(or task-hint "")
          :model ,(or model "unknown")
          :backend ,(or backend "provider-router")
          :reason ,(or reason "")
          :escalated ,(if escalated 1 0)
          :escalated-from ,(or escalated-from "")
          :cost-usd ,(coerce (or cost-usd 0.0) 'double-float)
          :latency-ms ,(or latency-ms 0)
          :success ,(if success 1 0)
          :tokens-in ,(or tokens-in 0)
          :tokens-out ,(or tokens-out 0))))
    (error (e) (%log :warn "chronicle" "record-delegation failed: ~A" e))))

(defun chronicle-record-graph-snapshot ()
  "Snapshot the current concept graph into the chronicle knowledge base.
   Decomposes the s-expression graph into relational nodes/edges for SQL traversal."
  (handler-case
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
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-graph"
          :source "memory" :sexp ,sexp-str
          :nodes-json ,nodes-json :edges-json ,edges-json))))
    (error (e) (%log :warn "chronicle" "record-graph-snapshot failed: ~A" e))))

(defun chronicle-record-signalograd-event (event-type &key cycle confidence stability novelty
                                                      reward accepted recall-hits checkpoint-path
                                                      checkpoint-digest detail)
  "Record an auditable Signalograd lifecycle event."
  (handler-case
      (ipc-call
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-signalograd-event"
          :event-type ,(or event-type "unknown")
        :cycle ,(or cycle 0)
        :confidence ,(coerce (or confidence 0.0) 'double-float)
        :stability ,(coerce (or stability 0.0) 'double-float)
        :novelty ,(coerce (or novelty 0.0) 'double-float)
        :reward ,(coerce (or reward 0.0) 'double-float)
        :accepted ,(if accepted 1 0)
        :recall-hits ,(or recall-hits 0)
        :checkpoint-path ,(or checkpoint-path "")
          :checkpoint-digest ,(or checkpoint-digest "")
          :detail ,(or detail ""))))
    (error (e) (%log :warn "chronicle" "record-signalograd-event failed: ~A" e))))

(defun chronicle-record-phoenix-event (event-type &key exit-code attempt max-attempts
                                                   recovery-ms detail)
  "Record a phoenix lifecycle event."
  (handler-case
      (ipc-call
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-phoenix-event"
          :event-type ,(or event-type "unknown")
        :exit-code ,(or exit-code 0)
        :attempt ,(or attempt 0)
        :max-attempts ,(or max-attempts 0)
          :recovery-ms ,(or recovery-ms 0)
          :detail ,(or detail ""))))
    (error (e) (%log :warn "chronicle" "record-phoenix-event failed: ~A" e))))

(defun chronicle-record-ouroboros-event (event-type &key component detail patch-size success)
  "Record an ouroboros lifecycle event."
  (handler-case
      (ipc-call
       (%sexp-to-ipc-string
        `(:component "chronicle" :op "record-ouroboros-event"
        :event-type ,(or event-type "unknown")
        :component ,(or component "")
        :detail ,(or detail "")
          :patch-size ,(or patch-size 0)
          :success ,(if success 1 0))))
    (error (e) (%log :warn "chronicle" "record-ouroboros-event failed: ~A" e))))

;;; --- Query API ---

(defun %chronicle-ipc-query-sexp (op &optional extra-plist)
  "Send a chronicle query via IPC and parse the result as sexp.
   EXTRA-PLIST is a plist of additional keyword-value pairs to include."
  (let* ((base (list :component "chronicle" :op op))
         (cmd (%sexp-to-ipc-string (append base (or extra-plist '()))))
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
    (list :since-ts since-ts)))

(defun chronicle-full-digest ()
  (%chronicle-ipc-query-sexp "full-digest"))

(defun chronicle-harmonic-history (&key (since-ts 0) (limit 50))
  (%chronicle-ipc-query-sexp "harmonic-history"
    (list :since-ts since-ts :limit limit)))

(defun chronicle-memory-history (&key (since-ts 0) (limit 50))
  (%chronicle-ipc-query-sexp "memory-history"
    (list :since-ts since-ts :limit limit)))

(defun chronicle-delegation-history (&key (since-ts 0) (limit 50))
  (%chronicle-ipc-query-sexp "delegation-history"
    (list :since-ts since-ts :limit limit)))

(defun chronicle-dashboard-json ()
  "Generate an A2UI Composite dashboard as JSON string."
  (or (ipc-extract-value
       (ipc-call (%sexp-to-ipc-string
                  '(:component "chronicle" :op "dashboard"))))
      "{}"))

;;; --- Graph Query API ---

(defun chronicle-graph-traverse (concept &key (max-hops 3) (snapshot-id 0))
  "Traverse the knowledge graph from CONCEPT up to MAX-HOPS using recursive CTE."
  (%chronicle-ipc-query-sexp "graph-traverse"
    (list :concept concept :max-hops max-hops :snapshot-id snapshot-id)))

(defun chronicle-graph-bridges (&key (snapshot-id 0))
  "Find interdisciplinary bridge edges in the knowledge graph."
  (%chronicle-ipc-query-sexp "graph-bridges"
    (list :snapshot-id snapshot-id)))

(defun chronicle-graph-domains (&key (snapshot-id 0))
  "Get domain distribution of the knowledge graph."
  (%chronicle-ipc-query-sexp "graph-domains"
    (list :snapshot-id snapshot-id)))

(defun chronicle-graph-central (&key (snapshot-id 0) (limit 20))
  "Find most central (highest-degree) concepts in the knowledge graph."
  (%chronicle-ipc-query-sexp "graph-central"
    (list :snapshot-id snapshot-id :limit limit)))

(defun chronicle-graph-evolution (&key (since-ts 0))
  "Track how the knowledge graph has grown/changed over time."
  (%chronicle-ipc-query-sexp "graph-evolution"
    (list :since-ts since-ts)))

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
