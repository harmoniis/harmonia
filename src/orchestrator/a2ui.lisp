;;; a2ui.lisp — A2UI component catalog loading, injection, template matching.

(in-package :harmonia)

;;; --- A2UI component catalog ---

(defvar *a2ui-catalog-cache* nil
  "Cached A2UI component catalog, loaded lazily from config/a2ui-catalog.sexp.")

(defun %load-a2ui-catalog ()
  "Load or return cached A2UI component catalog."
  (or *a2ui-catalog-cache*
      (let ((path (merge-pathnames "config/a2ui-catalog.sexp"
                                   (or (config-get-for "conductor" "state-root" "global")
                                       (namestring (user-homedir-pathname))))))
        (handler-case
            (with-open-file (in path :direction :input :if-does-not-exist nil)
              (when in
                (let ((content (make-string (file-length in))))
                  (read-sequence content in)
                  (setf *a2ui-catalog-cache* (string-trim '(#\Space #\Newline #\Tab) content)))))
          (error () nil)))))

(defun %a2ui-component-names ()
  "Extract a short summary of available A2UI component names for LLM context injection."
  (let ((catalog (%load-a2ui-catalog)))
    (if catalog
        (let ((names '())
              (pos 0))
          (loop
            (let ((start (search ":name \"" catalog :start2 pos)))
              (unless start (return))
              (let* ((from (+ start 7))
                     (end (position #\" catalog :start from)))
                (when end
                  (push (subseq catalog from end) names))
                (setf pos (1+ (or end from))))))
          (format nil "~{~A~^, ~}" (nreverse names)))
        "")))

(defun %a2ui-extract-text (payload)
  "Extract plain text from an A2UI component payload for text-only frontends.
   Best-effort: looks for text/body/label fields in the payload string."
  (or (handler-case
     (let ((text-start (search "\"text\":\"" payload)
   (error () nil)))
          (when text-start
            (let* ((from (+ text-start 8))
                   (to (position #\" payload :start from)))
              (when to (subseq payload from to))))))
      (handler-case

          (let ((body-start (search "\"body\":\"" payload)

        (error () nil)))
          (when body-start
            (let* ((from (+ body-start 8))
                   (to (position #\" payload :start from)))
              (when to (subseq payload from to))))))
      payload))
