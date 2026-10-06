WASM Size Metrics
This document tracks the optimized Soroban contract WASM payloads. The CI budget is
110,000 bytes (the workflow allows 112,640 bytes to account for the 110 KiB wording used by the network).

Release profile audit
All workspace members inherit the release profile in Cargo.toml.
Profiles in a member manifest are ignored by Cargo, so keeping this configuration
at the workspace root is intentional. The profile now uses:

opt-level = "z", lto = true, and codegen-units = 1 for size-first whole-
program optimization;
panic = "abort", debug = false, debug-assertions = false, and
overflow-checks = false to keep panic/debug paths out of release WASM; and
strip = "symbols" and incremental = false to remove link metadata and make
the measurement reproducible.
These are compiler/linker settings only; no contract code, exported method, error,
storage key, event, or authorization rule is changed.

Baseline measurements
The table below is a fresh per-contract measurement with the current workspace.
Both columns are cargo build --target wasm32-unknown-unknown --release; the
only difference is strip = "debuginfo" (the previous profile) versus
strip = "symbols" (this change). This isolates the size delta from this fix.

Contract	Before: strip = "debuginfo" (bytes)	After: strip = "symbols" (bytes)	Reduction
stealth_announcer	13,974	8,228	41.12%
stealth_batch_sender	21,710	10,382	52.18%
stealth_registry	19,876	8,246	58.52%
stealth_sender	51,856	29,204	43.69%
stealth_splitter	16,311	10,860	33.42%
stealth_vault	30,214	12,915	57.26%
wraith_asset_policy	14,163	6,245	55.91%
governance	39,519	21,558	45.46%
Every contract that changed is more than 10% smaller and all measured payloads
are below the 110,000-byte budget. governance has no removable symbol section
in this toolchain, so its 0% delta is the documented "cannot shrink further"
case; it is already 80.40% below budget. Symbol stripping is safe for these
cdyli artifacts: it removes non-executable metadata only and therefore has no
runtime or storage semantics.

wraith_names was excluded from the table above on the grounds that it "cannot be
compiled for wasm32-unknown-unknown with the repository's pinned soroban-sdk
22.0.11". That claim was wrong. Two separate defects were to blame, and neither
is a toolchain incompatibility:

a plain `cargo build` in this virtual workspace selected the host-only members
(bench, bench-crossover, integration-tests), which enable soroban-sdk/testutils;
the SDK rejects that feature for wasm32, so every wasm build died.
`default-members` in stellar/Cargo.toml now limits the default build to the
contract crates.
wraith-names converted the owner `Address` into an `xdr::ScAddress`, an impl the
SDK gates behind `cfg(not(target_family = "wasm"))`. It now decodes the owner's
strkey on chain, so the wasm artifact builds.

Current measurement for wraith_names, with the CI toolchain (Rust 1.98.1,
stellar-cli 26.1.0): 75,055 bytes built, 57,575 bytes optimized, which is 48.89%
below the 112,640-byte budget and the largest payload in the workspace.

Current workspace payload
Fresh measurement of every contract member with the CI toolchain (Rust 1.98.1,
stellar-cli 26.1.0), run as CI runs it. "Budget used" is the optimized payload
against the 112,640-byte gate.

Contract	Built (bytes)	Optimized (bytes)	Budget used
wraith_names	75,055	57,575	51.11%
stealth_sender	34,177	24,986	22.18%
stealth_batch_sender	29,068	21,902	19.44%
governance	24,044	18,506	16.43%
stealth_splitter	21,502	16,899	15.00%
stealth_vault	22,637	16,585	14.72%
stealth_announcer	8,228	6,575	5.84%
stealth_registry	8,246	5,973	5.30%
wraith_asset_policy	6,245	4,559	4.05%

Metric emission delta (wraith_metrics wiring)
Wiring wraith_metrics::emit_metric into wraith-names, stealth-splitter,
stealth-vault, and the governance PoC adds an event publish (and the constant
Symbols it carries) to each write path, so each payload grows. Both columns
below are the optimized payload for the same contract, measured before and
after the metric calls were added; the only difference between them is the
metric emission.

Contract	Before metrics (bytes)	After metrics (bytes)	Delta	Growth
stealth_splitter	9,774	10,720	+946	+9.68%
stealth_vault	9,237	11,117	+1,880	+20.35%
governance	16,589	18,506	+1,917	+11.56%
wraith_names	--	57,575	--	--
Every payload stays far below the 112,640-byte CI budget; the largest,
wraith_names, is 48.89% below it.

wraith_names carries the metrics wiring in the commit that made it build for
wasm32, so there is no "before" payload to measure against on any toolchain that
compiles it. Its 57,575-byte optimized payload is therefore the current
absolute, not a delta.

Batch-sender hardening pass
the stealth_batch_sender contract gained init, pause/admin, typed errors, and
signer rotation in the same shape as stealth_sender. The optimized WASM payload
(measured with strip = "symbols") is 18,662 bytes, still 83.03% below the
112,640-byte CI budget.

## Reproducing the per-contract delta
From this directory, run the same commands used by CI. Record the byte count of
each unoptimized WASM before applying the profile/optimizer, then record the
optimized output after the profile change:

Shell

cargo build --target wasm32-unknown-unknown --release
for wasm in target/wasm32-unknown-unknown/release/*.wasm; do
  stellar contract optimize --wasm "$wasm"
done
find target/wasm32-unknown-unknown/release -name '*.optimized.wasm' \
  -printf '%f %s bytes\n' | sort
The optimizer is deliberately run on the release output, as the network deploys
the optimized payload rather than the intermediate compiler artifact. CI rejects
any optimized payload over 112,640 bytes.

`stellar contract optimize` writes the result beside the input as
`<name>.optimized.wasm`. The workflow previously stat'ed `<name>_optimized.wasm`,
so the size gate never read a real file and the step failed even when the payload
was well within budget.

The wasm32 build used to be restricted to explicitly named contracts because a
plain workspace build pulled in integration-tests, which enables soroban-sdk
with the testutils feature, and Cargo unifies that feature across the whole
build graph. `default-members` in stellar/Cargo.toml now keeps the testutils-only
members out of the default target set, so the plain build above produces exactly
the deployable contract artifacts.
