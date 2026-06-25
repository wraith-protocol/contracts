# Use a specific, pinned base image for reproducibility
FROM debian:bookworm-slim@sha256:12c39665e988562b6b8b01c60af0f19a2879574164b18c6ebf21625a6a68ffbd

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

# Install rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# Add cargo to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Define stellar-cli version to pin
ARG STELLAR_CLI_VERSION="22.0.0"

# Install stellar-cli
RUN cargo install --locked stellar-cli --version ${STELLAR_CLI_VERSION}

# Create workspace directory
WORKDIR /workspace

# Copy the entire workspace repository (done by Docker build context)
# The rust-toolchain.toml is expected in /workspace/contracts/stellar/build/rust-toolchain.toml
# or we just rely on cargo recognizing it if we copy it to the root or the stellar contracts dir.
COPY . .

# Since rust-toolchain.toml is in contracts/stellar/build, let's copy it to contracts/stellar so rustup picks it up automatically
RUN cp contracts/stellar/build/rust-toolchain.toml contracts/stellar/rust-toolchain.toml

# Pre-install the pinned rust toolchain based on rust-toolchain.toml
RUN cd contracts/stellar && rustup show

# Set build arguments for the script
ARG COMMIT_HASH
ENV COMMIT_HASH=${COMMIT_HASH}

# Make build script executable and set it as the entrypoint
RUN chmod +x contracts/stellar/build/build.sh

ENTRYPOINT ["contracts/stellar/build/build.sh"]
