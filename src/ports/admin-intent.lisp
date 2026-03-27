;;; admin-intent.lisp — Port: Ed25519 admin intent verification.
;;;
;;; NOTE: admin-intent is not yet wired as an IPC component.
;;; Wrappers return sensible defaults until the Rust actor is connected.

(in-package :harmonia)

(defun admin-intent-last-error ()
  "admin-intent: not yet wired as IPC component")

(defun init-admin-intent-port ()
  "Stub: admin-intent IPC component not yet wired."
  t)

(defun admin-intent-verify-with-vault (action params sig-hex &optional (pubkey-symbol "admin-ed25519-pubkey"))
  "Verify signed admin intent against public key stored in vault.
   Returns nil (deny) until the IPC component is wired."
  (declare (ignorable action params sig-hex pubkey-symbol))
  (%log :warn "admin-intent" "verify-with-vault called on unwired IPC stub (action=~A)" action)
  nil)
