# WASM Size Metrics

This document tracks the size of the compiled Soroban contracts to ensure they remain under the network limit (typically around 64 KB to 110 KB absolute budget depending on the Soroban release version and network configurations).

Our target budget for these contracts is **< 110 KB**.

## Current Baseline (Post-Optimization Round 2)

Measurements reflect WASM payload sizes after Workspace `Cargo.toml` Release Profile optimizations (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = "debuginfo"`, `panic = "abort"`) have been applied.

| Contract | Before Optimization (`cargo build --release`) | After Cargo Profile Tweaks (`cargo build --release`) |
|----------|:---:|:---:|
| **stealth_announcer** | 16.19 KB | **2.01 KB** (2,059 bytes) |
| **stealth_registry** | 99.41 KB | **3.20 KB** (3,276 bytes) |
| **stealth_sender** | 111.81 KB | **6.34 KB** (6,491 bytes) |
| **wraith_names** | 115.21 KB | **9.53 KB** (9,755 bytes) |

> **Note**: A GitHub Actions CI Gate is now running on every PR in `stellar` to verify all generated `target/wasm32-unknown-unknown/release/*.wasm` files remain under 110,000 bytes (110 KB) after standard compiler optimizations, and `stellar contract optimize`.

---

## Size Budget Recipe: What to do if you exceed the limit

If a feature PR pushes a contract over the 110 KB CI gate, apply the following strategies to reduce contract size (no semantic changes required):

1. **Remove Formatted Strings / Panics:**
   - Avoid `format!`, `panic!("...")`, `assert!(..., "...")` and string manipulation. These introduce heavy panic and formatting machinery from the standard/core library into the WASM binary.
   - Use simple `panic!()` or return custom `Error` enums instead.
   
2. **Optimize Error Enums as `u32` Codes:**
   - Instead of large error structs, use `#contracterror` enums that get compiled down to small `u32` integers over the wire.
   
3. **Use Smaller Integer Types When Safely Bounded:**
   - If a counter is always small, `u32` operations take fewer WASM instructions than 128-bit or 256-bit BigInt math.
   
4. **Avoid Heavy Dependencies:**
   - Do not pull in large external crates (like `serde` with full macros, or `sha2` native rust implementations). Use the Soroban Environment `Env::crypto().sha256()` instead.

5. **Enable `wasm-opt` (if not already handled by CI):**
   - Use `soroban contract optimize` (which uses `wasm-opt` under the hood) locally to strip down the final binary by executing `stellar contract optimize --wasm target/wasm32-unknown-unknown/release/...`
