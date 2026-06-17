;;;; voice-policy.sexp — speech-to-text + text-to-speech routing definition.
;;;;
;;;; Mirrors model-policy.sexp: the Lisp router (voice-routing.lisp) selects an endpoint by
;;;; KIND (stt/tts) and tier; the Rust voice actor executes it. For CALLS the default favours
;;;; the lowest-latency endpoint in the eligible tier.
;;;;
;;;; To plug a CUSTOM OpenAI-compatible STT/TTS endpoint (e.g. a self-hosted Whisper + Chatterbox
;;;; cluster), keep its entry below and set:
;;;;   config-store:  voice/custom-stt-url   voice/custom-stt-model
;;;;                  voice/custom-tts-url   voice/custom-tts-model
;;;;   vault:         custom-stt-backend/custom-stt-api-key
;;;;                  custom-tts-backend/custom-tts-api-key
;;;; Once configured, the self-hosted cluster is the DEFAULT for ALL voice — SIP, WhatsApp, or any
;;;; audio source (lowest controlled latency, no external hop or rate limit). The endpoints below are
;;;; the FALLBACK ladder used only when the self-hosted endpoint is not configured (:eco/:auto rank
;;;; by latency, :premium by quality). An explicit :model "custom/stt" | "custom/tts" forces it.

(:speech-to-text
 (:default-tier :eco
  :endpoints
  ((:id "groq/whisper-large-v3-turbo" :provider "groq"      :tier :eco     :quality 8 :speed 9
    :tags (:fast :multilingual :low-latency))
   (:id "groq/whisper-large-v3"       :provider "groq"      :tier :eco     :quality 9 :speed 7
    :tags (:accurate :multilingual))
   (:id "openai/whisper-1"            :provider "openai"    :tier :eco     :quality 7 :speed 6
    :tags (:multilingual))
   (:id "custom/stt"                  :provider "custom"    :tier :premium :quality 9 :speed 8
    :tags (:custom :configurable))))

 :text-to-speech
 (:default-tier :eco
  :endpoints
  ((:id "elevenlabs/eleven_turbo_v2_5"      :provider "elevenlabs" :tier :eco     :quality 7 :speed 9
    :tags (:fast :low-latency))
   (:id "elevenlabs/eleven_multilingual_v2" :provider "elevenlabs" :tier :premium :quality 9 :speed 7
    :tags (:expressive :multilingual))
   (:id "custom/tts"                        :provider "custom"     :tier :premium :quality 9 :speed 8
    :tags (:custom :configurable)))))
