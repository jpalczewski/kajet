# TEI Metal Setup Guide

Text Embeddings Inference (TEI) on Apple Silicon requires tuning to avoid
sending mega-batches that cause 30s+ inference times with RoBERTa-large.

## Root Cause

Default TEI settings (`max_batch_tokens=16384`, no `--max-batch-requests`)
create batches of 32 tokens × 512 = up to 512 sequences per request.
RoBERTa-large has O(n²) attention; without Flash Attention on Metal, a
batch of 256 sequences × 512 tokens takes 20–30s per request.

## Recommended Configuration

```bash
text-embeddings-router \
  --model-id sdadas/mmlw-retrieval-roberta-large-v2 \
  --port 8080 --auto-truncate \
  --max-client-batch-size 32 \
  --max-batch-tokens 4096 \
  --max-batch-requests 4 \
  --dtype float16
```

### Flag Explanations

| Flag | Value | Why |
|------|-------|-----|
| `--max-client-batch-size` | 32 | Upper bound on sequences per request from kajet |
| `--max-batch-tokens` | 4096 | Total tokens per GPU batch; 4096/512=8 sequences max |
| `--max-batch-requests` | 4 | TEI won't form a batch larger than 4 client requests |
| `--dtype float16` | float16 | Halves memory bandwidth on Metal; RoBERTa-large fits |
| `--auto-truncate` | — | Silently truncates inputs > max_input_length instead of 413 |

### Quick Start with `just`

```bash
just tei
# or with a different model:
just tei model_id=intfloat/multilingual-e5-large
```

## Auto-Detection in kajet

When kajet connects to a TEI server, it probes `GET /info` and automatically
computes optimal batch size and concurrency:

```
computed_batch_size = max_batch_tokens / max_input_length
                    = 4096 / 512 = 8
computed_concurrency = min(config, max_batch_requests)
                     = min(4, 4) = 4
```

Look for this in logs:
```
INFO TEI /info auto-detected model_id=sdadas/mmlw-retrieval-roberta-large-v2
     computed_batch_size=8 computed_concurrency=4
```

If `/info` is unavailable (Ollama, LM Studio), kajet falls back to
`embedding.remote_max_batch_size` and `embedding.remote_max_concurrent_requests`
from config with a DEBUG log message.

## Manual Config Override

If you want to set these values explicitly in `~/.config/kajet/config.toml`:

```toml
[embedding]
backend = "remote"
base_url = "http://localhost:8080"
model = "sdadas/mmlw-retrieval-roberta-large-v2"
remote_max_batch_size = 8
remote_max_concurrent_requests = 4
```

## Performance Targets

- **501 chunks (full vault reindex):** < 60s
- **Single-file reindex:** < 5s
- **Baseline (before tuning):** ~596s for 501 chunks

## Other Models

| Model | max_input_length | Recommended max_batch_tokens |
|-------|-----------------|------------------------------|
| RoBERTa-large (1024d) | 512 | 4096 |
| nomic-embed-text-v1.5 (768d) | 8192 | 16384 |
| multilingual-e5-large (1024d) | 512 | 4096 |
| all-MiniLM-L6-v2 (384d) | 512 | 8192 |
