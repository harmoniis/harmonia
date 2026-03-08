# Search (Exa) -- Skill/MCP Definition

## Tool Name: `search_exa`

## Description

Web search via Exa.ai API. Returns structured search results with titles, URLs, and snippets. Supports neural semantic search for high-relevance results.

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Search query text"
    },
    "num_results": {
      "type": "integer",
      "default": 5,
      "description": "Number of results to return (max 10)"
    },
    "type": {
      "type": "string",
      "enum": ["keyword", "neural", "auto"],
      "default": "auto",
      "description": "Search type"
    }
  },
  "required": ["query"]
}
```

## Output Format

JSON object from Exa API containing `results` array:
```json
{
  "results": [
    {"title": "...", "url": "...", "score": 0.95, "publishedDate": "..."}
  ]
}
```

When consumed by Lisp, parsed into S-expression:
```lisp
((:title "..." :url "..." :score 0.95) ...)
```

## Vault Symbols Required

- `exa_api_key` -- Exa.ai API key

## FFI Entry Point

```c
char* harmonia_search_exa_query(const char* query);
```

## Example

```lisp
(tool op=search-exa query="rust async runtime" num-results=5)
```

## Error Handling

Returns `NULL` on failure. Check `harmonia_search_exa_last_error()` for details. Common errors:
- `missing secret: exa_api_key` -- Vault not initialized or key missing
- `curl exec failed` -- Network/curl issue
- `exa query failed` -- API returned error
