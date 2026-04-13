;;; repl-primitives.lisp — REPL primitive implementations (%prim-* functions).

(in-package :harmonia)

;;; Forward declarations for variables defined in repl-loop.lisp (loaded after us)
(defvar *repl-model-perf* nil)
(defvar *routing-tier* :auto)

;;; ═══════════════════════════════════════════════════════════════════════
;;; PRIMITIVES — system interface
;;; ═══════════════════════════════════════════════════════════════════════

;; ── recall: smart memory search ─────────────────────────────────────

(defun %prim-recall (query &rest kwargs &key (limit 5) (max-chars 1200) verbatim tags since)
  "Recall from L3 palace (user knowledge). Falls back to L2 chronicle.
   The palace stores user interactions and mined data.
   (recall \"Thomas\") → search palace drawers for Thomas-related content."
  (declare (ignore kwargs tags since))
  (let ((q (if (stringp query) query (princ-to-string query))))
    (cond
      (verbatim
       ;; Verbatim: exact match in palace
       (or (handler-case
               (when (fboundp 'palace-search)
                 (let ((result (funcall 'palace-search q :limit limit)))
                   (if result (format nil "~A" result) "(no verbatim match)")))
             (error () nil))
           "(verbatim unavailable)"))
      (t
       ;; Default: search palace (L3), fall back to memory store
       (or (handler-case
               (when (and (fboundp 'palace-search) (fboundp 'mempalace-port-ready-p)
                          (funcall 'mempalace-port-ready-p))
                 (let ((result (funcall 'palace-search q :limit limit)))
                   (when (and result (stringp result) (> (length result) 10))
                     result)))
             (error () nil))
           ;; Fallback to memory-semantic-recall-block (scans *memory-store*)
           (handler-case
               (let ((r (memory-semantic-recall-block q :limit limit :max-chars max-chars)))
                 (if (and r (> (length r) 0)) r "(no memories found)"))
             (error () nil))
           "(recall unavailable)")))))

;; ── ipc: generic system query ───────────────────────────────────────

(defun %prim-ipc (component op &rest kwargs)
  "Call any IPC component. Full power. Security is enforced in Rust,
not here — vault requires admin-intent signature, policy requires owner auth.
The REPL has full Lisp power; Rust is the boundary."
  (let* ((comp (if (stringp component) component (princ-to-string component)))
         (operation (if (stringp op) op (princ-to-string op)))
         (cmd (append (list :component comp :op operation) kwargs)))
    (or (ipc-call (%sexp-to-ipc-string cmd)) "(ipc: no response)")))

;; ── System info primitives ──────────────────────────────────────────

(defun %prim-env ()
  "The environment — derived from the actual primitive table."
  (format nil "~{~A~^ ~}" *repl-primitives*))

(defun %prim-introspect ()
  (or (handler-case
          (when (fboundp '%runtime-identity)
            (funcall '%runtime-identity))
        (error () nil))
      "(introspect unavailable)"))

(defun %prim-status ()
  "Runtime state — derivable, not memorized. The LLM calls this to know itself."
  (or (handler-case
          (let* ((cycle (if (and (boundp '*runtime*) *runtime*)
                         (runtime-state-cycle *runtime*) 0))
               (rewrite-count (if (and (boundp '*runtime*) *runtime*)
                                  (runtime-state-rewrite-count *runtime*) 0))
               (tier (if (boundp '*routing-tier*) (symbol-name *routing-tier*) "auto"))
               ;; Last model used — from REPL perf tracking.
               (last-model "")
               (last-fluency 0.0))
          (when (boundp '*repl-model-perf*)
            (maphash (lambda (model perf)
                       (let ((calls (or (getf perf :calls) 0)))
                         (when (> calls (or (getf (gethash last-model *repl-model-perf*) :calls) 0))
                           (setf last-model model)
                           (setf last-fluency (%repl-fluency model)))))
                     *repl-model-perf*))
          (format nil "cycle=~D tier=~A model=~A fluency=~,2F rewrites=~D"
                  cycle tier last-model last-fluency rewrite-count))
        (error () nil))
      "(status unavailable)"))

(defun %prim-chaos-risk ()
  (or (handler-case
          (let ((ctx (runtime-state-harmonic-context *runtime*)))
            (when ctx
              (let ((logistic (getf ctx :logistic)))
                (when logistic (getf logistic :chaos-risk)))))
        (error () nil))
      0.5))

(defun %prim-field ()
  "L1 global context: the map of what the agent knows and how to navigate.
   Returns a compact guide: capabilities, where to search, chain of thought.
   The model reads this FIRST to understand how to proceed."
  (let ((basin (or (handler-case (%prim-basin) (error () nil)) "?"))
        (mem-count (hash-table-count *memory-store*))
        (concept-count (hash-table-count *memory-concept-nodes*))
        (palace-ok (and (fboundp 'mempalace-port-ready-p) (funcall 'mempalace-port-ready-p)))
        (tier (if (boundp '*routing-tier*) (symbol-name *routing-tier*) "auto")))
    (format nil "GLOBAL CONTEXT:
basin=~A concepts=~D memories=~D palace=~A tier=~A
CHAIN: (field)→understand → (recall q)→user-data → (status)→system → (respond answer)
TOOLS: exec read-file grep list-files write-file fetch python search convert datamine
MEMORY: (recall q) searches palace for user knowledge. (store text) saves to palace.
SYSTEM: (status) (basin) (introspect) (models) for self-knowledge.
EXPLORE: (exec cmd) (fetch url) (python code) (search q) (datamine lode) for new data."
            basin mem-count concept-count (if palace-ok "ready" "offline") tier)))

(defun %prim-basin ()
  "Return basin status as structured string: basin=X dwell=N threshold=F"
  (or (handler-case
          (when (and (fboundp 'memory-field-port-ready-p)
                     (funcall 'memory-field-port-ready-p))
            (let* ((reply (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))
              (if (and reply (stringp reply))
                  (let* ((*read-eval* nil)
                         (*package* (find-package :harmonia))
                         (parsed (handler-case (read-from-string reply) (error () nil)))
                         (data (when (listp parsed) (cdr parsed)))
                         (basin (getf data :current))
                         (dwell (getf data :dwell-ticks))
                         (energy (getf data :coercive-energy))
                         (threshold (getf data :threshold)))
                    (format nil "basin=~A dwell=~A energy=~A threshold=~A"
                            (or basin "?") (or dwell "?") (or energy "?") (or threshold "?")))
                  "basin=unavailable")))
        (error () nil))
      "basin=unavailable"))

;; ── Workspace primitives (Rust actors — the agent's hands) ──────

(defun %prim-read-file (&rest args)
  (let ((path (first args))
        (offset (or (second args) 0))
        (limit (or (third args) 200)))
    (if (and path (stringp path))
        (or (handler-case (workspace-read-file path :offset offset :limit limit) (error () nil))
            "(read-file: not found)")
        "(read-file: path required)")))

(defun %prim-grep (&rest args)
  (let ((pattern (first args))
        (path (or (second args) ".")))
    (if (and pattern (stringp pattern))
        (or (handler-case (workspace-grep pattern path) (error () nil))
            "(grep: no results)")
        "(grep: pattern required)")))

(defun %prim-list-files (&rest args)
  (let ((path (or (first args) ".")))
    (or (handler-case (workspace-list-files path) (error () nil))
        "(list-files: error)")))

(defun %prim-file-exists (path)
  (if (and path (stringp path))
      (if (handler-case (workspace-file-exists-p path) (error () nil)) "exists" "not found")
      "(file-exists: path required)"))

(defun %prim-file-info (path)
  (if (and path (stringp path))
      (or (handler-case (workspace-file-info path) (error () nil))
          "(file-info: error)")
      "(file-info: path required)"))

(defun %prim-write-file (&rest args)
  (let ((path (first args))
        (content (or (second args) "")))
    (if (and path (stringp path))
        (or (handler-case (workspace-write-file path content) (error () nil))
            "(write-file: error)")
        "(write-file: path required)")))

(defun %prim-append-file (&rest args)
  (let ((path (first args))
        (content (or (second args) "")))
    (if (and path (stringp path))
        (or (handler-case (workspace-append-file path content) (error () nil))
            "(append-file: error)")
        "(append-file: path required)")))

(defun %prim-exec (&rest args)
  (let ((cmd (first args))
        (cmd-args (rest args)))
    (if (and cmd (stringp cmd))
        (or (handler-case
                (workspace-exec cmd (mapcar #'princ-to-string cmd-args))
              (error () nil))
            "(exec: error)")
        "(exec: command required)")))

;; ── Dreaming primitive ────────────────────────────────────────────

(defun %prim-dream ()
  (or (handler-case
          (when (and (fboundp 'memory-field-port-ready-p)
                     (funcall 'memory-field-port-ready-p))
            (let* ((report (memory-field-dream))
                   (results (when report (%apply-dream-results report))))
              (format nil "Dream: pruned=~D crystallized=~D"
                      (or (getf results :pruned) 0)
                      (or (getf results :crystallized) 0))))
        (error () nil))
      "(dream unavailable)"))

(defun %prim-meditate ()
  "Meditate: gather recent concepts from memory, strengthen their connections.
   Pure functional — reads from the field, not from hardcoded state."
  (or (handler-case
          (when (fboundp 'memory-meditate)
          ;; Gather concepts from recent entries (last N accessed).
          (let* ((recent (memory-recent :limit 10))
                 (concepts '()))
            (dolist (entry recent)
              (let ((text (%entry-text entry)))
                (when (stringp text)
                  (dolist (w (%split-words text))
                    (push w concepts)))))
            (let ((unique (remove-duplicates concepts :test #'string=)))
              (when (>= (length unique) 2)
                (let ((results (funcall 'memory-meditate
                                        (subseq unique 0 (min 15 (length unique)))
                                        :success t)))
                  (format nil "Meditate: ~D strengthened, ~D bridged"
                          (or (getf results :strengthened) 0)
                          (or (getf results :bridged) 0)))))))
        (error () nil))
      "(meditate: nothing to strengthen)"))

(defun %prim-models ()
  (or (handler-case (ipc-call (%sexp-to-ipc-string
                               '(:component "provider-router" :op "list-backends")))
        (error () nil))
      "(models unavailable)"))

(defun %prim-route-check (from to)
  (or (handler-case
          (ipc-call (%sexp-to-ipc-string
                     `(:component "harmonic-matrix" :op "route-allowed"
                       :from ,from :to ,to :signal 0.7 :noise 0.3)))
        (error () nil))
      "(route check unavailable)"))

;; ── Action primitives ───────────────────────────────────────────────

(defun %prim-store (content &rest kwargs &key tags)
  (declare (ignore kwargs))
  (let ((text (if (stringp content) content (princ-to-string content))))
    (handler-case (memory-put :daily text :tags (or tags '(:user-stored))) (error () nil))
    "(:ok stored)"))

(defun %prim-spawn (model &rest kwargs &key task workdir)
  "Spawn a CLI subagent. Non-blocking. Returns actor-id or :deferred."
  (declare (ignore kwargs))
  (let ((m (if (stringp model) model (princ-to-string model)))
        (t-text (or task ""))
        (wd (or workdir "")))
    (or (handler-case
            (when (fboundp 'tmux-spawn)
              (let ((actor-id (funcall 'tmux-spawn m wd t-text)))
                (if (and actor-id (>= actor-id 0))
                    (format nil "(:spawned :actor-id ~D :model \"~A\")" actor-id m)
                    "(:error \"spawn failed\")")))
          (error () nil))
        "(:error \"spawn unavailable\")")))

(defun %prim-tool (name &rest kwargs)
  "Execute any registered tool."
  (let* ((tool-name (if (stringp name) name (princ-to-string name)))
         (cmd (format nil "tool op=~A~{ ~A~}"
                      tool-name
                      (loop for (k v) on kwargs by #'cddr
                            collect (format nil "~A=~A"
                                            (string-downcase (symbol-name k))
                                            (if (stringp v) v (princ-to-string v)))))))
    (or (handler-case
            (when (fboundp '%maybe-handle-tool-command)
              (funcall '%maybe-handle-tool-command cmd))
          (error () nil))
        (format nil "(:error \"tool ~A failed\")" tool-name))))

(defun %prim-observe-route (from to &rest kwargs &key success latency-ms)
  (declare (ignore kwargs))
  (handler-case
      (ipc-call (%sexp-to-ipc-string
                 `(:component "harmonic-matrix" :op "observe-route"
                   :from ,from :to ,to
                   :success ,(if success t nil) :latency-ms ,(or latency-ms 0))))
    (error () nil))
  "(:ok observed)")

;; ── Evolution (vitruvian-gated) ─────────────────────────────────────

(defun %prim-evolve (&rest kwargs &key reason target)
  (declare (ignore kwargs))
  "Evolution request. Requires vitruvian readiness."
  (let ((ready (handler-case
                   (when (fboundp '%harmonic-plan-ready-p)
                     (funcall '%harmonic-plan-ready-p))
                 (error () nil))))
    (if ready
        (progn
          (handler-case
              (memory-put :daily
                          (format nil "Evolution requested: reason=~A target=~A" reason target)
                          :tags '(:evolution :request))
            (error () nil))
          (format nil "(:ok :evolution-requested :reason \"~A\" :target \"~A\")" reason target))
        "(:denied \"vitruvian readiness not met — chaos too high or signal too low\")")))

(defun %prim-rewrite-plan ()
  (let ((ctx (handler-case (runtime-state-harmonic-context *runtime*) (error () nil))))
    (if ctx
        (let ((plan (getf ctx :plan)))
          (if plan
              (format nil "(:rewrite-plan :ready ~A :signal ~A :noise ~A)"
                      (getf plan :ready)
                      (and (getf plan :vitruvian) (getf (getf plan :vitruvian) :signal))
                      (and (getf plan :vitruvian) (getf (getf plan :vitruvian) :noise)))
              "(no plan computed yet)"))
        "(harmonic context unavailable)")))

;;; ═══════════════════════════════════════════════════════════════════════
;;; PORT GUARD MACRO — eliminates handler-case/ready-check boilerplate
;;; ═══════════════════════════════════════════════════════════════════════

(defmacro %with-port-guard (port-name ready-fn error-prefix &body body)
  "Guard a REPL primitive with port readiness check and error handling.
   Eliminates the repeated handler-case/if-not-ready/error pattern."
  `(handler-case
       (if (not (,ready-fn))
           ,(format nil "(:error \"~A not ready\")" port-name)
           (let ((result (progn ,@body)))
             (if result (princ-to-string result)
                 ,(format nil "(:ok :~A-empty t)" error-prefix))))
     (error (e) (format nil ,(format nil "(:error \"~A: ~~A\")" error-prefix) e))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; MEMPALACE PRIMITIVES — graph-structured knowledge
;;; ═══════════════════════════════════════════════════════════════════════

(defun %prim-palace-search (query &rest kwargs &key room (limit 10))
  (declare (ignore kwargs))
  (%with-port-guard "mempalace" mempalace-port-ready-p "palace-search"
    (palace-search query :room room :limit limit)))

(defun %prim-palace-file (content room-id &rest kwargs &key tags)
  (declare (ignore kwargs))
  (%with-port-guard "mempalace" mempalace-port-ready-p "palace-file"
    (palace-file-drawer content room-id :tags tags)))

(defun %prim-palace-graph (from &rest kwargs &key (traversal "bfs") (depth 3))
  (declare (ignore kwargs))
  (%with-port-guard "mempalace" mempalace-port-ready-p "palace-graph"
    (palace-graph-query from :traversal traversal :depth depth)))

(defun %prim-palace-compress (&rest drawer-ids)
  (%with-port-guard "mempalace" mempalace-port-ready-p "palace-compress"
    (palace-compress drawer-ids)))

(defun %prim-palace-context (tier &rest kwargs &key domain query)
  (declare (ignore kwargs))
  (%with-port-guard "mempalace" mempalace-port-ready-p "palace-context"
    (palace-context tier :domain domain :query query)))

(defun %prim-palace-kg (op &rest args)
  "Knowledge graph operations: add, query, invalidate, timeline."
  (%with-port-guard "mempalace" mempalace-port-ready-p "palace-kg"
    (let* ((cmd `(:component "mempalace"
                  :op ,(concatenate 'string "kg-" (princ-to-string op))
                  ,@args))
           (reply (ipc-call (%sexp-to-ipc-string cmd))))
      (when (ipc-reply-ok-p reply) reply))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; TERRAPHON PRIMITIVES — platform datamining tools
;;; ═══════════════════════════════════════════════════════════════════════

(defun %prim-datamine (lode-id &rest args)
  "Datamine locally using a specific lode."
  (%with-port-guard "terraphon" terraphon-port-ready-p "datamine"
    (apply #'terraphon-datamine lode-id args)))

(defun %prim-datamine-remote (node-label lode-id &rest args)
  "Datamine on a remote node via Tailscale mesh. Routes through NodeRPC DatamineQuery."
  (handler-case
      (let* ((args-str (format nil "~{~A~^ ~}" args))
             (reply (ipc-call (%sexp-to-ipc-string
                                `(:component "tailnet" :op "send"
                                  :to ,node-label
                                  :payload ,(%sexp-to-ipc-string
                                              `(:method "datamine-query"
                                                :query-id ,(format nil "dq-~A" (get-universal-time))
                                                :lode-id ,lode-id
                                                :args ,args-str
                                                :timeout-ms 5000
                                                :compress t)))))))
        (if (and reply (ipc-reply-ok-p reply))
            reply
            (format nil "(:error \"datamine-remote to ~A failed: ~A\")" node-label (or reply "no reply"))))
    (error (e) (format nil "(:error \"datamine-remote: ~A\")" e))))

(defun %prim-datamine-for (&rest kwargs &key domain query (prefer "cascade"))
  (declare (ignore kwargs))
  (%with-port-guard "terraphon" terraphon-port-ready-p "datamine-for"
    (terraphon-datamine-for :domain domain :query query :prefer prefer)))

(defun %prim-lodes ()
  "List all available datamining tools."
  (%with-port-guard "terraphon" terraphon-port-ready-p "lodes"
    (terraphon-lodes)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; WEB + PYTHON PRIMITIVES — datamining and document processing
;;; ═══════════════════════════════════════════════════════════════════════

(defun %prim-fetch-url (url)
  "Fetch URL content via curl + markitdown for clean text extraction.
   Returns readable text, not raw HTML — Kolmogorov-minimal for the model."
  (if (and url (stringp url) (> (length url) 5))
      (or (handler-case
              ;; Use markitdown for clean extraction if available
              (let ((result (workspace-exec "python3"
                              (list "-c"
                                    (format nil "
import sys
try:
    from markitdown import MarkItDown
    m = MarkItDown()
    r = m.convert_url('~A')
    print(r.text_content[:6000])
except Exception as e:
    # Fallback: curl + basic text
    import subprocess
    html = subprocess.run(['curl', '-sL', '-m', '15', '~A'], capture_output=True, text=True).stdout
    # Strip HTML tags naively
    import re
    text = re.sub('<[^>]+>', ' ', html)
    text = re.sub('\\\\s+', ' ', text).strip()
    print(text[:6000])
" url url)))))
                (when (and result (stringp result) (> (length result) 0))
                  result))
            (error () nil))
          (format nil "(fetch error: ~A)" url))
      "(fetch: url required)"))

(defun %prim-browse (url &optional (macro "text") (arg ""))
  "Browser tool: fetch URL with extraction macro. Pure functional query interface.
   Macros: text, links, headings, tables, markdown, smart, structured, title, meta
   Model gets ONLY the extracted data, not the whole page. Kolmogorov-minimal."
  (if (and url (stringp url) (> (length url) 5))
      (or (handler-case
              (let* ((macro-str (if (stringp macro) macro (princ-to-string macro)))
                     (arg-str (if (stringp arg) arg (princ-to-string arg)))
                     (cmd (format nil "tool op=browser_search url=~A macro=~A~A"
                                  url macro-str
                                  (if (> (length arg-str) 0) (format nil " arg=~A" arg-str) ""))))
                (when (fboundp '%maybe-handle-tool-command)
                  (funcall '%maybe-handle-tool-command cmd)))
            (error (e)
              ;; Fallback: use fetch-url
              (%prim-fetch-url url)))
          (format nil "(browse error: ~A)" url))
      "(browse: url required)"))

(defun %prim-python (script)
  "Execute Python script via tmux. Returns stdout.
   For document processing, web scraping, data analysis."
  (if (and script (stringp script) (> (length script) 0))
      (or (handler-case
              (workspace-exec "python3" (list "-c" script))
            (error (e) (format nil "(python error: ~A)" e)))
          "(python: execution failed)")
      "(python: script required)"))

(defun %prim-search-web (query)
  "Search the web. Uses existing search-exa or search-brave tool."
  (if (and query (stringp query) (> (length query) 0))
      (or (handler-case
              (let ((reply (ipc-call
                            (%sexp-to-ipc-string
                             `(:component "workspace" :op "exec"
                               :command "curl"
                               :args ("-sL" "-m" "10"
                                      ,(format nil "https://html.duckduckgo.com/html/?q=~A"
                                               (substitute #\+ #\Space query))))))))
                (when (and reply (ipc-reply-ok-p reply))
                  (let ((text (ipc-extract-value reply)))
                    (when text (subseq text 0 (min (length text) 4000))))))
            (error () nil))
          (format nil "(search: no results for ~A)" query))
      "(search: query required)"))

(defun %prim-convert-doc (path)
  "Convert document to text using markitdown (if installed) or cat.
   Handles: PDF, DOCX, PPTX, XLSX, HTML, etc."
  (if (and path (stringp path) (> (length path) 0))
      (or (handler-case
              ;; Try markitdown first
              (let ((result (workspace-exec "python3"
                              (list "-c"
                                    (format nil "from markitdown import MarkItDown; m=MarkItDown(); r=m.convert('~A'); print(r.text_content[:8000])"
                                            path)))))
                (when (and result (stringp result) (> (length result) 0))
                  result))
            (error () nil))
          ;; Fallback: just cat the file
          (handler-case (workspace-read-file path :limit 200) (error () "(convert-doc: failed)")))
      "(convert-doc: path required)"))
