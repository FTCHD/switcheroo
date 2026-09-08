//! Command-line surface. Every command maps onto one `Core` method; output is a table or,
//! with `--json`, the same structs the HTTP API returns.

mod output;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand};

use crate::core::settings::VaultChoice;
use crate::core::switch::OpError;
use crate::core::{Core, CoreOpts};

pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_NOT_INSTALLED: i32 = 3;
pub const EXIT_NOTHING_LOGGED_IN: i32 = 4;

#[derive(Parser, Debug)]
#[command(
    name = "switcheroo",
    version,
    about = "Switch the logged-in account of developer CLIs",
    long_about = "Switcheroo captures the live login of CLIs like Claude Code, Codex, Vercel, Wrangler, \
                  npm or Fly into your OS credential store and restores any saved one with a single \
                  command. CLIs with their own account registry (GitHub CLI, Netlify) are driven natively.\n\n\
                  Typical flow:\n  switcheroo save claude-code          # remember the current login\n  \
                  switcheroo login claude-code         # log in as another account, remember it too\n  \
                  switcheroo use claude-code work@x.io # switch back and forth\n  switcheroo tray                      # menu-bar quick switcher + web UI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Machine-readable JSON output.
    #[arg(long, global = true, env = "SWITCHEROO_JSON")]
    pub json: bool,

    /// Where Switcheroo keeps its state (default: the user config dir).
    #[arg(long, global = true, env = "SWITCHEROO_DATA_DIR", value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Where secrets are stored: the OS credential store or a 0600 file.
    #[arg(long, global = true, env = "SWITCHEROO_VAULT", value_enum)]
    pub vault: Option<VaultChoice>,

    /// More log output (repeatable).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Only errors.
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show every detected provider, who is logged in, and the saved accounts (default).
    Status {
        /// Re-run the CLIs' whoami instead of using the cached identity.
        #[arg(long)]
        refresh: bool,
    },
    /// Describe all providers, including unsupported ones and what they touch.
    Providers,
    /// List saved accounts, optionally for one provider.
    List { provider: Option<String> },
    /// Save the provider's current login into the vault.
    Save {
        provider: String,
        /// A friendly name for this account (default: the email/username).
        #[arg(long, short)]
        label: Option<String>,
    },
    /// Switch the provider's live login to a saved account.
    Use {
        provider: String,
        /// Account id, email, label, or a unique part of one. Omit for an interactive picker.
        account: Option<String>,
    },
    /// Run the provider's own login here, then save the result.
    Login {
        provider: String,
        #[arg(long, short)]
        label: Option<String>,
    },
    /// Forget a saved account (and delete its stored credential).
    Remove {
        provider: String,
        account: String,
        /// Do not ask for confirmation.
        #[arg(long, short)]
        yes: bool,
    },
    /// Change a saved account's label.
    Rename { provider: String, account: String, label: String },
    /// Diagnostics: detection, vault health, shadowing env vars, running CLIs.
    Doctor,
    /// Run the web UI server (loopback only).
    Serve {
        /// host:port to listen on (loopback only). Default: a stable port derived from the data dir.
        #[arg(long, env = "SWITCHEROO_BIND")]
        bind: Option<String>,
        /// Development mode: fixed session token "dev" so a Vite dev server can proxy /api.
        #[arg(long)]
        dev: bool,
        /// Open the browser once listening.
        #[arg(long)]
        open: bool,
    },
    /// Run the system tray (menu bar) quick switcher together with the web UI server.
    Tray,
    /// Open the web UI in the browser, starting the server if needed.
    Open,
    /// Print shell completions.
    Completions { shell: clap_complete::Shell },
}

pub fn run(cli: Cli) -> i32 {
    let level = if cli.quiet {
        "error"
    } else {
        match cli.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).format_timestamp(None).init();

    match dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            match e.downcast_ref::<OpError>() {
                Some(OpError::NotInstalled(..)) => EXIT_NOT_INSTALLED,
                Some(OpError::NothingLoggedIn(..)) => EXIT_NOTHING_LOGGED_IN,
                None => EXIT_ERROR,
            }
        }
    }
}

fn dispatch(cli: Cli) -> Result<i32> {
    if let Some(Command::Completions { shell }) = cli.command {
        clap_complete::generate(shell, &mut Cli::command(), "switcheroo", &mut std::io::stdout());
        return Ok(EXIT_OK);
    }
    let json = cli.json;
    let command = cli.command.unwrap_or(Command::Status { refresh: false });
    let core = Core::open(CoreOpts { data_dir: cli.data_dir.clone(), vault: cli.vault, ..Default::default() })?;
    match command {
        Command::Status { refresh } => {
            let statuses = core.status_all(refresh);
            if json {
                output::json(&statuses)?;
            } else {
                output::status_table(&core, &statuses);
            }
        }
        Command::Providers => {
            let infos: Vec<_> = core.providers.iter().map(|p| p.meta().info()).collect();
            if json {
                output::json(&infos)?;
            } else {
                output::providers_table(&infos);
            }
        }
        Command::List { provider } => {
            let statuses: Vec<_> = match provider {
                Some(p) => vec![core.status(core.provider(&p)?, false)],
                None => core.status_all(false),
            };
            if json {
                let accounts: Vec<_> = statuses.iter().flat_map(|s| s.accounts.clone()).collect();
                output::json(&accounts)?;
            } else {
                output::accounts_table(&statuses);
            }
        }
        Command::Save { provider, label } => {
            let acct = core.save(&provider, label)?;
            if json {
                output::json(&acct)?;
            } else {
                println!("Saved {} → {} ({})", acct.provider, acct.label, acct.id);
            }
        }
        Command::Use { provider, account } => {
            let p = core.provider(&provider)?;
            let account = match account {
                Some(a) => a,
                None => output::pick_account(&core, p)?,
            };
            let out = core.use_account(&provider, &account)?;
            if json {
                output::json(&out)?;
            } else {
                println!("{} → {} ({})", p.meta().name, out.account.label, out.account.id);
                if let Some(prev) = &out.recaptured {
                    println!("  saved the previous login as {prev}");
                }
                output::print_warnings(&out.warnings);
            }
        }
        Command::Login { provider, label } => {
            let p = core.provider(&provider)?;
            let argv = p.login_command(&core.cx);
            if !json {
                eprintln!("Running: {}", argv.join(" "));
            }
            let acct = core.login_here(&provider, label)?;
            if json {
                output::json(&acct)?;
            } else {
                println!("Saved {} → {} ({})", acct.provider, acct.label, acct.id);
            }
        }
        Command::Remove { provider, account, yes } => {
            let p = core.provider(&provider)?;
            let target = core.resolve_account(p, &account)?;
            if !yes
                && !json
                && !output::confirm(&format!("Forget {} account {} ({})?", p.meta().name, target.label, target.id))?
            {
                bail!("cancelled");
            }
            let acct = core.remove(&provider, &target.id)?;
            if json {
                output::json(&acct)?;
            } else {
                println!("Removed {} → {}", acct.provider, acct.id);
            }
        }
        Command::Rename { provider, account, label } => {
            let acct = core.rename(&provider, &account, &label)?;
            if json {
                output::json(&acct)?;
            } else {
                println!("{} → {} is now \"{}\"", acct.provider, acct.id, acct.label);
            }
        }
        Command::Doctor => {
            let d = core.doctor();
            if json {
                output::json(&d)?;
            } else {
                output::doctor(&d);
            }
        }
        Command::Serve { bind, dev, open } => {
            let core = core_for_gui(cli.data_dir.clone(), cli.vault)?;
            crate::server::run_blocking(core, crate::server::ServeOpts { bind, dev, open })?;
        }
        Command::Tray => {
            let core = core_for_gui(cli.data_dir.clone(), cli.vault)?;
            crate::tray::run(core)?;
        }
        Command::Open => {
            crate::server::open_ui(core).context("opening the web UI")?;
        }
        Command::Completions { .. } => unreachable!(),
    }
    Ok(EXIT_OK)
}

/// Shared by `serve`/`tray`: a Core whose PATH includes the login shell's (GUI launches).
pub fn core_for_gui(data_dir: Option<PathBuf>, vault: Option<VaultChoice>) -> Result<Arc<Core>> {
    let mut cx = crate::core::Cx::from_process()?;
    cx.adopt_login_shell_path();
    Core::open(CoreOpts { data_dir, vault, cx: Some(cx), providers: None })
}
