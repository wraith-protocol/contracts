TARGET := riscv64imac-unknown-none-elf
BUILD_DIR := build/release
BUILD_DIR_DEBUG := build/debug

all: build

build: build-release

build-release:
	@mkdir -p $(BUILD_DIR)
	CC_riscv64imac_unknown_none_elf=riscv64-elf-gcc \
		RUSTFLAGS="-C link-arg=-s" \
		cargo build --target $(TARGET) --release -p wraith-stealth-lock -p wraith-names-type
	cp target/$(TARGET)/release/wraith-stealth-lock $(BUILD_DIR)/
	cp target/$(TARGET)/release/wraith-names-type $(BUILD_DIR)/

build-debug:
	@mkdir -p $(BUILD_DIR_DEBUG)
	CC_riscv64imac_unknown_none_elf=riscv64-elf-gcc \
		cargo build --target $(TARGET) -p wraith-stealth-lock -p wraith-names-type
	cp target/$(TARGET)/debug/wraith-stealth-lock $(BUILD_DIR_DEBUG)/
	cp target/$(TARGET)/debug/wraith-names-type $(BUILD_DIR_DEBUG)/

clean:
	cargo clean
	rm -rf build

.PHONY: all build build-release build-debug clean
