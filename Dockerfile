# Ver docs/11-INFRA-DEPLOY.md §3. debian-slim + glibc no lugar de
# musl + distroless (ADR-0007): o alocador do musl rende mal em cargas
# multithread com alocação intensa, que é o perfil do DSP com Rayon.
FROM rust:1.94-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin audio_api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/audio_api /usr/local/bin/mixlirous
COPY config ./config
COPY prompts ./prompts

RUN useradd -u 65532 -m app && chown -R app:app /app
USER 65532

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/mixlirous"]
