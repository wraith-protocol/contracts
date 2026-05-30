# Stellar Contract Wasm Size Budget

This document tracks the Wasm binary sizes of the Stellar contracts and sets budgets to ensure they remain within Soroban limits.

## Current Measurements (Initial)

| Contract | Debug | Release | Optimized | Budget | Headroom |
|---|---|---|---|---|---|
| `stealth-announcer` | 2.6 MB | 2.9 KB | 1.3 KB | 110 KB | 108.7 KB |
| `stealth-registry` | 2.6 MB | 5.5 KB | 2.1 KB | 110 KB | 107.9 KB |
| `stealth-sender` | 2.7 MB | 13 KB | 4.1 KB | 110 KB | 105.9 KB |
| `wraith-names` | 2.7 MB | 18 KB | 6.0 KB | 110 KB | 104 KB |

*\* `stellar contract optimize` is currently unavailable in the environment due to missing `wasm-opt`. Initial optimizations will focus on build profile settings.*

## Wasm Size Budget

Soroban has a hard limit on contract Wasm size (currently ~140 KB for some operations, but upload limits can be higher). We aim for a budget of **110 KB** per contract to allow for ~20% headroom.

## What to do if you exceed the budget

If a contract exceeds its budget, follow these steps:

1. **Audit Dependencies**: Check `Cargo.toml` for unused features or heavy dependencies. Use `cargo tree`.
2. **Profile Build**: Ensure `lto = "fat"`, `codegen-units = 1`, and `opt-level = "z"` are set in `Cargo.toml`.
3. **Refactor Code**:
    *   Replace `panic!`, `assert!`, and `unwrap()` with `Result` or `panic_with_error!`.
    *   Avoid `std::string::String` and `std::vec::Vec` where possible; use `soroban_sdk` equivalents.
    *   Minimize use of `format!` and complex formatting.
4. **Use `wasm-opt`**: If not already done, run `stellar contract optimize`.
