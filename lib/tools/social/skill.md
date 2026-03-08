# Social Media Posting -- Skill/MCP Definition

## Tool Name: `social_post`

## Description

Post content to social media platforms. Currently a skeleton -- not yet implemented. Will support multi-platform posting with optional media attachments.

## Input Schema (Planned)

```json
{
  "type": "object",
  "properties": {
    "platform": {
      "type": "string",
      "enum": ["twitter", "bluesky", "mastodon"],
      "description": "Target social media platform"
    },
    "text": {
      "type": "string",
      "description": "Post text content"
    },
    "media_path": {
      "type": "string",
      "description": "Optional path to media file to attach"
    }
  },
  "required": ["platform", "text"]
}
```

## Output Format (Planned)

JSON with post ID and URL:
```json
{"post_id": "...", "url": "https://..."}
```

## Vault Symbols Required (Planned)

- `twitter_api_key` -- Twitter/X API key
- `bluesky_api_key` -- Bluesky ATP password
- `mastodon_api_key` -- Mastodon access token

## FFI Entry Point (Planned)

```c
char* harmonia_social_post(const char* platform, const char* text, const char* media_path);
```

## Example

```lisp
(tool op=social-post platform="twitter" text="Hello from Harmonia!")
```

## Status

NOT YET IMPLEMENTED. Only version/healthcheck are available.
