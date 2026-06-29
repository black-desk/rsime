<!--
SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: GPL-3.0-or-later
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
│       ├── config.rs      # config 子命令集成测试
│       ├── shell_init.rs  # shell 快捷键绑定集成测试
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
├── Makefile               # build/test/clippy/cov/install 等目标（按 VCPKG_ROOT 自动选特性）
└── .github/workflows/     # CI/CD
```

## 架构

### rsime crate（库 + CLI）

`rsime` crate 分为库和 CLI 两部分：

- **库**（始终编译）：`lib.rs` 导出 `rime` 模块，提供 librime 的安全 Rust 封装
- **CLI**（需启用 `cli` feature）：`main.rs` 使用 `clap` derive 模式定义子命令

CLI 子命令：

- `tui` — 交互式 TUI，使用 ratatui + crossterm，通过 `/dev/tty` 读写终端；RIME 未消费的按键（见注意事项）自行直通
- `stdio` — 编辑器集成模式，Vim 风格按键输入，JSONL 输出；响应含 `consumed:bool`，RIME 未消费时为 `false`（见注意事项），由调用者直通字符
- `install` — 在线安装 RIME 输入方案（下载 plum 脚本并通过 bash 执行）
- `list-schemas` / `current-schema` / `set-schema` — 方案管理
- `shell-init` — 输出 shell 补全脚本（动态：`CompleteEnv` registration，覆盖所有子命令与运行时候选）和可选的快捷键绑定
- `config` — 读写持久化的 RIME 配置（`get/set <key> <value>`，写入 `user.yaml` 的 `var/option/*` 开关，后续 `tui` 调用自动沿用；`<key>` 按前缀提示常用开关，提示但不限制输入）

Feature flags：

- `cli`（可选）— 启用 CLI 二进制，引入 clap、crossterm、ratatui、ureq 依赖
- `bundled-vcpkg`（可选）— 转发到 `rime-sys/bundled-vcpkg`，自动编译 librime。
  用户需预装 vcpkg（设置 `VCPKG_ROOT` 或 `vcpkg` 在 `PATH` 中）

### TUI 模式与 shell 集成

`tui` 子命令在 **prompt 下方独立画 2 行**（组合行 + 候选行），完全不触碰 shell 的
prompt 行。rsime 不读取任何命令行上下文——它在自己的输入区里独立合成中文，退出时把
提交结果打印到 stdout，由各 shell 用原生变量插到光标处。支持 bash、zsh、fish。

**工作原理：** ratatui 的 `Viewport::Inline(N)` 把视口第 0 行锚定在创建终端时的光标
所在行（`compute_inline_size`）。`run_tui` 在创建 `Terminal` 前先发一个换行 `\n`，把
光标从 prompt 行下移到其下一行，于是视口锚定在 prompt 下方，prompt 行落在视口上方、
由 ratatui 差分渲染保护，从不被覆盖。退出时 `terminal.clear()` 只清 prompt 下方的两行
视口，再 `MoveTo(0, viewport_y - 1)` 把光标移回 prompt 行。

**环境变量：** 无。shell 绑定不再向 rsime 传任何上下文（旧版的 `RSIME_PROMPT` /
`RSIME_READLINE_LINE` / `RSIME_READLINE_POINT` / `RSIME_RESTORE_PROMPT` 已全部移除，
`ansi-to-tui` 依赖、prompt ANSI 解析、fish `ESC(B` 剥离、bash `${PS1@P}` 版本门控等
也随之删除）。

**shell 绑定（`print_shell_bind`，由 `shell-init --bind` 生成）：**

- bash：`bind -x` 绑定 → `output=$(rsime tui)` → 把 `$output` 插到 `$READLINE_POINT`
  处并前移光标；`bind -x` 返回后 readline 自动重绘
- zsh：`zle -N` widget → `output=$(rsime tui)` → `LBUFFER+="$output"`；`zle reset-prompt`
- fish：`rsime tui | read -l output; and commandline --insert "$output"; commandline -f repaint`
  （`commandline -f repaint` 强制全量重绘——rsime 在 prompt 下方画屏扰乱了 fish 基于内部屏幕
  模型的差分重绘，必须 force repaint；与 zsh 的 `zle reset-prompt`、bash 的
  `rl_forced_update_display` 对应，fzf 的 fish 绑定同样以此收尾）

**已知限制（bash）：prompt 行在 rsime 运行期间暂时变空。** bash 的 `bind -x` 在执行
绑定命令前会**无条件**调用 readline 的 `rl_clear_visible_line()` 把当前命令行整行擦掉
（见 bash 源码 `bashline.c` 的 `bash_execute_unix_command`，仅以终端具备 `ce` 能力为前提——
所有真实终端都满足，故必然擦除）。因此 bash 下触发 rsime 的瞬间 prompt 行变空，rsime 在其
下方独立画屏；rsime 退出后 bash 自行 `rl_forced_update_display` 把 prompt + 提交结果重画
回来，**功能正常**。zsh（zle widget）、fish（`commandline`）不擦行，prompt 全程可见。
这是 bash 侧的固定行为，**rsime 无法阻止**（擦除发生在 rsime 启动之前）。fzf 在 bash ≥ 4
的 Ctrl-T / Ctrl-R 上也是同样表现（它只是把自己的 UI 直接画在被擦掉的位置）。

**已知限制（屏幕底部）：** prompt 位于屏幕**最后 1–2 行**时，锚点下移的换行 + ratatui
自适应可能把 prompt 行卷入视口附近，导致 prompt 行被瞬时触碰（fish 差分重绘可能出现短暂
错位）。prompt 不在屏幕底部时完全干净。可选加固（后续）：启动时查终端尺寸与光标行，空间
不足时先发足够换行预留再锚定。绑定与集成测试见 `print_shell_bind` 与 `tests/shell_init.rs`。

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
  特殊按键映射、括号/引号键接管（见下）、自动命令（InsertLeave/WinLeave/BufLeave 清理）
- `ui.lua` — 浮动窗口渲染：preedit 显示、候选词列表（高亮当前选中）
- `plugin/rsime.lua` — 定义 `:RsimeEnable` / `:RsimeDisable` / `:RsimeToggle` 命令

配置项：

- `bin` — rsime 可执行文件路径（默认 `"rsime"`）
- `rime_user_data_dir` — RIME 用户数据目录（默认 nil，使用 `~/.config/rsime`）
- `special_keys` — Vim 按键到 RIME 按键的映射表

**括号/引号键接管（避免与 nvim-autopairs 等配对插件冲突）：** `handle_char` 在
`InsertCharPre` 里吞掉所有可打印字符（`vim.v.char = ""`）交给 RIME 异步全角重插。
而 nvim-autopairs 对 `(` 等用的是 `<expr>` keymap，按下时返回 `()<left>`——两者叠加时，
autopairs 产生的 `)` 也会被吞掉转交 RIME，再在 autopairs 已用 `<left>` 挪动过的光标处
异步重插，导致括号跑位、双括号挤在一起（如 `一段|文字` 按 `(` 错变成 `一（）|段文字`）。
为此 rsime 用自己的 buffer-local `<expr>` keymap 接管 `( ) [ ] { } " '` 这些键
（RIME 会全角化成 `（ ）【 】「 」“ ‘`，且正是 autopairs 绑定的键）：始终 `send_key`
给 RIME 并 `return ""`，使这些键不再触发 `InsertCharPre`，autopairs 无从介入。`activate`
前 `save_keymaps` 保存这些键原有的 buffer-local 映射，`deactivate` 时 `restore_keymaps`
还原（无原映射则删除），以免禁用 rsime 后破坏 autopairs。前提：rsime 的接管 keymap 需在
autopairs attach 之后注册才能压过它（正常用法——autopairs 随开屏 attach、用户手动
`:RsimeEnable`——天然满足）。

**未消费按键的直通（`consumed:false` 处理）：** RIME 未消费按键时（如 `ascii_punct` 开启下的标点，见注意事项），`InsertCharPre`/punct_keys 吞掉的字符需要重新插入。因 stdio 是异步的（`chansend` + `on_stdout`），`init.lua` 维护 FIFO `pending` 队列：`send_key(key, ch)` 记录可直通字符（`handle_char`/`punct_keys` 传字符，`handle_special`/`<Esc>` 等传 `nil` 占位防错位），`on_response` 每次取出一项，`consumed:false` 且该项为字符时重插（commit 在前、字符在后）。向后兼容：`resp.consumed ~= false`（nil/true）视为已消费，旧 rsime 无此字段不受影响。

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
- TUI 诊断日志：设置 `RSIME_LOG=<文件路径>` 环境变量（或 `--log`）会输出每帧绘制、
  按键事件、commit、最终 stdout 输出等信息，排查 shell 集成问题时无需改动绑定即可开启
- `install_cmd` 通过 HTTP 下载 plum 脚本并 pipe 给 bash，需要网络和 git
- Rust edition 2024，workspace resolver 3，`clap` derive 模式；`shell-init` 输出 `clap_complete` 的 `unstable-dynamic`（`CompleteEnv`）动态补全——`shell_init_cmd` 通过 self-spawn（`COMPLETE=<shell>` 调自身）取 registration 脚本，使所有子命令（含 `config` 的 `ArgValueCompleter` 运行时候选）共用动态入口，不再用静态 `generate`（两套都用 `complete -F`，无法共存）
- **RIME 未消费按键（DirectCommit）的处理**：`express_editor` 的 `DirectCommit`（librime `editor.cc`）对可打印字符只 `ctx->Commit()` 当前组合、返回 `kRejected` 丢弃字符本身，交给前端直通。典型场景是 `ascii_punct` 开启：punctuator 放行标点（`kNoop`）→ DirectCommit 丢弃 → 前端须直通，否则标点丢失。`tui` 作为最终前端，`process_key` 返回 not consumed 时自行直通可打印字符（0x21-0x7e）；`stdio` 作为代理只经 `consumed:bool` 字段如实报告（`commit` 仅含 RIME 真实 commit），由调用者决定直通（rsime.nvim 的处理见「未消费按键的直通」）
- `cli` feature 默认未启用，构建 CLI 需 `cargo build --features cli`
- LLM 运行 git commit 时必须加 `-s`（生成 `Signed-off-by`），并添加
  `Assisted-by: <agent>:<模型名称>` trailer（如 `Assisted-by: claude:glm5.2`）
- 当项目结构、架构或构建方式发生变化时，必须同步更新本文件（CLAUDE.md）
