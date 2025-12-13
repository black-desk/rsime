<!--
SPDX-FileCopyrightText: 2025 Chen Linxuan <me@black-desk.cn>

SPDX-License-Identifier: MIT
-->

# rsime

A command-line Chinese input tool powered by [RIME](https://rime.im/), designed
for TUI environments where no graphical input method is available.

en | [zh_CN](README.zh_CN.md)

> [!WARNING]
>
> This English README is translated from the Chinese version using LLM and may
> contain errors.

## Features

- **One-shot mode** — pass a pinyin key sequence as an argument and get Chinese
  output directly on stdout.
- **Interactive mode** — read lines from stdin for continuous input.
- Candidate display and selection.
- Schema selection via command-line flag.
- Supports `reload` and `exit` commands in interactive mode.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) toolchain
- [vcpkg](https://vcpkg.io/) (with `VCPKG_ROOT` set)
- C/C++ build toolchain (CMake, a C compiler, etc.)
- Git (for fetching submodules)

## Building

Clone with submodules and build using `make`:

```bash
git clone --recurse-submodules https://github.com/black-desk/rsime.git
cd rsime
make
```

The build process:

1. Installs the `rime` library via vcpkg.
2. Compiles the Rust project with `cargo build`, pointing to the vcpkg-built
   headers and libraries.

## Usage

**One-shot mode** — convert a pinyin sequence to Chinese characters:

```bash
./target/debug/rsime nihao
```

**Interactive mode** — read pinyin lines from stdin:

```bash
./target/debug/rsime
```

Type `exit` to quit, or `reload` to reinitialize RIME.

**Select a schema:**

```bash
./target/debug/rsime -s luna_pinyin_simp
```

**Show candidates for manual selection:**

```bash
./target/debug/rsime -p nihao
```

**Write debug log:**

```bash
./target/debug/rsime -l /tmp/rsime.log
```

### Environment Variables

| Variable | Description | Default |
|---|---|---|
| `RIME_SHARED_DATA_DIR` | Path to RIME shared data directory | `third_party/librime/data/minimal` |
| `RIME_USER_DATA_DIR` | Path to RIME user data directory | `/tmp/rime-user` |

## License

Unless otherwise specified, the code of this project is open source under the
GNU General Public License version 3 or any later version, while documentation,
configuration files, and scripts used in the development and maintenance process
are open source under the MIT License.

This project complies with the [REUSE specification].

You can use [reuse-tool](https://github.com/fsfe/reuse-tool) to generate the
SPDX list for this project:

```bash
reuse spdx
```

[REUSE specification]: https://reuse.software/spec-3.3/
