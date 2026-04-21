use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{bail, Result};
use clap::Parser;
use rime_api::{
    create_session, deploy_on_changed, finalize, get_schema_list, initialize,
    set_notification_handler, setup, DeployResult, Session, Traits,
};

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

fn convert(session: &Session, key_sequence: &str, pick: bool) -> Result<()> {
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
    println!("{}", output);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(packages) = &cli.install {
        return install_cmd(packages);
    }

    if cli.list_schemas {
        return list_schemas_cmd();
    }

    if cli.current_schema {
        let _traits = init_rime()?;
        let session = create_session()?;
        let status = session.status()?;
        println!("{}", status.schema_id());
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

    match cli.key_sequence {
        Some(ref seq) => convert(&session, seq, cli.pick)?,
        None => bail!("no key sequence provided (run with --help for usage)"),
    }

    let _ = session.close();
    finalize();
    Ok(())
}
