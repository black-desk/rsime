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
    output=$(RSIME_READLINE_LINE="$READLINE_LINE" RSIME_READLINE_POINT="$READLINE_POINT" rsime tui)
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
    local output
    output=$(RSIME_READLINE_LINE="$BUFFER" RSIME_READLINE_POINT="$CURSOR" rsime tui)
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
bind {fish_key} 'RSIME_READLINE_LINE=(commandline) RSIME_READLINE_POINT=(commandline --cursor) rsime tui | read -l output; and commandline --insert "$output"'"##);
        }
        _ => bail!("unsupported shell for keybinding: {shell}"),
    }
    Ok(())
}

fn shell_init_cmd(shell: &str, bind_key: Option<&str>) -> Result<()> {
    use clap_complete::Shell;

    let sh = shell.parse::<Shell>().map_err(|_| anyhow::anyhow!("unsupported shell: {shell}"))?;

    clap_complete::generate(sh, &mut Cli::command(), "rsime", &mut std::io::stdout());

    if let Some(key) = bind_key {
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
fn read_shell_context() -> Option<(String, usize)> {
    let line = std::env::var("RSIME_READLINE_LINE").ok()?;
    if line.is_empty() {
        return None;
    }
    let point = std::env::var("RSIME_READLINE_POINT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(line.chars().count());
    Some((line, point))
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
    let result = tui_loop(session, &mut terminal, &mut output, &mut cursor, &shell_ctx);

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
) -> Result<()> {
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

            let cand_text = candidates.join("  ");
            let cand_line = Paragraph::new(cand_text).style(Style::default());

            f.render_widget(comp_line, Rect { y: area.y, height: 1, ..area });
            f.render_widget(cand_line, Rect { y: area.y + 1, height: 1, ..area });
        })?;

        let char_count = output.chars().count();
        let ev = event::read()?;
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
