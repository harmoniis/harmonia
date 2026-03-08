# harmonia-elevenlabs

## Purpose

Text-to-speech synthesis via ElevenLabs API. Converts text to audio using a specified voice and writes the result to a file.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_elevenlabs_version` | `() -> *const c_char` | Version string |
| `harmonia_elevenlabs_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_elevenlabs_tts_to_file` | `(text: *const c_char, voice_id: *const c_char, out_path: *const c_char) -> i32` | Synthesize speech to file |
| `harmonia_elevenlabs_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_elevenlabs_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_ELEVENLABS_API_URL` | `https://api.elevenlabs.io/v1/text-to-speech/<voice_id>` | API endpoint |

Runtime config via config-store:
- `elevenlabs.default_voice` -- Default voice ID
- `elevenlabs.default_output_path` -- Default output directory

## Vault Symbols

- `elevenlabs_api_key` -- ElevenLabs API key

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_elevenlabs_tts_to_file"
  :string "Hello, this is Harmonia speaking."
  :string "rachel"
  :string "/tmp/harmonia/audio/greeting.mp3" :int)
```

## Self-Improvement Notes

- Uses `eleven_multilingual_v2` model by default.
- Audio is written directly to `out_path` via curl `-o` flag.
- The voice_id is embedded in the URL path; to change models, override `HARMONIA_ELEVENLABS_API_URL`.
- To add voice listing: call `GET /v1/voices` and cache the result.
- To add streaming: use ElevenLabs streaming endpoint with chunked output.
