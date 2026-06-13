# Shell 内联 preedit（渲染真实 prompt）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `rsime tui` 在 shell 快捷键绑定下，把 preedit 插入到 shell 的真实 prompt（保留颜色）里显示，并根治 prompt 残影问题。

**Architecture:** shell 的绑定把渲染好的 prompt（带 ANSI 颜色码）通过新环境变量 `RSIME_PROMPT` 传给 `rsime tui`。rsime 用 `ansi-to-tui` 把它解析成 ratatui 的带样式 spans，取最后一行作为 TUI 第一行前缀，拼接命令与 preedit，继续用 ratatui 渲染。视口仍为 `Inline(2)`，多行 prompt 上方各行在视口外不重画。

**Tech Stack:** Rust 2024、ratatui 0.30、ansi-to-tui 8.0、crossterm、clap。覆盖 bash/zsh/fish。

**参考 spec:** `docs/superpowers/specs/2026-06-13-shell-inline-preedit-design.md`

**约定：**
- 测试编译需要 rime-sys，本机用 `--features cli,bundled-vcpkg`（若已装系统 librime，`--features cli` 亦可）。
- 提交信息按 CLAUDE.md：`git commit -s` + `Assisted-by: agent:claude`。

---

## File Structure

- **Modify** `rsime/Cargo.toml` — 新增 `ansi-to-tui` 依赖（`cli` feature 下）。
- **Modify** `rsime/src/main.rs` — 核心逻辑：
  - 新增纯函数 `parse_prompt_spans`（解析 prompt → spans）、`build_shell_line`（拼装第一行）、`read_shell_prompt`（读环境变量）。
  - 新增 `bash_supports_prompt_expansion` / `parse_bash_version`（版本判断）。
  - 修改 `run_tui`（读取 prompt 并传入 loop）、`tui_loop`（draw 闭包新增"真实 prompt"分支）、`print_shell_bind`（三 shell 绑定加 `RSIME_PROMPT`）、`shell_init_cmd`（bash ≥4.4 检查）。
  - 末尾新增 `#[cfg(test)] mod ...` 单元测试。
- **Create** `rsime/tests/shell_init.rs` — shell-init 输出的集成测试。

---

## Task 1: 添加 ansi-to-tui 依赖

**Files:**
- Modify: `rsime/Cargo.toml`

- [ ] **Step 1: 编辑 Cargo.toml — feature 列表加入 ansi-to-tui**

把第 13 行：
```toml
cli = ["clap", "clap_complete", "crossterm", "ratatui", "ureq"]
```
改为：
```toml
cli = ["clap", "clap_complete", "crossterm", "ratatui", "ansi-to-tui", "ureq"]
```

- [ ] **Step 2: 编辑 Cargo.toml — 新增依赖项**

在 `ratatui = { version = "0.30", optional = true }` 这一行之后加入：
```toml
ansi-to-tui = { version = "8.0", optional = true }
```

- [ ] **Step 3: 验证能编译通过**

Run: `cargo build --features cli,bundled-vcpkg`
Expected: `Finished` （ansi-to-tui 8.0 对应 ratatui 0.30，依赖解析无冲突）。

- [ ] **Step 4: 提交**

```bash
git add rsime/Cargo.toml
git commit -s -m "build(deps): add ansi-to-tui for shell prompt color parsing" -m "Assisted-by: agent:claude"
```

---

## Task 2: 纯函数 parse_prompt_spans（TDD）

**Files:**
- Modify: `rsime/src/main.rs`（顶部 use 区 + 新增函数 + 末尾测试模块）

- [ ] **Step 1: 加入 IntoText trait 导入**

在 `rsime/src/main.rs` 顶部 use 区，紧跟 ratatui 相关 import 之后加入：
```rust
use ansi_to_tui::IntoText;
```

- [ ] **Step 2: 先写失败测试（在 main.rs 末尾追加测试模块）**

```rust
#[cfg(test)]
mod prompt_tests {
    use super::*;
    use ratatui::style::Color;

    fn spans_text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn parse_prompt_plain_text() {
        let spans = parse_prompt_spans("host> ");
        assert_eq!(spans_text(&spans), "host> ");
    }

    #[test]
    fn parse_prompt_keeps_color() {
        // green "foo", reset, then " bar"
        let spans = parse_prompt_spans("\x1b[32mfoo\x1b[0m bar");
        assert_eq!(spans_text(&spans), "foo bar");
        assert!(
            spans.iter().any(|s| s.style.fg == Some(Color::Green)),
            "green color should be preserved"
        );
    }

    #[test]
    fn parse_prompt_multiline_takes_last_line() {
        let spans = parse_prompt_spans("line one\n> ");
        assert_eq!(spans_text(&spans), "> ");
    }

    #[test]
    fn parse_prompt_trailing_newline() {
        // fish 可能输出尾部换行；取最后一个非空行
        let spans = parse_prompt_spans("host> \n");
        assert_eq!(spans_text(&spans), "host> ");
    }

    #[test]
    fn parse_prompt_empty_returns_empty() {
        let spans = parse_prompt_spans("");
        assert!(spans_text(&spans).is_empty());
    }
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test --features cli,bundled-vcpkg prompt_tests`
Expected: 编译失败（`cannot find function parse_prompt_spans`）。

- [ ] **Step 4: 实现 parse_prompt_spans**

在 `read_shell_context` 函数附近新增：
```rust
/// 把 shell 传来的渲染后 prompt（带 ANSI 颜色码）解析成 ratatui 的带样式 spans，
/// 取最后一个非空行（多行 prompt 只用光标所在的最后一行）。
/// 解析失败时退化为纯文本最后一行。
fn parse_prompt_spans(prompt: &str) -> Vec<Span<'static>> {
    let text = match prompt.into_text() {
        Ok(text) => text,
        Err(_) => {
            let last = prompt
                .lines()
                .rev()
                .find(|l| !l.is_empty())
                .unwrap_or("");
            return vec![Span::raw(last.to_string())];
        }
    };
    text.lines
        .iter()
        .rev()
        .find(|line| line.spans.iter().any(|s| !s.content.is_empty()))
        .map(|line| line.spans.clone())
        .unwrap_or_default()
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test --features cli,bundled-vcpkg prompt_tests`
Expected: 5 passed。

- [ ] **Step 6: 提交**

```bash
git add rsime/src/main.rs
git commit -s -m "feat(tui): parse shell prompt to ratatui spans via ansi-to-tui" -m "Assisted-by: agent:claude"
```

---

## Task 3: 纯函数 build_shell_line（TDD）

**Files:**
- Modify: `rsime/src/main.rs`

- [ ] **Step 1: 先写失败测试（追加到 prompt_tests 模块内）**

```rust
    #[test]
    fn build_shell_line_assembles_parts() {
        let prompt = vec![Span::raw("host> ")];
        // 命令 "cd rsime"，readline 光标在 col 2（"cd" 之后）
        // 已提交 out="AB"，rsime 光标在 1（out_left="A" out_right="B"）
        // preedit "niha"，composition 光标在 2（"ni" 之后）
        let line = build_shell_line(&prompt, "cd rsime", 2, "A", "niha", 2, "B");
        // host> + "cd" + "A" + "ni" + "|" + "ha" + "B" + " rsime"
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "host> cdAni|haB rsime");
    }

    #[test]
    fn build_shell_line_preedit_is_styled_yellow_underlined() {
        let prompt = vec![Span::raw("> ")];
        let line = build_shell_line(&prompt, "", 0, "", "ni", 2, "");
        let preedit_span = line
            .spans
            .iter()
            .find(|s| s.content.as_ref() == "ni")
            .expect("preedit span present");
        assert_eq!(preedit_span.style.fg, Some(Color::Yellow));
        assert!(preedit_span
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED));
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --features cli,bundled-vcpkg prompt_tests`
Expected: 编译失败（`cannot find function build_shell_line`）。

- [ ] **Step 3: 实现 build_shell_line**

在 `parse_prompt_spans` 之后新增：
```rust
/// 拼装 shell 模式下 TUI 第一行：
///   prompt spans + 光标前命令 + 已提交前段 + preedit + 已提交后段 + 光标后命令
/// preedit 用黄色+下划线标识"未提交"，其余命令/已提交文本用默认样式，prompt 保留各自颜色。
fn build_shell_line(
    prompt_spans: &[Span<'static>],
    line: &str,
    point: usize,
    out_left: &str,
    preedit: &str,
    cursor_pos: usize,
    out_right: &str,
) -> Line<'static> {
    let rl_before: String = line.chars().take(point).collect();
    let rl_after: String = line.chars().skip(point).collect();
    let preedit_before: String = preedit.chars().take(cursor_pos).collect();
    let preedit_after: String = preedit.chars().skip(cursor_pos).collect();
    let preedit_style = Style::default().fg(Color::Yellow).underlined();
    let plain = Style::default();

    let mut spans: Vec<Span<'static>> = prompt_spans.to_vec();
    spans.push(Span::styled(rl_before, plain));
    spans.push(Span::styled(out_left.to_string(), plain));
    spans.push(Span::styled(preedit_before, preedit_style));
    spans.push(Span::raw("|"));
    spans.push(Span::styled(preedit_after, preedit_style));
    spans.push(Span::styled(out_right.to_string(), plain));
    spans.push(Span::styled(rl_after, plain));
    Line::from(spans)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --features cli,bundled-vcpkg prompt_tests`
Expected: 7 passed。

- [ ] **Step 5: 提交**

```bash
git add rsime/src/main.rs
git commit -s -m "feat(tui): assemble shell inline line from prompt spans + preedit" -m "Assisted-by: agent:claude"
```

---

## Task 4: 接入 run_tui / tui_loop

**Files:**
- Modify: `rsime/src/main.rs`（`read_shell_prompt` 新增、`run_tui`、`tui_loop` 签名与 draw 闭包）

- [ ] **Step 1: 新增 read_shell_prompt 函数**

在 `parse_prompt_spans` 之后新增：
```rust
/// 从 RSIME_PROMPT 环境变量读取 shell 渲染好的 prompt 并解析成 spans。
/// 缺失或为空时返回 None（回退到非 shell 模式）。
fn read_shell_prompt() -> Option<Vec<Span<'static>>> {
    let prompt = std::env::var("RSIME_PROMPT").ok()?;
    if prompt.is_empty() {
        return None;
    }
    Some(parse_prompt_spans(&prompt))
}
```

- [ ] **Step 2: run_tui 读取 prompt 并传入 tui_loop**

在 `run_tui` 中，把：
```rust
    let shell_ctx = read_shell_context();
```
改为（在其后追加一行读取 prompt）：
```rust
    let shell_ctx = read_shell_context();
    let prompt = read_shell_prompt();
```
并把：
```rust
    let result = tui_loop(session, &mut terminal, &mut output, &mut cursor, &shell_ctx);
```
改为：
```rust
    let result = tui_loop(session, &mut terminal, &mut output, &mut cursor, &shell_ctx, &prompt);
```

- [ ] **Step 3: tui_loop 签名加 prompt 参数**

把 `tui_loop` 签名：
```rust
fn tui_loop(
    session: &Session,
    terminal: &mut Terminal<CrosstermBackend<std::fs::File>>,
    output: &mut String,
    cursor: &mut usize,
    shell_ctx: &Option<(String, usize)>,
) -> Result<()> {
```
改为：
```rust
fn tui_loop(
    session: &Session,
    terminal: &mut Terminal<CrosstermBackend<std::fs::File>>,
    output: &mut String,
    cursor: &mut usize,
    shell_ctx: &Option<(String, usize)>,
    prompt: &Option<Vec<Span<'static>>>,
) -> Result<()> {
```

- [ ] **Step 4: draw 闭包新增"真实 prompt"分支**

在 draw 闭包里，把当前第一段：
```rust
            let comp_line = if let Some((line, point)) = shell_ctx {
                // shell 模式：内联 preedit，用不同颜色区分已有命令和拼音输入
                //   ❯ echo hell niha| o world
                //   ↑          ↑ point    ↑ cursor in preedit
                let rl_before: String = line.chars().take(*point).collect();
                let rl_after: String = line.chars().skip(*point).collect();
                let cmd_style = Style::default().dim();
                let preedit_style = Style::default().fg(Color::Yellow);
                let spans = vec![
                    Span::styled("❯ ", cmd_style),
                    Span::styled(rl_before, cmd_style),
                    Span::styled(out_left, cmd_style),
                    Span::styled(preedit_with_cursor, preedit_style),
                    Span::styled(out_right, cmd_style),
                    Span::styled(rl_after, cmd_style),
                ];
                Paragraph::new(Line::from(spans))
            } else if preedit.is_empty() && candidates.is_empty() && output.is_empty() {
                Paragraph::new("❯ Type pinyin, Esc to finish").dim()
            } else {
                Paragraph::new(format!("❯ {}{}{}", out_left, preedit_with_cursor, out_right))
            };
```
改为（在最前面插入新的真实 prompt 分支，原 `❯` 分支降级为向后兼容）：
```rust
            let comp_line = if let Some(prompt_spans) = prompt {
                // shell 模式（真实 prompt）：prompt spans + 命令 + 内联 preedit
                let (line, point) = shell_ctx
                    .as_ref()
                    .map(|(l, p)| (l.as_str(), *p))
                    .unwrap_or(("", 0));
                Paragraph::new(build_shell_line(
                    prompt_spans,
                    line,
                    point,
                    &out_left,
                    &preedit,
                    cursor_pos,
                    &out_right,
                ))
            } else if let Some((line, point)) = shell_ctx {
                // 旧的内联模式（无 RSIME_PROMPT，向后兼容）：❯ + 命令 + preedit
                let rl_before: String = line.chars().take(*point).collect();
                let rl_after: String = line.chars().skip(*point).collect();
                let cmd_style = Style::default().dim();
                let preedit_style = Style::default().fg(Color::Yellow);
                let spans = vec![
                    Span::styled("❯ ", cmd_style),
                    Span::styled(rl_before, cmd_style),
                    Span::styled(out_left, cmd_style),
                    Span::styled(preedit_with_cursor, preedit_style),
                    Span::styled(out_right, cmd_style),
                    Span::styled(rl_after, cmd_style),
                ];
                Paragraph::new(Line::from(spans))
            } else if preedit.is_empty() && candidates.is_empty() && output.is_empty() {
                Paragraph::new("❯ Type pinyin, Esc to finish").dim()
            } else {
                Paragraph::new(format!("❯ {}{}{}", out_left, preedit_with_cursor, out_right))
            };
```

- [ ] **Step 5: 编译确认**

Run: `cargo build --features cli,bundled-vcpkg`
Expected: `Finished`，无警告错误。

- [ ] **Step 6: 全部测试仍通过**

Run: `cargo test --features cli,bundled-vcpkg`
Expected: 全部 passed（stdio 集成测试不受影响）。

- [ ] **Step 7: 提交**

```bash
git add rsime/src/main.rs
git commit -s -m "feat(tui): render real shell prompt with inline preedit" -m "Assisted-by: agent:claude"
```

---

## Task 5: bash 版本检查提到 4.4（TDD 版本函数）

`${PS1@P}` 需要 bash ≥ 4.4，把 `shell_init_cmd` 中现有的 major<4 检查改为按 major.minor 判断。

**Files:**
- Modify: `rsime/src/main.rs`（`shell_init_cmd` + 新增两个纯函数 + 测试模块）

- [ ] **Step 1: 先写失败测试（main.rs 末尾新增模块）**

```rust
#[cfg(test)]
mod bash_version_tests {
    use super::*;

    #[test]
    fn parse_bash_version_works() {
        assert_eq!(parse_bash_version("5.2"), (5, 2));
        assert_eq!(parse_bash_version("4.4"), (4, 4));
        assert_eq!(parse_bash_version("3.2"), (3, 2));
        assert_eq!(parse_bash_version("garbage"), (0, 0));
        assert_eq!(parse_bash_version("5"), (5, 0));
    }

    #[test]
    fn ps1_at_p_support_threshold() {
        assert!(!bash_supports_prompt_expansion(3, 2));
        assert!(!bash_supports_prompt_expansion(4, 0));
        assert!(!bash_supports_prompt_expansion(4, 3));
        assert!(bash_supports_prompt_expansion(4, 4));
        assert!(bash_supports_prompt_expansion(5, 0));
        assert!(bash_supports_prompt_expansion(5, 2));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --features cli,bundled-vcpkg bash_version_tests`
Expected: 编译失败（`cannot find function parse_bash_version`）。

- [ ] **Step 3: 实现两个纯函数**

在 `shell_init_cmd` 之前新增：
```rust
/// 解析 "major.minor" 形式的 bash 版本字符串。
fn parse_bash_version(ver: &str) -> (u32, u32) {
    let mut parts = ver.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor)
}

/// ${PS1@P}（prompt 展开）需要 bash >= 4.4。
fn bash_supports_prompt_expansion(major: u32, minor: u32) -> bool {
    (major, minor) >= (4, 4)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --features cli,bundled-vcpkg bash_version_tests`
Expected: 2 passed。

- [ ] **Step 5: shell_init_cmd 改用新检查**

把 `shell_init_cmd` 中的：
```rust
        if matches!(sh, Shell::Bash) {
            let output = Command::new("bash")
                .arg("-c")
                .arg("echo \"${BASH_VERSINFO[0]}\"")
                .output()?;
            let major = String::from_utf8_lossy(&output.stdout).trim().parse::<u32>().unwrap_or(0);
            if major < 4 {
                bail!(
                    "bash {major} does not support bind -x with READLINE_LINE (requires bash >= 4).\n\
                     Consider using zsh or fish, or installing a newer bash via Homebrew."
                );
            }
        }
```
改为：
```rust
        if matches!(sh, Shell::Bash) {
            let output = Command::new("bash")
                .arg("-c")
                .arg("echo \"${BASH_VERSINFO[0]}.${BASH_VERSINFO[1]}\"")
                .output()?;
            let (major, minor) =
                parse_bash_version(String::from_utf8_lossy(&output.stdout).trim());
            if !bash_supports_prompt_expansion(major, minor) {
                bail!(
                    "bash {major}.{minor} does not support ${{PS1@P}} prompt expansion (requires bash >= 4.4).\n\
                     Consider using zsh or fish, or installing a newer bash via Homebrew."
                );
            }
        }
```

- [ ] **Step 6: 编译确认**

Run: `cargo build --features cli,bundled-vcpkg`
Expected: `Finished`。

- [ ] **Step 7: 提交**

```bash
git add rsime/src/main.rs
git commit -s -m "fix(tui): require bash >= 4.4 for PS1@P prompt expansion" -m "Assisted-by: agent:claude"
```

---

## Task 6: 更新三 shell 绑定 + 集成测试

**Files:**
- Modify: `rsime/src/main.rs`（`print_shell_bind` 的 bash/zsh/fish 分支）
- Create: `rsime/tests/shell_init.rs`

- [ ] **Step 1: bash 分支加 RSIME_PROMPT**

把 `print_shell_bind` 的 `Shell::Bash` 分支里：
```rust
    output=$(RSIME_READLINE_LINE="$READLINE_LINE" RSIME_READLINE_POINT="$READLINE_POINT" rsime tui)
```
改为：
```rust
    output=$(RSIME_PROMPT="${{PS1@P}}" RSIME_READLINE_LINE="$READLINE_LINE" RSIME_READLINE_POINT="$READLINE_POINT" rsime tui)
```

- [ ] **Step 2: zsh 分支加 RSIME_PROMPT**

把 `Shell::Zsh` 分支里（上一轮刚加过 LINE/POINT 的那行）：
```rust
    output=$(RSIME_READLINE_LINE="$BUFFER" RSIME_READLINE_POINT="$CURSOR" rsime tui)
```
改为：
```rust
    output=$(RSIME_PROMPT="${{(%)PROMPT}}" RSIME_READLINE_LINE="$BUFFER" RSIME_READLINE_POINT="$CURSOR" rsime tui)
```

- [ ] **Step 3: fish 分支加 RSIME_PROMPT**

把 `Shell::Fish` 分支的 `bind` 行：
```rust
bind {fish_key} 'RSIME_READLINE_LINE=(commandline) RSIME_READLINE_POINT=(commandline --cursor) rsime tui | read -l output; and commandline --insert "$output"'"
```
改为：
```rust
bind {fish_key} 'RSIME_PROMPT=(fish_prompt) RSIME_READLINE_LINE=(commandline) RSIME_READLINE_POINT=(commandline --cursor) rsime tui | read -l output; and commandline --insert "$output"'"
```

- [ ] **Step 4: 编译确认**

Run: `cargo build --features cli,bundled-vcpkg`
Expected: `Finished`。

- [ ] **Step 5: 创建集成测试文件**

新建 `rsime/tests/shell_init.rs`：
```rust
// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use assert_cmd::Command;

fn shell_init_stdout(shell: &str) -> String {
    let mut cmd = Command::cargo_bin("rsime").unwrap();
    cmd.args(["shell-init", shell, "--bind", r"\ei"]);
    String::from_utf8_lossy(
        &cmd.assert()
            .success()
            .get_output()
            .clone()
            .stdout,
    )
    .to_string()
}

#[test]
fn zsh_binding_passes_prompt() {
    let out = shell_init_stdout("zsh");
    assert!(
        out.contains(r#"RSIME_PROMPT="${(%)PROMPT}""#),
        "zsh binding should pass RSIME_PROMPT, got:\n{out}"
    );
    assert!(out.contains(r#"RSIME_READLINE_LINE="$BUFFER""#));
}

#[test]
fn fish_binding_passes_prompt() {
    let out = shell_init_stdout("fish");
    assert!(out.contains("RSIME_PROMPT=(fish_prompt)"));
    assert!(out.contains("RSIME_READLINE_LINE=(commandline)"));
}

/// bash 的 shell-init 会先做版本门控（系统 bash < 4.4 会 bail）。
/// macOS 自带 bash 3.2 → 跳过；bash ≥ 4.4 → 断言绑定含 RSIME_PROMPT。
#[test]
fn bash_binding_passes_prompt() {
    let ver = std::process::Command::new("bash")
        .arg("-c")
        .arg("echo \"${BASH_VERSINFO[0]}.${BASH_VERSINFO[1]}\"")
        .output();
    let ok = ver
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let mut p = v.split('.');
            let maj = p.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let min = p.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            (maj, min) >= (4, 4)
        })
        .unwrap_or(false);
    if !ok {
        eprintln!("skip bash binding test: system bash < 4.4");
        return;
    }
    let out = shell_init_stdout("bash");
    assert!(
        out.contains(r#"RSIME_PROMPT="${PS1@P}""#),
        "bash binding should pass RSIME_PROMPT, got:\n{out}"
    );
}
```

- [ ] **Step 6: 运行集成测试**

Run: `cargo test --features cli,bundled-vcpkg shell_init`
Expected: zsh/fish passed；bash 在系统 bash ≥4.4 时 passed（<4.4 时 skip）。

- [ ] **Step 7: 手动校验生成的 zsh 脚本语法**

Run: `./target/debug/rsime shell-init zsh --bind '\ei' | awk '/^# rsime TUI keybinding/{p=1} p' | zsh -n && echo OK`
Expected: `OK`（zsh 语法无误）。若用 release：把 `target/debug` 换 `target/release`。

- [ ] **Step 8: 提交**

```bash
git add rsime/src/main.rs rsime/tests/shell_init.rs
git commit -s -m "feat(tui): pass RSIME_PROMPT in bash/zsh/fish keybindings" -m "Assisted-by: agent:claude"
```

---

## Task 7: 手动端到端验证 + 文档检查

**Files:** 无代码改动；仅人工验证与可选文档更新。

- [ ] **Step 1: 重新构建**

Run: `cargo build --release --features cli,bundled-vcpkg`
Expected: `Finished`。

- [ ] **Step 2: zsh 实测**

在真实 zsh 终端：
```zsh
eval "$(./target/release/rsime shell-init zsh --bind '\ei')"
echo hello <按 Meta-i 唤起 rsime> <输入 niha 选词 Enter>
```
预期：唤起时第一行显示真实 zsh prompt（保留颜色）+ 命令 + 内联 preedit（黄+下划线），第二行候选词；**无 prompt 残影**；Esc/Enter 退出后中文正确插入命令行。

- [ ] **Step 3: fish 实测**

在真实 fish 终端重复 Step 2（用 `rsime shell-init fish`）。预期同上。

- [ ] **Step 4: bash 实测（bash ≥4.4）**

在真实 bash 终端重复 Step 2（用 `rsime shell-init bash`）。预期同上。

- [ ] **Step 5: 回归 —— 无 shell 上下文**

直接运行 `./target/release/rsime tui`（不经过 shell 绑定）。
预期：仍显示原来的 `❯ Type pinyin, Esc to finish` / `❯ <preedit>` 行为（向后兼容）。

- [ ] **Step 6: 检查 CLAUDE.md**

检查 `CLAUDE.md` 中 `shell-init` 子命令与注意事项描述是否需要补充 `RSIME_PROMPT`。若本功能未改变项目结构/架构/构建方式的大方向，可不动；若有必要，补充一行说明 shell 绑定会传递 `RSIME_PROMPT`。

- [ ] **Step 7: 最终提交（若有文档/小修）**

```bash
git add -A
git commit -s -m "docs: note RSIME_PROMPT env var in shell-init" -m "Assisted-by: agent:claude"
```
（若无改动则跳过此步。）

---

## Self-Review（plan 写完后自查，已执行）

- **Spec 覆盖：** Task1=依赖；Task2/3/4=spec 第 2 节（解析/渲染/run_tui 接入）；Task6=spec 第 3 节（三 shell 绑定）；Task5=spec 第 3 节末尾（bash 4.4）；Task7=spec 第 6 节（手测）+ 回归。兜底（RSIME_PROMPT 缺失回退）由 Task4 的分支顺序覆盖；多行 prompt 取最后行由 Task2 覆盖。✓
- **占位符：** 无 TBD/TODO；每步含完整代码或命令。✓
- **类型一致：** `parse_prompt_spans` 返回 `Vec<Span<'static>>`，`build_shell_line` 形参 `&[Span<'static>]`、返回 `Line<'static>`，`read_shell_prompt` 返回 `Option<Vec<Span<'static>>>`，`tui_loop` 形参 `&Option<Vec<Span<'static>>>`，全程一致。`bash_supports_prompt_expansion`/`parse_bash_version` 命名一致。✓
