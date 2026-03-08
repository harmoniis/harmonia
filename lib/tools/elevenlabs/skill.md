# ElevenLabs (Text-to-Speech) -- Skill/MCP Definition

## Tool Name: `elevenlabs_tts`

## Description

Text-to-speech synthesis using ElevenLabs API. Converts text to natural-sounding audio and saves to a file.

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "text": {
      "type": "string",
      "description": "Text to synthesize into speech"
    },
    "voice_id": {
      "type": "string",
      "description": "ElevenLabs voice ID (e.g., 'rachel', 'adam')"
    },
    "output_path": {
      "type": "string",
      "description": "Absolute path for output audio file (.mp3)"
    }
  },
  "required": ["text", "voice_id", "output_path"]
}
```

## Output Format

Returns 0 on success, -1 on failure. Audio file is written to `output_path`.

## Vault Symbols Required

- `elevenlabs_api_key` -- ElevenLabs API key

## Config-Store Keys

- `elevenlabs.default_voice` -- Fallback voice ID
- `elevenlabs.default_output_path` -- Fallback output directory

## FFI Entry Point

```c
int harmonia_elevenlabs_tts_to_file(const char* text, const char* voice_id, const char* out_path);
```

## Example

```lisp
(tool op=elevenlabs-tts text="Hello world" voice-id="rachel" output-path="/tmp/audio/out.mp3")
```

## Error Handling

Returns -1 on failure. Check `harmonia_elevenlabs_last_error()` for details. Common errors:
- `missing secret: elevenlabs_api_key` -- Vault not initialized or key missing
- `elevenlabs tts failed` -- API error (quota exceeded, invalid voice)
- `curl exec failed` -- Network/curl issue
