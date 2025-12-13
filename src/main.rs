use std::fs::{self, File};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{bail, Result};
use clap::{CommandFactory, Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::Paragraph;
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};
use rime_api::{
    create_session, deploy_on_changed, finalize, get_schema_list, initialize,
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
#[command(name = "rsime", about = "Chinese input via RIME for TUI")]
struct Cli {
    /// Write log output to FILE
    #[arg(short, long)]
    log: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive TUI mode for composing Chinese text
    Tui,

    /// Stdio mode for editor integration (Vim-style key input, JSONL output)
    Stdio,

    /// Install RIME input schemas via plum (no local plum needed)
    Install {
        /// Schema packages to install (default: :preset)
        packages: Vec<String>,
    },

    /// List available input schemas
    ListSchemas,

    /// Show current input schema
    CurrentSchema,

    /// Set active input schema
    SetSchema {
        /// Schema ID to activate
        schema_id: String,
    },

    /// Output shell init script (completion + optional keybinding via --shell-bind)
    ShellInit {
        /// Shell type (bash, zsh, fish)
        shell: String,

        /// Bind TUI mode to a key (default: \\ei)
        #[arg(long, num_args = 0..=1)]
        bind: Option<Option<String>>,
    },
}

// JSON output types

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

    // Auto-install preset schemas on first run
    if !PathBuf::from(&user_data_dir).exists() {
        eprintln!("No RIME user data found, installing preset schemas...");
        install_cmd(&[])?;
    }

    let mut traits = Traits::new();
    traits.set_app_name("rime.console");
    // Intentionally use the same directory for both shared and user data.
    // This project typically runs without a system-wide RIME installation,
    // so there is no shared data directory available.  Leaving the shared
    // data directory empty would cause RIME to fall back to the current
    // working directory, which would then fail at deployment time.
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
    output=$(rsime tui)
    [[ -n "$output" ]] && READLINE_LINE="${{READLINE_LINE:0:$READLINE_POINT}}$output${{READLINE_LINE:$READLINE_POINT}}"
    READLINE_POINT=$(( READLINE_POINT + ${{#output}} ))
}}
bind -x '"{bind}": rsime-widget'"#);
        }
        Shell::Zsh => {
            // Convert bash-style \C-q to zsh ^Q
            let zsh_key = bind.replace("\\C-", "^").replace("\\e", "^[").replace('\\', "");
            println!(r#"# rsime TUI keybinding
rsime-widget() {{
    local output
    output=$(rsime tui)
    [[ -n "$output" ]] && LBUFFER+="$output"
}}
zle -N rsime-widget
bindkey '{zsh_key}' rsime-widget"#);
        }
        Shell::Fish => {
            // Convert \C-q to \cq
            let fish_key = bind.to_lowercase().replace("\\c-", "\\c");
            println!(r##"# rsime TUI keybinding
bind {fish_key} 'rsime tui | read -l output; and commandline --insert "$output"'"##);
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

/// Parse a Vim-style key notation into a RIME key code.
/// Returns None for `<Esc>` (used as exit signal when not composing).
fn parse_vim_key(input: &str) -> Option<i32> {
    let key = input.trim();
    if key.is_empty() {
        return None;
    }
    // <SpecialKey>
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
    // Single character
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

        // Esc while not composing → exit
        if key_code == KEY_ESCAPE as i32 {
            let composing = session
                .context()
                .map(|ctx| ctx.composition().length > 0)
                .unwrap_or(false);
            if !composing {
                break;
            }
            // Esc while composing: let RIME handle it (cancels composition)
        }

        session.process_key(rime_api::KeyEvent {
            key_code,
            modifiers: 0,
        });

        let mut commit = String::new();
        while let Some(c) = session.commit() {
            commit.push_str(c.text());
        }

        let (preedit, candidates, highlighted) = match session.context() {
            Some(ctx) => {
                let comp = ctx.composition();
                let menu = ctx.menu();
                let preedit = comp.preedit.unwrap_or("").to_string();
                let cands: Vec<JsonCandidate> = menu
                    .candidates
                    .iter()
                    .map(|c| JsonCandidate {
                        text: c.text.to_string(),
                        comment: c.comment.map(String::from),
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

fn run_tui(session: &Session) -> Result<()> {
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;

    // Set raw mode on /dev/tty (not stdin, which may be a pipe inside $())
    let tty_fd = tty.as_raw_fd();

    // Redirect stdout -> /dev/tty so crossterm's cursor position query reaches the terminal
    // (inside $(), fd 1 is a pipe, so the DSR escape would go to the pipe instead of the terminal)
    let saved_stdout = unsafe { libc::dup(1) };
    if saved_stdout == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    unsafe { libc::dup2(tty_fd, 1) };

    let backend = CrosstermBackend::new(tty);
    let terminal_result = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(2),
        },
    );

    // Restore stdout
    unsafe { libc::dup2(saved_stdout, 1) };
    unsafe { libc::close(saved_stdout) };

    let mut terminal = terminal_result?;

    let mut output = String::new();
    let mut cursor: usize = 0;

    let result = tui_loop(session, &mut terminal, &mut output, &mut cursor);

    // Restore terminal
    terminal.clear()?;
    terminal.show_cursor()?;

    result?;

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
) -> Result<()> {
    loop {
        let ctx = session.context();
        let (preedit, candidates, _highlighted) = match &ctx {
            Some(ctx) => {
                let comp = ctx.composition();
                let menu = ctx.menu();
                let preedit = comp.preedit.unwrap_or("").to_string();
                let cands: Vec<String> = menu
                    .candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let sel = if i == menu.highlighted_candidate_index { ">" } else { " " };
                        format!("{}{}.{}", sel, i + 1, c.text)
                    })
                    .collect();
                (preedit, cands, menu.highlighted_candidate_index)
            }
            None => (String::new(), Vec::new(), 0),
        };

        let mut committed = String::new();
        while let Some(commit) = session.commit() {
            committed.push_str(commit.text());
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

            let left: String = output.chars().take(*cursor).collect();
            let right: String = output.chars().skip(*cursor).collect();

            let comp_line = if preedit.is_empty() && candidates.is_empty() && output.is_empty() {
                Paragraph::new("❯ Type pinyin, Esc to finish").dim()
            } else {
                Paragraph::new(format!("❯ {}{}|{}", left, preedit, right))
            };

            let cand_text = candidates.join("  ");
            let cand_line = Paragraph::new(cand_text).style(Style::default());

            f.render_widget(comp_line, Rect { height: 1, ..area });
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
                        session.process_key(rime_api::KeyEvent { key_code: KEY_RETURN as i32, modifiers: 0 });
                    }
                    KeyCode::Backspace if !preedit.is_empty() => {
                        session.process_key(rime_api::KeyEvent { key_code: KEY_BACKSPACE as i32, modifiers: 0 });
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
                        session.process_key(rime_api::KeyEvent { key_code: c as i32, modifiers: 0 });
                    }
                    KeyCode::Up => {
                        session.process_key(rime_api::KeyEvent { key_code: KEY_UP as i32, modifiers: 0 });
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        session.process_key(rime_api::KeyEvent { key_code: KEY_DOWN as i32, modifiers: 0 });
                    }
                    KeyCode::PageUp => {
                        session.process_key(rime_api::KeyEvent { key_code: KEY_PAGEUP as i32, modifiers: 0 });
                    }
                    KeyCode::PageDown => {
                        session.process_key(rime_api::KeyEvent { key_code: KEY_PAGEDOWN as i32, modifiers: 0 });
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
            let mut session = create_session()?;
            let status = session.status()?;
            println!("{}", status.schema_id());
            let _ = session.close();
            finalize();
            Ok(())
        }
        Commands::SetSchema { schema_id } => {
            let _traits = init_rime()?;
            let mut session = create_session()?;
            session.select_schema(&schema_id)?;
            println!("{}", schema_id);
            let _ = session.close();
            finalize();
            Ok(())
        }
        Commands::Tui => {
            let _traits = init_rime()?;
            let mut session = create_session()?;
            let result = run_tui(&session);
            let _ = session.close();
            finalize();
            result
        }
        Commands::Stdio => {
            let _traits = init_rime()?;
            let mut session = create_session()?;
            let result = run_stdio(&session);
            let _ = session.close();
            finalize();
            result
        }
    }
}
