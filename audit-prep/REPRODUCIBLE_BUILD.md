# Reproducible Build Instructions

## Overview

This document provides step-by-step instructions for reproducing the exact WASM binaries of the Wraith Protocol Stellar contracts. Reproducible builds are critical for audit transparency and mainnet deployment verification.

**Target:** Bit-for-bit identical WASM files  
**Method:** Deterministic compilation with pinned toolchain  
**Verification:** SHA-256 hash comparison

---

## Prerequisites

### System Requirements

**Supported Platforms:**
- Linux (x86_64, aarch64)
- macOS (Intel, Apple Silicon)
- Windows (WSL2 required)

**Minimum Resources:**
- 4GB RAM
- 2GB free disk space
- Internet connection (for initial setup)

### Required Tools

1. **Rust Toolchain:** 1.75.0 (pinned)
2. **Soroban CLI:** 22.0.1
3. **Git:** 2.40+
4. **Build Tools:** `gcc`, `make`, `pkg-config`

---

## Installation Steps

### Step 1: Install Rust

```bash
# Install rustup (Rust toolchain manager)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Source the environment
source $HOME/.cargo/env

# Install specific Rust version
rustup install 1.75.0
rustup default 1.75.0

# Add WASM target
rustup target add wasm32-unknown-unknown

# Verify installation
rustc --version
# Expected: rustc 1.75.0 (82e1608df 2023-12-21)
```

### Step 2: Install Soroban CLI

```bash
# Install Soroban CLI v22.0.1
cargo install --locked soroban-cli --version 22.0.1

# Verify installation
soroban --version
# Expected: soroban 22.0.1
```

### Step 3: Install Build Dependencies

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev
```

**macOS:**
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Or use Homebrew
brew install pkg-config openssl
```

**Windows (WSL2):**
```bash
# Inside WSL2 Ubuntu
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev
```

---

## Build Process

### Step 1: Clone Repository

```bash
# Clone the repository
git clone https://github.com/wraith-protocol/contracts.git
cd contracts

# Checkout the audit tag
git checkout stellar-audit-v1.0.0

# Verify you're on the correct commit
git rev-parse HEAD
# Expected: [COMMIT_HASH_TBD]
```

### Step 2: Build Contracts

```bash
# Navigate to Stellar directory
cd stellar

# Clean any previous builds
cargo clean

# Build all contracts in release mode with optimizations
cargo build --target wasm32-unknown-unknown --release --workspace

# Contracts are located in:
# target/wasm32-unknown-unknown/release/*.wasm
```

### Step 3: Optimize WASMs

Soroban requires optimized WASMs. The SDK includes an optimizer:

```bash
# Optimize each contract
for contract in stealth-announcer stealth-registry stealth-sender wraith-names; do
  soroban contract optimize \
    --wasm target/wasm32-unknown-unknown/release/$contract.wasm \
    --wasm-out target/wasm32-unknown-unknown/release/${contract}_optimized.wasm
done
```

**Alternative:** Use `stellar/build.sh` script:

```bash
# Use the provided build script
chmod +x build.sh
./build.sh

# Optimized WASMs are in: build/
```

---

## Build Verification

### Step 1: Generate Hashes

```bash
# Navigate to build output directory
cd build/

# Generate SHA-256 hashes for all contracts
for contract in *.wasm; do
  echo "$(sha256sum $contract | cut -d' ' -f1) $contract"
done > checksums.txt

# View checksums
cat checksums.txt
```

### Step 2: Compare with Reference Hashes

**Reference Hashes (TBD):**

```
[SHA256_HASH_1] stealth_announcer.wasm
[SHA256_HASH_2] stealth_registry.wasm
[SHA256_HASH_3] stealth_sender.wasm
[SHA256_HASH_4] wraith_names.wasm
```

### Step 3: Verify Match

```bash
# Compare your hashes with reference
diff checksums.txt ../audit-prep/reference-checksums.txt

# No output = perfect match
```

---

## Deterministic Build Configuration

### Cargo.toml Settings

All contracts use the following release profile:

```toml
[profile.release]
opt-level = "z"          # Optimize for size
overflow-checks = true   # Keep overflow checks
debug = false            # No debug symbols
strip = "debuginfo"      # Strip debug info
lto = true               # Link-time optimization
codegen-units = 1        # Single codegen unit for better optimization
panic = "abort"          # Abort on panic (smaller WASM)
```

### Environment Variables

Set these for deterministic builds:

```bash
export RUSTFLAGS="-C link-arg=-s"
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
```

### Dependency Locking

```bash
# Verify Cargo.lock is present and up-to-date
ls -la Cargo.lock

# If missing, regenerate:
cargo update

# But for audits, use committed Cargo.lock:
git checkout Cargo.lock
```

---

## Build Artifacts

### Output Files

After successful build, you should have:

```
stellar/build/
├── stealth_announcer.wasm       (~15 KB optimized)
├── stealth_registry.wasm        (~22 KB optimized)
├── stealth_sender.wasm          (~28 KB optimized)
└── wraith_names.wasm            (~42 KB optimized)
```

### WASM Structure

Each WASM file contains:

- Contract code (Rust compiled to WASM)
- Soroban SDK runtime
- Contract metadata (spec, version)
- No debug symbols (stripped)

### Inspecting WASM

```bash
# Install wasm-tools
cargo install wasm-tools

# Inspect WASM metadata
wasm-tools print stealth_announcer.wasm | head -n 50

# Check WASM size
ls -lh build/*.wasm

# Validate WASM
wasm-tools validate stealth_announcer.wasm
```

---

## CI/CD Reproducibility

### GitHub Actions

The CI pipeline performs reproducible builds:

```yaml
name: Stellar Reproducible Build

on:
  push:
    tags:
      - 'stellar-audit-v*'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust 1.75.0
        uses: dtolnay/rust-toolchain@1.75.0
        with:
          targets: wasm32-unknown-unknown
      
      - name: Install Soroban CLI
        run: cargo install --locked soroban-cli --version 22.0.1
      
      - name: Build Contracts
        run: |
          cd stellar
          cargo build --target wasm32-unknown-unknown --release --workspace
      
      - name: Optimize WASMs
        run: |
          cd stellar
          ./build.sh
      
      - name: Generate Checksums
        run: |
          cd stellar/build
          sha256sum *.wasm > checksums.txt
      
      - name: Upload Artifacts
        uses: actions/upload-artifact@v4
        with:
          name: stellar-contracts
          path: stellar/build/*
```

---

## Troubleshooting

### Issue: Different Hash

**Possible Causes:**
1. Wrong Rust version
2. Wrong Soroban version
3. Wrong commit/tag
4. Modified source files
5. Different optimization flags

**Solution:**
```bash
# Verify toolchain
rustc --version
soroban --version

# Verify commit
git rev-parse HEAD
git status

# Clean and rebuild
cargo clean
rm -rf target/ build/
./build.sh
```

### Issue: Build Fails

**Common Errors:**

**Error: "cannot find -lssl"**
```bash
# Install OpenSSL dev libraries
sudo apt-get install libssl-dev  # Linux
brew install openssl              # macOS
```

**Error: "linker 'cc' not found"**
```bash
# Install build tools
sudo apt-get install build-essential  # Linux
xcode-select --install                # macOS
```

**Error: "wasm32-unknown-unknown not installed"**
```bash
rustup target add wasm32-unknown-unknown
```

### Issue: Optimization Fails

**Error: "soroban contract optimize failed"**
```bash
# Check Soroban version
soroban --version

# Reinstall if needed
cargo install --force --locked soroban-cli --version 22.0.1
```

---

## Docker-Based Reproducible Build

For maximum reproducibility, use Docker:

### Dockerfile

```dockerfile
FROM rust:1.75.0

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Soroban CLI
RUN cargo install --locked soroban-cli --version 22.0.1

# Add WASM target
RUN rustup target add wasm32-unknown-unknown

# Set working directory
WORKDIR /workspace

# Copy source
COPY . .

# Build
WORKDIR /workspace/stellar
RUN cargo clean
RUN cargo build --target wasm32-unknown-unknown --release --workspace
RUN ./build.sh

# Generate checksums
WORKDIR /workspace/stellar/build
RUN sha256sum *.wasm > checksums.txt

# Output checksums
CMD ["cat", "checksums.txt"]
```

### Docker Build Commands

```bash
# Build Docker image
docker build -t wraith-stellar-build .

# Run build
docker run --rm wraith-stellar-build

# Extract artifacts
docker run --rm -v $(pwd)/output:/output wraith-stellar-build \
  sh -c "cp /workspace/stellar/build/*.wasm /output/"
```

---

## Verification Checklist

Before submitting for audit:

- [ ] Rust version 1.75.0 confirmed
- [ ] Soroban CLI 22.0.1 confirmed
- [ ] Commit hash matches audit tag
- [ ] All WASMs build successfully
- [ ] All WASMs optimized successfully
- [ ] SHA-256 hashes match reference
- [ ] WASM sizes within expected ranges
- [ ] WASMs validate with `wasm-tools`
- [ ] Docker build also produces matching hashes
- [ ] CI build produces matching artifacts

---

## Reference Checksums

**To Be Generated at Audit Tag:**

```
# stellar/build/checksums.txt
[TBD] stealth_announcer.wasm
[TBD] stealth_registry.wasm
[TBD] stealth_sender.wasm
[TBD] wraith_names.wasm
```

These checksums will be generated when the `stellar-audit-v1.0.0` tag is created and will serve as the canonical reference for audit verification.

---

## Deployment Verification

After mainnet deployment:

```bash
# Fetch deployed WASM from Stellar
soroban contract fetch \
  --id [CONTRACT_ID] \
  --network mainnet \
  --out deployed.wasm

# Compare with local build
sha256sum deployed.wasm
diff deployed.wasm build/stealth_announcer.wasm

# Should be identical
```

---

## Additional Resources

- [Soroban Documentation](https://soroban.stellar.org/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [WASM Spec](https://webassembly.github.io/spec/)
- [Reproducible Builds Project](https://reproducible-builds.org/)

---

## Support

For build issues:

- **GitHub Issues:** https://github.com/wraith-protocol/contracts/issues
- **Audit Contact:** security@usewraith.xyz
- **Build Logs:** Include full `cargo build --verbose` output

---

**Last Updated:** 2026-06-26  
**Document Version:** 1.0.0  
**Rust Version:** 1.75.0  
**Soroban Version:** 22.0.1  
**SDK Version:** 22.0.0
