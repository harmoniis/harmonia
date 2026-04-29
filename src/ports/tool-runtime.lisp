;;; tool-runtime.lisp — Port: search tool dispatch via IPC.
;;;
;;; Search tools (exa, brave) dispatched through workspace exec to the
;;; Rust tool binaries. Grok live search as final fallback via LLM.

(in-package :harmonia)

(defparameter *tool-libs* (make-hash-table :test 'equal))

(defun init-tool-runtime-port ()
  "Initialize tool runtime. Verify search tools are accessible."
  (let ((exa-ok (handler-case (workspace-exec "which" '("search-exa")) (error () nil)))
        (brave-ok (handler-case (workspace-exec "which" '("search-brave")) (error () nil))))
    (%log :info "tool-runtime" "Search tools: exa=~A brave=~A"
          (if exa-ok "available" "missing")
          (if brave-ok "available" "missing"))
    t))

(defun tool-runtime-list ()
  "Return list of loaded tool names."
  (let ((names '()))
    (maphash (lambda (k v) (declare (ignore v)) (push k names)) *tool-libs*)
    (nreverse names)))

(defun search-exa (query)
  "Search via Exa API. Routes through workspace exec to the search-exa binary,
   or falls back to IPC if the tool is registered as an actor."
  (let ((q (if (stringp query) query (princ-to-string query))))
    (or (handler-case
            (let ((reply (ipc-call (%sexp-to-ipc-string
                                     `(:component "workspace" :op "exec"
                                       :cmd "sh"
                                       :args ("-c" ,(format nil "curl -sL -H 'x-api-key: '$(cat ~/.harmoniis/harmonia/vault.db 2>/dev/null || echo '') -H 'Content-Type: application/json' -d '{\"query\":\"~A\",\"numResults\":5}' https://api.exa.ai/search 2>/dev/null | head -c 4000" q)))))))
              (when (and reply (ipc-reply-ok-p reply))
                (ipc-extract-value reply)))
          (error () nil))
        (error "exa search failed for: ~A" query))))

(defun search-brave (query)
  "Search via Brave Search API. Routes through workspace exec."
  (let ((q (if (stringp query) query (princ-to-string query))))
    (or (handler-case
            (let ((reply (ipc-call (%sexp-to-ipc-string
                                     `(:component "workspace" :op "exec"
                                       :cmd "sh"
                                       :args ("-c" ,(format nil "curl -sL -H 'X-Subscription-Token: '$(cat ~/.harmoniis/harmonia/vault.db 2>/dev/null || echo '') 'https://api.search.brave.com/res/v1/web/search?q=~A&count=5' 2>/dev/null | head -c 4000" q)))))))
              (when (and reply (ipc-reply-ok-p reply))
                (ipc-extract-value reply)))
          (error () nil))
        (error "brave search failed for: ~A" query))))

(defun %grok-live-search-prompt (query)
  (let ((template (load-prompt :evolution :grok-live-search nil
                   "You are the truth-seeking search subagent. Use live web and X search when useful. Prioritize factual accuracy over style.

Query: ~A

Return concise markdown with these headings only: Summary, Evidence, Uncertainty. Include source links or domains when available.")))
    (format nil template query)))

(defun %preferred-truth-seeking-model ()
  "Return the first model with :truth-seeking feature, or fallback."
  (or (and (fboundp '%truth-seeking-models)
           (car (funcall '%truth-seeking-models)))
      "x-ai/grok-4.1-fast"))

(defun search-grok-live (query)
  (backend-complete (%grok-live-search-prompt query)
                    (%preferred-truth-seeking-model)))

(defun search-web (query)
  (harmonic-matrix-route-or-error "orchestrator" "search-exa")
  (handler-case
      (let ((res (search-exa query)))
        (harmonic-matrix-observe-route "orchestrator" "search-exa" t 1)
        (harmonic-matrix-observe-route "search-exa" "memory" t 1)
        res)
    (error (_)
      (declare (ignore _))
      (harmonic-matrix-observe-route "orchestrator" "search-exa" nil 1)
      (harmonic-matrix-route-or-error "orchestrator" "search-brave")
      (handler-case
          (let ((res (search-brave query)))
            (harmonic-matrix-observe-route "orchestrator" "search-brave" t 1)
            (harmonic-matrix-observe-route "search-brave" "memory" t 1)
            res)
        (error (__)
          (declare (ignore __))
          (harmonic-matrix-observe-route "orchestrator" "search-brave" nil 1)
          (harmonic-matrix-route-or-error "orchestrator" "provider-router")
          (let ((res (search-grok-live query)))
            (harmonic-matrix-observe-route "orchestrator" "provider-router" t 1)
            (harmonic-matrix-observe-route "provider-router" "memory" t 1)
            res))))))
