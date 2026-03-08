# harmonia-openrouter

## Purpose

LLM router backend with native provider adapters and OpenRouter compatibility fallback.

Supported providers (selected by model prefix):
- `openrouter/<model>`
- `openai/<model>`
- `anthropic/<model>`
- `xai/<model>` or `x-ai/<model>`
- `google/<model>` (AI Studio)
- `vertex/<model>` or `google-vertex/<model>`
- `amazon/<model>` / `bedrock/<model>` / `nova/<model>`
- `groq/<model>`
- `alibaba/<model>`
- `qwen/<model>` (Alibaba-compatible alias)

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_openrouter_version` | `() -> *const c_char` | Version string |
| `harmonia_openrouter_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_openrouter_init` | `() -> i32` | Initialize vault connection |
| `harmonia_openrouter_complete` | `(prompt: *const c_char, model: *const c_char) -> *mut c_char` | Chat completion (returns text) |
| `harmonia_openrouter_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_openrouter_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_OPENROUTER_DEFAULT_MODEL` | -- | Default model ID |
| `HARMONIA_LLM_DEFAULT_MODEL` | -- | Global default model ID (highest priority) |
| `HARMONIA_MODEL_DEFAULT` | -- | Fallback default model |
| `HARMONIA_LLM_FALLBACK_MODELS` | -- | Global fallback model list |
| `HARMONIA_OPENROUTER_FALLBACK_MODELS` | -- | Comma-separated fallback model list |
| `HARMONIA_LLM_DISABLE_OPENROUTER_FALLBACK` | `false` | Disable native-provider -> OpenRouter credential fallback |
| `HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS` | `10` | HTTP connect timeout |
| `HARMONIA_OPENROUTER_MAX_TIME_SECS` | `45` | HTTP max time |
| `HARMONIA_OPENAI_BASE_URL` | `https://api.openai.com/v1/chat/completions` | OpenAI endpoint override |
| `HARMONIA_XAI_BASE_URL` | `https://api.x.ai/v1/chat/completions` | xAI endpoint override |
| `HARMONIA_GROQ_BASE_URL` | `https://api.groq.com/openai/v1/chat/completions` | Groq endpoint override |
| `HARMONIA_ALIBABA_BASE_URL` | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions` | Alibaba compatible endpoint override |
| `HARMONIA_GOOGLE_AI_STUDIO_BASE_URL` | `https://generativelanguage.googleapis.com/v1beta` | Google AI Studio API base |
| `HARMONIA_GOOGLE_VERTEX_PROJECT_ID` | -- | Vertex project fallback when not in vault |
| `HARMONIA_GOOGLE_VERTEX_LOCATION` | `us-central1` | Vertex location fallback when not in vault |
| `HARMONIA_GOOGLE_VERTEX_ENDPOINT` | `https://{location}-aiplatform.googleapis.com` | Vertex endpoint override |
| `HARMONIA_ANTHROPIC_VERSION` | `2023-06-01` | Anthropic API version header |
| `HARMONIA_ANTHROPIC_MAX_TOKENS` | `1024` | Anthropic max tokens |

## Vault Symbols

- OpenRouter: `openrouter` / `openrouter-api-key`
- OpenAI: `openai-api-key`
- Anthropic: `anthropic-api-key`
- xAI: `xai-api-key`
- Google AI Studio: `google-ai-studio-api-key` (or `gemini-api-key`)
- Google Vertex: `google-vertex-access-token`, `google-vertex-project-id`, optional `google-vertex-location`
- Bedrock/Nova: optional AWS creds (`aws-access-key-id`, `aws-secret-access-key`, `aws-session-token`, `aws-region`)
- Groq: `groq-api-key`
- Alibaba: `alibaba-api-key` (or `dashscope-api-key`)

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_openrouter_init" :int)
(let ((reply (cffi:foreign-funcall "harmonia_openrouter_complete"
               :string "Explain monads in one sentence."
               :string "anthropic/claude-sonnet-4" :pointer)))
  (cffi:foreign-string-to-lisp reply))
```

## Self-Improvement Notes

- Model selection: explicit model > `HARMONIA_LLM_DEFAULT_MODEL` > `HARMONIA_MODEL_DEFAULT` > `HARMONIA_OPENROUTER_DEFAULT_MODEL` > first fallback model.
- On primary model failure, automatically tries each fallback model in order.
- Native provider keys are resolved from component-scoped vault access.
- If native provider key is missing, request can degrade to OpenRouter (unless disabled).
- Bedrock model IDs are normalized from `amazon/nova-*` style to Bedrock Converse IDs (`amazon.nova-*:0`).
- Sends `HTTP-Referer: https://harmoniis.local` and `X-Title: Harmonia Agent` for OpenRouter calls.
- To add streaming: implement SSE parsing for chunked responses.
- To add token counting: parse `usage` field from response.
