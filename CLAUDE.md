<!--
SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# rsime

命令行中文输入工具，基于 RIME，面向无图形输入法的 TUI 环境。

贡献指南见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 架构

CLI 使用 `clap` derive 模式定义子命令：

- `tui` — 交互式 TUI，使用 ratatui + crossterm，通过 `/dev/tty` 读写终端
- `stdio` — 编辑器集成模式，Vim 风格按键输入，JSONL 输出
- `install` — 在线安装 RIME 输入方案（下载 plum 脚本并通过 bash 执行）
- `list-schemas` / `current-schema` / `set-schema` — 方案管理
- `shell-init` — 输出 shell 补全脚本和可选的快捷键绑定

## Neovim 插件

`misc/rsime.nvim/` 构成 Neovim 插件（`lua/rsime/` 和 `plugin/rsime.lua`）。通过 `rsime stdio` 子进程在
Neovim 中实现中文输入，纯 Lua 实现，无外部依赖。用户通过 `require("rsime").setup{}`
配置，提供 `:RsimeEnable` / `:RsimeDisable` / `:RsimeToggle` 命令。

## CI/CD

GitHub Actions（`.github/workflows/`）：
- `ci.yaml` — generic lint、vcpkg bundled-vcpkg 构建+覆盖率、系统 librime 构建
- `cd.yaml` — 占位，未配置实际部署

## RIME 交互

RIME 交互通过本地 `rime-sys` crate（`rime-sys/`）。使用 `bindgen` 从
`librime.h` 生成 FFI 绑定，通过 `pkg-config` 查找 librime（vcpkg 静态库或
系统动态库）。按键码使用 `rime_api::KEY_*` 常量（从 `rime-sys` 的
`RimeKeyCode_XK_*` 重新导出），不要使用硬编码数字。

`rime-sys` 支持 `bundled-vcpkg` feature：启用后 build.rs 自动调用 vcpkg
编译 librime，用户无需手动安装。vcpkg overlay port 在
`rime-sys/vcpkg-overlay/rime/`，配置文件 `vcpkg.json` 和
`vcpkg-configuration.json` 也在 `rime-sys/` 下。

静态链接时的传递依赖通过 vcpkg overlay port 的补丁声明在 `rime.pc` 的
`Libs.private` 中，`build.rs` 仅额外处理平台相关的 C++ 运行时
（Linux: `-lstdc++`，macOS: `-lc++`）。

`init_rime()` 中 shared_data_dir 和 user_data_dir **故意设为同一目录**，
因为本项目通常不依赖系统级 RIME 安装。留空会导致 RIME 回退到当前工作目录。
首次运行时若用户数据目录不存在，会自动调用 `install_cmd` 安装预设方案。

## 注意事项

- `run_tui` 中使用 `libc::dup/dup2` 重定向 stdout 到 `/dev/tty`，以便在 `$()`
  子 shell 中工作时 crossterm 的光标查询能到达终端
- `install_cmd` 通过 HTTP 下载 plum 脚本并 pipe 给 bash，需要网络和 git
- Rust edition 2024，`clap` derive 模式，`clap_complete` 生成 shell 补全
- LLM 运行 git commit 时必须加 `-s`（生成 `Signed-off-by`），并添加
  `Assisted-by: agent:<模块名>` trailer（如 `Assisted-by: agent:claude`）
- 当项目结构、架构或构建方式发生变化时，必须同步更新本文件（CLAUDE.md）
