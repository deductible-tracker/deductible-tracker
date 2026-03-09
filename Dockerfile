# syntax=docker/dockerfile:1

ARG ENABLE_OCR=0

FROM dhi.io/rust:1 AS rust-toolchain

# Stage 1: Build (with BuildKit cache mounts)
FROM oraclelinux:10-slim AS builder
WORKDIR /app

ARG ENABLE_OCR
RUN microdnf install -y \
      oracle-instantclient-release-23ai-el9 \
    && microdnf install -y \
      gcc \
      make \
  nodejs \
  npm \
      perl \
      git \
      ca-certificates \
      libaio \
      libnsl \
      oracle-instantclient-basic \
      oracle-instantclient-devel \
    && microdnf clean all

COPY --from=rust-toolchain /usr/local /usr/local

ENV CARGO_HOME=/cargo
ENV CARGO_TARGET_DIR=/app/target-ol9
ENV RUSTFLAGS=""
ENV PATH=/usr/local/bin:${PATH}
ENV LD_LIBRARY_PATH=/usr/local/lib:/usr/lib/oracle/23/client64/lib
ENV OCI_LIB_DIR=/usr/lib/oracle/23/client64/lib
ENV OCI_INC_DIR=/usr/include/oracle/23/client64

# Copy manifest first and fetch dependencies to cache them
COPY Cargo.toml Cargo.lock package.json package-lock.json ./

# Create a tiny dummy source file so cargo recognizes a target
# (allows `cargo fetch` to run using only the manifest files)
RUN mkdir -p src && echo 'fn main() { println!("__dummy__"); }' > src/main.rs

RUN --mount=type=cache,target=/cargo/registry \
  --mount=type=cache,target=/cargo/git \
  --mount=type=cache,target=/app/target-ol9 \
  cargo fetch

RUN set -eux; \
    npm install --include=optional; \
    arch="$(uname -m)"; \
    case "$arch" in \
      aarch64|arm64) npm install --no-save @tailwindcss/oxide-linux-arm64-gnu ;; \
      x86_64|amd64) npm install --no-save @tailwindcss/oxide-linux-x64-gnu ;; \
      *) echo "Unsupported architecture for Tailwind native bindings: $arch" >&2; exit 1 ;; \
    esac


# Copy full source and build the real binaries
COPY . .
RUN --mount=type=cache,target=/cargo/registry \
    --mount=type=cache,target=/cargo/git \
    --mount=type=cache,target=/app/target-ol9 \
    if [ "${ENABLE_OCR}" = "1" ]; then \
      echo "ERROR: ENABLE_OCR=1 is not supported in the minimal hardened builder without extra native OCR packages." >&2; \
      exit 1; \
    fi; \
    CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 CARGO_PROFILE_RELEASE_STRIP=false cargo build --release --bins -j1 && \
    RUST_ENV=development PREPARE_ASSETS_ONLY=1 ./target-ol9/release/deductible-tracker && \
    cp /app/target-ol9/release/deductible-tracker /app/deductible-tracker && \
    cp /app/target-ol9/release/migrate /app/migrate

# Stage 2: Runtime
# Use OL 9 with the Oracle Instant Client repository enabled via the release package.
FROM oraclelinux:10-slim AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd -r -g appuser -m -d /home/appuser appuser

# Minimal runtime deps
# - libaio: async I/O required by Oracle OCI
# - libnsl: Oracle Net; OL 9 ships libnsl.so.3 (libnsl2)
# - openssl: TLS support
RUN microdnf install -y \
      oracle-instantclient-release-23ai-el9 \
    && microdnf install -y \
      libaio \
      libnsl \
      openssl \
      oracle-instantclient-basiclite \
    && microdnf clean all && \
    rm -f /usr/lib/oracle/23/client64/bin/adrci /usr/lib/oracle/23/client64/bin/genezi && \
    rm -f /usr/lib/oracle/23/client64/lib/ojdbc8.jar /usr/lib/oracle/23/client64/lib/xstreams.jar && \
    rm -f /usr/lib/oracle/23/client64/lib/libocci.so /usr/lib/oracle/23/client64/lib/libocci.so.10.1 /usr/lib/oracle/23/client64/lib/libocci.so.11.1 /usr/lib/oracle/23/client64/lib/libocci.so.12.1 /usr/lib/oracle/23/client64/lib/libocci.so.18.1 /usr/lib/oracle/23/client64/lib/libocci.so.19.1 /usr/lib/oracle/23/client64/lib/libocci.so.20.1 /usr/lib/oracle/23/client64/lib/libocci.so.21.1 /usr/lib/oracle/23/client64/lib/libocci.so.22.1 /usr/lib/oracle/23/client64/lib/libocci.so.23.1 && \
    rm -f /usr/share/oracle/23/client64/doc/BASIC_LITE_LICENSE /usr/share/oracle/23/client64/doc/BASIC_LITE_README && \
    rm -rf /usr/lib/oracle/23/client64/lib/network && \
    ln -sf /usr/lib64/libnsl.so.3 /usr/lib64/libnsl.so.1 2>/dev/null || true

# Optional OCR runtime libs (only installed when ENABLE_OCR=1)
ARG ENABLE_OCR=0
RUN if [ "${ENABLE_OCR}" = "1" ]; then \
      # Try installing tesseract & leptonica via microdnf. These packages may
      # require EPEL or additional repos on some Oracle Linux installs. If your
      # environment doesn't provide them, consider building a Debian-based
      # runtime image or providing the libs another way.
      microdnf install -y tesseract leptonica && microdnf clean all || true; \
    fi

ENV LD_LIBRARY_PATH=/usr/lib/oracle/23/client64/lib
ENV OCI_LIB_DIR=/usr/lib/oracle/23/client64/lib
ENV HOME=/home/appuser
ENV RUST_ENV=production
ENV TNS_ADMIN=/app/wallet

# Copy binaries and assets
COPY --from=builder --chown=appuser:appuser /app/deductible-tracker /app/deductible-tracker
COPY --from=builder --chown=appuser:appuser /app/migrate /app/migrate
COPY --chown=appuser:appuser migrations /app/migrations
COPY --from=builder --chown=appuser:appuser /app/static /app/static
COPY --from=builder --chown=appuser:appuser /app/public /app/public
RUN rm -rf /app/static/assets

USER appuser

EXPOSE 8080
CMD ["./deductible-tracker"]
