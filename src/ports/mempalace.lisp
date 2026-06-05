;;; mempalace.lisp — Port: Graph-structured knowledge palace via IPC.
;;;
;;; The palace is a graph. Nodes are concepts (wings, rooms, entities, tunnels).
;;; Edges are typed relationships with temporal validity.
;;; Drawers store verbatim content. AAAK compresses for context efficiency.
;;;
;;; All IPC replies from the Rust engine follow (:ok ...) form.
;;;
;;; HOMOICONIC: all IPC commands are built as Lisp LISTS, then serialized
;;; with %sexp-to-ipc-string. S-expressions are law — no format strings.

(in-package :harmonia)

(defparameter *mempalace-ready* nil)

;;; ─── Reply parsing ──────────────────────────────────────────────────

(defun %parse-port-reply (reply)
  "Parse an IPC reply sexp, stripping the leading :ok status marker.
Returns a plist on success, nil on failure. Safe: no eval, no crash.
Shared by all ports — one function, not duplicated."
  (when (and reply (stringp reply) (ipc-reply-ok-p reply))
    (let ((*read-eval* nil))
      (handler-case
          (let ((parsed (read-from-string reply)))
            (cond
              ((and (listp parsed) (eq (car parsed) :ok))
               (cdr parsed))
              ((listp parsed) parsed)
              (t nil)))
        (error () nil)))))

;;; ─── Port lifecycle ─────────────────────────────────────────────────

(defun mempalace-port-ready-p ()
  *mempalace-ready*)

(defun init-mempalace-port ()
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           '(:component "mempalace" :op "health")))))
    (setf *mempalace-ready* (and reply (ipc-reply-ok-p reply)))
    *mempalace-ready*))

;;; ─── Self-organizing structure ──────────────────────────────────────

(defun %palace-name (value)
  (string-downcase
   (cond
     ((keywordp value) (symbol-name value))
     ((symbolp value) (symbol-name value))
     ((stringp value) value)
     (t (princ-to-string value)))))

(defun %palace-domain-name (domain)
  (let ((name (%palace-name (or domain :generic))))
    (if (member name '("music" "math" "engineering" "cognitive" "life" "system" "generic")
                :test #'string=)
        name
        "generic")))

(defun %palace-class-domain (class)
  (case class
    ((:tool :system) "system")
    ((:skill) "engineering")
    ((:soul :genesis) "cognitive")
    ((:daily :interaction) "life")
    (t "generic")))

(defun %palace-concept-label (concept)
  (concatenate 'string "concept:" concept))

(defun %palace-node-id (kind label domain)
  (let ((result (handler-case
                    (palace-add-node kind label domain)
                  (error () nil))))
    (when (listp result) (getf result :id))))

(defun %palace-link (source target kind weight)
  (when (and source target (not (= source target)))
    (handler-case
        (palace-add-edge source target kind weight)
      (error () nil))))

(defun %palace-ensure-room (room &key wing domain)
  "Get or create a room and connect it to its wing."
  (let* ((room-label (%palace-name room))
         (domain-name (%palace-domain-name (or domain wing)))
         (wing-label (%palace-domain-name (or wing domain-name)))
         (wing-id (%palace-node-id "wing" wing-label domain-name))
         (room-id (%palace-node-id "room" room-label domain-name)))
    (%palace-link wing-id room-id "contains" 1.0)
    room-id))

(defun %palace-room-for-class (class)
  "Map memory class to palace room. Policy-driven with generic fallback.
   Reads :class-defaults from :routing section of config/memory-routing.sexp."
  (let* ((routing (when (and (boundp '*memory-routing-config*) *memory-routing-config*)
                    (getf *memory-routing-config* :routing)))
         (defaults (when routing (getf routing :class-defaults))))
    (if defaults
        (let ((entry (getf defaults class)))
          (if entry
              (getf entry :palace-room)
              (string-downcase (symbol-name class))))
        (string-downcase (symbol-name class)))))

;;; ─── Graph operations ───────────────────────────────────────────────

(defun palace-add-node (kind label domain)
  "Add a node to the palace graph."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "add-node"
                  :kind ,kind :label ,label :domain ,domain))))))

(defun palace-add-edge (source target kind weight)
  "Add an edge between two nodes."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "add-edge"
                  :source ,source :target ,target :kind ,kind :weight ,weight))))))

(defun palace-graph-query (from &key (traversal "bfs") (depth 3))
  "Traverse the palace graph from a starting node."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "query-graph"
                  :from ,from :traversal ,traversal :depth ,depth))))))

(defun palace-find-tunnels ()
  "Find cross-wing bridge nodes (tunnels)."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                '(:component "mempalace" :op "find-tunnels"))))))

(defun palace-graph-stats ()
  "Return graph statistics."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                '(:component "mempalace" :op "graph-stats"))))))

(defun %palace-tag-concepts (tags)
  (remove-duplicates
   (remove-if (lambda (w) (< (length w) 3))
              (mapcar #'%palace-name (or tags '())))
   :test #'string=))

(defun %palace-entry-concepts (content tags indexed-concepts)
  (let* ((text (if (stringp content) content (princ-to-string content)))
         (concepts (or indexed-concepts
                       (append (when (fboundp '%split-words) (%split-words text))
                               (%palace-tag-concepts tags))))
         (clean (remove-duplicates
                 (remove-if (lambda (w) (< (length w) 3))
                            (mapcar #'%palace-name concepts))
                 :test #'string=)))
    (subseq clean 0 (min 16 (length clean)))))

(defun %palace-concept-domain (concept)
  (if (fboundp '%concept-domain)
      (%palace-domain-name (%concept-domain concept))
      "generic"))

(defun %palace-build-entry-graph (class content tags &key concepts)
  "Construct the L3 palace graph for a filed memory entry."
  (when (mempalace-port-ready-p)
    (let* ((room-name (%palace-room-for-class class))
           (entry-concepts (%palace-entry-concepts content tags concepts))
           (concept-domains
             (mapcar (lambda (c) (list c (%palace-concept-domain c))) entry-concepts))
           (room-domain (%palace-class-domain class))
           (domains (remove-duplicates
                     (append (list room-domain) (mapcar #'second concept-domains))
                     :test #'string=))
           (primary-domain room-domain)
           (room-id (%palace-ensure-room room-name
                                          :wing primary-domain
                                          :domain primary-domain))
           (wing-ids '())
           (concept-nodes '()))
      (dolist (domain domains)
        (let ((wing-id (%palace-node-id "wing" domain domain)))
          (when wing-id
            (push (cons domain wing-id) wing-ids)
            (%palace-link wing-id room-id "contains" 1.0))))
      (dolist (pair concept-domains)
        (destructuring-bind (concept domain) pair
          (let ((concept-id (%palace-node-id "concept" (%palace-concept-label concept) domain)))
            (when concept-id
              (push (list concept-id domain concept) concept-nodes)
              (%palace-link room-id concept-id "contains" 0.9)))))
      (loop for lefts on concept-nodes do
        (loop for right in (cdr lefts) do
          (destructuring-bind (left-id left-domain left-concept) (car lefts)
            (declare (ignore left-concept))
            (destructuring-bind (right-id right-domain right-concept) right
              (declare (ignore right-concept))
              (%palace-link left-id right-id
                            (if (string= left-domain right-domain)
                                "relates-to"
                                "bridges")
                            (if (string= left-domain right-domain) 0.55 0.75))))))
      (when (> (length domains) 1)
        (let* ((sorted-domains (sort (copy-list domains) #'string<))
               (tunnel-label (format nil "tunnel:~A:~{~A~^-~}" room-name sorted-domains))
               (tunnel-id (%palace-node-id "tunnel" tunnel-label primary-domain)))
          (dolist (wing wing-ids)
            (%palace-link (cdr wing) tunnel-id "bridges" 0.8))
          (%palace-link tunnel-id room-id "bridges" 0.8)))
      (list :room-id room-id
            :wings (length domains)
            :concepts (length concept-nodes)
            :edges (length concept-nodes)))))

(defun %palace-file-memory-entry (class content &key tags concepts id)
  "File one memory into the palace drawer store and knowledge graph. ID is the
chronicle memory-entry id; it keys the drawer for boot reconciliation. The result
plist carries :drawer-filed — t only when a drawer was actually created (the
engine rejects degenerate content), so reconciliation counts real work."
  (let* ((text (if (stringp content) content (princ-to-string content)))
         (graph (%palace-build-entry-graph class text tags :concepts concepts))
         (room-id (getf graph :room-id))
         (drawer (when room-id
                   (palace-file-drawer text room-id
                                       :tags (mapcar #'%palace-name (or tags '()))
                                       :entry-id id))))
    (append graph (list :drawer-filed (and drawer t)))))

;;; ─── Drawer operations ──────────────────────────────────────────────

(defun palace-file-drawer (content room-id &key tags entry-id)
  "Store verbatim content in a drawer. When ENTRY-ID is supplied the drawer is
keyed to its chronicle memory entry so boot reconciliation can diff against it."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "file-drawer"
                  :content ,content :room ,room-id
                  ,@(when entry-id (list :entry-id (princ-to-string entry-id)))
                  :tags ,(format nil "~{~A~^ ~}" (or tags '()))))))))

(defun palace-search (query &key room (limit 10))
  "Search drawers by query with optional room filter."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                (if room
                    `(:component "mempalace" :op "search"
                      :query ,query :room ,room :limit ,limit)
                    `(:component "mempalace" :op "search"
                      :query ,query :limit ,limit)))))))

(defun palace-get-drawer (id)
  "Get a specific drawer by ID."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "get-drawer" :id ,id))))))

(defun palace-entry-ids ()
  "Return a hash-table set of the chronicle entry-ids the palace has filed
(drawers with a Memory source). This is the diff key for boot reconciliation."
  (let ((have (make-hash-table :test #'equal)))
    (when (mempalace-port-ready-p)
      (let ((reply (%parse-port-reply
                    (ipc-call (%sexp-to-ipc-string
                               '(:component "mempalace" :op "entry-ids"))))))
        (dolist (id (getf reply :ids))
          (setf (gethash (princ-to-string id) have) t))))
    have))

;;; ─── AAAK compression ──────────────────────────────────────────────

(defun palace-compress (drawer-ids)
  "Compress drawers into AAAK format."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "compress"
                  :ids ,(format nil "~{~D~^ ~}" drawer-ids)))))))

(defun palace-codebook-lookup (code-or-entity)
  "Look up entity<->code mapping in the persistent codebook."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "codebook"
                  :query ,code-or-entity))))))

;;; ─── Tiered context retrieval ───────────────────────────────────────

(defun palace-context (tier &key domain query)
  "Retrieve tiered context: l0 (identity), l1 (essential), l2 (filtered), l3 (deep)."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                (let ((base `(:component "mempalace"
                              :op ,(concatenate 'string "context-"
                                                (string-downcase (princ-to-string tier))))))
                  (when domain
                    (setf base (append base `(:domain ,domain))))
                  (when query
                    (setf base (append base `(:query ,query))))
                  base))))))
