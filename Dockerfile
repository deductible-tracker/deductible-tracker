# syntax=docker/dockerfile:1

ARG PREBUILD_TESTS=0
ARG BUILD_JOBS=2

FROM dhi.io/rust:1 AS rust-toolchain

FROM dhi.io/node:24-dev AS builder
WORKDIR /app

ARG PREBUILD_TESTS
ARG BUILD_JOBS

RUN apt-get update \
  && apt-get install -y --no-install-recommends build-essential pkg-config \
  && rm -rf /var/lib/apt/lists/*

COPY --from=rust-toolchain /usr/local /usr/local

ENV CARGO_HOME=/cargo
ENV CARGO_TARGET_DIR=/app/target-debian13
ENV RUSTUP_HOME=/usr/local/rustup
ENV RUSTUP_TOOLCHAIN=stable
ENV RUSTFLAGS=""
ENV PATH=/usr/local/cargo/bin:/usr/local/bin:${PATH}

COPY Cargo.toml Cargo.lock package.json package-lock.json ./

RUN mkdir -p src && echo 'fn main() { println!("__dummy__"); }' > src/main.rs

RUN --mount=type=cache,target=/cargo/registry \
  --mount=type=cache,target=/cargo/git \
  --mount=type=cache,target=/app/target-debian13 \
  cargo fetch

RUN set -eux; \
    npm ci --include=optional; \
    arch="$(uname -m)"; \
    case "$arch" in \
      aarch64|arm64) npm install --no-save @tailwindcss/oxide-linux-arm64-gnu ;; \
      x86_64|amd64) npm install --no-save @tailwindcss/oxide-linux-x64-gnu ;; \
      *) echo "Unsupported architecture for Tailwind native bindings: $arch" >&2; exit 1 ;; \
    esac

COPY . .
# Build all three binaries in one cargo invocation to maximize shared compilation,
# then run prepare-assets to fingerprint static assets.
RUN --mount=type=cache,target=/cargo/registry \
    --mount=type=cache,target=/cargo/git \
    --mount=type=cache,target=/app/target-debian13 \
    set -eux; \
    cargo build --locked --release --no-default-features \
      --features server,asset-pipeline \
      --bin prepare-assets --bin deductible-tracker --bin migrate \
      -j"${BUILD_JOBS}" && \
    RUST_ENV=development ./target-debian13/release/prepare-assets && \
    cp /app/target-debian13/release/deductible-tracker /app/deductible-tracker && \
    cp /app/target-debian13/release/migrate /app/migrate

RUN rm -rf /app/node_modules /app/.parcel-cache /app/.vite /tmp/* /app/package-lock.json /app/.npm || true

RUN --mount=type=cache,target=/cargo/registry \
    --mount=type=cache,target=/cargo/git \
    --mount=type=cache,target=/app/target-debian13 \
    if [ "${PREBUILD_TESTS}" = "1" ]; then \
      CARGO_BUILD_JOBS="${BUILD_JOBS}" \
      CARGO_INCREMENTAL=0 \
      CARGO_PROFILE_DEV_DEBUG=0 \
      RUSTFLAGS="-C debuginfo=0" \
      cargo build --locked --no-default-features --bin migrate -j"${BUILD_JOBS}" && \
      cargo test --locked --no-default-features -j"${BUILD_JOBS}" --lib --test integration_js_jest --test integration_receipts_audit --no-run; \
    fi

# --- Minimal runtime image matching builder glibc (Debian 13 / trixie) ---
FROM debian:trixie-slim AS runtime
WORKDIR /app

RUN groupadd -g 65532 nonroot && \
    useradd -u 65532 -g nonroot -s /usr/sbin/nologin -d /home/nonroot -m nonroot && \
    apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    mkdir -p /app/static /app/wallet && chown -R nonroot:nonroot /app

ENV HOME=/home/nonroot
ENV RUST_ENV=production
ENV TNS_ADMIN=/app/wallet
ENV DB_WALLET_DIR=/app/wallet

COPY --from=builder --chown=nonroot:nonroot /app/deductible-tracker /app/deductible-tracker
COPY --from=builder --chown=nonroot:nonroot /app/migrate /app/migrate
COPY --chown=nonroot:nonroot migrations /app/migrations
COPY --from=builder --chown=nonroot:nonroot /app/static/index.html /app/static/index.html
COPY --from=builder --chown=nonroot:nonroot /app/static/fonts /app/static/fonts
COPY --from=builder --chown=nonroot:nonroot /app/public /app/public
COPY --chown=nonroot:nonroot Wallet_deductibledb /app/wallet

USER nonroot

EXPOSE 8080
CMD ["./deductible-tracker"]
