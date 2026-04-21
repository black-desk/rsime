use std::fs::{self, File};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{bail, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rime_api::{
    create_session, deploy_on_changed, finalize, get_schema_list, initialize,
    set_notification_handler, setup, DeployResult, Session, Traits,
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
    /// Pinyin key sequence to convert
    key_sequence: Option<String>,

    /// Select input schema by ID
    #[arg(short, long)]
    schema: Option<String>,

    /// Write log output to FILE
    #[arg(short, long)]
    log: Option<String>,

    /// Show candidates instead of auto-selecting
    #[arg(short, long)]
    pick: bool,

    /// List available input schemas
    #[arg(long)]
    list_schemas: bool,

    /// Show current input schema
    #[arg(long)]
    current_schema: bool,

    /// Install RIME input schemas via plum (no local plum needed)
    #[arg(long, num_args = 0..)]
    install: Option<Vec<String>>,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Interactive TUI mode for composing Chinese text
    #[arg(long)]
    tui: bool,
}

// JSON output types

#[derive(Serialize)]
struct JsonSchema {
    schema_id: String,
    name: String,
}

#[derive(Serialize)]
struct JsonCurrentSchema {
    schema_id: String,
}

#[derive(Serialize)]
struct JsonConversion {
    output: String,
}

#[derive(Serialize)]
struct JsonCandidate {
    text: String,
    comment: Option<String>,
}

#[derive(Serialize)]
struct JsonCandidates {
    candidates: Vec<JsonCandidate>,
}

fn print_json(value: &impl Serialize) {
    println!("{}", serde_json::to_string(value).unwrap());
}

fn init_rime() -> Result<Traits> {
    let mut traits = Traits::new();
    traits.set_app_name("rime.console");
    let shared_data_dir =
        std::env::var("RIME_SHARED_DATA_DIR").unwrap_or_else(|_| "third_party/librime/data/minimal".to_string());
    let user_data_dir = std::env::var("RIME_USER_DATA_DIR")
        .unwrap_or_else(|_| default_user_data_dir().to_string_lossy().to_string());
    traits.set_shared_data_dir(&shared_data_dir);
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

fn list_schemas_cmd(json: bool) -> Result<()> {
    let _traits = init_rime()?;
    let schemas = get_schema_list();
    if json {
        let out: Vec<JsonSchema> = schemas
            .iter()
            .map(|s| JsonSchema {
                schema_id: s.schema_id.clone(),
                name: s.name.clone(),
            })
            .collect();
        print_json(&out);
    } else {
        for s in &schemas {
            println!("{}\t{}", s.schema_id, s.name);
        }
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

fn get_candidates(session: &Session) -> Vec<JsonCandidate> {
    let Some(ctx) = session.context() else {
        return Vec::new();
    };
    let menu = ctx.menu();
    menu.candidates
        .iter()
        .map(|c| JsonCandidate {
            text: c.text.to_string(),
            comment: c.comment.map(String::from),
        })
        .collect()
}

fn show_candidates(session: &Session, json: bool) {
    let candidates = get_candidates(session);
    if json {
        print_json(&JsonCandidates { candidates });
    } else {
        for (i, cand) in candidates.iter().enumerate() {
            let comment = cand.comment.as_deref().unwrap_or("");
            println!("{}. {}{}", i + 1, cand.text, comment);
        }
    }
}

fn convert(session: &Session, key_sequence: &str, pick: bool, json: bool) -> Result<()> {
    session.simulate_key_sequence(key_sequence)?;

    let mut output = String::new();
    loop {
        while let Some(commit) = session.commit() {
            output.push_str(commit.text());
        }
        let composing = session
            .context()
            .map(|ctx| ctx.composition().length > 0)
            .unwrap_or(false);
        if !composing {
            break;
        }
        if pick {
            show_candidates(session, json);
            return Ok(());
        } else {
            session.simulate_key_sequence(" ")?;
        }
    }
    if json {
        print_json(&JsonConversion { output });
    } else {
        println!("{}", output);
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
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(tty_fd, &mut termios) } == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    let saved_termios = termios;
    unsafe { libc::cfmakeraw(&mut termios) };
    if unsafe { libc::tcsetattr(tty_fd, libc::TCSANOW, &termios) } == -1 {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut tty = tty;
    crossterm::execute!(tty, EnterAlternateScreen, terminal::Clear(terminal::ClearType::All))?;
    let backend = CrosstermBackend::new(tty);
    let mut terminal = Terminal::new(backend)?;

    let mut output = String::new();

    let result = tui_loop(session, &mut terminal, &mut output);

    // Restore terminal
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    // Restore original terminal settings
    unsafe { libc::tcsetattr(tty_fd, libc::TCSANOW, &saved_termios) };

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

        while let Some(commit) = session.commit() {
            output.push_str(commit.text());
        }

        terminal.draw(|f| {
            let area = f.area();
            let rows = Rect { y: area.height.saturating_sub(2), height: 2, ..area };

            let comp_line = if preedit.is_empty() && candidates.is_empty() {
                Paragraph::new("Type pinyin, Esc to finish").dim()
            } else {
                Paragraph::new(preedit.clone())
            };

            let cand_text = candidates.join("  ");
            let cand_line = Paragraph::new(cand_text).style(Style::default());

            f.render_widget(comp_line, Rect { height: 1, ..rows });
            f.render_widget(cand_line, Rect { y: rows.y + 1, height: 1, ..rows });
        })?;

        let ev = event::read()?;
        match ev {
            Event::Key(key) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                let rime_key = match key.code {
                    KeyCode::Char(c) => Some(rime_api::KeyEvent { key_code: c as i32, modifiers: 0 }),
                    KeyCode::Backspace => Some(rime_api::KeyEvent { key_code: 65288, modifiers: 0 }),
                    KeyCode::Enter => Some(rime_api::KeyEvent { key_code: 65293, modifiers: 0 }),
                    KeyCode::Esc => None,
                    KeyCode::Up => Some(rime_api::KeyEvent { key_code: 65362, modifiers: 0 }),
                    KeyCode::Down | KeyCode::Tab => Some(rime_api::KeyEvent { key_code: 65364, modifiers: 0 }),
                    KeyCode::PageUp => Some(rime_api::KeyEvent { key_code: 65365, modifiers: 0 }),
                    KeyCode::PageDown => Some(rime_api::KeyEvent { key_code: 65366, modifiers: 0 }),
                    _ => None,
                };

                match rime_key {
                    None => break,
                    Some(rk) => {
                        session.process_key(rk);
                    }
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

    if let Some(packages) = &cli.install {
        return install_cmd(packages);
    }

    if cli.list_schemas {
        return list_schemas_cmd(cli.json);
    }

    if cli.current_schema {
        let _traits = init_rime()?;
        let session = create_session()?;
        let status = session.status()?;
        if cli.json {
            print_json(&JsonCurrentSchema {
                schema_id: status.schema_id().to_string(),
            });
        } else {
            println!("{}", status.schema_id());
        }
        return Ok(());
    }

    if let Some(path) = &cli.log {
        let file = File::create(path)?;
        *LOG_FILE.lock().unwrap() = Some(file);
    }

    let _traits = init_rime()?;

    let mut session = create_session()?;

    if let Some(schema) = &cli.schema {
        session.select_schema(schema)?;
    }

    if cli.tui {
        let result = run_tui(&session);
        let _ = session.close();
        finalize();
        return result;
    }

    match cli.key_sequence {
        Some(ref seq) => convert(&session, seq, cli.pick, cli.json)?,
        None => bail!("no key sequence provided (run with --help for usage)"),
    }

    let _ = session.close();
    finalize();
    Ok(())
}
