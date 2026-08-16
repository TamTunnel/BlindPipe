# Stage 1: Builder
FROM rust:1.80-slim-bookworm AS builder

WORKDIR /usr/src/app

# Install dependencies for ort and compiling
RUN apt-get update && apt-get install -y pkg-config libssl-dev curl ca-certificates && rm -rf /var/lib/apt/lists/*

# Download Model and Tokenizer during build so it's cached in the image
COPY scripts/download_model.sh ./scripts/
RUN chmod +x ./scripts/download_model.sh && ./scripts/download_model.sh dslim/bert-base-NER

# Create a dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
COPY Cargo.toml ./
RUN cargo fetch
RUN cargo build --release
RUN rm src/main.rs

# Build the real app
COPY src/ ./src/
COPY tests/ ./tests/
RUN cargo build --release

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
