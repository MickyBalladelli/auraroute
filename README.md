# AuraRoute

![Rust](https://img.shields.io/badge/Rust-2021-orange)
![Tokio](https://img.shields.io/badge/runtime-Tokio-blue)
![Axum](https://img.shields.io/badge/web-Axum-green)
![License](https://img.shields.io/badge/license-MIT-lightgrey)
![Status](https://img.shields.io/badge/status-phase%201-yellow)

AuraRoute is a lightweight local LLM routing proxy. It exposes an OpenAI-compatible chat completions endpoint, scores incoming prompts, checks local resource pressure, and streams the request to a local model endpoint.

It is designed to sit between tools like Cline and local LLM backends such as Ollama.

## What It Does

- Accepts `POST /v1/chat/completions`
- Parses OpenAI-style chat payloads
- Extracts user prompt text
- Estimates prompt complexity from token count, code syntax density, and architecture keywords
- Reads local resource state with a safe VRAM mock fallback
- Routes low-complexity requests to local Ollama-compatible upstream
- Keeps all requests on local LLM infrastructure
- Streams upstream responses back as Axum SSE without buffering the full response
- Enables permissive CORS for localhost tool integrations

## Runtime Route

```text
Cline / Client
    |
    v
AuraRoute :8080
    |
    +--> Local upstream: http://localhost:11434/v1/chat/completions
```

## Project Layout

```text
src/
  main.rs      # Axum app, routing orchestration, CORS, server bind
  models.rs    # OpenAI/Cline serde structs and prompt extraction
  scorer.rs    # Complexity score engine, 1..=5
  hardware.rs  # Resource governor and VRAM fallback logic
  proxy.rs     # Async SSE streaming proxy
```

## Routing Logic

AuraRoute calculates a complexity score from `1` to `5`.

- `token_count > 1500` adds weight
- Dense code syntax adds weight
- Architecture keywords like `architect`, `refactor`, `bottleneck`, and `deadlock` add weight
- Score is clamped to `5`

Local pressure is detected when:

- complexity score is `>= 4`
- complexity score is `2` or `3` and free VRAM is below `3000 MB`

## Configuration

Local upstream is configured with an environment variable:

```bash
AURAROUTE_LOCAL_UPSTREAM=http://localhost:11434/v1/chat/completions
```

If unset, AuraRoute uses `http://localhost:11434/v1/chat/completions`.

Resource mock override:

```bash
AURAROUTE_FREE_VRAM_MB=2500 cargo run
```

On Linux, CPU usage is estimated from `/proc/stat`. On other platforms, CPU can be mocked:

```bash
AURAROUTE_CPU_USAGE_PCT=42 cargo run
```

## Run

```bash
cargo run
```

AuraRoute listens on:

```text
http://127.0.0.1:8080
```

Chat completions endpoint:

```text
POST /v1/chat/completions
```

## Tests

```bash
cargo test
```

The integration test starts local-only mock SSE servers on ephemeral localhost ports and verifies that AuraRoute streams chunks through the proxy path.

## Example Request

```bash
curl -N http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "auraroute",
    "stream": true,
    "messages": [
      {
        "role": "user",
        "content": "Refactor this async Rust proxy and explain possible deadlocks."
      }
    ]
  }'
```

## Development Status

AuraRoute is in Phase 1. The core networking, serde, scoring, resource guard, configurable local upstream, and SSE stream proxy are implemented. Integration tests cover the local mock SSE proxy path.

## License

MIT
