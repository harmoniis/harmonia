# Whisper (Speech-to-Text) -- Skill/MCP Definition

## Tool Name: `whisper_transcribe`

## Description

Speech-to-text transcription using OpenAI Whisper API. Accepts an audio file and returns the transcribed text.

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "audio_path": {
      "type": "string",
      "description": "Absolute path to audio file (mp3, mp4, wav, m4a, webm)"
    },
    "language": {
      "type": "string",
      "description": "ISO-639-1 language code hint (optional)"
    }
  },
  "required": ["audio_path"]
}
```

## Output Format

JSON from OpenAI API:
```json
{"text": "The transcribed text content..."}
```

## Vault Symbols Required

- `openai_api_key` -- OpenAI API key

## FFI Entry Point

```c
char* harmonia_whisper_transcribe(const char* audio_path);
```

## Example

```lisp
(tool op=whisper-transcribe audio-path="/tmp/harmonia/recording.mp3")
```

## Error Handling

Returns `NULL` on failure. Check `harmonia_whisper_last_error()` for details. Common errors:
- `missing secret: openai_api_key` -- Vault not initialized or key missing
- `curl exec failed` -- Network/curl issue
- `whisper transcribe failed` -- API error (file too large, invalid format)
