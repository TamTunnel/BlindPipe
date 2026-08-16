# PromptVeil: Zero-Trust PII Obfuscating AI Gateway Proxy

PromptVeil is a high-performance, on-premise Layer 7 reverse/forward proxy designed to intercept outbound unstructured AI payloads (e.g., OpenAI, Anthropic, or custom endpoints), detect and mask Personally Identifiable Information (PII) using a hybrid Regex + contextual Small NER model pipeline, maintain a bidirectional in-memory pseudonymization vault, and de-anonymize responses before returning them to the client.

---

## Key Features

- **Hybrid Detection Pipeline**:
  - **Tier 1 (Deterministic Regex)**: High-entropy strings, API keys (OpenAI, AWS, GitHub), Credit Card numbers (Luhn validated), Social Security Numbers (SSN), IPv4/IPv6 addresses, UUIDs.
  - **Tier 2 (Contextual NER / GLiNER)**: Names, Email addresses, Phone numbers, Physical addresses, Organizations.
- **Two-Way Pseudonymization Vault**:
  - Deterministic synthetic replacement tokens (`<PERSON_1>`, `<EMAIL_ADDRESS_1>`, etc.).
  - LRU in-memory bidirectional mapping isolated per session (`X-Session-ID`).
  - TTL-based expiration.
- **Protocol & Streaming Support**:
  - Recursive JSON walker redacting string values without altering schema or types.
  - Full support for Server-Sent Events (SSE) `text/event-stream` streaming responses with chunk boundary token reconstruction.
  - Fast linear-time reverse de-anonymization using Aho-Corasick.
- **Container Security**:
  - Multi-stage build with baked GLiNER weights for air-gapped / isolated execution.
  - Rootless non-root container user (`appuser:10001`).

---

## Quickstart

### Running with Docker Compose

```bash
docker compose up --build
```

The proxy will start on `http://0.0.0.0:8080`.

### Health Check

```bash
curl http://localhost:8080/healthz
```

---

## Configuration (`config.yaml`)

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
