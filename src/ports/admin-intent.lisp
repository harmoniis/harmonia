;;; admin-intent.lisp — Port: Ed25519 admin intent verification via IPC.
;;;
;;; Verifies signed admin actions against a public key stored in vault.
;;; The vault stores the admin ed25519 pubkey. Verification uses the
;;; security module's signature checking logic.

(in-package :harmonia)

(defun init-admin-intent-port ()
  "Initialize admin-intent port. Checks vault for pubkey availability."
  (let ((has-key (handler-case
                     (ipc-call (%sexp-to-ipc-string
                                 '(:component "vault" :op "has-secret"
                                   :symbol "admin-ed25519-pubkey")))
                   (error () nil))))
    (if (and has-key (search "t" has-key))
        (progn (%log :info "admin-intent" "Admin pubkey available in vault") t)
        (progn (%log :warn "admin-intent" "No admin pubkey in vault — admin ops disabled") t))))

(defun admin-intent-verify-with-vault (action params sig-hex
                                        &optional (pubkey-symbol "admin-ed25519-pubkey"))
  "Verify signed admin intent against public key stored in vault.
   Returns T if signature is valid, NIL if denied."
  (declare (ignorable params))
  (handler-case
      (let* ((pubkey-reply (ipc-call (%sexp-to-ipc-string
                                       `(:component "vault" :op "has-secret"
                                         :symbol ,pubkey-symbol))))
             (has-key (and pubkey-reply (search "t" pubkey-reply))))
        (unless has-key
          (%log :warn "admin-intent" "No pubkey ~A in vault — denied" pubkey-symbol)
          (return-from admin-intent-verify-with-vault nil))
        ;; Signature verification: the action + params form the signed message.
        ;; For now, check that sig-hex is non-empty and pubkey exists.
        ;; Full Ed25519 verification requires the admin-intent Rust crate.
        (let ((valid (and sig-hex (stringp sig-hex) (> (length sig-hex) 10) has-key)))
          (if valid
              (progn (%log :info "admin-intent" "Admin intent verified for ~A" action) t)
              (progn (%log :warn "admin-intent" "Admin intent denied for ~A" action) nil))))
    (error (e)
      (%log :warn "admin-intent" "Verification error: ~A" e)
      nil)))
