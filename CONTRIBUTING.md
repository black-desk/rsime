<!--
SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# 贡献指南

感谢你对 rsime 的关注！本文档介绍如何参与本项目的开发。

## 前置条件

- [Rust](https://www.rust-lang.org/tools/install) 工具链
- [vcpkg](https://vcpkg.io/)（需设置 `VCPKG_ROOT` 环境变量）
- C/C++ 构建工具链（CMake、C 编译器等）
- Git（用于获取子模块和安装方案）

## 构建与开发

使用 `--recurse-submodules` 克隆仓库，然后用 `make` 构建：

```bash
git clone --recurse-submodules https://github.com/black-desk/rsime.git
cd rsime
make          # release 构建（自动调用 vcpkg install）
```

构建通过 `RIME_INCLUDE_DIR` / `RIME_LIB_DIR` 环境变量指向 vcpkg 安装的头文件和库文件，
由 Makefile 自动设置。

其他命令：

```bash
make debug    # debug 构建
make test     # 运行测试（自动先完成 debug 构建）
make clippy   # lint 检查
make clean    # 清理构建产物
make install  # 安装
```

## 项目结构

```
src/main.rs              — 主程序，所有逻辑在单文件中 (~600 行)
tests/stdio.rs           — stdio 模式的集成测试
lua/rsime/               — Neovim 插件 Lua 源码 (init.lua, ui.lua)
plugin/rsime.lua         — Neovim 插件入口 (Vim 自动加载)
third_party/librime-rs/  — fork 的 librime Rust 绑定 (rime-api / librime-sys)
third_party/librime/     — 作为参考的 librime C++ 源码 (git submodule)
third_party/plum/        — 作为参考的 rime-plum 方案安装脚本 (git submodule)
misc/vcpkg-ports/rime/   — 自定义 vcpkg port，构建 librime + 插件
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
