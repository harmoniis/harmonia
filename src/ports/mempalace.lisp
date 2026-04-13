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
                           '(:component "mempalace" :op "init")))))
    (setf *mempalace-ready* (and reply (ipc-reply-ok-p reply)))
    *mempalace-ready*))

;;; ─── Self-organizing structure ──────────────────────────────────────

(defvar *palace-room-cache* (make-hash-table :test 'equal)
  "Cache: domain-string → room-id. Rooms are created on demand.")

(defun %palace-ensure-room (domain)
  "Get or create a room for DOMAIN. Pure functional: structure emerges from data.
   Returns room ID. Caches for performance. DuplicateNode = already exists (ok)."
  (or (gethash domain *palace-room-cache*)
      (when (mempalace-port-ready-p)
        (let ((result (handler-case
                          (palace-add-node "room" domain (or domain "generic"))
                        (error () nil))))
          (let ((id (when (listp result) (getf result :id))))
            (when id
              (setf (gethash domain *palace-room-cache*) id)
              id))))))

(defun %palace-room-for-class (class)
  "Map memory class to a domain string. The domain becomes a room name.
   New classes automatically create new rooms — no hardcoded mapping."
  (string-downcase (symbol-name class)))

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

;;; ─── Drawer operations ──────────────────────────────────────────────

(defun palace-file-drawer (content room-id &key tags)
  "Store verbatim content in a drawer."
  (when (mempalace-port-ready-p)
    (%parse-port-reply
     (ipc-call (%sexp-to-ipc-string
                `(:component "mempalace" :op "file-drawer"
                  :content ,content :room ,room-id
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
                              :op ,(concatenate 'string "context-" (princ-to-string tier)))))
                  (when domain
                    (setf base (append base `(:domain ,domain))))
                  (when query
                    (setf base (append base `(:query ,query))))
                  base))))))
