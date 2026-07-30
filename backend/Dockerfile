# CONCH Platform - Rust Backend Dockerfile
FROM rust:1.88 AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Update Rust toolchain
RUN rustup update

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Build dependencies first (for caching)
# Stub all binary entry points so Cargo can resolve the full dependency graph
RUN mkdir -p src/bin \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && echo "fn main() {}" > src/bin/conch.rs \
    && echo "fn main() {}" > src/bin/agent.rs
RUN cargo build --release
RUN rm -rf src

# Copy source and rebuild (touch forces Cargo to recompile)
COPY src ./src
RUN touch src/main.rs src/lib.rs src/bin/conch.rs src/bin/agent.rs && cargo build --release

# Production image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 conch

# Copy binaries
COPY --from=builder /app/target/release/conch-server /usr/local/bin/conch-server
COPY --from=builder /app/target/release/conch /usr/local/bin/conch
COPY --from=builder /app/target/release/conch-agent /usr/local/bin/conch-agent

# Switch to non-root user
USER conch

# Expose port
EXPOSE 3000

# Run the server
CMD ["conch-server"]
