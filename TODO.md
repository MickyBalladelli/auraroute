Add provider presets for local backends
Support ollama, llama.cpp, vllm, maybe lmstudio, all local only.



Add real tokenizer support in main path
Current main uses whitespace count. Wire back tokenizers so scoring is less cave math.


Add request logging
Log route choice, latency, token estimate, complexity score. No prompt text by default.