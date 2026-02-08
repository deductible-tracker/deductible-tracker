# Stage 1: Build
FROM rust:1.93.0-slim-bookworm AS builder

WORKDIR /app

# Reduce memory usage during build
ENV CARGO_BUILD_JOBS=1 \
    CARGO_PROFILE_RELEASE_LTO=false \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=8 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=2

# Install dependencies required for building (and linking if dynamic)
# We need libaio for Oracle
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libaio1 \
    unzip \
    wget \
    build-essential

# We need the Oracle Instant Client SDK to link against 'oracle' crate
# Note: This is a simplification. Real-world OCI linking often requires specific paths.
# Since we are targeting ARM64, we need ARM64 client if we are building ON ARM64.
# If we are cross-compiling, it's harder.
# We will assume this Dockerfile is built WITH the target platform (e.g. docker buildx --platform linux/arm64)

# Create a dummy project to cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true 
# (The above might fail if oracle crate build script fails due to missing OCI libs, but we haven't installed them yet)

# Install Oracle Instant Client (Basic + SDK)
# This part is tricky to get right for both AMD64 and ARM64 in one file without args.
# I'll rely on a script or just download the ARM64 one since the target is ARM.
# Actually, let's just attempt to build. If 'oracle' crate is used, we need OCI.
# If we can't easily install OCI in the builder, the build will fail.

# To be safe for this specific request "deploy to Oracle Arm", I will hardcode ARM64 download if possible,
# or assume the builder environment has it. 
# BUT, to make this robust for the user who might run it locally (x86), I should probably use a conditional.

# For now, I'll skip the complex OCI setup in the builder and rely on the fact that
# if the user runs this locally on a Mac, they aren't using this Dockerfile for dev usually.
# On CI, we will use QEMU to build for ARM64.

# Let's try to install the libs.
RUN mkdir -p /opt/oracle
WORKDIR /opt/oracle
# Downloading ARM64 Instant Client
RUN wget https://download.oracle.com/otn_software/linux/instantclient/1919000/instantclient-basic-linux.arm64-19.19.0.0.0dbru.zip -O basic.zip && \
    wget https://download.oracle.com/otn_software/linux/instantclient/1919000/instantclient-sdk-linux.arm64-19.19.0.0.0dbru.zip -O sdk.zip && \
    unzip basic.zip && \
    unzip sdk.zip && \
    rm *.zip && \
    mv instantclient_19_19 instantclient

ENV LD_LIBRARY_PATH=/opt/oracle/instantclient
ENV OCI_LIB_DIR=/opt/oracle/instantclient
ENV OCI_INC_DIR=/opt/oracle/instantclient/sdk/include

WORKDIR /app
COPY . .
# Touch main.rs to force rebuild
RUN touch src/main.rs
RUN cargo build --release
RUN cargo build --release --bin migrate

# Stage 2: Runtime
FROM oraclelinux:9-slim

WORKDIR /app

# Create a non-root user
RUN groupadd -r appuser && useradd -r -g appuser appuser

# Install runtime deps
RUN microdnf install -y libaio openssl && microdnf clean all

# Copy Oracle Instant Client from builder
COPY --from=builder /opt/oracle/instantclient /opt/oracle/instantclient

COPY --from=builder /app/target/release/deductible-tracker /app/deductible-tracker
COPY --from=builder /app/target/release/migrate /app/migrate
COPY migrations /app/migrations
COPY static /app/static

ENV LD_LIBRARY_PATH=/opt/oracle/instantclient

# Give ownership to the appuser
RUN chown -R appuser:appuser /app

USER appuser

EXPOSE 8080

CMD ["./deductible-tracker"]
