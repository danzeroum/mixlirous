# ============================================================================
# Stage 1: Build frontend (Node.js)
# ============================================================================
FROM node:22-slim AS frontend-builder
WORKDIR /app/ui
COPY ui/package.json ui/package-lock.json* ./
RUN npm ci --prefer-offline
COPY ui/ .
RUN npm run build

# ============================================================================
# Stage 2: Build backend (Rust)
# ============================================================================
FROM rust:1.94-bookworm AS rust-builder
WORKDIR /app
COPY . .
COPY --from=frontend-builder /app/ui/dist /app/ui/dist
RUN cargo build --release --bin audio_api

# ============================================================================
# Stage 3: Runtime (debian-slim)
# ============================================================================
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /app/target/release/audio_api /usr/local/bin/mixlirous
COPY config ./config
COPY prompts ./prompts
RUN mkdir -p /app/data
RUN useradd -u 65532 -m app && chown -R app:app /app
USER 65532
ENV MIXLIROUS_NO_BROWSER=1
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/mixlirous"]