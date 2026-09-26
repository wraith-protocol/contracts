TARGET := riscv64imac-unknown-none-elf
BUILD_DIR := build/release
BUILD_DIR_DEBUG := build/debug
LOCK_SCRIPT := wraith-stealth-lock
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
		cargo build --target $(TARGET) --release -p $(LOCK_SCRIPT) -p wraith-names-type
	cp target/$(TARGET)/release/$(LOCK_SCRIPT) $(BUILD_DIR)/
	cp target/$(TARGET)/release/wraith-names-type $(BUILD_DIR)/

build-debug:
	@mkdir -p $(BUILD_DIR_DEBUG)
	CC_riscv64imac_unknown_none_elf=$(RISCV_CC) \
		cargo build --target $(TARGET) -p $(LOCK_SCRIPT) -p wraith-names-type
	cp target/$(TARGET)/debug/$(LOCK_SCRIPT) $(BUILD_DIR_DEBUG)/
	cp target/$(TARGET)/debug/wraith-names-type $(BUILD_DIR_DEBUG)/

# Transaction-level tests for the lock script (issue #195).
#
# These execute the *compiled* RISC-V artifact inside ckb-testtool's in-process
# simulator, so the lock script has to be built for the RISC-V target first. The
# binary is passed explicitly so the tests never depend on a stale artifact.
test: build-release
	WRAITH_LOCK_BIN=$(CURDIR)/$(BUILD_DIR)/$(LOCK_SCRIPT) \
		cargo test -p wraith-stealth-lock-integration

# Same as `test`, but always leaves the fixture directory in place so CI can
# upload it as an artifact for debugger replay.
test-ci: build-release
	WRAITH_LOCK_BIN=$(CURDIR)/$(BUILD_DIR)/$(LOCK_SCRIPT) \
	WRAITH_CKB_FIXTURE_DIR=$(CURDIR)/target/ckb-fixtures \
		cargo test -p wraith-stealth-lock-integration

clean:
	cargo clean
	rm -rf build

.PHONY: all build build-release build-debug test test-ci clean
