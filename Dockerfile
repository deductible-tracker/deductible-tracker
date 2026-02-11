# syntax=docker/dockerfile:1.4

# Stage 1: Build (with BuildKit cache mounts)
FROM rust:1.93.0-slim-bookworm AS builder
WORKDIR /app

ARG TARGETARCH

# Oracle Instant Client configuration (centralized for maintainability)
ARG ORACLE_IC_BASE_URL="https://download.oracle.com/otn_software/linux/instantclient"
ARG ORACLE_IC_VERSION_PATH="1919000"
ARG ORACLE_IC_VERSION_FULL="19.19.0.0.0dbru"

# Install build dependencies in a single layer and clean up
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libaio1 unzip wget build-essential ca-certificates \
  && rm -rf /var/lib/apt/lists/*

ENV CARGO_TARGET_DIR=/app/target
ENV RUSTFLAGS=""

# Install Oracle Instant Client for the build target architecture
RUN set -eux; \
  mkdir -p /opt/oracle; cd /opt/oracle; \
  case "${TARGETARCH:-amd64}" in \
    arm64) \
      B_BASIC="${ORACLE_IC_BASE_URL}/${ORACLE_IC_VERSION_PATH}/instantclient-basic-linux.arm64-${ORACLE_IC_VERSION_FULL}.zip"; \
      B_SDK="${ORACLE_IC_BASE_URL}/${ORACLE_IC_VERSION_PATH}/instantclient-sdk-linux.arm64-${ORACLE_IC_VERSION_FULL}.zip";; \
    amd64|x86_64) \
      B_BASIC="${ORACLE_IC_BASE_URL}/${ORACLE_IC_VERSION_PATH}/instantclient-basic-linux.x64-${ORACLE_IC_VERSION_FULL}.zip"; \
      B_SDK="${ORACLE_IC_BASE_URL}/${ORACLE_IC_VERSION_PATH}/instantclient-sdk-linux.x64-${ORACLE_IC_VERSION_FULL}.zip";; \
    *) echo "ERROR: Unsupported arch ${TARGETARCH}, cannot install Oracle Instant Client"; exit 1;; \
  esac; \
  wget -q "$B_BASIC" -O basic.zip; unzip basic.zip; rm basic.zip; \
  wget -q "$B_SDK" -O sdk.zip; unzip sdk.zip; rm sdk.zip; \
  mv instantclient_19_19 instantclient; \
  test -d instantclient

ENV LD_LIBRARY_PATH=/opt/oracle/instantclient
ENV OCI_LIB_DIR=/opt/oracle/instantclient
ENV OCI_INC_DIR=/opt/oracle/instantclient/sdk/include

# Copy manifest first and fetch dependencies to cache them
COPY Cargo.toml Cargo.lock ./

# Create a tiny dummy source file so cargo recognizes a target
# (allows `cargo fetch` to run using only the manifest files)
RUN mkdir -p src && echo 'fn main() { println!("__dummy__"); }' > src/main.rs

RUN --mount=type=cache,target=/root/.cargo/registry \
  --mount=type=cache,target=/root/.cargo/git \
  --mount=type=cache,target=/app/target \
  cargo fetch


# Copy full source and build the real binaries
COPY . .
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --bins && \
    cp /app/target/release/deductible-tracker /app/deductible-tracker && \
    cp /app/target/release/migrate /app/migrate

# Stage 2: Runtime
# Use OL 9 – Oracle Instant Client 19.x is certified on OL 9;
# OL 10's newer glibc/TLS stack causes silent TCPS connection failures.
FROM oraclelinux:9-slim AS runtime
WORKDIR /app

# Minimal runtime deps (libnsl required by IC 19.x for Oracle Net)
RUN microdnf install -y libaio libnsl openssl && microdnf clean all

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
