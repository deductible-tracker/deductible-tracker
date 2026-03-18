# syntax=docker/dockerfile:1

ARG PREBUILD_TESTS=0

FROM dhi.io/rust:1 AS rust-toolchain

# Stage 1: Build (with BuildKit cache mounts)
FROM oraclelinux:9-slim AS builder
WORKDIR /app

ARG PREBUILD_TESTS
RUN microdnf install -y \
      oracle-instantclient-release-23ai-el9 \
    && microdnf install -y \
      gcc \
      make \
      curl \
      perl \
      git \
      ca-certificates \
      libaio \
      libnsl \
      oracle-instantclient-basic \
      oracle-instantclient-devel \
    && curl -fsSL https://rpm.nodesource.com/setup_24.x | bash - \
    && microdnf install -y nodejs \
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
    npm ci --include=optional; \
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
    set -eux; \
    app_features="server"; \
    CARGO_PROFILE_RELEASE_LTO=true \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_STRIP=true \
    cargo build --locked --release --no-default-features --features server,asset-pipeline --bin prepare-assets -j1 && \
    RUST_ENV=development ./target-ol9/release/prepare-assets && \
    CARGO_PROFILE_RELEASE_LTO=true \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_STRIP=true \
    cargo build --locked --release --no-default-features --features "${app_features}" --bin deductible-tracker -j1 && \
    CARGO_PROFILE_RELEASE_LTO=true \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_STRIP=true \
    cargo build --locked --release --no-default-features --bin migrate -j1 && \
    cp /app/target-ol9/release/deductible-tracker /app/deductible-tracker && \
    cp /app/target-ol9/release/migrate /app/migrate

# Remove build-time dev artifacts that should not be copied into runtime
RUN rm -rf /app/node_modules /app/.parcel-cache /app/.vite /tmp/* /app/package-lock.json /app/.npm || true

RUN --mount=type=cache,target=/cargo/registry \
    --mount=type=cache,target=/cargo/git \
    --mount=type=cache,target=/app/target-ol9 \
    if [ "${PREBUILD_TESTS}" = "1" ]; then \
      CARGO_BUILD_JOBS=1 \
      CARGO_INCREMENTAL=0 \
      CARGO_PROFILE_DEV_DEBUG=0 \
      RUSTFLAGS="-C debuginfo=0" \
      cargo build --locked --no-default-features --bin migrate -j1 && \
      cargo test --locked --no-default-features -j1 --lib --test integration_js_jest --test integration_receipts_audit --no-run; \
    fi

# Stage 2: Runtime
# Use Oracle Linux 9 slim as the base for the final image.
# We then prune it further by removing unneeded tools and files.
FROM oraclelinux:9-slim AS runtime
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
    # Remove microdnf and other package management tools to harden and shrink image
    rm -rf /var/cache/dnf /var/cache/yum && \
    # Prune Oracle Instant Client (already doing this, but being thorough)
    rm -f /usr/lib/oracle/23/client64/bin/adrci /usr/lib/oracle/23/client64/bin/genezi && \
    rm -f /usr/lib/oracle/23/client64/lib/ojdbc* /usr/lib/oracle/23/client64/lib/xstreams.jar && \
    rm -f /usr/lib/oracle/23/client64/lib/libocci* && \
    rm -f /usr/share/oracle/23/client64/doc/BASIC_LITE_LICENSE /usr/share/oracle/23/client64/doc/BASIC_LITE_README && \
    rm -rf /usr/lib/oracle/23/client64/lib/network && \
    ln -sf /usr/lib64/libnsl.so.3 /usr/lib64/libnsl.so.1 2>/dev/null || true

ENV LD_LIBRARY_PATH=/usr/lib/oracle/23/client64/lib
ENV OCI_LIB_DIR=/usr/lib/oracle/23/client64/lib
ENV HOME=/home/appuser
ENV RUST_ENV=production
ENV TNS_ADMIN=/app/wallet

# Copy binaries and assets
COPY --from=builder --chown=appuser:appuser /app/deductible-tracker /app/deductible-tracker
COPY --from=builder --chown=appuser:appuser /app/migrate /app/migrate
COPY --chown=appuser:appuser migrations /app/migrations
COPY --from=builder --chown=appuser:appuser /app/static/index.html /app/static/index.html
COPY --from=builder --chown=appuser:appuser /app/static/fonts /app/static/fonts
COPY --from=builder --chown=appuser:appuser /app/public /app/public

USER appuser

EXPOSE 8080
CMD ["./deductible-tracker"]
