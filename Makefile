.PHONY: all
all: release

.PHONY: release
release:
	vcpkg install --allow-unsupported
	$(eval VCPKG_DIR := $(shell find vcpkg_installed -mindepth 1 -maxdepth 1 -type d ! -name "vcpkg" | head -n 1))
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo build --release

.PHONY: debug
debug:
	vcpkg install --allow-unsupported
	$(eval VCPKG_DIR := $(shell find vcpkg_installed -mindepth 1 -maxdepth 1 -type d ! -name "vcpkg" | head -n 1))
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/debug/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/debug/lib" \
		cargo build

.PHONY: install
install: release
	cargo install --path .

