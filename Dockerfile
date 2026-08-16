# Stage 1: Builder
FROM rust:1.80-slim-bookworm AS builder

WORKDIR /usr/src/app

# Install dependencies for compilation and downloading model
RUN apt-get update && apt-get install -y pkg-config libssl-dev curl ca-certificates && rm -rf /var/lib/apt/lists/*

# Download Model and Tokenizer during build so it's cached in the image
COPY scripts/download_model.sh ./scripts/
RUN chmod +x ./scripts/download_model.sh && ./scripts/download_model.sh dslim/bert-base-NER

# Create dummy src to pre-fetch and pre-compile dependencies for fast caching
RUN mkdir -p src && echo "pub fn dummy() {}" > src/lib.rs && echo "fn main() {}" > src/main.rs
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch
RUN cargo build --release || true
RUN rm -rf src

# Copy real source code
COPY src/ ./src/
RUN cargo build --release --bin promptveil

# Stage 2: Runtime (Distroless)
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/promptveil /app/promptveil
COPY --from=builder /usr/src/app/models /app/models

ENV SERVER_PORT=8080
ENV GLINER_MODEL_PATH=/app/models
ENV RUST_LOG=info

EXPOSE 8080

CMD ["/app/promptveil"]
