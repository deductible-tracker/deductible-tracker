# syntax=docker/dockerfile:1.4

# Stage 1: Build (with BuildKit cache mounts)
FROM rust:1.93.0-slim-bookworm AS builder
WORKDIR /app

ARG TARGETARCH

# Install build dependencies in a single layer and clean up
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libaio1 unzip wget build-essential ca-certificates \
  && rm -rf /var/lib/apt/lists/*

ENV CARGO_TARGET_DIR=/app/target
ENV RUSTFLAGS="-C target-cpu=native"

# Install Oracle Instant Client for the build target architecture
RUN set -eux; \
  mkdir -p /opt/oracle; cd /opt/oracle; \
  case "${TARGETARCH:-amd64}" in \
    arm64) \
      B_BASIC="https://download.oracle.com/otn_software/linux/instantclient/1919000/instantclient-basic-linux.arm64-19.19.0.0.0dbru.zip"; \
      B_SDK="https://download.oracle.com/otn_software/linux/instantclient/1919000/instantclient-sdk-linux.arm64-19.19.0.0.0dbru.zip";; \
    amd64|x86_64) \
      B_BASIC="https://download.oracle.com/otn_software/linux/instantclient/1919000/instantclient-basic-linux.x64-19.19.0.0.0dbru.zip"; \
      B_SDK="https://download.oracle.com/otn_software/linux/instantclient/1919000/instantclient-sdk-linux.x64-19.19.0.0.0dbru.zip";; \
    *) echo "Unsupported arch ${TARGETARCH}, skipping instant client install"; exit 0;; \
  esac; \
  wget -q "$B_BASIC" -O basic.zip; unzip basic.zip; rm basic.zip; \
  wget -q "$B_SDK" -O sdk.zip; unzip sdk.zip; rm sdk.zip; \
  mv instantclient_19_19 instantclient || true

ENV LD_LIBRARY_PATH=/opt/oracle/instantclient
ENV OCI_LIB_DIR=/opt/oracle/instantclient
ENV OCI_INC_DIR=/opt/oracle/instantclient/sdk/include

# Copy manifest first and build dummy to cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && printf "fn main() { println!(\"dummy\"); }\n" > src/main.rs

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release

# Copy full source and build the real binaries
COPY . .
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --bins && \
    cp /app/target/release/deductible-tracker /app/deductible-tracker && \
    cp /app/target/release/migrate /app/migrate

# Stage 2: Runtime
FROM oraclelinux:10-slim AS runtime
WORKDIR /app

# Minimal runtime deps
RUN microdnf install -y libaio openssl && microdnf clean all

# Copy Oracle Instant Client from builder
COPY --from=builder /opt/oracle/instantclient /opt/oracle/instantclient
ENV LD_LIBRARY_PATH=/opt/oracle/instantclient

# Copy binaries and assets
COPY --from=builder /app/deductible-tracker /app/deductible-tracker
COPY --from=builder /app/migrate /app/migrate
COPY migrations /app/migrations
COPY static /app/static

# Create non-root user and set permissions
RUN groupadd -r appuser && useradd -r -g appuser appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 8080
CMD ["./deductible-tracker"]
