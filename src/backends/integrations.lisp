;;; integrations.lisp — CFFI bridges for communication/search/voice tools.

(in-package :harmonia)

(defparameter *integration-libs* (make-hash-table :test 'equal))

(cffi:defcfun ("harmonia_whatsapp_send_text" %wa-send-text) :int (to :string) (text :string))
(cffi:defcfun ("harmonia_whatsapp_store_linked_device" %wa-store-device) :int (device-id :string) (creds :string))
(cffi:defcfun ("harmonia_whatsapp_last_error" %wa-last-error) :pointer)
(cffi:defcfun ("harmonia_whatsapp_free_string" %wa-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_telegram_send_text" %tg-send-text) :int (chat-id :string) (text :string))
(cffi:defcfun ("harmonia_telegram_last_error" %tg-last-error) :pointer)
(cffi:defcfun ("harmonia_telegram_free_string" %tg-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_slack_send_text" %sl-send-text) :int (channel :string) (text :string))
(cffi:defcfun ("harmonia_slack_last_error" %sl-last-error) :pointer)
(cffi:defcfun ("harmonia_slack_free_string" %sl-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_mattermost_send_text" %mm-send-text) :int (channel :string) (text :string))
(cffi:defcfun ("harmonia_mattermost_last_error" %mm-last-error) :pointer)
(cffi:defcfun ("harmonia_mattermost_free_string" %mm-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_nostr_publish_text" %nostr-publish) :int (text :string))
(cffi:defcfun ("harmonia_nostr_last_error" %nostr-last-error) :pointer)
(cffi:defcfun ("harmonia_nostr_free_string" %nostr-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_email_client_send" %email-send) :int (to :string) (subject :string) (body :string))
(cffi:defcfun ("harmonia_email_client_last_error" %email-last-error) :pointer)
(cffi:defcfun ("harmonia_email_client_free_string" %email-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_search_exa_query" %exa-query) :pointer (query :string))
(cffi:defcfun ("harmonia_search_exa_last_error" %exa-last-error) :pointer)
(cffi:defcfun ("harmonia_search_exa_free_string" %exa-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_search_brave_query" %brave-query) :pointer (query :string))
(cffi:defcfun ("harmonia_search_brave_last_error" %brave-last-error) :pointer)
(cffi:defcfun ("harmonia_search_brave_free_string" %brave-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_whisper_transcribe" %whisper-transcribe) :pointer (audio-path :string))
(cffi:defcfun ("harmonia_whisper_last_error" %whisper-last-error) :pointer)
(cffi:defcfun ("harmonia_whisper_free_string" %whisper-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_elevenlabs_tts_to_file" %eleven-tts) :int (text :string) (voice-id :string) (out-path :string))
(cffi:defcfun ("harmonia_elevenlabs_last_error" %eleven-last-error) :pointer)
(cffi:defcfun ("harmonia_elevenlabs_free_string" %eleven-free-string) :void (ptr :pointer))

(defun %load-lib (id file)
  (setf (gethash id *integration-libs*)
        (cffi:load-foreign-library (%release-lib-path file))))

(defun init-integrations-backends ()
  (ensure-cffi)
  (%load-lib "whatsapp" "libharmonia_whatsapp.dylib")
  (%load-lib "telegram" "libharmonia_telegram.dylib")
  (%load-lib "slack" "libharmonia_slack.dylib")
  (%load-lib "mattermost" "libharmonia_mattermost.dylib")
  (%load-lib "nostr" "libharmonia_nostr.dylib")
  (%load-lib "email-client" "libharmonia_email_client.dylib")
  (%load-lib "search-exa" "libharmonia_search_exa.dylib")
  (%load-lib "search-brave" "libharmonia_search_brave.dylib")
  (%load-lib "whisper" "libharmonia_whisper.dylib")
  (%load-lib "elevenlabs" "libharmonia_elevenlabs.dylib")
  t)

(defun %ptr-string (ptr free-fn)
  (if (cffi:null-pointer-p ptr)
      nil
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (funcall free-fn ptr))))

(defun %last-error-string (getter free-fn)
  (let ((ptr (funcall getter)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (funcall free-fn ptr)))))

(defun whatsapp-store-linked-device (device-id creds)
  (let ((rc (%wa-store-device device-id creds)))
    (unless (zerop rc)
      (error "whatsapp store device failed: ~A"
             (%last-error-string #'%wa-last-error #'%wa-free-string)))
    t))

(defun whatsapp-send-text (to text)
  (let ((rc (%wa-send-text to text)))
    (unless (zerop rc)
      (error "whatsapp send failed: ~A"
             (%last-error-string #'%wa-last-error #'%wa-free-string)))
    "WHATSAPP_OK"))

(defun telegram-send-text (chat-id text)
  (let ((rc (%tg-send-text chat-id text)))
    (unless (zerop rc)
      (error "telegram send failed: ~A"
             (%last-error-string #'%tg-last-error #'%tg-free-string)))
    "TELEGRAM_OK"))

(defun slack-send-text (channel text)
  (let ((rc (%sl-send-text channel text)))
    (unless (zerop rc)
      (error "slack send failed: ~A"
             (%last-error-string #'%sl-last-error #'%sl-free-string)))
    "SLACK_OK"))

(defun mattermost-send-text (channel text)
  (let ((rc (%mm-send-text channel text)))
    (unless (zerop rc)
      (error "mattermost send failed: ~A"
             (%last-error-string #'%mm-last-error #'%mm-free-string)))
    "MATTERMOST_OK"))

(defun nostr-publish-text (text)
  (let ((rc (%nostr-publish text)))
    (unless (zerop rc)
      (error "nostr publish failed: ~A"
             (%last-error-string #'%nostr-last-error #'%nostr-free-string)))
    "NOSTR_OK"))

(defun email-send (to subject body)
  (let ((rc (%email-send to subject body)))
    (unless (zerop rc)
      (error "email send failed: ~A"
             (%last-error-string #'%email-last-error #'%email-free-string)))
    "EMAIL_OK"))

(defun search-exa (query)
  (let ((ptr (%exa-query query)))
    (or (%ptr-string ptr #'%exa-free-string)
        (error "exa query failed: ~A"
               (%last-error-string #'%exa-last-error #'%exa-free-string)))))

(defun search-brave (query)
  (let ((ptr (%brave-query query)))
    (or (%ptr-string ptr #'%brave-free-string)
        (error "brave query failed: ~A"
               (%last-error-string #'%brave-last-error #'%brave-free-string)))))

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
      (let ((res (search-brave query)))
        (harmonic-matrix-observe-route "orchestrator" "search-brave" t 1)
        (harmonic-matrix-observe-route "search-brave" "memory" t 1)
        res))))

(defun whisper-transcribe (audio-path)
  (let ((ptr (%whisper-transcribe audio-path)))
    (or (%ptr-string ptr #'%whisper-free-string)
        (error "whisper transcribe failed: ~A"
               (%last-error-string #'%whisper-last-error #'%whisper-free-string)))))

(defun elevenlabs-tts-to-file (text voice-id out-path)
  (let ((rc (%eleven-tts text voice-id out-path)))
    (unless (zerop rc)
      (error "elevenlabs tts failed: ~A"
             (%last-error-string #'%eleven-last-error #'%eleven-free-string)))
    out-path))
