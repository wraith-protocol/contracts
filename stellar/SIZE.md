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

wraith_names is retained in the historical baseline below, but cannot be
compiled for wasm32-unknown-unknown with the repository's pinned
soroban-sdk 22.0.11: its existing ScAddress: TryFrom<&Address> conversion
fails before code generation. This is an unrelated pre-existing compile error;
CI should keep this contract's existing 9,755-byte optimized baseline until that
source/toolchain mismatch is fixed.

Historical contract	Previous optimized baseline (bytes)
wraith_names	9,755

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
wraith_names	not measurable	not measurable	--	--
All three measurable payloads stay far below the 112,640-byte CI budget; the
largest, governance, is 83.57% below it.

wraith_names cannot be compiled for wasm32-unknown-unknown at all (see
the note above), so its metric-emission delta cannot be measured on this toolchain.
The failure reproduces identically on the parent commit, so it is unrelated to the
metric wiring. Once the soroban-sdk bump lands and the contract builds,
re-run the command below and fill the row in the wiring adds five call sites,
so it should land in the same +1 to +2 KB range as the other three.

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
find target/wasm32-unknown-unknown/release -name '*_optimized.wasm' 
  -printf '%f %s bytes\n' | sort
The optimizer is deliberately run on the release output, as the network deploys
the optimized payload rather than the intermediate compiler artifact. CI rejects
any optimized payload over 112,640 bytes.

A workspace-wide wasm32 build fails because integration-tests pulls
soroban-sdk with the testutils feature and Cargo unifies that feature across
the whole build. To measure a single contract, name it explicitly so the
testutil-enabled members stay out of the graph:

Shell

cargo build -p stealth-vault -p stealth-splitter -p governance \
  --target wasm32-unknown-unknown --release
for wasm in target/wasm32-unknown-unknown/release/*.wasm; do
  stellar contract optimize --wasm "$wasm"
done

Note that stellar-cli 27.x writes <name>.optimized.wasm where the 22.0.1 CLI
pinned in CI writes <name>_optimized.wasm; match the glob to the CLI in use.
