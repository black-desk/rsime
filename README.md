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

- **Interactive TUI mode** — type pinyin and select candidates directly in the terminal.
- **Stdio mode** — for editor integration (Vim-style key input, JSONL output).
- Online schema installation via plum (no local plum needed).
- List / switch input schemas.
- Shell completion and keybinding support.

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

**Interactive TUI input:**

```bash
rsime tui
```

**Stdio mode (editor integration):**

```bash
rsime stdio
```

Accepts Vim-style key notation (e.g. `<CR>`, `<Space>`, `<Esc>`, `<BS>`, `<Up>`, `<Down>`, etc.), one key per line, and outputs JSONL responses.

**Install schemas online (via plum, no local plum needed):**

```bash
rsime install               # install preset schemas
rsime install double-pinyin  # install a specific package
```

**List available schemas:**

```bash
rsime list-schemas
```

**Show current schema:**

```bash
rsime current-schema
```

**Set active input schema:**

```bash
rsime set-schema double_pinyin_flypy
```

**Shell init (completion + optional keybinding):**

```bash
rsime shell-init bash          # completion only
rsime shell-init bash --bind   # completion + Alt-I binding for TUI
rsime shell-init zsh --bind    # zsh version
rsime shell-init fish --bind   # fish version
```

**Write debug log:**

```bash
rsime -l /tmp/rsime.log tui
```

### Environment Variables

| Variable | Description | Default |
|---|---|---|
| `RIME_USER_DATA_DIR` | Path to RIME user data directory (also used as shared data directory) | `~/.config/rsime` |

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
