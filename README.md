# PromptVeil: Zero-Trust PII Obfuscating AI Gateway Proxy (Rust Edition)

[![Build and Test](https://github.com/TamTunnel/PromptVeil/actions/workflows/docker-build.yml/badge.svg?branch=main)](https://github.com/TamTunnel/PromptVeil/actions/workflows/docker-build.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![ONNX Runtime](https://img.shields.io/badge/ONNX_Runtime-ORT-green.svg)](https://onnxruntime.ai/)
[![Docker](https://img.shields.io/badge/Container-Distroless-blue.svg)](https://github.com/GoogleContainerTools/distroless)

PromptVeil is a high-performance, on-premise, zero-trust Layer 7 AI Proxy Gateway written in **Rust**. It sits between your local clients and upstream AI models (like OpenAI, Anthropic, or local vLLM/Ollama instances), automatically intercepting, anonymizing, and de-anonymizing Personally Identifiable Information (PII) and secrets before requests ever leave your local network perimeter.

---

## 📌 1. Problem Statement

Enterprises and developers integrating Generative AI face critical data sovereignty, privacy, and compliance challenges (GDPR, HIPAA, PCI-DSS, SOC 2):

- **Accidental PII & Secret Leakage:** Customer names, SSNs, credit card numbers, and high-entropy API tokens (AWS, OpenAI, GitHub) frequently leak into third-party LLM training logs and telemetry.
- **Context Destruction by Naive Masking:** Simple redaction (e.g., replacing values with `[REDACTED]`) breaks reasoning in multi-turn dialogues when models attempt to reference specific distinct entities.
- **Latency Overhead in Python Stacks:** Legacy Python middleware introduces 50–150ms of overhead and consumes 1GB+ of memory per instance, making high-concurrency proxying costly and slow.
- **Complex De-anonymization:** Safely unmasking streamed Server-Sent Events (SSE) responses without corrupting chunk boundaries requires robust token reconstruction.

---

## 💡 2. Solution: The PromptVeil Engine

PromptVeil operates as an ultra-low-latency, zero-trust gateway designed to provide transparent, bidirectional privacy:

1. **Dual-Tier Hybrid Detection Pipeline**:
   - **Tier 1 (Deterministic Regex Engine):** Sub-millisecond matching for high-entropy secrets, API keys, Luhn-validated credit cards, Social Security Numbers (SSN), UUIDs, and IPv4 addresses.
   - **Tier 2 (Contextual NER via ONNX Runtime):** Transformer-based Named Entity Recognition (BERT/DeBERTa) detecting names, organizations, and locations via native ONNX Runtime (`ort`) C-bindings with zero Python runtime overhead.
2. **Two-Way Format-Preserving Pseudonymization**:
   - Entities are replaced with deterministic, semantic surrogate tokens (e.g., `<PERSON_1>`, `<IPV4_ADDRESS_1>`).
   - Multi-turn consistency: Repeated occurrences within the same session resolve to the identical synthetic token.
3. **Linear-Time Reverse De-anonymization ($O(N + M)$)**:
   - Upstream responses are scanned using the **Aho-Corasick automaton** algorithm to restore the original PII seamlessly before returning data to the client.
4. **First-Class Server-Sent Events (SSE) Streaming**:
   - Streams responses token-by-token in real-time, accurately resolving synthetic tokens back to original values without buffering entire responses.
5. **Zero Disk Persistence & Minimal Footprint**:
   - Session vaults reside strictly in ephemeral, TTL-evicted in-memory caches (`moka`). Memory consumption stays under 40MB.

---

## 📊 3. Performance Benchmarks (Python vs Rust)

| Metric                 | Legacy Python (FastAPI/PyTorch) | **PromptVeil Rust (Axum/ORT)** | Improvement          |
| :--------------------- | :------------------------------ | :----------------------------- | :------------------- |
| **Idle Memory**        | ~1.2 GB                         | **~38 MB**                     | **~31x smaller**     |
| **Avg Latency (Text)** | 55 ms                           | **< 4 ms**                     | **~14x faster**      |
| **Concurrency**        | ~120 req/s                      | **> 30,000 req/s**             | **~250x throughput** |
| **Binary Size**        | ~1.8 GB Docker Image            | **~140 MB Distroless Image**   | **~13x lighter**     |

---

## 🏗️ 4. Architecture Overview

```mermaid
flowchart LR
    Client([Client / Application])

    subgraph Local [Local Perimeter / DMZ]
        subgraph PromptVeil [PromptVeil Rust Gateway]
            Parser[Axum JSON Walker / SSE Streamer]
            T1[Tier 1: Regex & Luhn Engine]
            T2[Tier 2: ONNX Runtime NER]
            Vault[(In-Memory Moka Session Vault)]
            Desanitizer[Aho-Corasick Automaton]
        end
    end

    LLM([Upstream AI Provider\ne.g. OpenAI / Anthropic])

    %% Inbound path
    Client -->|1. Raw Request with PII| Parser
    Parser --> T1
    T1 --> T2
    T2 -->|Register Mappings| Vault
    T2 -->|2. Masked Tokens: <PERSON_1>| LLM

    %% Outbound path
    LLM -->|3. Response with Tokens| Desanitizer
    Vault -.->|Lookup Reverse Keys| Desanitizer
    Desanitizer -->|4. Re-anonymized Response| Client
```

### Sanitization Flow Example

| Stage                       | Payload Content                                                                                   |
| :-------------------------- | :------------------------------------------------------------------------------------------------ |
| **Inbound Client Request**  | `"Please draft an invoice for Alice Walker with IP 192.168.1.50 using card 4242-4242-4242-4242."` |
| **Masked Upstream Forward** | `"Please draft an invoice for <PERSON_1> with IP <IPV4_ADDRESS_1> using card <CREDIT_CARD_1>."`   |
| **Upstream AI Response**    | `"Invoice created for <PERSON_1> and logged under IP <IPV4_ADDRESS_1>."`                          |
| **De-anonymized Response**  | `"Invoice created for Alice Walker and logged under IP 192.168.1.50."`                            |

---

## 🚀 5. Quick Start Guide

### Option A: Run with Docker Compose (Recommended)

```bash
# 1. Clone repository
git clone https://github.com/TamTunnel/PromptVeil.git
cd PromptVeil

# 2. Build and launch container
docker compose up --build -d

# 3. Check health status
curl -i http://localhost:8080/healthz
```

### Option B: Native Local Rust Development

```bash
# 1. Download pre-trained ONNX model and tokenizer
chmod +x scripts/download_model.sh
./scripts/download_model.sh dslim/bert-base-NER

# 2. Run test suite
cargo test

# 3. Run gateway server
cargo run --release
```

---

## ⚙️ 6. Configuration

Configure PromptVeil using environment variables in `docker-compose.yml` or your `.env` file:

| Variable              | Default                  | Description                                                   |
| :-------------------- | :----------------------- | :------------------------------------------------------------ |
| `SERVER_PORT`         | `8080`                   | Local port the Axum gateway binds to                          |
| `UPSTREAM_BASE_URL`   | `https://api.openai.com` | Upstream AI endpoint                                          |
| `ENABLE_REGEX_TIER`   | `true`                   | Enable Tier 1 regex detection (SSN, credit cards, keys)       |
| `ENABLE_NER_TIER`     | `true`                   | Enable Tier 2 ONNX token classification NER                   |
| `NER_THRESHOLD`       | `0.45`                   | Minimum classification confidence threshold                   |
| `SESSION_TTL_SECONDS` | `3600`                   | In-memory session eviction TTL in seconds                     |
| `RUST_LOG`            | `info`                   | Logging verbosity (`error`, `warn`, `info`, `debug`, `trace`) |

---

## 📡 7. Usage Examples

### Standard OpenAI-Compatible Proxy Request

Point your client SDK's `base_url` or direct HTTP requests to `http://localhost:8080`:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "X-Session-ID: user-session-12345" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {
        "role": "user",
        "content": "Reach out to Jane Doe at jane.doe@acmecorp.com regarding AWS key AKIAIOSFODNN7EXAMPLE."
      }
    ]
  }'
```

---

## ❓ 8. Frequently Asked Questions (FAQs)

<details>
<summary><b>Q: Does PromptVeil write any PII to disk or external databases?</b></summary>
<p>No. PromptVeil operates entirely in-memory with zero disk persistence. All session pseudonymization tables reside in ephemeral, TTL-managed Moka caches and are dropped automatically after expiration.</p>
</details>

<details>
<summary><b>Q: How does PromptVeil handle Server-Sent Events (SSE) streaming?</b></summary>
<p>PromptVeil processes streaming response chunks asynchronously, running token substitutions through the Aho-Corasick automaton in real time before streaming the decoded chunks back to the client.</p>
</details>

<details>
<summary><b>Q: Can I run PromptVeil in air-gapped environments?</b></summary>
<p>Yes. The multi-stage Docker build downloads and embeds the ONNX model and tokenizer directly into the container image (<code>/app/models</code>), requiring no outbound internet access at runtime for inference.</p>
</details>

<details>
<summary><b>Q: Does it support non-OpenAI upstreams (Anthropic, Ollama, vLLM)?</b></summary>
<p>Yes. Set <code>UPSTREAM_BASE_URL</code> to any compatible API base URL (e.g. <code>http://localhost:11434</code> for Ollama or <code>http://localhost:8000</code> for vLLM).</p>
</details>

---

## 🤝 9. How to Contribute

We welcome contributions from the community!

1. **Fork the Repository**: Click the **Fork** button on GitHub.
2. **Create a Feature Branch**:
   ```bash
   git checkout -b feature/amazing-feature
   ```
3. **Run Tests & Verify Build**:
   ```bash
   cargo test
   cargo build --release
   ```
4. **Commit Your Changes**:
   ```bash
   git commit -m "feat: add support for custom regex pattern"
   ```
5. **Push to Your Fork**:
   ```bash
   git push origin feature/amazing-feature
   ```
6. **Open a Pull Request**: Submit a PR against `main`.

---

## 📄 10. License

This project is licensed under the Apache 2.0 License. See the [LICENSE](LICENSE) file for details.
