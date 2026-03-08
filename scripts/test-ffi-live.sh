#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] build core dylibs"
cargo build --release -p harmonia-http -p harmonia-recovery -p harmonia-rust-forge -p harmonia-fs -p harmonia-browser -p harmonia-cron-scheduler

echo "[2/3] run core ffi checks from SBCL"
WORKDIR="$(mktemp -d /tmp/harmonia-core-live-XXXXXX)"
FSROOT="$WORKDIR/fsroot"
RECLOG="$WORKDIR/recovery.log"
PUSHLOG="$WORKDIR/push.log"
mkdir -p "$FSROOT"

HARMONIA_FS_ROOT="$FSROOT" HARMONIA_RECOVERY_LOG="$RECLOG" sbcl --disable-debugger \
  --eval '(load #P"~/quicklisp/setup.lisp")' \
  --eval '(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_http.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_recovery.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_rust_forge.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_fs.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_browser.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_cron_scheduler.dylib")' \
  --eval '(cffi:defcfun ("harmonia_http_request" hreq) :pointer (method :string) (url :string))' \
  --eval '(cffi:defcfun ("harmonia_http_free_string" hfree) :void (p :pointer))' \
  --eval '(cffi:defcfun ("harmonia_recovery_record" rrec) :int (kind :string) (detail :string))' \
  --eval '(cffi:defcfun ("harmonia_recovery_tail_lines" rtail) :pointer (limit :int))' \
  --eval '(cffi:defcfun ("harmonia_recovery_free_string" rfree) :void (p :pointer))' \
  --eval '(cffi:defcfun ("harmonia_rust_forge_build_package" fbuild) :int (workdir :string) (pkg :string))' \
  --eval '(cffi:defcfun ("harmonia_fs_write" fsw) :int (path :string) (content :string))' \
  --eval '(cffi:defcfun ("harmonia_fs_read" fsr) :pointer (path :string))' \
  --eval '(cffi:defcfun ("harmonia_fs_free_string" fsfree) :void (p :pointer))' \
  --eval '(cffi:defcfun ("harmonia_browser_fetch_title" btitle) :pointer (url :string))' \
  --eval '(cffi:defcfun ("harmonia_browser_free_string" bfree) :void (p :pointer))' \
  --eval '(cffi:defcfun ("harmonia_cron_scheduler_reset" creset) :int)' \
  --eval '(cffi:defcfun ("harmonia_cron_scheduler_add_job" cadd) :int (name :string) (interval :int))' \
  --eval '(cffi:defcfun ("harmonia_cron_scheduler_due_jobs" cdue) :pointer (now :long-long))' \
  --eval '(cffi:defcfun ("harmonia_cron_scheduler_free_string" cfree) :void (p :pointer))' \
  --eval '(let ((p (hreq "GET" "https://openrouter.ai"))) (if (cffi:null-pointer-p p) (sb-ext:exit :code 2) (progn (format t "~&HTTP_OK=1~%") (hfree p))))' \
  --eval '(unless (zerop (rrec "panic" "simulated crash")) (sb-ext:exit :code 3))' \
  --eval '(let ((p (rtail 5))) (if (cffi:null-pointer-p p) (sb-ext:exit :code 4) (progn (format t "~&RECOVERY_TAIL=~A~%" (cffi:foreign-string-to-lisp p)) (rfree p))))' \
  --eval '(unless (zerop (fsw "snapshots/one.txt" "fs-ok")) (sb-ext:exit :code 5))' \
  --eval '(let ((p (fsr "snapshots/one.txt"))) (if (cffi:null-pointer-p p) (sb-ext:exit :code 6) (progn (format t "~&FS_READ=~A~%" (cffi:foreign-string-to-lisp p)) (fsfree p))))' \
  --eval "(unless (zerop (fbuild \"$ROOT_DIR\" \"harmonia-fs\")) (sb-ext:exit :code 7))" \
  --eval '(let ((p (btitle "https://openrouter.ai"))) (if (cffi:null-pointer-p p) (sb-ext:exit :code 8) (progn (format t "~&BROWSER_TITLE=~A~%" (cffi:foreign-string-to-lisp p)) (bfree p))))' \
  --eval '(unless (zerop (creset)) (sb-ext:exit :code 10))' \
  --eval '(unless (zerop (cadd "heartbeat" 1)) (sb-ext:exit :code 11))' \
  --eval '(sleep 1.2)' \
  --eval '(let ((p (cdue 0))) (if (cffi:null-pointer-p p) (sb-ext:exit :code 12) (progn (format t "~&CRON_DUE=~A~%" (cffi:foreign-string-to-lisp p)) (cfree p))))' \
  --quit

echo "[3/3] verify side effects"
test -f "$RECLOG"
test -f "$FSROOT/snapshots/one.txt"
echo "Core live checks complete."
