;;; evolution-versioning.lisp — Versioned evolution snapshots (latest + versions/vN).

(in-package :harmonia)

(defparameter *evolution-doc-root* nil)
(defparameter *evolution-latest-dir* nil)
(defparameter *evolution-versions-dir* nil)
(defparameter *evolution-version-file* nil)

(defun %evolution-doc-root ()
  "Resolve evolution doc root at runtime."
  (let ((sys (or (and (fboundp 'config-get-for)
                      (funcall 'config-get-for "evolution" "system-dir" "global"))
                 (%boot-env "HARMONIA_SYSTEM_DIR"))))
    (if (and sys (> (length sys) 0))
        (let ((base (if (char= (char sys (1- (length sys))) #\/)
                        sys
                        (concatenate 'string sys "/"))))
          (pathname (concatenate 'string base "evolution/")))
        (merge-pathnames "../boot/evolution/"
                         (make-pathname :name nil :type nil :defaults *boot-file*)))))
(defparameter *evolution-current-version* 0)
(defparameter *evolution-latest-files-cache* '())
(defparameter *legacy-evolution-latest-files*
  '("current-state.sexp" "scorecard.sexp" "changelog.sexp" "rewrite-roadmap.sexp"
    ;; Legacy fallbacks for old layouts
    "current-state.md" "scorecard.md" "changelog.md" "rewrite-roadmap.md"))

(defun %ensure-dir (dir)
  (ensure-directories-exist (merge-pathnames ".keep" dir))
  dir)

(defun %copy-file-bytes (src dst)
  (with-open-file (in src :direction :input
                          :element-type '(unsigned-byte 8))
    (ensure-directories-exist dst)
    (with-open-file (out dst :direction :output
                             :if-exists :supersede
                             :if-does-not-exist :create
                             :element-type '(unsigned-byte 8))
      (let ((buf (make-array 4096 :element-type '(unsigned-byte 8))))
        (loop for n = (read-sequence buf in)
              while (> n 0)
              do (write-sequence buf out :end n))))))

(defun %read-file-text (path)
  (with-open-file (in path :direction :input)
    (let ((content (make-string (file-length in))))
      (read-sequence content in)
      content)))

(defun %file-byte-size (path)
  (with-open-file (in path :direction :input
                           :element-type '(unsigned-byte 8))
    (file-length in)))

(defun %latest-doc-files ()
  "Return latest evolution doc files. Prefers .sexp; falls back to .md."
  (let ((sexp-files (directory (merge-pathnames "*.sexp" *evolution-latest-dir*))))
    (sort (if sexp-files
              sexp-files
              (directory (merge-pathnames "*.md" *evolution-latest-dir*)))
          #'string<
          :key #'namestring)))

(defun %latest-snapshot-files ()
  (sort (append (directory (merge-pathnames "*.md" *evolution-latest-dir*))
                (directory (merge-pathnames "*.sexp" *evolution-latest-dir*)))
        #'string<
        :key #'namestring))

(defun %path-last-dir-name (path)
  (let* ((dirs (pathname-directory (pathname path)))
         (last (car (last dirs))))
    (typecase last
      (string last)
      (symbol (string-downcase (symbol-name last)))
      (t (princ-to-string last)))))

(defun %parse-version-from-dir (path)
  (let ((name (%path-last-dir-name path)))
    (when (and (> (length name) 1)
               (char-equal (char name 0) #\v))
      (ignore-errors
        (parse-integer (subseq name 1))))))

(defun evolution-list-versions ()
  (let* ((raw (append (directory (merge-pathnames "v*/" *evolution-versions-dir*))
                      (directory (merge-pathnames "v*" *evolution-versions-dir*))))
         (dirs (remove-duplicates raw :key #'namestring :test #'string=)))
    (sort (remove nil (mapcar #'%parse-version-from-dir dirs)) #'<)))

(defun %read-version-file ()
  (when (probe-file *evolution-version-file*)
    (with-open-file (in *evolution-version-file* :direction :input)
      (let ((*read-eval* nil))
        (let ((value (read in nil nil)))
          (if (and (integerp value) (>= value 0)) value nil))))))

(defun %write-version-file (version)
  (ensure-directories-exist *evolution-version-file*)
  (with-open-file (out *evolution-version-file* :direction :output
                                             :if-exists :supersede
                                             :if-does-not-exist :create)
    (let ((*print-pretty* t))
      (prin1 version out)
      (terpri out)))
  version)

(defun %version-dir (version)
  (merge-pathnames (format nil "v~D/" version) *evolution-versions-dir*))

(defun %snapshot-latest-to-version (version)
  (let ((target (%version-dir version)))
    (%ensure-dir target)
    (dolist (src (%latest-snapshot-files))
      (%copy-file-bytes src (merge-pathnames (file-namestring src) target)))
    target))

(defun %latest-has-docs-p ()
  (not (null (%latest-doc-files))))

(defun %migrate-legacy-evolution-layout ()
  (%ensure-dir *evolution-latest-dir*)
  (dolist (name *legacy-evolution-latest-files*)
    (let ((src (merge-pathnames name *evolution-doc-root*))
          (dst (merge-pathnames name *evolution-latest-dir*)))
      (when (probe-file src)
        (unless (probe-file dst)
          (%copy-file-bytes src dst))))))

(defun %refresh-evolution-latest-cache ()
  (setf *evolution-latest-files-cache*
        (mapcar (lambda (path)
                  (list :file (file-namestring path)
                        :bytes (%file-byte-size path)))
                (%latest-snapshot-files))))

(defun %append-latest-changelog-entry (version reason note)
  "Prepend a new entry to changelog.sexp (list of version records, newest first)."
  (let ((path (merge-pathnames "changelog.sexp" *evolution-latest-dir*)))
    (multiple-value-bind (_sec _min _hour day month year)
        (decode-universal-time (get-universal-time))
      (declare (ignore _sec _min _hour))
      (let ((entry (list :version version
                         :date (format nil "~4,'0D-~2,'0D-~2,'0D" year month day)
                         :trigger (or reason :unknown)
                         :note note))
            (existing (when (probe-file path)
                        (handler-case
                            (with-open-file (s path :direction :input)
                              (let ((*read-eval* nil)) (read s)))
                          (error () nil)))))
        (ensure-directories-exist path)
        (with-open-file (out path :direction :output
                                  :if-exists :supersede
                                  :if-does-not-exist :create)
          (let ((*print-pretty* t) (*print-right-margin* 120))
            (prin1 (cons entry (if (listp existing) existing nil)) out)
            (terpri out)))))))

(defun %latest-changelog-max-version ()
  "Find the highest version number in the changelog.  Tries .sexp first, falls back to .md."
  (let ((sexp-path (merge-pathnames "changelog.sexp" *evolution-latest-dir*))
        (md-path (merge-pathnames "changelog.md" *evolution-latest-dir*))
        (max-v 0))
    (cond
      ;; Sexp changelog: list of plists, each with :version
      ((probe-file sexp-path)
       (handler-case
           (with-open-file (in sexp-path :direction :input)
             (let* ((*read-eval* nil)
                    (entries (read in nil nil)))
               (when (listp entries)
                 (dolist (entry entries)
                   (let ((v (getf entry :version)))
                     (when (and (integerp v) (> v max-v))
                       (setf max-v v)))))))
         (error () nil)))
      ;; Legacy md changelog: scan for "## v<N>" headings
      ((probe-file md-path)
       (with-open-file (in md-path :direction :input)
         (loop for line = (read-line in nil nil)
               while line do
               (let ((pos (search "## v" line :test #'char-equal)))
                 (when pos
                   (let* ((start (+ pos 4))
                          (end start))
                     (loop while (and (< end (length line))
                                       (digit-char-p (char line end)))
                           do (incf end))
                     (when (> end start)
                       (let ((parsed (ignore-errors (parse-integer line :start start :end end))))
                         (when (and parsed (> parsed max-v))
                           (setf max-v parsed)))))))))))
    max-v))

(defun evolution-current-version ()
  *evolution-current-version*)

(defun evolution-load-latest-snapshot ()
  "Return latest evolution docs as a list of (:file ... :content ...).
   Loads .sexp files as structured data, .md files as raw text."
  (let ((docs '()))
    (dolist (path (%latest-doc-files))
      (let ((ext (pathname-type path)))
        (push (list :file (file-namestring path)
                    :content (if (string-equal ext "sexp")
                                 (handler-case
                                     (with-open-file (s path :direction :input)
                                       (let ((*read-eval* nil)) (read s)))
                                   (error () (%read-file-text path)))
                                 (%read-file-text path)))
              docs)))
    (nreverse docs)))

(defun init-evolution-versioning ()
  "Initialize evolution docs layout and load latest version metadata at boot."
  (setf *evolution-doc-root* (%evolution-doc-root))
  (setf *evolution-latest-dir* (merge-pathnames "latest/" *evolution-doc-root*))
  (setf *evolution-versions-dir* (merge-pathnames "versions/" *evolution-doc-root*))
  (setf *evolution-version-file* (merge-pathnames "version.sexp" *evolution-doc-root*))
  (%ensure-dir *evolution-doc-root*)
  (%ensure-dir *evolution-latest-dir*)
  (%ensure-dir *evolution-versions-dir*)
  (%migrate-legacy-evolution-layout)

  (let* ((versions (evolution-list-versions))
         (max-version (if versions (car (last versions)) 0))
         (from-file (or (%read-version-file) 0))
         (from-changelog (%latest-changelog-max-version))
         (resolved (max max-version from-file from-changelog)))
    ;; If latest has docs but versions/ is empty, seed the first immutable snapshot
    ;; with the resolved lineage version (e.g., changelog already at v4).
    (when (and (zerop max-version)
               (> resolved 0)
               (%latest-has-docs-p))
      (%snapshot-latest-to-version resolved))
    ;; First bootstrap: if latest exists but no version has ever been snapshotted,
    ;; seed v1 from latest so history is navigable from boot one.
    (when (and (zerop resolved) (%latest-has-docs-p))
      (%snapshot-latest-to-version 1)
      (setf resolved 1))
    (setf *evolution-current-version* resolved)
    (%write-version-file resolved)
    (%refresh-evolution-latest-cache)
    (when *runtime*
      (runtime-log *runtime* :evolution-version-init
                   (list :version resolved
                         :latest-files (length *evolution-latest-files-cache*)))))

  *evolution-current-version*)

(defun evolution-snapshot-latest (&key reason note)
  "Create a new immutable snapshot from latest/ into versions/vN and bump version."
  (let* ((current (or *evolution-current-version* 0))
         (next (1+ current)))
    (when (fboundp 'signalograd-checkpoint-latest)
      (signalograd-checkpoint-latest :runtime *runtime*))
    (%append-latest-changelog-entry next reason note)
    (let ((path (%snapshot-latest-to-version next)))
      (setf *evolution-current-version* next)
      (%write-version-file next)
      (%refresh-evolution-latest-cache)
      (when *runtime*
        (runtime-log *runtime* :evolution-version-snapshot
                     (list :version next :reason reason :note note :path (namestring path))))
      (list :version next :path (namestring path)))))
