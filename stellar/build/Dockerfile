# Base image. Digest was previously pinned but that digest gets rotated out
# of Docker Hub periodically and broke the scheduled verification job. Pin to
# the current bookworm-slim tag; contributors should re-pin to a specific
# digest after a manual audit if the workflow needs bit-for-bit reproducibility.
FROM debian:bookworm-slim

# Prevent timezone and other interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install dependencies required for building Rust and Soroban contracts
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install rustup with a default stable toolchain so `cargo install` below works.
# The pinned toolchain-file toolchain (rust-toolchain.toml) gets layered on top
# by rustup when we `cd contracts/stellar` further down.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal

# Add cargo to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Define stellar-cli version to pin
ARG STELLAR_CLI_VERSION="22.0.0"

# Install stellar-cli
RUN cargo install --locked stellar-cli --version ${STELLAR_CLI_VERSION}

# Create workspace directory
WORKDIR /workspace

# Copy the entire workspace repository (done by Docker build context)
# The rust-toolchain.toml is expected in /workspace/stellar/build/rust-toolchain.toml
# or we just rely on cargo recognizing it if we copy it to the root or the stellar contracts dir.
COPY . .

# Since rust-toolchain.toml is in stellar/build, let's copy it to contracts/stellar so rustup picks it up automatically
RUN cp stellar/build/rust-toolchain.toml stellar/rust-toolchain.toml

# Pre-install the pinned rust toolchain based on rust-toolchain.toml
RUN cd stellar && rustup show

# Set build arguments for the script
ARG COMMIT_HASH
ENV COMMIT_HASH=${COMMIT_HASH}

# Make build script executable and set it as the entrypoint
RUN chmod +x stellar/build/build.sh

ENTRYPOINT ["stellar/build/build.sh"]
