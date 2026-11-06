# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# Stage 1: Build the Rust binary
# -----------------------------------------------------------------------------
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build optimized release binary
RUN cargo build --release --bin court

# -----------------------------------------------------------------------------
# Stage 2: Minimal runtime image
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install ca-certificates and minimal runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled court binary
COPY --from=builder /app/target/release/court /usr/local/bin/court

# Set environment
ENV RUST_BACKTRACE=1

# Expose default proxy port if running in network mode
EXPOSE 12345

# Default entrypoint runs the MCP proxy server
ENTRYPOINT ["court", "proxy"]

