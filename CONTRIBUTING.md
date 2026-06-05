<!--
SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# 贡献指南

感谢你对 rsime 的关注！本文档介绍如何参与本项目的开发。

## 前置条件

- [Rust](https://www.rust-lang.org/tools/install) 工具链
- C/C++ 构建工具链（CMake、C 编译器等）
- Git（用于获取子模块和安装方案）
- librime，两种方式二选一：
  - **系统安装**：如 `apt install librime-dev`（Ubuntu）或 `brew install librime`（macOS）
  - **vcpkg 自动编译**：需安装 [vcpkg](https://vcpkg.io/en/getting-started.html) 并设置 `VCPKG_ROOT` 环境变量

## 构建与开发

使用 `--recurse-submodules` 克隆仓库：

```bash
git clone --recurse-submodules https://github.com/black-desk/rsime.git
cd rsime
```

**使用系统 librime：**

```bash
cargo build --features cli
```

**使用 vcpkg 自动编译 librime：**

```bash
export VCPKG_ROOT=/path/to/vcpkg  # 需事先安装 vcpkg
cargo build --features cli,bundled-vcpkg
```

常用命令：

```bash
cargo test --all-features          # 运行测试
cargo clippy --all-features        # lint 检查
make install                       # 安装到 ~/.cargo/bin/（自动检测 VCPKG_ROOT）
```

## 项目结构

```
rsime/src/main.rs        — CLI 入门（#[cfg(feature = "cli")]）
rsime/src/lib.rs         — 库入口，导出 rime 模块
rsime/src/rime.rs        — librime 安全封装（Session/Config/Levers API 等）
rsime/tests/stdio.rs     — stdio 模式的集成测试
rime-sys/                — 本地 rime-sys crate (bindgen + pkg-config)
rime-sys/vcpkg-overlay/  — 自定义 vcpkg port，构建 librime + 插件
rsime.nvim/              — Neovim 插件（lua/rsime/ + plugin/rsime.lua）
lua/ -> rsime.nvim/lua   — 符号链接，兼容 packpath
plugin/ -> rsime.nvim/plugin
.format/                 — editorconfig / prettierrc (根目录 symlink 引用)
scripts/ls-todo.sh       — 列出项目中的 TODO/FIXME 项
```

## 测试

测试通过 `assert_cmd` 以子进程方式运行 `rsime stdio`。`setup_rime_env()` 将
`~/.config/rsime` 复制到临时目录并设为 `RIME_USER_DATA_DIR`，确保测试有可用的
输入方案。

运行测试前需要先完成 debug 构建（`make test` 会自动处理）。

## Commit 约定

- **语言**：commit message、注释、文档、帮助信息等统一使用中文编写
- **前缀**：使用常规前缀，参考 `git log` 中的现有风格：
  `feat:`, `fix:`, `docs:`, `build(vcpkg):`, `feat(nvim):` 等
- **Signed-off-by**：commit 时加 `-s`，生成 `Signed-off-by` trailer
- **Assisted-by**：使用 AI 辅助开发时，添加 `Assisted-by: agent:<工具名>` trailer，
  如 `Assisted-by: agent:claude`

## 许可证

- 代码：GPL-3.0-or-later
- 文档、配置文件、脚本：MIT

本项目遵循 [REUSE 规范](https://reuse.software/spec-3.3/)。所有文件须包含
SPDX `SPDX-FileCopyrightText` 和 `SPDX-License-Identifier` 头。

可使用 [reuse-tool](https://github.com/fsfe/reuse-tool) 检查：

```bash
reuse lint
```
