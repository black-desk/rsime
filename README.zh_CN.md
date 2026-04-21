<!--
SPDX-FileCopyrightText: 2025 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# rsime

一个基于 [RIME](https://rime.im/) 的命令行中文输入工具，适用于没有图形输入法的 TUI 环境。

[en](README.md) | zh_CN

## 功能

- **单次转换** — 传入拼音，直接输出中文。
- 在线安装输入方案（通过 plum，无需本地安装）。
- 列出 / 切换输入方案。
- 候选词显示与选择。

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

**转换拼音：**

```bash
rsime nihao    # 输出：你好
```

**选择输入方案：**

```bash
rsime -s double_pinyin_flypy nihao
```

**列出可用方案：**

```bash
rsime --list-schemas
```

**查看当前方案：**

```bash
rsime --current-schema
```

**在线安装方案（通过 plum，无需本地安装）：**

```bash
rsime --install               # 安装预设方案
rsime --install double-pinyin  # 安装指定方案
```

**显示候选词以供手动选择：**

```bash
rsime -p nihao
```

**写入调试日志：**

```bash
rsime -l /tmp/rsime.log nihao
```

### 环境变量

| 变量 | 说明 | 默认值 |
|---|---|---|
| `RIME_SHARED_DATA_DIR` | RIME 共享数据目录路径 | `third_party/librime/data/minimal` |
| `RIME_USER_DATA_DIR` | RIME 用户数据目录路径 | `~/.config/rsime` |

## 许可证

如无特殊说明，该项目的代码以 GNU 通用公共许可协议第三版或任何更新的版本开源，文档、配置文件以及开发维护过程中使用的脚本等以 MIT 许可证开源。

该项目遵守 [REUSE 规范]。

你可以使用 [reuse-tool](https://github.com/fsfe/reuse-tool) 生成这个项目的 SPDX 列表：

```bash
reuse spdx
```

[REUSE 规范]: https://reuse.software/spec-3.3/
