<!--
SPDX-FileCopyrightText: 2025 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# rsime

一个基于 [RIME](https://rime.im/) 的命令行中文输入工具，适用于没有图形输入法的 TUI 环境。

[en](README.md) | zh_CN

## 功能

- **单次模式** — 将拼音按键序列作为参数传入，直接在标准输出获得中文结果。
- **交互模式** — 从标准输入逐行读取，持续进行输入转换。
- 候选词显示与选择。
- 通过命令行参数选择输入方案。
- 交互模式下支持 `reload` 和 `exit` 命令。

## 前置条件

- [Rust](https://www.rust-lang.org/tools/install) 工具链
- [vcpkg](https://vcpkg.io/)（需设置 `VCPKG_ROOT`）
- C/C++ 构建工具链（CMake、C 编译器等）
- Git（用于获取子模块）

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

**单次模式** — 将拼音序列转换为中文：

```bash
./target/debug/rsime nihao
```

**交互模式** — 从标准输入读取拼音行：

```bash
./target/debug/rsime
```

输入 `exit` 退出，输入 `reload` 重新初始化 RIME。

**选择输入方案：**

```bash
./target/debug/rsime -s luna_pinyin_simp
```

**显示候选词以供手动选择：**

```bash
./target/debug/rsime -p nihao
```

**写入调试日志：**

```bash
./target/debug/rsime -l /tmp/rsime.log
```

### 环境变量

| 变量 | 说明 | 默认值 |
|---|---|---|
| `RIME_SHARED_DATA_DIR` | RIME 共享数据目录路径 | `third_party/librime/data/minimal` |
| `RIME_USER_DATA_DIR` | RIME 用户数据目录路径 | `/tmp/rime-user` |

## 许可证

如无特殊说明，该项目的代码以 GNU 通用公共许可协议第三版或任何更新的版本开源，文档、配置文件以及开发维护过程中使用的脚本等以 MIT 许可证开源。

该项目遵守 [REUSE 规范]。

你可以使用 [reuse-tool](https://github.com/fsfe/reuse-tool) 生成这个项目的 SPDX 列表：

```bash
reuse spdx
```

[REUSE 规范]: https://reuse.software/spec-3.3/
