TARGET := riscv64imac-unknown-none-elf
BUILD_DIR := build/release
BUILD_DIR_DEBUG := build/debug
LOCK_SCRIPT := wraith-stealth-lock
NAMES_TYPE_SCRIPT := wraith-names-type
FUZZ_TARGETS := lock_witness_and_args names_type_script ckb_auth_inputs cell_deps_and_announcement ckb_tx_layout
FUZZ_SAMPLE_RUNS ?= 512
FUZZ_SCHEDULED_SECONDS ?= 180
# ckb-std's build script probes for `riscv64-unknown-elf-gcc` (the name shipped by
# Debian/Ubuntu's gcc-riscv64-unknown-elf). Allow overriding for toolchains that
# install the same compiler under a different name, e.g. Homebrew's riscv64-elf-gcc.
RISCV_CC ?= riscv64-unknown-elf-gcc

all: build

build: build-release

build-release:
	@mkdir -p $(BUILD_DIR)
	CC_riscv64imac_unknown_none_elf=$(RISCV_CC) \
		RUSTFLAGS="-C link-arg=-s" \
		cargo build --target $(TARGET) --release -p $(LOCK_SCRIPT) -p $(NAMES_TYPE_SCRIPT)
	cp target/$(TARGET)/release/$(LOCK_SCRIPT) $(BUILD_DIR)/
	cp target/$(TARGET)/release/$(NAMES_TYPE_SCRIPT) $(BUILD_DIR)/

build-debug:
	@mkdir -p $(BUILD_DIR_DEBUG)
	CC_riscv64imac_unknown_none_elf=$(RISCV_CC) \
		cargo build --target $(TARGET) -p $(LOCK_SCRIPT) -p $(NAMES_TYPE_SCRIPT)
	cp target/$(TARGET)/debug/$(LOCK_SCRIPT) $(BUILD_DIR_DEBUG)/
	cp target/$(TARGET)/debug/$(NAMES_TYPE_SCRIPT) $(BUILD_DIR_DEBUG)/

# Transaction-level VM tests and deterministic fuzz corpus replay (issues #195, #281).
#
# Executes both compiled RISC-V artifacts inside ckb-testtool's in-process
# simulator and replays the retained CKB fuzz corpus + bounded deterministic sample.
test: build-release
	WRAITH_LOCK_BIN=$(CURDIR)/$(BUILD_DIR)/$(LOCK_SCRIPT) \
	WRAITH_NAMES_TYPE_BIN=$(CURDIR)/$(BUILD_DIR)/$(NAMES_TYPE_SCRIPT) \
		cargo test -p wraith-stealth-lock-integration
	cd fuzz && WRAITH_CKB_FUZZ_CASES=4096 cargo test --release

# Same as `test`, plus formatting checks and persistent CKB-debugger fixtures for CI.
test-ci: build-release
	WRAITH_LOCK_BIN=$(CURDIR)/$(BUILD_DIR)/$(LOCK_SCRIPT) \
	WRAITH_NAMES_TYPE_BIN=$(CURDIR)/$(BUILD_DIR)/$(NAMES_TYPE_SCRIPT) \
	WRAITH_CKB_FIXTURE_DIR=$(CURDIR)/target/ckb-fixtures \
	WRAITH_CKB_VM_FUZZ_ITERS=256 \
		cargo test -p wraith-stealth-lock-integration
	cd fuzz && WRAITH_CKB_FUZZ_CASES=4096 cargo test --release

# Bounded deterministic libFuzzer sample across all 5 CKB targets (requires cargo-fuzz + nightly).
fuzz-sample:
	@set -e; cd fuzz; \
	for target in $(FUZZ_TARGETS); do \
		cargo fuzz run "$$target" "corpus/$$target" -- -seed=281 -runs=$(FUZZ_SAMPLE_RUNS); \
	done

# Larger scheduled libFuzzer corpus sweep across all 5 CKB targets.
fuzz-scheduled:
	@set -e; cd fuzz; \
	for target in $(FUZZ_TARGETS); do \
		cargo fuzz run "$$target" "corpus/$$target" -- -max_total_time=$(FUZZ_SCHEDULED_SECONDS); \
	done

clean:
	cargo clean
	rm -rf build

.PHONY: all build build-release build-debug test test-ci fuzz-sample fuzz-scheduled clean
