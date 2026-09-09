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
    /// Usage and remaining quota for the signed-in account of each CLI that reports it.
    Usage {
        provider: Option<String>,
        /// Ask the service again instead of using the cached answer (cached for a minute).
        #[arg(long)]
        refresh: bool,
    },
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
    /// Run the system tray (menu bar) quick switcher with the web UI. Detaches into the background.
    Tray {
        /// Stay attached to this terminal (what the detached child and the autostart entry run).
        #[arg(long)]
        foreground: bool,
        /// Stop the background tray.
        #[arg(long)]
        stop: bool,
    },
    /// Open the web UI in the browser, starting the server if needed.
    Open,
    /// Install the latest release over this binary (restarts the tray if it is running).
    Update {
        /// Only report whether a newer release exists.
        #[arg(long)]
        check: bool,
    },
    /// Start the tray automatically when you log in.
    Autostart {
        #[command(subcommand)]
        action: Option<AutostartAction>,
    },
    /// Print shell completions.
    Completions { shell: clap_complete::Shell },
}

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum AutostartAction {
    /// Register `switcheroo tray` to start at login.
    Enable,
    /// Remove the registration.
    Disable,
    /// Show whether it is registered and where.
    Status,
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
    // Long-running or self-referential commands do not get the update nudge appended.
    let nudge = !json
        && !matches!(
            cli.command,
            Some(Command::Serve { .. })
                | Some(Command::Tray { .. })
                | Some(Command::Update { .. })
                | Some(Command::Open)
        );
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
            let infos: Vec<_> = core.providers.iter().map(|p| p.info()).collect();
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
                bail!("canceled");
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
        Command::Usage { provider, refresh } => {
            let selected: Vec<&dyn crate::providers::Provider> = match &provider {
                Some(p) => vec![core.provider(p)?],
                None => core.providers.iter().map(|p| p.as_ref()).filter(|p| p.supports_usage()).collect(),
            };
            let rows: Vec<(String, anyhow::Result<Option<crate::core::model::Usage>>)> =
                selected.iter().map(|p| (p.meta().name.to_string(), core.usage(*p, refresh))).collect();
            if json {
                let out: Vec<serde_json::Value> = selected
                    .iter()
                    .zip(&rows)
                    .map(|(p, (_, r))| match r {
                        Ok(u) => serde_json::json!({ "provider": p.meta().id, "usage": u }),
                        Err(e) => serde_json::json!({ "provider": p.meta().id, "error": format!("{e:#}") }),
                    })
                    .collect();
                output::json(&out)?;
            } else {
                output::usage_table(&rows);
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
            crate::server::run_blocking(core.clone(), crate::server::ServeOpts { bind, dev, open })?;
        }
        Command::Tray { foreground, stop } => {
            if stop {
                match crate::tray::stop(&core)? {
                    Some(pid) => println!("Stopped the tray (pid {pid})."),
                    None => println!("The tray is not running."),
                }
            } else if foreground {
                let core = core_for_gui(cli.data_dir.clone(), cli.vault)?;
                crate::tray::run(core)?;
            } else {
                let vault = cli.vault.map(|v| match v {
                    VaultChoice::Auto => "auto",
                    VaultChoice::Keychain => "keychain",
                    VaultChoice::File => "file",
                });
                match crate::tray::spawn_detached(&core, vault)? {
                    crate::tray::Started::AlreadyRunning(info) => {
                        println!("Switcheroo is already running in the background (pid {}).", info.pid);
                        println!("Web UI: {}", info.url);
                    }
                    crate::tray::Started::Spawned(info) => {
                        println!("Switcheroo is running in the background (pid {}).", info.pid);
                        println!("Web UI: {}", info.url);
                        println!(
                            "Stop it with `switcheroo tray --stop`; logs in {}",
                            core.dirs.data.join("tray.log").display()
                        );
                    }
                }
            }
        }
        Command::Open => {
            crate::server::open_ui(core.clone()).context("opening the web UI")?;
        }
        Command::Update { check } => {
            if check {
                let info = core.update_status(true)?;
                if json {
                    output::json(&info)?;
                } else if info.available {
                    println!(
                        "Update available: v{} (you have v{}). Run `switcheroo update`.",
                        info.latest, info.current
                    );
                } else {
                    println!("Up to date (v{}).", info.current);
                }
            } else {
                let was_running = crate::server::running_server(&core).filter(|i| crate::core::proc::pid_alive(i.pid));
                let installed = core.install_update()?;
                if json {
                    output::json(&installed)?;
                } else {
                    println!("Installed Switcheroo v{} at {}", installed.version, installed.path.display());
                }
                if was_running.is_some() {
                    let _ = crate::tray::stop(&core);
                    match crate::tray::spawn_detached(&core, None) {
                        Ok(_) if !json => println!("Restarted the tray on the new version."),
                        Err(e) if !json => println!("The tray was stopped but could not be restarted: {e:#}"),
                        _ => {}
                    }
                }
            }
        }
        Command::Autostart { action } => {
            let st = match action.unwrap_or(AutostartAction::Status) {
                AutostartAction::Enable => core.set_autostart(true)?,
                AutostartAction::Disable => core.set_autostart(false)?,
                AutostartAction::Status => core.autostart()?,
            };
            if json {
                output::json(&st)?;
            } else {
                println!("Start at login: {}", if st.enabled { "enabled" } else { "disabled" });
                println!("location: {}", st.location);
                println!("command:  {}", st.command.join(" "));
                if st.enabled {
                    println!("The tray starts at your next login. Run `switcheroo tray` to start it now.");
                }
            }
        }
        Command::Completions { .. } => unreachable!(),
    }
    if nudge
        && let Ok(info) = core.update_status(false)
        && info.available
    {
        eprintln!(
            "\nUpdate available: Switcheroo v{} (you have v{}). Run `switcheroo update`.",
            info.latest, info.current
        );
    }
    Ok(EXIT_OK)
}

/// Shared by `serve`/`tray`: a Core whose PATH includes the login shell's (GUI launches).
pub fn core_for_gui(data_dir: Option<PathBuf>, vault: Option<VaultChoice>) -> Result<Arc<Core>> {
    let mut cx = crate::core::Cx::from_process()?;
    cx.adopt_login_shell_path();
    Core::open(CoreOpts { data_dir, vault, cx: Some(cx), providers: None })
}
