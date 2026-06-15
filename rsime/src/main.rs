// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use std::fs::{self, File};
use std::os::fd::FromRawFd;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{bail, Result};
use clap::{CommandFactory, Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};
use ansi_to_tui::IntoText;
use rsime::rime::{
    deploy_on_changed, finalize, get_schema_list, initialize,
    set_notification_handler, setup, DeployResult, Session, Traits,
    KEY_BACKSPACE, KEY_DELETE, KEY_DOWN, KEY_END, KEY_ESCAPE, KEY_HOME, KEY_LEFT, KEY_PAGEDOWN,
    KEY_PAGEUP, KEY_RETURN, KEY_RIGHT, KEY_TAB, KEY_UP, KEY_SPACE,
};
use serde::Serialize;

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

fn log(msg: &str) {
    let Ok(mut guard) = LOG_FILE.lock() else { return };
    let Some(file) = guard.as_mut() else { return };
    let _ = writeln!(file, "{}", msg);
}

fn default_user_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config").join("rsime")
}

#[derive(Parser)]
#[command(name = "rsime", about = "基于 RIME 的命令行中文输入工具", version)]
struct Cli {
    /// 将日志输出写入文件
    #[arg(short, long)]
    log: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 交互式 TUI 模式，在终端中输入拼音并选择候选词
    Tui,

    /// Stdio 模式，用于编辑器集成（Vim 风格按键输入，JSONL 输出）
    Stdio,

    /// 在线安装 RIME 输入方案（通过 plum，无需本地安装）
    Install {
        /// 要安装的方案包（默认安装 :preset）
        packages: Vec<String>,
    },

    /// 列出可用的输入方案
    ListSchemas,

    /// 显示当前使用的输入方案
    CurrentSchema,

    /// 切换输入方案
    SetSchema {
        /// 要激活的方案 ID
        schema_id: String,
    },

    /// 输出 shell 初始化脚本（补全 + 可选快捷键绑定）
    ShellInit {
        /// Shell 类型 (bash, zsh, fish)
        shell: String,

        /// 绑定 TUI 模式到快捷键（默认: \\ei）
        #[arg(long, num_args = 0..=1)]
        bind: Option<Option<String>>,
    },
}

// JSON 输出类型

#[derive(Serialize)]
struct JsonCandidate {
    text: String,
    comment: Option<String>,
}

#[derive(Serialize)]
struct JsonStdioResponse {
    commit: String,
    preedit: String,
    candidates: Vec<JsonCandidate>,
    highlighted: usize,
}

fn init_rime() -> Result<Traits> {
    let user_data_dir = std::env::var("RIME_USER_DATA_DIR")
        .unwrap_or_else(|_| default_user_data_dir().to_string_lossy().to_string());

    // 首次运行时自动安装预设方案
    if !PathBuf::from(&user_data_dir).exists() {
        eprintln!("No RIME user data found, installing preset schemas...");
        install_cmd(&[])?;
    }

    let mut traits = Traits::new();
    traits.set_app_name("rime.console");
    // 故意将 shared_data_dir 和 user_data_dir 设为同一目录。
    // 本项目通常不依赖系统级 RIME 安装，因此没有独立的共享数据目录。
    // 留空会导致 RIME 回退到当前工作目录，从而在部署阶段出错。
    traits.set_shared_data_dir(&user_data_dir);
    traits.set_user_data_dir(&user_data_dir);
    setup(&mut traits);
    initialize(&mut traits);

    set_notification_handler(|t, v| {
        log(&format!("[{}] {}", t, v));
    });

    log("initializing...");
    match deploy_on_changed() {
        DeployResult::Success => {}
        DeployResult::Failure => {
            log("deployment failed");
        }
    }
    log("ready.");
    Ok(traits)
}

fn list_schemas_cmd() -> Result<()> {
    let _traits = init_rime()?;
    let schemas = get_schema_list();
    for s in &schemas {
        println!("{}\t{}", s.schema_id, s.name);
    }
    Ok(())
}

fn install_cmd(packages: &[String]) -> Result<()> {
    let rime_dir = std::env::var("RIME_USER_DATA_DIR")
        .unwrap_or_else(|_| default_user_data_dir().to_string_lossy().to_string());
    fs::create_dir_all(&rime_dir)?;

    let script_url = "https://raw.githubusercontent.com/rime/plum/master/rime-install";
    let script = ureq::get(script_url).call()?.into_body().read_to_string()?;

    let targets = if packages.is_empty() {
        vec![":preset".to_string()]
    } else {
        packages.to_vec()
    };

    let plum_dir = PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()),
    )
    .join(".config")
    .join("rsime-plum");

    let mut child = Command::new("bash")
        .env("rime_dir", &rime_dir)
        .env("plum_dir", &plum_dir)
        .env("no_update", "1")
        .stdout(unsafe {
            let fd = libc::dup(libc::STDERR_FILENO);
            std::process::Stdio::from(std::os::fd::OwnedFd::from_raw_fd(fd))
        })
        .arg("-s")
        .arg("--")
        .args(&targets)
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(script.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        bail!(
            "plum install failed with exit code {}",
            status.code().unwrap_or(1)
        );
    }
    Ok(())
}

fn print_shell_bind(shell: &str, key: Option<&str>) -> Result<()> {
    use clap_complete::Shell;

    let shell = shell.parse::<Shell>().map_err(|_| anyhow::anyhow!("unsupported shell: {shell}"))?;
    let default_bind = "\\ei";
    let bind = key.unwrap_or(default_bind);

    match shell {
        Shell::Bash => {
            println!(r#"# rsime TUI keybinding
stty -ixon
rsime-widget() {{
    local output
    output=$(RSIME_PROMPT="${{PS1@P}}" RSIME_READLINE_LINE="$READLINE_LINE" RSIME_READLINE_POINT="$READLINE_POINT" rsime tui)
    [[ -n "$output" ]] && READLINE_LINE="${{READLINE_LINE:0:$READLINE_POINT}}$output${{READLINE_LINE:$READLINE_POINT}}"
    READLINE_POINT=$(( READLINE_POINT + ${{#output}} ))
}}
bind -x '"{bind}": rsime-widget'"#);
        }
        Shell::Zsh => {
            // 将 bash 风格的 \C-q 转换为 zsh 的 ^Q
            let zsh_key = bind.replace("\\C-", "^").replace("\\e", "^[").replace('\\', "");
            println!(r#"# rsime TUI keybinding
rsime-widget() {{
    local output _rp
    # 完整渲染 prompt：(e) 先执行 prompt_subst（${{...}}/$((...))/函数调用，Powerlevel10k 等靠它），
    # 再 (%) 做 % 码展开。${{(%):-...}} 用 subst 展开取结果，去掉尾部换行。
    _rp=${{(%):-${{(e)PROMPT}}}}
    _rp=${{_rp%$'\n'}}
    output=$(RSIME_PROMPT="$_rp" RSIME_READLINE_LINE="$BUFFER" RSIME_READLINE_POINT="$CURSOR" rsime tui)
    [[ -n "$output" ]] && LBUFFER+="$output"
    zle reset-prompt
}}
zle -N rsime-widget
bindkey '{zsh_key}' rsime-widget"#);
        }
        Shell::Fish => {
            // 将 \C-q 转换为 \cq
            let fish_key = bind.to_lowercase().replace("\\c-", "\\c");
            println!(r##"# rsime TUI keybinding
bind {fish_key} 'RSIME_PROMPT=(fish_prompt) RSIME_READLINE_LINE=(commandline) RSIME_READLINE_POINT=(commandline --cursor) rsime tui | read -l output; and commandline --insert "$output"'"##);
        }
        _ => bail!("unsupported shell for keybinding: {shell}"),
    }
    Ok(())
}

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

fn shell_init_cmd(shell: &str, bind_key: Option<&str>) -> Result<()> {
    use clap_complete::Shell;

    let sh = shell.parse::<Shell>().map_err(|_| anyhow::anyhow!("unsupported shell: {shell}"))?;

    clap_complete::generate(sh, &mut Cli::command(), "rsime", &mut std::io::stdout());

    if let Some(key) = bind_key {
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
        println!();
        print_shell_bind(shell, Some(key))?;
    }
    Ok(())
}

/// 将 Vim 风格的按键表示法解析为 RIME 键码。
/// 对于 `<Esc>` 返回 None（在非组合状态下用作退出信号）。
fn parse_vim_key(input: &str) -> Option<i32> {
    let key = input.trim();
    if key.is_empty() {
        return None;
    }
    // <特殊按键>
    if key.starts_with('<') && key.ends_with('>') {
        let inner = &key[1..key.len() - 1];
        return match inner {
            "CR" | "Return" | "Enter" => Some(KEY_RETURN as i32),
            "BS" | "BackSpace" => Some(KEY_BACKSPACE as i32),
            "Space" => Some(KEY_SPACE as i32),
            "Esc" | "Escape" => Some(KEY_ESCAPE as i32),
            "Up" => Some(KEY_UP as i32),
            "Down" => Some(KEY_DOWN as i32),
            "Left" => Some(KEY_LEFT as i32),
            "Right" => Some(KEY_RIGHT as i32),
            "Tab" => Some(KEY_TAB as i32),
            "Del" | "Delete" => Some(KEY_DELETE as i32),
            "Home" => Some(KEY_HOME as i32),
            "End" => Some(KEY_END as i32),
            "PageUp" => Some(KEY_PAGEUP as i32),
            "PageDown" => Some(KEY_PAGEDOWN as i32),
            _ => None,
        };
    }
    // 单个字符
    if key.chars().count() == 1 {
        return Some(key.chars().next().unwrap() as i32);
    }
    None
}

fn run_stdio(session: &Session) -> Result<()> {
    use std::io::{BufRead, BufWriter};

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = BufWriter::new(stdout.lock());

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break; // EOF
        }

        let key = line.trim();
        if key.is_empty() {
            continue;
        }

        let key_code = match parse_vim_key(key) {
            Some(kc) => kc,
            None => continue,
        };

        // 非组合状态下按 Esc → 退出
        if key_code == KEY_ESCAPE as i32 {
            let composing = session
                .context()
                .map(|ctx| ctx.composition().length > 0)
                .unwrap_or(false);
            if !composing {
                break;
            }
            // 组合状态下按 Esc：交给 RIME 处理（取消组合）
        }

        let _consumed = session.process_key(rsime::rime::KeyEvent::new(key_code));

        let mut commit = String::new();
        while let Some(c) = session.commit() {
            commit.push_str(&c.text());
        }

        let (preedit, candidates, highlighted) = match session.context() {
            Some(ctx) => {
                let comp = ctx.composition();
                let menu = ctx.menu();
                let preedit = comp.preedit.unwrap_or_default();
                let cands: Vec<JsonCandidate> = menu
                    .candidates
                    .iter()
                    .map(|c| JsonCandidate {
                        text: c.text.clone(),
                        comment: c.comment.clone(),
                    })
                    .collect();
                (preedit, cands, menu.highlighted_candidate_index)
            }
            None => (String::new(), Vec::new(), 0),
        };

        let resp = JsonStdioResponse {
            commit,
            preedit,
            candidates,
            highlighted,
        };
        serde_json::to_writer(&mut writer, &resp)?;
        writeln!(writer)?;
        writer.flush()?;
    }

    Ok(())
}

/// 从环境变量中读取 shell 传递的命令行上下文。
/// bash（$READLINE_LINE/$READLINE_POINT）、zsh（$BUFFER/$CURSOR）
/// 和 fish（commandline/commandline --cursor）的快捷键绑定都会把
/// 命令行内容与光标位置传入这两个环境变量。
///
/// 命令行为空（在空命令行上触发）时，若绑定了 RSIME_PROMPT 则仍返回空命令
/// 上下文（point=0），让真实 prompt 能正常渲染；否则返回 None 退回独立模式。
fn read_shell_context() -> Option<(String, usize)> {
    let line = std::env::var("RSIME_READLINE_LINE").ok()?;
    // 空命令行：若没有 shell prompt，退回独立模式；若有，返回空命令上下文
    if line.is_empty() {
        if std::env::var("RSIME_PROMPT").map(|v| !v.is_empty()).unwrap_or(false) {
            return Some((String::new(), 0));
        }
        return None;
    }
    let point = std::env::var("RSIME_READLINE_POINT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(line.chars().count());
    Some((line, point))
}

/// 剥掉 ansi-to-tui 处理不了的转义序列。
///
/// ansi-to-tui 只认 `ESC[`（CSI）和 `ESC]`（OSC）。对其它转义（最典型的是 fish 的
/// 字符集指定 `ESC(B`），它只吞掉 `ESC`、把后续字节当字面量，于是出现 `(B` 残留。
/// 这里把这些非 CSI/OSC 转义（`ESC` + 中间字节 `0x20..0x2f`* + 终止字节 `0x30..0x7e`）
/// 整段删掉，SGR（`ESC[...m`）与 OSC（`ESC]...`）原样保留交给 ansi-to-tui。
fn strip_unhandled_escapes(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b
            && i + 1 < bytes.len()
            && bytes[i + 1] != b'['
            && bytes[i + 1] != b']'
        {
            // 非 CSI/OSC 转义：跳过 ESC + 中间字节 + 一个终止字节
            i += 1; // ESC
            while i < bytes.len() && (0x20..=0x2f).contains(&bytes[i]) {
                i += 1; // 中间字节
            }
            if i < bytes.len() && (0x30..=0x7e).contains(&bytes[i]) {
                i += 1; // 终止字节
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 把 shell 传来的渲染后 prompt（带 ANSI 颜色码）解析成 ratatui 的带样式 spans，
/// 取最后一个非空行（多行 prompt 只用光标所在的最后一行）。
/// 解析失败时退化为纯文本最后一行。
fn parse_prompt_spans(prompt: &str) -> Vec<Span<'static>> {
    let prompt = strip_unhandled_escapes(prompt);
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

/// 从 RSIME_PROMPT 环境变量读取 shell 渲染好的 prompt 并解析成 spans。
/// 缺失或为空时返回 None（回退到非 shell 模式）。
fn read_shell_prompt() -> Option<Vec<Span<'static>>> {
    let prompt = std::env::var("RSIME_PROMPT").ok()?;
    if prompt.is_empty() {
        return None;
    }
    Some(parse_prompt_spans(&prompt))
}

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

fn run_tui(session: &Session) -> Result<()> {
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;

    let tty_fd = tty.as_raw_fd();

    // 将 stdout 重定向到 /dev/tty，使 crossterm 的光标位置查询（DSR \x1b[6n）能到达终端。
    // crossterm 的 cursor::position() 通过 io::stdout()（fd 1）发送查询，而非后端的 writer，
    // 因此整个 TUI 会话（包括清理阶段的 terminal.clear()）期间都必须保持重定向。
    // （在 $() 中，fd 1 是管道，DSR 转义序列会发送到管道而非终端）
    let saved_stdout = unsafe { libc::dup(1) };
    if saved_stdout == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    unsafe { libc::dup2(tty_fd, 1) };

    // 读取 shell 传递的命令行上下文
    let shell_ctx = read_shell_context();
    let prompt = read_shell_prompt();
    // 诊断日志：便于排查各 shell 传参与 prompt 解析
    log(&format!(
        "shell_ctx: line={:?} point={:?}; prompt_some={}",
        shell_ctx.as_ref().map(|(l, _)| l.as_str()),
        shell_ctx.as_ref().map(|(_, p)| *p),
        prompt.is_some(),
    ));
    if let Ok(v) = std::env::var("RSIME_READLINE_LINE") {
        log(&format!("RSIME_READLINE_LINE raw ({} chars) = {:?}", v.chars().count(), v));
    }
    if let Ok(v) = std::env::var("RSIME_READLINE_POINT") {
        log(&format!("RSIME_READLINE_POINT raw = {:?}", v));
    }
    if let Ok(v) = std::env::var("RSIME_PROMPT") {
        log(&format!(
            "RSIME_PROMPT raw ({} chars) = {:?}",
            v.chars().count(),
            v
        ));
    }
    if let Some(spans) = &prompt {
        let t: String = spans.iter().map(|s| s.content.as_ref()).collect();
        log(&format!("parsed prompt spans ({}): text={:?}", spans.len(), t));
    }
    // 无论有无 shell 上下文，viewport 始终 2 行：
    //   无上下文：preedit 行 + 候选词行
    //   有上下文：内联 preedit 的命令行 + 候选词行

    let backend = CrosstermBackend::new(tty);
    let mut terminal = match Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(2),
        },
    ) {
        Ok(t) => t,
        Err(e) => {
            unsafe { libc::dup2(saved_stdout, 1) };
            unsafe { libc::close(saved_stdout) };
            return Err(e.into());
        }
    };

    let mut output = String::new();
    let mut cursor: usize = 0;

    let viewport_y = terminal.get_frame().area().y;
    let result = tui_loop(session, &mut terminal, &mut output, &mut cursor, &shell_ctx, &prompt);

    // 恢复终端状态（此时 stdout 仍指向 /dev/tty，cursor::position() 可正常工作）
    // terminal.clear() 清除视口内容并恢复光标到清除前的位置。
    // 但该位置可能在视口的任意行（取决于最后一次 draw 写入内容的位置），
    // 因此用视口起点的绝对坐标定位光标，而非相对移动。
    let cleanup = (|| -> Result<()> {
        terminal.clear()?;
        // 将光标移到视口起始行行首（即 TUI 开始前的光标位置）
        crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveTo(0, viewport_y))?;
        terminal.show_cursor()?;
        Ok(())
    })();

    // 清理完成，恢复 stdout 以便输出最终结果
    unsafe { libc::dup2(saved_stdout, 1) };
    unsafe { libc::close(saved_stdout) };

    result?;
    cleanup?;

    log(&format!("FINAL output to stdout = {:?}", output));
    if !output.is_empty() {
        println!("{}", output);
    }
    Ok(())
}

fn tui_loop(
    session: &Session,
    terminal: &mut Terminal<CrosstermBackend<std::fs::File>>,
    output: &mut String,
    cursor: &mut usize,
    shell_ctx: &Option<(String, usize)>,
    prompt: &Option<Vec<Span<'static>>>,
) -> Result<()> {
    let mut frame = 0u32;
    loop {
        let ctx = session.context();
        let (preedit, cursor_pos, candidates, _highlighted) = match &ctx {
            Some(ctx) => {
                let comp = ctx.composition();
                let menu = ctx.menu();
                let preedit = comp.preedit.unwrap_or_default();
                let cursor_pos = comp.cursor_pos;
                let cands: Vec<String> = menu
                    .candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let sel = if i == menu.highlighted_candidate_index { ">" } else { " " };
                        format!("{}{}.{}", sel, i + 1, c.text)
                    })
                    .collect();
                (preedit, cursor_pos, cands, menu.highlighted_candidate_index)
            }
            None => (String::new(), 0, Vec::new(), 0),
        };

        let mut committed = String::new();
        while let Some(commit) = session.commit() {
            committed.push_str(&commit.text());
        }
        if !committed.is_empty() {
            let byte_pos = output
                .char_indices()
                .nth(*cursor)
                .map(|(i, _)| i)
                .unwrap_or(output.len());
            output.insert_str(byte_pos, &committed);
            *cursor += committed.chars().count();
            log(&format!(
                "commit: {:?} -> output={:?} cursor={}",
                committed, output, cursor
            ));
        }

        terminal.draw(|f| {
            let area = f.area();

            let out_left: String = output.chars().take(*cursor).collect();
            let out_right: String = output.chars().skip(*cursor).collect();

            // preedit 内部光标位置：将 | 插入到 cursor_pos 处
            let preedit_before: String = preedit.chars().take(cursor_pos).collect();
            let preedit_after: String = preedit.chars().skip(cursor_pos).collect();
            let preedit_with_cursor = format!("{}|{}", preedit_before, preedit_after);

            // 第一行：命令行内容
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

            let cand_text = candidates.join("  ");
            let cand_line = Paragraph::new(cand_text).style(Style::default());

            f.render_widget(comp_line, Rect { y: area.y, height: 1, ..area });
            f.render_widget(cand_line, Rect { y: area.y + 1, height: 1, ..area });

            // 每帧诊断：记录视口位置、各段内容，排查"行消失/重复"
            log(&format!(
                "draw f={} y={} h={}: preedit={:?} cp={} cands={} output={:?} cursor={}",
                frame, area.y, area.height,
                preedit, cursor_pos, candidates.len(), output, cursor,
            ));
            frame += 1;
        })?;

        let char_count = output.chars().count();
        let ev = event::read()?;
        log(&format!("event: {:?}", ev));
        match ev {
            Event::Key(key) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Enter if preedit.is_empty() => break,
                    KeyCode::Enter => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(KEY_RETURN as i32));
                    }
                    KeyCode::Backspace if !preedit.is_empty() => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(KEY_BACKSPACE as i32));
                    }
                    KeyCode::Backspace if *cursor > 0 => {
                        let byte_pos = output
                            .char_indices()
                            .nth(*cursor - 1)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        let end_byte = output
                            .char_indices()
                            .nth(*cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(output.len());
                        output.drain(byte_pos..end_byte);
                        *cursor -= 1;
                    }
                    KeyCode::Delete if preedit.is_empty() && *cursor < char_count => {
                        let byte_pos = output
                            .char_indices()
                            .nth(*cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(output.len());
                        let end_byte = output
                            .char_indices()
                            .nth(*cursor + 1)
                            .map(|(i, _)| i)
                            .unwrap_or(output.len());
                        output.drain(byte_pos..end_byte);
                    }
                    KeyCode::Left if preedit.is_empty() && *cursor > 0 => {
                        *cursor -= 1;
                    }
                    KeyCode::Right if preedit.is_empty() && *cursor < char_count => {
                        *cursor += 1;
                    }
                    KeyCode::Char(c) => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(c as i32));
                    }
                    KeyCode::Up => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(KEY_UP as i32));
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(KEY_DOWN as i32));
                    }
                    KeyCode::PageUp => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(KEY_PAGEUP as i32));
                    }
                    KeyCode::PageDown => {
                        let _consumed = session.process_key(rsime::rime::KeyEvent::new(KEY_PAGEDOWN as i32));
                    }
                    _ => {}
                }
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(path) = &cli.log {
        let file = File::create(path)?;
        *LOG_FILE.lock().unwrap() = Some(file);
    } else if let Ok(path) = std::env::var("RSIME_LOG") {
        // 便于从 shell 快捷键绑定里开启日志（无需改绑定里的 rsime tui 调用）
        if !path.is_empty() {
            let file = File::create(&path)?;
            *LOG_FILE.lock().unwrap() = Some(file);
        }
    }

    match cli.command {
        Commands::Install { packages } => install_cmd(&packages),
        Commands::ShellInit { shell, bind } => {
            let key = bind.map(|opt| opt.unwrap_or("\\ei".to_string()));
            shell_init_cmd(&shell, key.as_deref())
        }
        Commands::ListSchemas => list_schemas_cmd(),
        Commands::CurrentSchema => {
            let _traits = init_rime()?;
            let mut session = Session::new()?;
            let status = session.status()?;
            println!("{}", status.schema_id());
            let _ = session.close();
            finalize();
            Ok(())
        }
        Commands::SetSchema { schema_id } => {
            let _traits = init_rime()?;
            let mut session = Session::new()?;
            session.select_schema(&schema_id)?;
            println!("{}", schema_id);
            let _ = session.close();
            finalize();
            Ok(())
        }
        Commands::Tui => {
            let _traits = init_rime()?;
            let mut session = Session::new()?;
            let result = run_tui(&session);
            let _ = session.close();
            finalize();
            result
        }
        Commands::Stdio => {
            let _traits = init_rime()?;
            let mut session = Session::new()?;
            let result = run_stdio(&session);
            let _ = session.close();
            finalize();
            result
        }
    }
}

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

    #[test]
    fn strip_unhandled_escapes_removes_charset_designation() {
        // fish 的字符集指定 ESC(B；SGR（ESC[92m / ESC[m）必须保留
        let s = strip_unhandled_escapes("\x1b[92mfoo\x1b(B\x1b[m bar");
        assert_eq!(s, "\x1b[92mfoo\x1b[m bar");
    }

    #[test]
    fn strip_unhandled_escapes_keeps_csi_and_osc() {
        // CSI 与 OSC 原样保留，交给 ansi-to-tui
        let s = strip_unhandled_escapes("\x1b[31ma\x1b]0;title\x07b");
        assert_eq!(s, "\x1b[31ma\x1b]0;title\x07b");
    }

    #[test]
    fn parse_prompt_handles_fish_charset_escape() {
        // 真实 fish prompt 片段（含多处 ESC(B），不应有 "(B" 残留
        let spans = parse_prompt_spans("\x1b[92mblack_desk\x1b(B\x1b[m@\x1b[32m~/D\x1b(B\x1b[m> ");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "black_desk@~/D> ");
        assert!(!text.contains("(B"), "no (B leak");
    }
}

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
