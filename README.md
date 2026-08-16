# PromptVeil 🛡️
### Zero-Trust PII Obfuscating AI Gateway Proxy

[![Build and Test Docker Image](https://github.com/TamTunnel/PromptVeil/actions/workflows/docker-build.yml/badge.svg)](https://github.com/TamTunnel/PromptVeil/actions/workflows/docker-build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Python 3.11+](https://img.shields.io/badge/Python-3.11+-3776AB.svg?logo=python&logoColor=white)](https://www.python.org/)
[![FastAPI](https://img.shields.io/badge/Framework-FastAPI-009688.svg?logo=fastapi&logoColor=white)](https://fastapi.tiangolo.com/)
[![Docker](https://img.shields.io/badge/Container-Docker-2496ED.svg?logo=docker&logoColor=white)](https://www.docker.com/)

**PromptVeil** is an ultra-low latency, zero-trust Layer 7 middleware gateway that strips sensitive Personally Identifiable Information (PII) and credentials from unstructured LLM payloads before they exit your private perimeter. 

---

## 📌 1. Problem Statement

Enterprises and developers integrating Generative AI (OpenAI, Anthropic, Gemini, DeepSeek, etc.) face strict data sovereignty and compliance risks (GDPR, HIPAA, PCI-DSS, SOC 2):

* **Accidental PII Leakage:** Customer names, SSNs, credit card numbers, and internal emails leak into third-party LLM training runs and API logs.
* **Secrets Ingestion:** Accidental pasting of high-entropy API tokens (AWS, OpenAI, GitHub) in user prompts.
* **Context Destruction by Naive Masking:** Simple redaction (e.g., `[REDACTED]`) breaks reasoning in multi-turn dialogues when models reference specific entities.
* **Latency Overhead & Latency Spikes:** Heavy NLP models or cloud redaction APIs introduce hundreds of milliseconds of latency, degrading interactive chat experiences.

---

## 💡 2. Solution: The PromptVeil Engine

PromptVeil operates as an on-premise, zero-trust gateway positioned directly between your application clients and external AI APIs:

1. **Hybrid Two-Tier Detection Pipeline**:
   * **Tier 1 (Deterministic Regex Engine):** Sub-millisecond matching for high-entropy secrets, API keys, Luhn-validated credit cards, Social Security Numbers (SSN), UUIDs, and IP addresses.
   * **Tier 2 (Contextual NER via GLiNER):** Lightweight Transformer-based Named Entity Recognition detecting names, organizations, email addresses, phone numbers, and physical addresses.
2. **Two-Way Format-Preserving Pseudonymization**:
   * Entities are replaced with deterministic, semantic surrogate tokens (e.g., `<PERSON_1>`, `<EMAIL_ADDRESS_1>`).
   * Multi-turn consistency: Repeated occurrences within the same session resolve to the identical synthetic token.
3. **Linear-Time Reverse De-anonymization ($O(N + M)$)**:
   * Upstream responses are scanned via **Aho-Corasick automaton** string matching to restore the original PII seamlessly before reaching the client.
4. **First-Class Server-Sent Events (SSE) Streaming**:
   * Dynamic token-boundary buffering that accurately unmasks streamed text tokens in real-time across chunk boundaries.
5. **Zero Disk Persistence**:
   * Session vaults reside strictly in ephemeral, TTL-evicted memory—no raw PII ever touches disk.

---

## 🏗️ 3. Architecture Overview

```mermaid
flowchart LR
    Client([Client / Application])
    
    subgraph Local [Local Perimeter / DMZ]
        subgraph PromptVeil [PromptVeil Gateway]
            Parser[JSON Walker / SSE Parser]
            T1[Tier 1: Regex Engine]
            T2[Tier 2: GLiNER NER]
            Vault[(In-Memory Session Vault)]
            Desanitizer[Aho-Corasick Desanitizer]
        end
    end
    
    LLM([Upstream AI Provider\ne.g. OpenAI / Anthropic])
    
    %% Inbound path
    Client -->|1. Raw Payload with PII| Parser
    Parser --> T1
    T1 --> T2
    T2 -->|Register Mappings| Vault
    T2 -->|2. Sanitized Tokens: <PERSON_1>| LLM
    
    %% Outbound path
    LLM -->|3. Response with Tokens| Desanitizer
    Vault -.->|Lookup Keys| Desanitizer
    Desanitizer -->|4. Reconstructed Original Payload| Client
```

### Sanitization Flow Example

| Stage | Payload Content |
| :--- | :--- |
| **Inbound Client Request** | `"Please draft an invoice for Alice Walker at alice@example.com using card 4242-4242-4242-4242."` |
| **Masked Upstream Forward** | `"Please draft an invoice for <PERSON_1> at <EMAIL_ADDRESS_1> using card <CREDIT_CARD_1>."` |
| **Upstream AI Response** | `"Invoice created for <PERSON_1> and sent to <EMAIL_ADDRESS_1>."` |
| **De-anonymized Response** | `"Invoice created for Alice Walker and sent to alice@example.com."` |

---

## 🚀 4. Quick Start Guide

### Prerequisites
* [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/) installed.

### Option A: Run with Docker Compose (Recommended)

```bash
# 1. Clone the repository
git clone https://github.com/TamTunnel/PromptVeil.git
cd PromptVeil

# 2. Build and launch the container
docker compose up --build -d

# 3. Verify health status
curl -i http://localhost:8080/healthz
```

### Option B: Local Python Development

```bash
# 1. Create and activate virtual environment
python3 -m venv .venv
source .venv/bin/activate

# 2. Install dependencies
pip install -r requirements.txt

# 3. Download model weights
python download_model.py

# 4. Start the proxy server
uvicorn app.main:app --host 0.0.0.0 --port 8080 --reload
```

---

## ⚙️ 5. Configuration (`config.yaml`)

Configuration is managed via `config.yaml` or environment variables:

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 2

upstream:
  default_base_url: "https://api.openai.com"
  timeout_seconds: 60.0

engine:
  model_name: "urchade/gliner_multi_pii-v1"
  ner_threshold: 0.45
  labels:
    - "person"
    - "email address"
    - "phone number"
    - "organization"
    - "address"
  enable_regex_tier: true

vault:
  session_ttl_seconds: 3600
  max_sessions: 10000
```

---

## 📡 6. Usage Examples

### Standard OpenAI Chat Completion Proxy Request

Simply point your client or SDK `base_url` to `http://localhost:8080/v1`:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "X-Session-ID: session_abc123" \
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

## ❓ 7. Frequently Asked Questions (FAQs)

<details>
<summary><b>Q: Does PromptVeil write any PII to disk or external databases?</b></summary>
<p>No. PromptVeil operates entirely in-memory with zero disk persistence. All session vaults use an LRU eviction policy with configurable TTLs (default 3600 seconds).</p>
</details>

<details>
<summary><b>Q: What happens if an entity is split across streaming SSE chunks?</b></summary>
<p>PromptVeil includes a rolling buffer in its SSE stream handler that detects token boundaries (e.g. <code>&lt;PERSON_...&gt;</code>) across chunk deltas, guaranteeing tokens are never partially emitted or broken during transit.</p>
</details>

<details>
<summary><b>Q: Can I run PromptVeil in air-gapped environments?</b></summary>
<p>Yes. The multi-stage Docker build downloads and bakes the GLiNER model weights directly into the container image (<code>/app/models/gliner</code>), requiring no outbound internet access at runtime.</p>
</details>

<details>
<summary><b>Q: Does it support custom upstream endpoints (Anthropic, Ollama, vLLM)?</b></summary>
<p>Yes. Set <code>upstream.default_base_url</code> in <code>config.yaml</code> to any compatible API base URL.</p>
</details>

---

## 🤝 8. How to Contribute

We welcome contributions from the community!

1. **Fork the Repository**: Click the **Fork** button at the top right of this repository.
2. **Create a Feature Branch**:
   ```bash
   git checkout -b feature/amazing-feature
   ```
3. **Commit Your Changes**:
   ```bash
   git commit -m "feat: add support for custom regex rules"
   ```
4. **Run Local Tests**:
   ```bash
   pytest tests/
   ```
5. **Push to Your Fork**:
   ```bash
   git push origin feature/amazing-feature
   ```
6. **Open a Pull Request**: Submit a PR to the `main` branch with a clear description of your changes.

---

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.
