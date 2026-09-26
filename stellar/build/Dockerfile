# Base image, pinned by dated tag and index digest. The dated tag is retained
# on Docker Hub, and the digest makes the pin immutable. Bump both together
# following SUPPLY_CHAIN.md; Dependabot proposes the update weekly.
FROM debian:bookworm-20260918-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251

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

# Install a pinned rustup-init, verified against its published SHA-256, with no
# default toolchain. The toolchain itself comes from rust-toolchain.toml below.
ARG RUSTUP_VERSION="1.29.1"
ARG RUSTUP_INIT_SHA256="dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71"
RUN curl --proto '=https' --tlsv1.2 -sSfL -o /tmp/rustup-init \
      "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
    && echo "${RUSTUP_INIT_SHA256}  /tmp/rustup-init" | sha256sum -c - \
    && chmod +x /tmp/rustup-init \
    && /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain none \
    && rm /tmp/rustup-init

# Add cargo to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Create workspace directory
WORKDIR /workspace

# Install the pinned toolchain from rust-toolchain.toml before anything else so
# that stellar-cli and the contracts are compiled by the same rustc.
COPY stellar/build/rust-toolchain.toml stellar/rust-toolchain.toml
RUN cd stellar && rustup toolchain install && rustc --version

# Define stellar-cli version to pin. --locked uses the crate's own Cargo.lock,
# so every transitive dependency is checksum-verified against crates.io.
ARG STELLAR_CLI_VERSION="22.0.0"

# Install stellar-cli
RUN cd stellar && cargo install --locked stellar-cli --version "=${STELLAR_CLI_VERSION}"

# Copy the entire workspace repository (done by Docker build context)
COPY . .

# Since rust-toolchain.toml is in stellar/build, let's copy it to contracts/stellar so rustup picks it up automatically
RUN cp stellar/build/rust-toolchain.toml stellar/rust-toolchain.toml

# Set build arguments for the script
ARG COMMIT_HASH
ENV COMMIT_HASH=${COMMIT_HASH}

# Make build script executable and set it as the entrypoint
RUN chmod +x stellar/build/build.sh

ENTRYPOINT ["stellar/build/build.sh"]
