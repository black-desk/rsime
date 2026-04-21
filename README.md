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

- **One-shot conversion** — pass a pinyin key sequence, get Chinese output.
- Online schema installation via plum (no local plum needed).
- List / switch input schemas.
- Candidate display and selection.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) toolchain
- [vcpkg](https://vcpkg.io/) (with `VCPKG_ROOT` set)
- C/C++ build toolchain (CMake, a C compiler, etc.)
- Git (for fetching submodules and installing schemas)

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

**Convert pinyin:**

```bash
rsime nihao    # outputs: 你好
```

**Select a schema:**

```bash
rsime -s double_pinyin_flypy nihao
```

**List available schemas:**

```bash
rsime --list-schemas
```

**Show current schema:**

```bash
rsime --current-schema
```

**Install schemas online (via plum, no local plum needed):**

```bash
rsime --install               # install preset schemas
rsime --install double-pinyin  # install a specific package
```

**Show candidates for manual selection:**

```bash
rsime -p nihao
```

**Write debug log:**

```bash
rsime -l /tmp/rsime.log nihao
```

### Environment Variables

| Variable | Description | Default |
|---|---|---|
| `RIME_SHARED_DATA_DIR` | Path to RIME shared data directory | `third_party/librime/data/minimal` |
| `RIME_USER_DATA_DIR` | Path to RIME user data directory | `~/.config/rsime` |

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
