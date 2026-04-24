# SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
#
# SPDX-License-Identifier: MIT

VCPKG_TRIPLET_DIR := $(shell find vcpkg_installed -mindepth 1 -maxdepth 1 -type d ! -name "vcpkg" 2>/dev/null | head -n 1)

.PHONY: all vcpkg release debug test clippy check install clean

all: release

ifeq ($(VCPKG_TRIPLET_DIR),)

release debug test clippy check install: vcpkg
	$(MAKE) $@
else

export RIME_INCLUDE_DIR := $(CURDIR)/$(VCPKG_TRIPLET_DIR)/include
export RIME_LIB_DIR     := $(CURDIR)/$(VCPKG_TRIPLET_DIR)/lib

release debug:
	cargo build $(if $(filter release,$@),--release)

clippy test check:
	cargo $@

install:
	cargo install --path .

endif

vcpkg:
	vcpkg install --allow-unsupported

clean:
	rm -rf vcpkg_installed
	cargo clean
