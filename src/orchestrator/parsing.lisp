;;; parsing.lisp — Parsing helpers for conductor: tag extraction, number parsing, string splitting.

(in-package :harmonia)

;;; --- Safe numeric parser (replaces read-from-string on external data) ---

(defun %safe-parse-number (text)
  "Parse TEXT as a decimal number. No Lisp reader macros. Signals error on non-numeric input."
  (let ((trimmed (string-trim '(#\Space #\Tab) (or text ""))))
    (when (zerop (length trimmed)) (error "empty numeric value"))
    ;; Try integer first
    (handler-case (parse-integer trimmed :junk-allowed nil)
      (error ()
        ;; Validate characters before using reader
        (unless (every (lambda (c) (find c "0123456789.eE+-")) trimmed)
          (error "not a number: ~A" trimmed))
        (let ((*read-eval* nil) (*read-base* 10))
          (let ((val (read-from-string trimmed)))
            (unless (realp val) (error "not a number: ~A" trimmed))
            val))))))

(defun %safe-parse-policy-value (text)
  "Parse TEXT as a safe policy value: numbers, strings, keywords, or lists of these.
   No reader macros. No arbitrary code execution."
  (let ((trimmed (string-trim '(#\Space #\Tab) (or text ""))))
    (when (zerop (length trimmed)) (error "empty policy value"))
    ;; Reject reader macro attacks
    (when (search "#." trimmed)
      (error "reader macro attack detected in policy value: ~A" trimmed))
    (let ((*read-eval* nil) (*read-base* 10))
      (let ((val (read-from-string trimmed)))
        ;; Validate: only allow numbers, strings, keywords, symbols, and lists of these
        (labels ((safe-value-p (v)
                   (or (numberp v) (stringp v) (keywordp v) (symbolp v)
                       (null v)
                       (and (listp v) (every #'safe-value-p v)))))
          (unless (safe-value-p val)
            (error "unsafe policy value type: ~A" (type-of val)))
          val)))))

;;; --- Tag extraction and string splitting ---

(defun %extract-tag-value (prompt tag)
  (let* ((needle (format nil "~A=" tag))
         (start (search needle prompt :test #'char-equal)))
    (when start
      (let* ((from (+ start (length needle)))
             (space (position #\Space prompt :start from)))
        (subseq prompt from (or space (length prompt)))))))

(defun %split-by-comma (text)
  (let ((parts '())
        (start 0))
    (loop for i = (position #\, text :start start)
          do (push (string-trim '(#\Space #\Tab) (subseq text start (or i (length text)))) parts)
          if (null i) do (return)
          do (setf start (1+ i)))
    (remove-if (lambda (s) (zerop (length s))) (nreverse parts))))

(defun %split-by-char (text ch)
  (let ((parts '())
        (start 0))
    (loop for i = (position ch text :start start)
          do (push (subseq text start (or i (length text))) parts)
          if (null i) do (return)
          do (setf start (1+ i)))
    (nreverse parts)))

(defun %starts-with-p (text prefix)
  (let ((s (or text ""))
        (p (or prefix "")))
    (and (>= (length s) (length p))
         (string-equal p s :end2 (length p)))))

(defun %unix-time-ms ()
  (multiple-value-bind (sec usec) (sb-ext:get-time-of-day)
    (+ (* sec 1000) (truncate usec 1000))))

(defun %url-decode-min (text)
  ;; Minimal decoder for codemode step values passed as key=value tokens.
  (let ((s (or text "")))
    (setf s (substitute #\Space #\+ s))
    (with-output-to-string (out)
      (loop for i from 0 below (length s) do
        (let ((ch (char s i)))
          (if (and (char= ch #\%) (<= (+ i 2) (1- (length s))))
              (let* ((hex (subseq s (1+ i) (+ i 3)))
                     (code (handler-case (parse-integer hex :radix 16) (error () nil))))
                (if code
                    (progn
                      (write-char (code-char code) out)
                      (incf i 2))
                    (write-char ch out)))
              (write-char ch out)))))))
