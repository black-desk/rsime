.PHONY: all
all: release

.PHONY: vcpkg
vcpkg:
	vcpkg install --allow-unsupported
	$(eval VCPKG_DIR := $(shell find vcpkg_installed -mindepth 1 -maxdepth 1 -type d ! -name "vcpkg" | head -n 1))

.PHONY: release
release: vcpkg
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo build --release

.PHONY: debug
debug: vcpkg
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo build

.PHONY: install
install: vcpkg
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo install --path .

.PHONY: clean
clean:
	rm -r vcpkg_installed
	cargo clean

.PHONY: test
test: debug
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo test

.PHONY: clippy
clippy: debug
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo clippy --all-targets
