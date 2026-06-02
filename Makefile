# SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
#
# SPDX-License-Identifier: MIT

VCPKG_TRIPLET_DIR := $(shell find vcpkg_installed -mindepth 1 -maxdepth 1 -type d ! -name "vcpkg" 2>/dev/null | head -n 1)

.PHONY: all vcpkg release debug test cov clippy check install clean

all: release

ifeq ($(VCPKG_TRIPLET_DIR),)

release debug test cov clippy check install: vcpkg
	$(MAKE) $@

else

export PKG_CONFIG_PATH := $(CURDIR)/$(VCPKG_TRIPLET_DIR)/lib/pkgconfig:$(PKG_CONFIG_PATH)

release debug:
	cargo build $(if $(filter release,$@),--release)

clippy test check:
	cargo $@

cov:
	cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

install:
	cargo install --path .

endif

vcpkg:
	vcpkg install --allow-unsupported

clean:
	rm -rf vcpkg_installed
	cargo clean
