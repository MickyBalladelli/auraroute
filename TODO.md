Add provider presets for local backends
Support ollama, llama.cpp, vllm, maybe lmstudio, all local only.

Make routing smarter
Instead of just “pressure true/false”, route between local models:
small fast model
code model
large reasoning model

Add real tokenizer support in main path
Current main uses whitespace count. Wire back tokenizers so scoring is less cave math.

Add health checks
/health and /v1/models, so Cline and humans can see what AuraRoute can reach.

Add more tests
Test bad upstream, interrupted stream, invalid JSON, and routing decisions.

Add request logging
Log route choice, latency, token estimate, complexity score. No prompt text by default.