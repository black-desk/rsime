<!--
SPDX-FileCopyrightText: 2025 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# rsime

一个基于 [RIME](https://rime.im/) 的命令行中文输入工具，适用于没有图形输入法的 TUI 环境。

## 功能

- **交互式 TUI 模式** — 在终端中直接输入拼音并选择候选词。
- **Stdio 模式** — 适用于编辑器集成（Vim 风格按键输入，JSONL 输出）。
- **Neovim 插件** — 在 Neovim 中直接输入中文，无需系统级输入法框架。
- 在线安装输入方案（通过 plum，无需本地安装）。
- 列出 / 切换输入方案。
- Shell 补全与快捷键绑定。

## 前置条件

- [Rust](https://www.rust-lang.org/tools/install) 工具链
- [vcpkg](https://vcpkg.io/)（需设置 `VCPKG_ROOT`）
- C/C++ 构建工具链（CMake、C 编译器等）
- Git（用于获取子模块和安装方案）

## 构建

使用 `--recurse-submodules` 克隆仓库，然后用 `make` 构建：

```bash
git clone --recurse-submodules https://github.com/black-desk/rsime.git
cd rsime
make
```

构建过程：

1. 通过 vcpkg 安装 `rime` 库。
2. 使用 `cargo build` 编译 Rust 项目，并指向 vcpkg 构建的头文件和库文件。

## 使用

**交互式 TUI 输入：**

```bash
rsime tui
```

**Stdio 模式（编辑器集成）：**

```bash
rsime stdio
```

接受 Vim 风格的按键表示法（如 `<CR>`、`<Space>`、`<Esc>`、`<BS>`、`<Up>`、`<Down>` 等），每行一个按键，输出 JSONL 格式的响应。

**安装输入方案（通过 plum，无需本地安装）：**

```bash
rsime install               # 安装预设方案
rsime install double-pinyin  # 安装指定方案
```

**列出可用方案：**

```bash
rsime list-schemas
```

**查看当前方案：**

```bash
rsime current-schema
```

**切换输入方案：**

```bash
rsime set-schema double_pinyin_flypy
```

**Shell 初始化（补全 + 可选快捷键绑定）：**

```bash
rsime shell-init bash          # 仅补全
rsime shell-init bash --bind   # 补全 + Alt-I 绑定 TUI
rsime shell-init zsh --bind    # zsh 版本
rsime shell-init fish --bind   # fish 版本
```

**写入调试日志：**

```bash
rsime -l /tmp/rsime.log tui
```

### 环境变量

| 变量 | 说明 | 默认值 |
|---|---|---|
| `RIME_USER_DATA_DIR` | RIME 用户数据目录路径（同时用作共享数据目录） | `~/.config/rsime` |

## Neovim 插件

rsime 自带 Neovim 插件，通过 `rsime stdio` 子进程在 Neovim 中实现中文输入。纯 Lua 实现，无外部依赖。

### 安装

将本仓库加入 Neovim 的 `runtimepath`，或使用插件管理器：

**lazy.nvim：**

```lua
{
  "black-desk/rsime",
  build = "make",
  opts = {},
}
```

**手动：**

```bash
# 构建完成后，在 init.lua 中添加：
vim.opt.rtp:prepend("/path/to/rsime")
require("rsime").setup{}
```

### 配置

```lua
require("rsime").setup{
  -- rsime 二进制路径（默认从 PATH 查找）
  bin = "rsime",

  -- RIME 用户数据目录（默认使用 rsime 自身默认值 ~/.config/rsime）
  rime_user_data_dir = nil,
}
```

### 命令

| 命令 | 说明 |
|---|---|
| `:RsimeEnable` | 在当前 buffer 激活中文输入 |
| `:RsimeDisable` | 停用当前 buffer 的中文输入 |
| `:RsimeToggle` | 切换激活/停用状态 |

### 快捷键配置示例

将 `<Alt-i>` 映射为切换 rsime，在普通模式下同时进入插入模式：

```lua
vim.keymap.set({ "n", "i" }, "<M-i>", function()
  require("rsime").toggle()
  if vim.fn.mode() == "n" then
    vim.cmd("startinsert")
  end
end, { desc = "Toggle rsime" })
```

### 使用方式

1. 执行 `:RsimeEnable`，进入插入模式
2. 输入拼音，候选窗会以浮动窗口形式显示
3. 用数字键 `1`-`9`、`0` 选择候选词
4. `<Space>` 或 `<CR>` 确认首选词
5. `<Esc>` 取消当前组合
6. `<Up>`/`<Down>` 在候选列表中移动

非组合状态下，所有按键正常工作，不影响英文输入。

## 许可证

如无特殊说明，该项目的代码以 GNU 通用公共许可协议第三版或任何更新的版本开源，文档、配置文件以及开发维护过程中使用的脚本等以 MIT 许可证开源。

该项目遵守 [REUSE 规范]。

你可以使用 [reuse-tool](https://github.com/fsfe/reuse-tool) 生成这个项目的 SPDX 列表：

```bash
reuse spdx
```

[REUSE 规范]: https://reuse.software/spec-3.3/
