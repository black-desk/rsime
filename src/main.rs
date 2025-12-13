use std::fs::File;
use std::io::{stdin, BufRead, Write};
use std::sync::Mutex;

use anyhow::Result;
use clap::Parser;
use rime_api::{
    create_session, finalize, full_deploy_and_wait, initialize, set_notification_handler, setup,
    DeployResult, Session, Traits,
};

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

fn log(msg: &str) {
    let Ok(mut guard) = LOG_FILE.lock() else { return };
    let Some(file) = guard.as_mut() else { return };
    let _ = writeln!(file, "{}", msg);
}

#[derive(Parser)]
#[command(name = "rsime", about = "Emergency Chinese input for TUI")]
struct Cli {
    /// Pinyin key sequence to convert (omit for interactive mode)
    key_sequence: Option<String>,

    /// Select input schema by ID
    #[arg(short, long)]
    schema: Option<String>,

    /// Write log output to FILE
    #[arg(short, long)]
    log: Option<String>,

    /// Show candidates for selection when input doesn't end with a digit
    #[arg(short, long)]
    pick: bool,
}

fn init_rime() -> Result<Traits> {
    let mut traits = Traits::new();
    traits.set_app_name("rime.console");
    let shared_data_dir =
        std::env::var("RIME_SHARED_DATA_DIR").unwrap_or_else(|_| "third_party/librime/data/minimal".to_string());
    let user_data_dir =
        std::env::var("RIME_USER_DATA_DIR").unwrap_or_else(|_| "/tmp/rime-user".to_string());
    traits.set_shared_data_dir(&shared_data_dir);
    traits.set_user_data_dir(&user_data_dir);
    setup(&mut traits);
    initialize(&mut traits);

    set_notification_handler(|t, v| {
        log(&format!("[{}] {}", t, v));
    });

    log("initializing...");
    match full_deploy_and_wait() {
        DeployResult::Success => {}
        DeployResult::Failure => {
            log("deployment failed");
        }
    }
    log("ready.");
    Ok(traits)
}

fn show_candidates(session: &Session) {
    let Some(ctx) = session.context() else {
        return;
    };
    let menu = ctx.menu();
    for (i, cand) in menu.candidates.iter().enumerate() {
        eprintln!(
            "{}. {}{}",
            i + 1,
            cand.text,
            cand.comment.unwrap_or("")
        );
    }
}

fn cli_mode(session: &Session, key_sequence: &str, pick: bool) -> Result<()> {
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
            show_candidates(session);
            return Ok(());
        } else {
            session.simulate_key_sequence(" ")?;
        }
    }
    print!("{}", output);
    Ok(())
}

fn interactive_mode(session: &mut Session) -> Result<()> {
    let stdin = stdin();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        let line = if line.is_empty() { "\r" } else { &line };

        if !session.find_session() {
            *session = create_session()?;
        }

        if line == "exit" {
            break;
        }
        if line == "reload" {
            let _ = session.close();
            finalize();
            log("initializing...");
            let mut traits = Traits::new();
            traits.set_app_name("rime.console");
            setup(&mut traits);
            initialize(&mut traits);
            match full_deploy_and_wait() {
                DeployResult::Success => {}
                DeployResult::Failure => {
                    log("deployment failed");
                }
            }
            log("ready.");
            *session = create_session()?;
            continue;
        }

        match session.simulate_key_sequence(line) {
            Ok(()) => print_session(session),
            Err(_) => log(&format!("Error processing key sequence: {}", line)),
        }
    }
    Ok(())
}

fn print_status(session: &Session) {
    let status = match session.status() {
        Ok(s) => s,
        Err(_) => return,
    };
    println!("schema: {} / {}", status.schema_id(), status.schema_name());
    print!("status: ");
    if status.is_disabled {
        print!("disabled ");
    }
    if status.is_composing {
        print!("composing ");
    }
    if status.is_ascii_mode {
        print!("ascii ");
    }
    if status.is_full_shape {
        print!("full_shape ");
    }
    if status.is_simplified {
        print!("simplified ");
    }
    println!();
}

fn print_composition(ctx: &rime_api::Context) {
    let comp = ctx.composition();
    let preedit = match comp.preedit {
        Some(p) => p,
        None => return,
    };
    let bytes = preedit.as_bytes();
    let start = comp.sel_start;
    let end = comp.sel_end;
    let cursor = comp.cursor_pos;
    let mut i = 0;
    let mut char_idx = 0;
    while i <= bytes.len() {
        if start < end {
            if char_idx == start {
                print!("[");
            } else if char_idx == end {
                print!("]");
            }
        }
        if char_idx == cursor {
            print!("|");
        }
        if i < bytes.len() {
            let ch = &preedit[i..];
            let ch_len = ch.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            print!("{}", &preedit[i..i + ch_len]);
            i += ch_len;
            char_idx += 1;
        } else {
            break;
        }
    }
    println!();
}

fn print_menu(ctx: &rime_api::Context) {
    let menu = ctx.menu();
    if menu.num_candidates == 0 {
        return;
    }
    println!(
        "page: {}{} (of size {})",
        menu.page_no + 1,
        if menu.is_last_page { "$" } else { " " },
        menu.page_size
    );
    for (i, cand) in menu.candidates.iter().enumerate() {
        let highlighted = i == menu.highlighted_candidate_index;
        print!(
            "{}. {}{}{}{}\n",
            i + 1,
            if highlighted { "[" } else { " " },
            cand.text,
            if highlighted { "]" } else { " " },
            cand.comment.unwrap_or("")
        );
    }
}

fn print_context(session: &Session) {
    if let Some(ctx) = session.context() {
        let comp = ctx.composition();
        if comp.length > 0 {
            print_composition(&ctx);
        } else {
            println!("(not composing)");
        }
        print_menu(&ctx);
    }
}

fn print_session(session: &Session) {
    if let Some(commit) = session.commit() {
        println!("commit: {}", commit.text());
    }
    print_status(session);
    print_context(session);
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(path) = &cli.log {
        let file = File::create(path)?;
        *LOG_FILE.lock().unwrap() = Some(file);
    }

    let _traits = init_rime()?;

    let mut session = create_session()?;

    if let Some(schema) = &cli.schema {
        session.select_schema(schema)?;
    }

    match cli.key_sequence {
        Some(ref seq) => cli_mode(&session, seq, cli.pick)?,
        None => interactive_mode(&mut session)?,
    }

    let _ = session.close();
    finalize();
    Ok(())
}
