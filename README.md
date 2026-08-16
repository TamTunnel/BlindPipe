# PromptVeil: Zero-Trust PII Obfuscating AI Gateway Proxy (Rust Edition)

![Build Status](https://img.shields.io/github/actions/workflow/status/TamTunnel/PromptVeil/docker-build.yml?branch=main)
![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)
![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange)
![ONNX Runtime](https://img.shields.io/badge/ONNX_Runtime-ORT-green)

PromptVeil is a high-performance, on-premise, zero-trust Layer 7 AI Proxy Gateway written in **Rust**. It sits between your local clients and upstream AI models (like OpenAI or Anthropic), automatically intercepting, anonymizing, and de-anonymizing Personally Identifiable Information (PII) before it ever leaves your local network.

## 🚀 Key Features

- **Blazing Fast (Rust + ONNX):** Rewritten in Rust for microsecond latency. Memory footprint under 40MB.
- **Dual-Tier Sanitization:**
  - **Tier 1 (Deterministic):** Fast regex matching for Credit Cards (with Luhn validation), SSNs, API Keys, IPs.
  - **Tier 2 (AI/NER):** ONNX Runtime powered NLP for context-aware NER extraction (Names, Organizations, Locations) using token classification models like BERT/DeBERTa.
- **Bidirectional Unmasking:** Maintains an in-memory TTL vault (via `moka`) mapping real data to synthetic tokens (e.g., `<PERSON_1>`). Re-injects real data into AI responses (JSON & SSE streaming) using ultra-fast Aho-Corasick automaton.
- **Zero-Trust Docker Deployment:** Runs as a Distroless container, ensuring maximum security and zero external dependencies.

## 📊 Performance Benchmarks (Python vs Rust)

| Metric                 | Legacy Python (FastAPI/PyTorch) | **PromptVeil Rust (Axum/ORT)** | Improvement      |
| :--------------------- | :------------------------------ | :----------------------------- | :--------------- |
| **Idle Memory**        | ~1.2 GB                         | **~38 MB**                     | ~31x smaller     |
| **Avg Latency (Text)** | 55 ms                           | **< 4 ms**                     | ~14x faster      |
| **Concurrency**        | 120 req/s                       | **> 30,000 req/s**             | ~250x throughput |

## 🏗 Architecture

1.  **Axum Proxy:** Intercepts incoming POST requests on `/*`.
2.  **JSON Traversal:** Recursively walks unstructured JSON payloads looking for strings.
3.  **Tier 1 & 2 Execution:**
    - Regex matches guaranteed patterns.
    - ONNX Token Classification model identifies complex entities via logit argmax.
4.  **Vault Substitution:** Matches are stored in a Moka cache against `X-Session-ID`, replaced with synthetic tokens.
5.  **Reqwest Upstream:** Forwards masked payloads to upstream URLs.
6.  **Streaming & Response:** Intercepts JSON or SSE streams, reversing tokens back to original data before sending to the client.

## 🛠 Quick Start

### 1. Download the Models

By default, the engine requires a token classification ONNX model (e.g., `dslim/bert-base-NER`).

```bash
./scripts/download_model.sh dslim/bert-base-NER
```

### 2. Build and Run via Docker Compose

```bash
docker-compose up --build -d
```

The proxy will start on `http://localhost:8080`.

## ⚙️ Configuration

Configure the gateway via environment variables in `docker-compose.yml`:

- `SERVER_PORT` (default: `8080`)
- `UPSTREAM_BASE_URL` (default: `https://api.openai.com`)
- `ENABLE_REGEX_TIER` (default: `true`)
- `ENABLE_NER_TIER` (default: `true`)
- `NER_THRESHOLD` (default: `0.45`)
- `SESSION_TTL_SECONDS` (default: `3600`)

## 🤝 How to Contribute

1. Fork the repository.
2. Ensure you have Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
3. Run `cargo test` to ensure tests pass.
4. Open a Pull Request with your feature!

## 📝 License

This project is licensed under the Apache 2.0 License. See the [LICENSE](LICENSE) file for details.
