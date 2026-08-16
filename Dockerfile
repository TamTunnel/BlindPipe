# Stage 1: Builder
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/app

# Install build dependencies for compilation and downloading model
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    ca-certificates \
    build-essential \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Download Model and Tokenizer during build
COPY scripts/download_model.sh ./scripts/
RUN chmod +x ./scripts/download_model.sh && ./scripts/download_model.sh dslim/bert-base-NER

# Pre-fetch dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "pub fn dummy() {}" > src/lib.rs && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

# Copy real source and compile release binary
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
