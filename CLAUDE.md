<!--
SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# rsime

命令行中文输入工具，基于 RIME，面向无图形输入法的 TUI 环境。

贡献指南见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 项目结构

```
rsime/                     # Cargo workspace（resolver 3）
├── rsime/                 # 主 crate：库 + CLI 二进制
│   ├── src/
│   │   ├── lib.rs         # 库入口，导出 pub mod rime
│   │   ├── rime.rs        # librime 安全封装（Session/Config/Levers API 等）
│   │   └── main.rs        # CLI 入口（#[cfg(feature = "cli")]）
│   └── tests/
│       └── stdio.rs       # stdio 集成测试
├── rime-sys/              # FFI 绑定 crate
│   ├── build.rs           # bindgen + pkg-config / vcpkg
│   ├── src/lib.rs         # 生成的绑定 + rime_struct! 宏
│   ├── wrapper.h          # bindgen 入口头文件
│   ├── include/           # keycodes.h, modifiers.h
│   ├── vcpkg-overlay/rime/ # vcpkg overlay port + 补丁
│   ├── vcpkg.json
│   └── vcpkg-configuration.json
├── rsime.nvim/            # Neovim 插件
│   ├── lua/rsime/
│   │   ├── init.lua       # 主逻辑：job 管理、按键处理、自动命令
│   │   └── ui.lua         # 浮动窗口 UI：候选词显示
│   └── plugin/rsime.lua   # :RsimeEnable/:RsimeDisable/:RsimeToggle 命令
├── lua -> rsime.nvim/lua  # 符号链接，兼容 packpath
├── plugin -> rsime.nvim/plugin
├── scripts/               # 辅助脚本（如 ls-todo.sh）
├── Makefile               # install 目标（自动检测 VCPKG_ROOT）
└── .github/workflows/     # CI/CD
```

## 架构

### rsime crate（库 + CLI）

`rsime` crate 分为库和 CLI 两部分：

- **库**（始终编译）：`lib.rs` 导出 `rime` 模块，提供 librime 的安全 Rust 封装
- **CLI**（需启用 `cli` feature）：`main.rs` 使用 `clap` derive 模式定义子命令

CLI 子命令：

- `tui` — 交互式 TUI，使用 ratatui + crossterm，通过 `/dev/tty` 读写终端
- `stdio` — 编辑器集成模式，Vim 风格按键输入，JSONL 输出
- `install` — 在线安装 RIME 输入方案（下载 plum 脚本并通过 bash 执行）
- `list-schemas` / `current-schema` / `set-schema` — 方案管理
- `shell-init` — 输出 shell 补全脚本和可选的快捷键绑定

Feature flags：

- `cli`（可选）— 启用 CLI 二进制，引入 clap、crossterm、ratatui、ureq 依赖
- `bundled-vcpkg`（可选）— 转发到 `rime-sys/bundled-vcpkg`，自动编译 librime。
  用户需预装 vcpkg（设置 `VCPKG_ROOT` 或 `vcpkg` 在 `PATH` 中）

### rime.rs 安全封装

`rsime/src/rime.rs` 对 `rime-sys` 的原始 FFI 绑定进行安全封装，提供：

- **生命周期管理**：`setup()` / `initialize()` / `finalize()` / `DeployResult`
- **Session**：按键输入、候选词选择/迭代、commit 获取、schema 切换
- **KeyEvent**：键码 + 修饰符，builder 风格 API（`.shift()` / `.ctrl()` / `.alt()`）
- **Context**：Composition（preedit）、Menu（候选词列表）、select labels
- **Config**：schema/default/user 配置读写，list/map 迭代器
- **Levers API**：CustomSettings、SwitcherSettings、UserDict 迭代/备份/恢复/导入导出
- **通知**：全局 notification handler，deploy 结果跟踪
- **按键码**：从 `rime-sys` 的 `RimeKeyCode_XK_*` 重新导出为 `KEY_*` 常量（如 `KEY_RETURN`、`KEY_SPACE`）
- **修饰符**：从 `rime-sys` 的 `RimeModifier_k*` 重新导出为 `MODIFIER_*` 常量

**不要使用硬编码数字作为按键码**，始终使用 `KEY_*` 和 `MODIFIER_*` 常量。

### Neovim 插件

`rsime.nvim/` 构成 Neovim 插件。通过 `rsime stdio` 子进程在 Neovim 中实现中文输入，
纯 Lua 实现，无外部依赖。用户通过 `require("rsime").setup{}` 配置。

根目录的 `lua/` 和 `plugin/` 是指向 `rsime.nvim/` 对应目录的符号链接，用于兼容
`packpath` 加载方式。

- `init.lua` — 管理 rsime 子进程（jobstart/chansend）、InsertCharPre 按键拦截、
  特殊按键映射、自动命令（InsertLeave/WinLeave/BufLeave 清理）
- `ui.lua` — 浮动窗口渲染：preedit 显示、候选词列表（高亮当前选中）
- `plugin/rsime.lua` — 定义 `:RsimeEnable` / `:RsimeDisable` / `:RsimeToggle` 命令

配置项：

- `bin` — rsime 可执行文件路径（默认 `"rsime"`）
- `rime_user_data_dir` — RIME 用户数据目录（默认 nil，使用 `~/.config/rsime`）
- `special_keys` — Vim 按键到 RIME 按键的映射表

## CI/CD

GitHub Actions（`.github/workflows/`）：

- `ci.yaml` — 四矩阵测试（Linux + macOS）×（系统 librime + bundled-vcpkg），
  使用 cargo-llvm-cov 生成覆盖率，generic lint 检查
- `cd.yaml` — 占位，未配置实际部署

## RIME 交互（rime-sys）

RIME 交互通过本地 `rime-sys` crate（`rime-sys/`）。使用 `bindgen` 从
`wrapper.h` 生成 FFI 绑定，通过 `pkg-config` 查找 librime（vcpkg 静态库或
系统动态库）。

`rime-sys` 支持 `bundled-vcpkg` feature：启用后 build.rs 自动调用 vcpkg
编译 librime，用户无需手动安装。vcpkg overlay port 在
`rime-sys/vcpkg-overlay/rime/`，包含 librime 和 librime-lua 的补丁。
配置文件 `vcpkg.json` 和 `vcpkg-configuration.json` 也在 `rime-sys/` 下。

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
- Rust edition 2024，workspace resolver 3，`clap` derive 模式，`clap_complete` 生成 shell 补全
- `cli` feature 默认未启用，构建 CLI 需 `cargo build --features cli`
- LLM 运行 git commit 时必须加 `-s`（生成 `Signed-off-by`），并添加
  `Assisted-by: agent:<模块名>` trailer（如 `Assisted-by: agent:claude`）
- 当项目结构、架构或构建方式发生变化时，必须同步更新本文件（CLAUDE.md）
