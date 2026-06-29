# SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
#
# SPDX-License-Identifier: MIT

# 特性选择：默认启用 CLI；若设置了 VCPKG_ROOT 则追加 bundled-vcpkg（自动编译 librime）。
# 可用 `make <target> FEATURES=...` 或导出 FEATURES 环境变量覆盖。
ifndef FEATURES
  ifeq ($(VCPKG_ROOT),)
    FEATURES = cli
  else
    FEATURES = cli,bundled-vcpkg
  endif
endif

CARGO ?= cargo

.PHONY: help all build test clippy fmt fmt-check install clean

help: ## 显示所有目标
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

all: build ## 构建（默认目标）

build: ## 构建 workspace（按 VCPKG_ROOT 自动选择系统/vcpkg 特性）
	$(CARGO) build --features $(FEATURES) --workspace

test: ## 运行测试并生成 lcov.info 覆盖率（cargo-llvm-cov；CI 走 autotools action 时由此产出覆盖率）
	$(CARGO) llvm-cov --features $(FEATURES) --workspace --lcov --output-path lcov.info

clippy: ## 运行 clippy
	$(CARGO) clippy --features $(FEATURES) --workspace

fmt: ## 格式化代码
	$(CARGO) fmt

fmt-check: ## 检查代码格式（不修改）
	$(CARGO) fmt --check

install: ## 安装 rsime CLI 到 ~/.cargo/bin（自动检测 VCPKG_ROOT）
	if [ -n "$$VCPKG_ROOT" ]; then \
		cargo install --path rsime --features cli,bundled-vcpkg; \
	else \
		cargo install --path rsime --features cli; \
	fi

clean: ## 清理构建产物
	$(CARGO) clean
