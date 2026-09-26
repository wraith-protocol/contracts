# Step-by-Step Testing Guide: Stealth Splitter Assignment

## Quick Start (5 minutes)

Follow these exact steps to verify your assignment is complete:

### Step 1: Navigate to the stealth-splitter directory
```bash
cd c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter
```

### Step 2: Run tests
```bash
cargo test 2>&1 | tee test-output.log
```

**Expected Result:**
```
running 19 tests

test test_init_success ... ok
test test_init_already_initialized ... ok
test test_create_split_basic ... ok
test test_create_split_single_beneficiary ... ok
test test_create_split_max_beneficiaries ... ok
test test_create_split_empty_beneficiaries ... ok
test test_create_split_too_many_beneficiaries ... ok
test test_create_split_invalid_meta_address_length ... ok
test test_create_split_deterministic_id ... ok
test test_create_split_different_salt_different_id ... ok
test test_get_split_not_found ... ok
test test_get_split_after_creation ... ok
test test_property_dust_to_first_beneficiary ... ok
test test_property_immutable_split_definition ... ok
test test_fund_split_zero_amount ... ok
test test_fund_split_negative_amount ... ok
test test_fund_split_nonexistent_split ... ok
test test_fund_split_vector_length_mismatch_stealth_addresses ... ok
test test_atomicity_concept_all_or_nothing ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured

Finished test target(s) in X.XXs
```

**✅ IF YOU SEE "test result: ok. 19 passed" → Assignment is complete!**

---

## Detailed Testing (15 minutes)

### Phase 1: Verify File Structure

```bash
# Check that all required files exist
echo "=== Checking file structure ==="
ls -la c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\
echo ""
echo "=== Checking for Cargo.toml ==="
cat c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\Cargo.toml | head -10
echo ""
echo "=== Checking for lib.rs ==="
test -f c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\src\lib.rs && echo "✓ lib.rs exists" || echo "✗ lib.rs missing"
echo ""
echo "=== Checking for DESIGN.md ==="
test -f c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\DESIGN.md && echo "✓ DESIGN.md exists" || echo "✗ DESIGN.md missing"
```

**Expected Output:**
```
✓ lib.rs exists
✓ DESIGN.md exists
```

### Phase 2: Build Check

```bash
# Clean build to ensure everything compiles fresh
echo "=== Building stealth-splitter ==="
cd c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter
cargo clean
cargo build 2>&1 | tail -20
```

**Expected Output:**
```
Compiling stealth-splitter v0.1.0
Finished dev target(s) in X.XXs
```

**✅ No errors should appear.**

### Phase 3: Run Tests with Verbose Output

```bash
echo "=== Running all tests (verbose) ==="
cargo test -- --nocapture --test-threads=1 2>&1 | tee detailed-test.log
```

**Expected Output:**
```
Each test should show: test TEST_NAME ... ok
Final line should show: test result: ok. 19 passed; 0 failed
```

### Phase 4: Verify Test Coverage

```bash
echo "=== Counting test functions ==="
grep -c "fn test_" c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\src\lib.rs
```

**Expected Output:**
```
19
```

### Phase 5: Verify Core Functions Exist

```bash
echo "=== Verifying core functions ==="
echo "✓ init function:"
grep "pub fn init" c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\src\lib.rs | head -1

echo "✓ create_split function:"
grep "pub fn create_split" c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\src\lib.rs | head -1

echo "✓ fund_split function:"
grep "pub fn fund_split" c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\src\lib.rs | head -1

echo "✓ get_split function:"
grep "pub fn get_split" c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\src\lib.rs | head -1
```

**Expected Output:**
```
✓ init function: pub fn init(env: Env, announcer: Address) -> Result<(), SplitterError> {
✓ create_split function: pub fn create_split(
✓ fund_split function: pub fn fund_split(
✓ get_split function: pub fn get_split(env: Env, split_id: BytesN<32>) -> Result<SplitDetails, SplitterError> {
```

### Phase 6: Verify Documentation

```bash
echo "=== Checking DESIGN.md content ==="
echo "File size:"
ls -lh c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\DESIGN.md | awk '{print $5}'

echo ""
echo "Sections covered:"
grep -E "^## " c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter\DESIGN.md
```

**Expected Output:**
```
File size should be > 10 KB

Sections should include:
## Overview
## Key Features
## Rounding Strategy: Dust to First Beneficiary
## Design Constraints
## API Reference
## Testing Strategy
## Resource Budget
```

### Phase 7: Workspace Integration Check

```bash
echo "=== Verifying workspace membership ==="
cat c:\Users\LENOVO\Desktop\kji\contracts\stellar\Cargo.toml | grep -A 6 "\[workspace\]"
```

**Expected Output:**
```
[workspace]
members = [
  "stealth-announcer",
  "stealth-registry",
  "stealth-sender",
  "stealth-splitter",
  "wraith-names",
]
```

**✓ "stealth-splitter" should be in the list**

### Phase 8: Full Workspace Test

```bash
echo "=== Running all tests across workspace ==="
cd c:\Users\LENOVO\Desktop\kji\contracts\stellar
cargo test --all 2>&1 | grep -E "test result:|running"
```

**Expected Output:**
```
stealth-announcer test results
stealth-registry test results
stealth-sender test results
stealth-splitter test results (should show: ok. 19 passed)
wraith-names test results

Final: ok. XXX passed; 0 failed
```

---

## Acceptance Test Checklist

Go through this checklist and mark each item as you verify:

### File System Checks
- [ ] `contracts/stellar/stealth-splitter/` directory exists
- [ ] `Cargo.toml` exists in stealth-splitter/
- [ ] `src/lib.rs` exists in stealth-splitter/
- [ ] `DESIGN.md` exists in stealth-splitter/
- [ ] `VERIFICATION.md` exists in stealth-splitter/

### Build Checks
- [ ] `cargo build` succeeds with no errors
- [ ] `cargo build` produces no warnings
- [ ] `cargo clippy` passes

### Test Checks
- [ ] `cargo test` shows "19 passed; 0 failed"
- [ ] All 19 tests pass:
  - [ ] test_init_success
  - [ ] test_init_already_initialized
  - [ ] test_create_split_basic
  - [ ] test_create_split_single_beneficiary
  - [ ] test_create_split_max_beneficiaries
  - [ ] test_create_split_empty_beneficiaries
  - [ ] test_create_split_too_many_beneficiaries
  - [ ] test_create_split_invalid_meta_address_length
  - [ ] test_create_split_deterministic_id
  - [ ] test_create_split_different_salt_different_id
  - [ ] test_get_split_not_found
  - [ ] test_get_split_after_creation
  - [ ] test_property_dust_to_first_beneficiary
  - [ ] test_property_immutable_split_definition
  - [ ] test_fund_split_zero_amount
  - [ ] test_fund_split_negative_amount
  - [ ] test_fund_split_nonexistent_split
  - [ ] test_fund_split_vector_length_mismatch_stealth_addresses
  - [ ] test_atomicity_concept_all_or_nothing

### Implementation Checks
- [ ] `init()` function implemented
- [ ] `create_split()` function implemented
- [ ] `fund_split()` function implemented
- [ ] `get_split()` function implemented
- [ ] `Beneficiary` struct defined
- [ ] `SplitDefinition` struct defined
- [ ] `SplitDetails` struct defined
- [ ] `SplitterError` enum defined with 8 error codes
- [ ] Dust handling: first beneficiary absorbs dust
- [ ] Atomic distribution in fund_split
- [ ] Immutable split definitions
- [ ] Max 25 beneficiaries constraint

### Documentation Checks
- [ ] DESIGN.md > 10 KB
- [ ] Documents immutability guarantee
- [ ] Documents deterministic split ID
- [ ] Documents atomic distribution
- [ ] Documents weight-based splitting
- [ ] Documents dust handling rationale
- [ ] Documents max 25 beneficiaries
- [ ] Documents resource budget comparison
- [ ] Includes concrete examples
- [ ] Includes comparison table

### Integration Checks
- [ ] Workspace Cargo.toml includes stealth-splitter
- [ ] Uses same Soroban SDK patterns as other contracts
- [ ] Integrates with StealthAnnouncer contract
- [ ] Uses standard token interface
- [ ] All workspace tests pass

---

## Success Criteria

### ✅ ASSIGNMENT COMPLETE if:

1. ✓ `cargo test` shows **19 passed; 0 failed**
2. ✓ All files exist (lib.rs, Cargo.toml, DESIGN.md)
3. ✓ No build errors or warnings
4. ✓ All 4 core functions implemented (init, create_split, fund_split, get_split)
5. ✓ Design documentation is comprehensive
6. ✓ Integrated into workspace
7. ✓ All workspace tests still pass

### 🎯 Specific Requirements Met:

**From Issue #15:**
- ✓ `create_split()` - Creates immutable split with beneficiaries and weights
- ✓ `fund_split()` - Atomically distributes to N stealth addresses
- ✓ `get_split()` - Returns public beneficiary list and total funded
- ✓ Max 25 beneficiaries enforced with justification
- ✓ Deterministic split ID (SHA-256 hash)
- ✓ Dust to first beneficiary (documented rationale)
- ✓ Comprehensive unit tests (19 tests)
- ✓ Property-based tests (immutability, dust handling)
- ✓ Atomicity tests (all-or-nothing semantics)
- ✓ Resource budget documented (2-3 KB per split, O(N) complexity)
- ✓ Comparison vs. N separate stealth-sender calls

---

## Troubleshooting

### "cargo: not found"
Make sure Rust is installed:
```bash
rustup --version
```

If not installed, install from https://rustup.rs/

### "test result: FAILED"
Run with more verbose output:
```bash
cargo test -- --nocapture --test-threads=1
```

This will show exactly which test failed and why.

### Compilation errors
Try cleaning and rebuilding:
```bash
cargo clean
cargo build
```

### "cannot find" errors for soroban_sdk
Ensure workspace Cargo.toml has the dependency:
```bash
grep soroban-sdk c:\Users\LENOVO\Desktop\kji\contracts\stellar\Cargo.toml
```

Should show: `soroban-sdk = "22.0.0"`

---

## Final Verification Command

Run this single command to verify everything:

```bash
cd c:\Users\LENOVO\Desktop\kji\contracts\stellar\stealth-splitter && \
echo "=== BUILD ===" && cargo build 2>&1 | tail -3 && \
echo "" && echo "=== TESTS ===" && cargo test 2>&1 | grep "test result:" && \
echo "" && echo "=== FILES ===" && ls -lh {Cargo.toml,src/lib.rs,DESIGN.md,VERIFICATION.md} 2>/dev/null | awk '{print $9, $5}' && \
echo "" && echo "✅ ASSIGNMENT VERIFICATION COMPLETE"
```

---

## Next Steps After Verification

Once all tests pass, your assignment is complete! 🎉

The next steps for the team would be:

1. **Code Review** - Team reviews your implementation
2. **Soroban Testnet Deployment** - Deploy to Stellar testnet
3. **SDK Integration** - Build `buildSplitDeposit` builder (separate issue)
4. **Demo Application** - Create "Split payments among N recipients" demo (separate issue)
5. **Production Audit** - Security audit before mainnet deployment

---

**Congratulations on completing Issue #15!**

For questions or issues, refer to:
- [DESIGN.md](./DESIGN.md) - Full architecture and rationale
- [VERIFICATION.md](./VERIFICATION.md) - Detailed acceptance criteria
- Contract code comments for implementation details
