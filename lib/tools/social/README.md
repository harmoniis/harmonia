# harmonia-social

## Purpose

Social media posting tool. Currently a skeleton crate with version and healthcheck exports; platform-specific posting (Twitter/X, Bluesky, etc.) will be implemented as the agent's social presence grows.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_social_version` | `() -> *const c_char` | Version string |
| `harmonia_social_healthcheck` | `() -> i32` | Returns 1 if alive |

## Configuration

None yet. Future vault symbols:
- `twitter_api_key` -- Twitter/X API credentials
- `bluesky_api_key` -- Bluesky ATP credentials

## Usage from Lisp

```lisp
;; Currently only healthcheck available
(cffi:foreign-funcall "harmonia_social_healthcheck" :int) ;; => 1
```

## Self-Improvement Notes

- Skeleton crate; no dependencies yet, no real posting logic.
- Planned FFI: `harmonia_social_post(platform, text, media_path) -> *mut c_char` returning post ID/URL.
- Each platform will need its own vault symbol for API credentials.
- Consider rate limiting and content moderation before posting.
- Media attachments should be uploaded via s3 first, then referenced by URL.
