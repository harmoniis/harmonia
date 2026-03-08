# harmonia-whisper

## Purpose

Speech-to-text transcription via OpenAI Whisper API. Takes an audio file path and returns the transcribed text as JSON.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_whisper_version` | `() -> *const c_char` | Version string |
| `harmonia_whisper_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_whisper_transcribe` | `(audio_path: *const c_char) -> *mut c_char` | Transcribe audio file, returns JSON |
| `harmonia_whisper_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_whisper_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_WHISPER_API_URL` | `https://api.openai.com/v1/audio/transcriptions` | API endpoint |
| `HARMONIA_WHISPER_MODEL` | `whisper-1` | Model name |

## Vault Symbols

- `openai_api_key` -- OpenAI API key

## Usage from Lisp

```lisp
(let ((result (cffi:foreign-funcall "harmonia_whisper_transcribe"
                :string "/tmp/harmonia/audio/recording.mp3" :pointer)))
  (parse-json (cffi:foreign-string-to-lisp result)))
```

## Self-Improvement Notes

- Uses `curl -F file=@<path>` for multipart upload to the Whisper API.
- Returns raw JSON `{"text": "transcribed content..."}`.
- Supports all Whisper-compatible formats: mp3, mp4, mpeg, mpga, m4a, wav, webm.
- To add language hint: add `-F language=<code>` parameter.
- To add streaming: use the Whisper streaming endpoint when available.
