.PHONY: all
all: vcpkg-build

.PHONY: vcpkg-build
vcpkg-build:
	vcpkg install
	$(eval VCPKG_DIR := $(shell find vcpkg_installed -mindepth 1 -maxdepth 1 -type d ! -name "vcpkg" | head -n 1))
	env \
		RIME_INCLUDE_DIR="$(CURDIR)/$(VCPKG_DIR)/include" \
		RIME_LIB_DIR="$(CURDIR)/$(VCPKG_DIR)/lib" \
		cargo build

