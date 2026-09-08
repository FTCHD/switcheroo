//! Embedded HTTP server: JSON API + SSE + the embedded web bundle, loopback only.
//! `serve(core, opts)` is used by both `switcheroo serve` and the tray.

mod api;
mod auth;
mod events;
pub mod port;
mod static_files;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use axum::Router;
use axum::routing::{get, patch, post};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::core::Core;
use crate::core::fsutil::{read_opt, remove_opt, write_atomic};

#[derive(Clone, Debug, Default)]
pub struct ServeOpts {
    pub bind: Option<String>,
    /// Fixed session token `dev` so a Vite dev server can proxy `/api`.
    pub dev: bool,
    pub open: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<Core>,
    pub session: Arc<String>,
    pub dev: bool,
    pub port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub pid: u32,
    pub port: u16,
    pub url: String,
}

/// Bind, write `server.json`, and return the address plus the future that serves until
/// `shutdown` resolves.
pub async fn serve(
    core: Arc<Core>,
    opts: ServeOpts,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(ServerInfo, impl std::future::Future<Output = Result<()>>)> {
    let bind = opts.bind.clone().or_else(|| core.settings.read().ok().and_then(|s| s.bind.clone()));
    let addr: SocketAddr = match bind {
        Some(b) => b.parse().with_context(|| format!("invalid bind address {b}"))?,
        None => SocketAddr::from(([127, 0, 0, 1], port::deterministic_port(&core.dirs.data))),
    };
    if !addr.ip().is_loopback() {
        bail!("refusing to listen on {addr}: Switcheroo only serves loopback addresses");
    }
    let listener = match port::bind_with_retry(addr, 6, Duration::from_millis(250)).await {
        Ok(l) => l,
        Err(e) if opts.bind.is_none() => {
            log::warn!("{e}; falling back to an ephemeral port");
            TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?
        }
        Err(e) => return Err(e),
    };
    let local = listener.local_addr()?;
    let session = if opts.dev { "dev".to_string() } else { auth::new_token() };
    let state = AppState { core: core.clone(), session: Arc::new(session), dev: opts.dev, port: local.port() };

    let app = Router::new()
        .route("/api/status", get(api::status))
        .route("/api/providers", get(api::providers))
        .route("/api/providers/{id}", get(api::provider))
        .route("/api/providers/{id}/save", post(api::save))
        .route("/api/providers/{id}/use", post(api::use_account))
        .route("/api/providers/{id}/login", post(api::login))
        .route("/api/providers/{id}/refresh", post(api::refresh))
        .route("/api/accounts/{provider}/{account}", patch(api::rename).delete(api::remove))
        .route("/api/settings", get(api::get_settings).put(api::put_settings))
        .route("/api/doctor", get(api::doctor))
        .route("/api/events", get(events::sse))
        .fallback(static_files::serve)
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::guard))
        .with_state(state);

    let info =
        ServerInfo { pid: std::process::id(), port: local.port(), url: format!("http://127.0.0.1:{}", local.port()) };
    write_atomic(&core.dirs.server_file(), &serde_json::to_vec_pretty(&info)?, 0o600)?;
    spawn_state_watcher(core.clone());

    let server_file = core.dirs.server_file();
    let fut = async move {
        let r = axum::serve(listener, app).with_graceful_shutdown(shutdown).await;
        let _ = remove_opt(&server_file);
        r.context("http server")
    };
    Ok((info, fut))
}

/// Re-read `state.json` when another process (the CLI) writes it, so SSE/tray stay current.
fn spawn_state_watcher(core: Arc<Core>) {
    use notify::{Event, RecursiveMode, Watcher};
    let dir = core.dirs.data.clone();
    let state_file = core.dirs.state_file();
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("state watcher unavailable: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
            log::warn!("state watcher unavailable: {e}");
            return;
        }
        while let Ok(ev) = rx.recv() {
            let touched = match &ev {
                Ok(ev) => ev.paths.iter().any(|p| p == &state_file),
                Err(_) => true,
            };
            if !touched {
                continue;
            }
            // coalesce bursts (tmp + rename)
            std::thread::sleep(Duration::from_millis(150));
            while rx.try_recv().is_ok() {}
            if let Err(e) = core.reload_state() {
                log::warn!("reloading state: {e:#}");
            }
        }
    });
}

pub fn run_blocking(core: Arc<Core>, opts: ServeOpts) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let shutdown = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        let (info, fut) = serve(core, opts.clone(), shutdown).await?;
        eprintln!("Switcheroo web UI: {}", info.url);
        if opts.dev {
            eprintln!("dev mode: session token is \"dev\"; run `npm run dev` in web/ and open the Vite URL");
        }
        if opts.open {
            let _ = open::that(&info.url);
        }
        fut.await
    })
}

/// Read `server.json` and return it if that server is actually listening.
pub fn running_server(core: &Core) -> Option<ServerInfo> {
    let bytes = read_opt(&core.dirs.server_file()).ok()??;
    let info: ServerInfo = serde_json::from_slice(&bytes).ok()?;
    let addr = SocketAddr::from(([127, 0, 0, 1], info.port));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)).ok().map(|_| info)
}

/// Open the web UI, starting a detached `serve` if nothing is listening.
pub fn open_ui(core: Arc<Core>) -> Result<()> {
    if let Some(info) = running_server(&core) {
        open::that(&info.url)?;
        eprintln!("Opened {}", info.url);
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("serve").arg("--data-dir").arg(&core.dirs.data);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().context("starting the server")?;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(250));
        if let Some(info) = running_server(&core) {
            open::that(&info.url)?;
            eprintln!("Started the server and opened {}", info.url);
            return Ok(());
        }
    }
    bail!("the server did not start within 10 seconds; run `switcheroo serve` to see why")
}
