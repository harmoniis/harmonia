;;; repl-primitives.lisp — REPL primitive implementations (%prim-* functions).

(in-package :harmonia)

;;; Forward declarations for variables defined in repl-loop.lisp (loaded after us)
(defvar *repl-model-perf* nil)
(defvar *routing-tier* :auto)

(defun %temp-path (name)
  "Derive a temp file path from state-root. Falls back to /tmp."
  (let ((root (or (let ((sr (handler-case (sb-ext:posix-getenv "HARMONIA_STATE_ROOT")
                           (error () nil))))
                    (when (and sr (stringp sr) (> (length sr) 0))
                      (let ((tmp (concatenate 'string sr "/tmp")))
                        (ensure-directories-exist (concatenate 'string tmp "/"))
                        tmp)))
                  "/tmp")))
    (format nil "~A/~A" root name)))

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
  "L1 global context: derived from live state and *primitive-dispatch*.
   The model reads this FIRST to understand how to proceed."
  (let ((basin (or (handler-case (%prim-basin) (error () nil)) "?"))
        (mem-count (hash-table-count *memory-store*))
        (concept-count (hash-table-count *memory-concept-nodes*))
        (palace-ok (and (fboundp 'mempalace-port-ready-p) (funcall 'mempalace-port-ready-p)))
        (tier (if (boundp '*routing-tier*) (symbol-name *routing-tier*) "auto")))
    ;; TOOLS section: derived from *primitive-dispatch*
    (let ((tool-names '()))
      (maphash (lambda (name prim)
                 (declare (ignore prim))
                 (when (member name '(exec read-file grep list-files write-file
                                      fetch python search convert datamine browse markitdown))
                   (push (string-downcase (symbol-name name)) tool-names)))
               *primitive-dispatch*)
      (format nil "GLOBAL CONTEXT:
basin=~A concepts=~D memories=~D palace=~A tier=~A
CHAIN: (field)->understand -> (recall q)->user-data -> (status)->system -> (respond answer)
TOOLS: ~{~A~^ ~}
MEMORY: (recall q) searches palace for user knowledge. (store text) saves to palace.
SYSTEM: (status) (basin) (introspect) (models) for self-knowledge.
EXPLORE: (exec cmd) (fetch url) (python code) (search q) (datamine lode) for new data."
              basin mem-count concept-count (if palace-ok "ready" "offline") tier
              (sort tool-names #'string<)))))

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
  "Fetch URL content. Tries Rust hfetch via IPC first, falls back to Python markitdown."
  (if (and url (stringp url) (> (length url) 5))
      (or
       ;; Primary: route through Rust hfetch tool via IPC
       (handler-case
           (let ((reply (ipc-call (%sexp-to-ipc-string
                                    `(:component "workspace" :op "exec"
                                      :cmd "curl" :args ,(format nil "-sL -m 15 ~A" url))))))
             (when (and reply (ipc-reply-ok-p reply))
               (let ((text (ipc-extract-value reply)))
                 (when (and text (stringp text) (> (length text) 0))
                   (subseq text 0 (min (length text) 8000))))))
         (error () nil))
       ;; Fallback: Python markitdown via temp file
       (handler-case
           (let ((script (format nil "
import sys
try:
    from markitdown import MarkItDown
    m = MarkItDown()
    r = m.convert_url('~A')
    print(r.text_content[:8000])
except Exception as e:
    import subprocess
    html = subprocess.run(['curl', '-sL', '-m', '15', '~A'], capture_output=True, text=True).stdout
    import re
    text = re.sub('<[^>]+>', ' ', html)
    text = re.sub(r'\\s+', ' ', text).strip()
    print(text[:8000])
" url url)))
             (workspace-write-file (%temp-path "harmonia-fetch.py") script)
             (let ((result (workspace-exec "python3" (list (%temp-path "harmonia-fetch.py")))))
               (when (and result (stringp result) (> (length result) 0))
                 result)))
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
  "Execute Python script. Large scripts written to temp file to avoid
   escaping issues with inline -c. Pure functional: no side effects beyond exec."
  (if (and script (stringp script) (> (length script) 0))
      (or (handler-case
              (if (or (> (length script) 200)
                      (position #\Newline script)
                      (position #\' script))
                  ;; Large/complex scripts: write to temp file, execute file
                  (progn
                    (workspace-write-file (%temp-path "harmonia-py-exec.py") script)
                    (workspace-exec "python3" (list (%temp-path "harmonia-py-exec.py"))))
                  ;; Short simple scripts: inline -c
                  (workspace-exec "python3" (list "-c" script)))
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

;;; ═══════════════════════════════════════════════════════════════════════
;;; PRIMITIVE REGISTRATION — defprimitive populates *primitive-dispatch*
;;; ═══════════════════════════════════════════════════════════════════════
;;;
;;; NOTE: IPC calls within primitives are synchronous. This is correct
;;; for the current REPL model: forms evaluate sequentially, and a form's
;;; result may be consumed by the next form via let bindings. Async IPC
;;; would require a promise/future evaluator — a separate evolution target.

;; ── Self-discovery ────────────────────────────────────────────────────
(defprimitive env "()" "All available primitives." (%prim-env))
(defprimitive field "()" "L1 global context map." (%prim-field))
(defprimitive recall "(query &key limit max-chars verbatim tags since)" "Search palace/chronicle." (apply #'%prim-recall args))
(defprimitive ipc "(component op &rest kwargs)" "Generic IPC query." (apply #'%prim-ipc args))
(defprimitive introspect "()" "Runtime identity." (%prim-introspect))
(defprimitive status "()" "Runtime state." (%prim-status))
(defprimitive chaos-risk "()" "Current chaos risk." (%prim-chaos-risk))
(defprimitive basin "()" "Memory field basin status." (%prim-basin))
(defprimitive models "()" "Available LLM backends." (%prim-models))
(defprimitive route-check "(from to)" "Check harmonic matrix route." (apply #'%prim-route-check args))

;; ── Composition (common names models expect) ──────────────────────────
(defprimitive format "(fmt &rest args)" "Format a string." (apply #'format nil args))
(defprimitive str "(&rest parts)" "Join parts into string." (apply #'concatenate 'string (mapcar #'princ-to-string args)))
(defprimitive cat "(&rest parts)" "Alias for str." (apply #'concatenate 'string (mapcar #'princ-to-string args)))
(defprimitive concat "(&rest parts)" "Alias for str." (apply #'concatenate 'string (mapcar #'princ-to-string args)))
(defprimitive join "(list)" "Join list with spaces." (format nil "~{~A~^ ~}" (first args)))
(defprimitive getf "(plist key)" "Property list access." (getf (first args) (second args)))
(defprimitive length "(seq)" "Length of sequence." (length (first args)))
(defprimitive subseq "(seq start &optional end)" "Subsequence." (apply #'subseq args))
(defprimitive concatenate "(&rest strings)" "Concatenate strings." (apply #'concatenate 'string args))
(defprimitive string-downcase "(s)" "Downcase string." (string-downcase (first args)))
(defprimitive string-upcase "(s)" "Upcase string." (string-upcase (first args)))
(defprimitive to-string "(x)" "Convert to string." (princ-to-string (first args)))
(defprimitive princ-to-string "(x)" "Convert to string." (princ-to-string (first args)))

;; ── Arithmetic & comparison ───────────────────────────────────────────
(defprimitive + "(&rest nums)" "Addition." (apply #'+ args))
(defprimitive - "(&rest nums)" "Subtraction." (apply #'- args))
(defprimitive * "(&rest nums)" "Multiplication." (apply #'* args))
(defprimitive / "(a b)" "Division." (/ (first args) (second args)))
(defprimitive > "(a b)" "Greater than." (> (first args) (second args)))
(defprimitive < "(a b)" "Less than." (< (first args) (second args)))
(defprimitive = "(a b)" "Numeric equal." (= (first args) (second args)))
(defprimitive not "(x)" "Logical not." (not (first args)))
(defprimitive and "(&rest vals)" "Logical and." (every #'identity args))
(defprimitive or "(&rest vals)" "Logical or." (some #'identity args))

;; ── List operations ───────────────────────────────────────────────────
(defprimitive list "(&rest items)" "Construct list." args)
(defprimitive car "(list)" "First element." (car (first args)))
(defprimitive cdr "(list)" "Rest of list." (cdr (first args)))
(defprimitive cadr "(list)" "Second element." (cadr (first args)))
(defprimitive caddr "(list)" "Third element." (caddr (first args)))
(defprimitive nth "(n list)" "Nth element." (nth (first args) (second args)))
(defprimitive first "(list)" "First element." (first (first args)))
(defprimitive second "(list)" "Second element." (second (first args)))
(defprimitive third "(list)" "Third element." (third (first args)))
(defprimitive fourth "(list)" "Fourth element." (fourth (first args)))
(defprimitive rest "(list)" "Rest of list." (rest (first args)))
(defprimitive last "(list)" "Last element." (car (last (first args))))
(defprimitive cons "(head tail)" "Construct pair." (cons (first args) (second args)))
(defprimitive append "(a b)" "Append lists." (append (first args) (second args)))
(defprimitive mapcar "(fn list)" "Map function over list." (mapcar (first args) (second args)))
(defprimitive remove-if "(pred list)" "Remove matching." (remove-if (first args) (second args)))
(defprimitive assoc "(key alist)" "Association list lookup." (assoc (first args) (second args)))

;; ── Workspace tools (Rust actors) ─────────────────────────────────────
(defprimitive read-file "(path &optional offset limit)" "Read file." (apply #'%prim-read-file args))
(defprimitive grep "(pattern &optional path)" "Search files." (apply #'%prim-grep args))
(defprimitive list-files "(&optional path)" "List directory." (apply #'%prim-list-files args))
(defprimitive file-exists "(path)" "Check file exists." (%prim-file-exists (first args)))
(defprimitive file-info "(path)" "File metadata." (%prim-file-info (first args)))
(defprimitive write-file "(path content)" "Write file." (apply #'%prim-write-file args))
(defprimitive append-file "(path content)" "Append to file." (apply #'%prim-append-file args))
(defprimitive exec "(cmd &rest args)" "Execute shell command." (apply #'%prim-exec args))

;; ── Action primitives ─────────────────────────────────────────────────
(defprimitive store "(content &key tags)" "Store to palace." (apply #'%prim-store args))
(defprimitive spawn "(model &key task workdir)" "Spawn subagent." (apply #'%prim-spawn args))
(defprimitive tool "(name &rest kwargs)" "Execute tool." (apply #'%prim-tool args))
(defprimitive observe-route "(from to &key success latency-ms)" "Log route." (apply #'%prim-observe-route args))

;; ── Memory field maintenance ──────────────────────────────────────────
(defprimitive dream "()" "Field self-maintenance." (%prim-dream))
(defprimitive meditate "()" "Hebbian edge strengthening." (%prim-meditate))

;; ── Evolution (vitruvian-gated) ───────────────────────────────────────
(defprimitive evolve "(&key reason target)" "Request evolution." (apply #'%prim-evolve args))
(defprimitive rewrite-plan "()" "Show rewrite plan." (%prim-rewrite-plan))

;; ── MemPalace (graph-structured knowledge) ────────────────────────────
(defprimitive palace-search "(query &key room limit)" "Search palace." (apply #'%prim-palace-search args))
(defprimitive palace-file "(content room-id &key tags)" "File to drawer." (apply #'%prim-palace-file args))
(defprimitive palace-graph "(from &key traversal depth)" "Graph query." (apply #'%prim-palace-graph args))
(defprimitive palace-compress "(&rest drawer-ids)" "Compress drawers." (apply #'%prim-palace-compress args))
(defprimitive palace-context "(tier &key domain query)" "Palace context." (apply #'%prim-palace-context args))
(defprimitive palace-kg "(op &rest args)" "Knowledge graph ops." (apply #'%prim-palace-kg args))

;; ── Terraphon (platform datamining) ───────────────────────────────────
(defprimitive datamine "(lode-id &rest args)" "Datamine locally." (apply #'%prim-datamine args))
(defprimitive datamine-remote "(node-label lode-id &rest args)" "Datamine remotely." (apply #'%prim-datamine-remote args))
(defprimitive datamine-for "(&key domain query prefer)" "Datamine by domain." (apply #'%prim-datamine-for args))
(defprimitive lodes "()" "List datamining tools." (%prim-lodes))

;; ── Web + Python ──────────────────────────────────────────────────────
(defprimitive fetch-url "(url)" "Fetch URL content." (%prim-fetch-url (first args)))
(defprimitive fetch "(url)" "Alias for fetch-url." (%prim-fetch-url (first args)))
(defprimitive browse "(url &optional macro arg)" "Browser tool." (apply #'%prim-browse args))
(defprimitive python "(script)" "Execute Python." (%prim-python (first args)))
(defprimitive py "(script)" "Alias for python." (%prim-python (first args)))
(defprimitive search-web "(query)" "Web search." (%prim-search-web (first args)))
(defprimitive search "(query)" "Alias for search-web." (%prim-search-web (first args)))
(defprimitive convert-doc "(path)" "Convert document to text." (%prim-convert-doc (first args)))
(defprimitive convert "(path)" "Alias for convert-doc." (%prim-convert-doc (first args)))
(defprimitive markitdown "(path)" "Alias for convert-doc." (%prim-convert-doc (first args)))

;; ── Tmux interactive terminal control ──────────────────────────────────
(defprimitive tmux-create "(name &optional workdir)" "Create a tmux session."
  (let ((name (first args))
        (workdir (or (second args) ".")))
    (or (handler-case
            (workspace-exec "tmux" (list "new-session" "-d" "-s" name "-c" workdir))
          (error () nil))
        (format nil "(:error \"tmux-create ~A failed\")" name))))

(defprimitive tmux-send "(session keys)" "Send keystrokes to a tmux session."
  (let ((session (first args))
        (keys (second args)))
    (or (handler-case
            (workspace-exec "tmux" (list "send-keys" "-t" session keys "Enter"))
          (error () nil))
        (format nil "(:error \"tmux-send to ~A failed\")" session))))

(defprimitive tmux-read "(session &optional lines)" "Capture output from a tmux session pane."
  (let ((session (first args))
        (lines (or (second args) 50)))
    (or (handler-case
            (workspace-exec "tmux" (list "capture-pane" "-t" session "-p" "-S"
                                         (format nil "-~D" lines)))
          (error () nil))
        (format nil "(:error \"tmux-read ~A failed\")" session))))

(defprimitive tmux-list "()" "List all tmux sessions."
  (or (handler-case (workspace-exec "tmux" '("list-sessions")) (error () nil))
      "(no tmux sessions)"))

(defprimitive tmux-kill "(session)" "Kill a tmux session."
  (let ((session (first args)))
    (or (handler-case
            (workspace-exec "tmux" (list "kill-session" "-t" session))
          (error () nil))
        (format nil "(:error \"tmux-kill ~A failed\")" session))))

;; ── MCP A2A: bidirectional collaboration via Rust MCP actor ────────────
;; All MCP communication goes through the Rust MCP actor via IPC dispatch.
;; Lisp primitives are thin wrappers around IPC calls to the mcp component.

(defprimitive mcp-connect "(server-name &key command)" "Connect to a local MCP server (A2A)."
  (let ((name (first args))
        (cmd (getf (cdr args) :command)))
    (or (handler-case
            (ipc-call (%sexp-to-ipc-string
                        `(:component "mcp" :op "connect"
                          :server ,name ,@(when cmd (list :command cmd)))))
          (error () nil))
        (format nil "(:error \"mcp-connect ~A failed\")" name))))

(defprimitive mcp-call "(server tool-name &rest tool-args)" "Call a tool on a connected MCP peer."
  (let ((server (first args))
        (tool (second args))
        (tool-args (third args)))
    (or (handler-case
            (ipc-call (%sexp-to-ipc-string
                        `(:component "mcp" :op "call-tool"
                          :server ,server :tool ,tool
                          :arguments ,(or tool-args "{}"))))
          (error () nil))
        (format nil "(:error \"mcp-call ~A/~A failed\")" server tool))))

(defprimitive mcp-list "(&optional server)" "List tools on a connected MCP peer."
  (let ((server (or (first args) "")))
    (or (handler-case
            (ipc-call (%sexp-to-ipc-string
                        `(:component "mcp" :op "list-tools"
                          ,@(when (> (length server) 0) (list :server server)))))
          (error () nil))
        "(:error \"mcp-list failed\")")))

(defprimitive mcp-peers "()" "List all connected MCP peers."
  (or (handler-case
          (ipc-call (%sexp-to-ipc-string
                      '(:component "mcp" :op "list-peers")))
        (error () nil))
      "(:error \"mcp-peers failed\")"))

;; ── Task decomposition (recursive subagent tree) ──────────────────────
(defprimitive decompose "(&rest subtasks)" "Decompose task into parallel subtasks."
  (if (and args (> (length args) 0))
      (handler-case
          (let ((results (funcall '%decompose-and-solve args)))
            (format nil "(:decomposed ~{~A~^ ~})"
                    (mapcar (lambda (r)
                              (format nil "(:task \"~A\" :agent-id ~A :status ~A)"
                                      (or (getf r :task) "?")
                                      (or (getf r :agent-id) "nil")
                                      (or (getf r :status) :unknown)))
                            results)))
        (error (e) (format nil "(:error \"decompose: ~A\")" e)))
      "(:error \"decompose: provide subtasks\")"))

;; ── Finalize: compute *repl-primitives* from dispatch table ───────────
(setf *repl-primitives* (%compute-repl-primitives))
