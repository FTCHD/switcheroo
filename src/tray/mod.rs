//! `switcheroo tray`: a menu-bar/tray quick switcher plus the embedded web server. The GUI
//! event loop owns the main thread; a tokio runtime on another thread runs the server; all
//! account operations run on worker threads and report back through the event loop proxy.

mod daemon;
mod icon;
mod menu;
mod platform;

pub use daemon::{Started, spawn_detached, stop};

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::MenuEvent;
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::core::Core;
use crate::core::model::{ProviderStatus, Usage};
use crate::server::{self, ServeOpts};

pub enum UserEvent {
    Menu(MenuEvent),
    /// Fresh provider statuses (computed off the main thread) → rebuild the menu.
    Statuses {
        providers: Vec<ProviderStatus>,
        autostart: bool,
        usage: std::collections::HashMap<String, Usage>,
        update: Option<crate::core::update::UpdateInfo>,
    },
    /// Something changed; recompute statuses.
    Dirty,
    Error(String),
}

pub fn run(core: Arc<Core>) -> Result<()> {
    platform::detach_console_if_gui_launch();

    // The server, on its own runtime thread.
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let (info, server_fut) = rt
        .block_on(server::serve(core.clone(), ServeOpts::default(), std::future::pending()))
        .context("starting the embedded server")?;
    rt.spawn(async move {
        if let Err(e) = server_fut.await {
            log::error!("server stopped: {e:#}");
        }
    });
    log::info!("web UI at {}", info.url);
    let url = info.url.clone();

    // `switcheroo tray --stop` (SIGTERM) and Ctrl-C: drop server.json, then exit.
    {
        let server_file = core.dirs.server_file();
        rt.spawn(async move {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            let _ = crate::core::fsutil::remove_opt(&server_file);
            std::process::exit(0);
        });
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        let mut event_loop = event_loop;
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        run_loop(core, rt, url, event_loop)
    }
    #[cfg(not(target_os = "macos"))]
    {
        run_loop(core, rt, url, event_loop)
    }
}

fn run_loop(
    core: Arc<Core>,
    rt: tokio::runtime::Runtime,
    url: String,
    event_loop: tao::event_loop::EventLoop<UserEvent>,
) -> Result<()> {
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Menu(e));
    }));

    // Bus → Dirty (coalesced by the status recompute).
    {
        let proxy = event_loop.create_proxy();
        let mut rx = core.bus.subscribe();
        rt.spawn(async move {
            use tokio::sync::broadcast::error::RecvError;
            while let Ok(_) | Err(RecvError::Lagged(_)) = rx.recv().await {
                let _ = proxy.send_event(UserEvent::Dirty);
            }
        });
    }
    // Periodic refresh (cheap: cached identities), so tokens refreshed by CLIs are re-read.
    {
        let proxy = event_loop.create_proxy();
        rt.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                let _ = proxy.send_event(UserEvent::Dirty);
            }
        });
    }

    let mut tray: Option<TrayIcon> = None;
    let mut last_error: Option<String> = None;
    let mut computing = false;
    let mut dirty_while_computing = false;
    let proxy = event_loop.create_proxy();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                let menu = menu::loading_menu();
                match TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_tooltip("Switcheroo")
                    .with_icon(icon::tray_icon())
                    .with_icon_as_template(true)
                    .build()
                {
                    Ok(t) => tray = Some(t),
                    Err(e) => {
                        log::error!("tray icon unavailable: {e}; the web UI keeps running at {url}");
                    }
                }
                platform::wake_main_run_loop();
                computing = true;
                spawn_status(core.clone(), proxy.clone());
            }
            Event::UserEvent(UserEvent::Dirty) => {
                if computing {
                    dirty_while_computing = true;
                } else {
                    computing = true;
                    spawn_status(core.clone(), proxy.clone());
                }
            }
            Event::UserEvent(UserEvent::Statuses { providers, autostart, usage, update }) => {
                computing = false;
                let settings = core.settings.read().map(|s| s.clone()).unwrap_or_default();
                let menu =
                    menu::build(&providers, &settings, autostart, &usage, update.as_ref(), last_error.as_deref());
                if let Some(t) = &tray {
                    t.set_menu(Some(Box::new(menu)));
                }
                if dirty_while_computing {
                    dirty_while_computing = false;
                    computing = true;
                    spawn_status(core.clone(), proxy.clone());
                }
            }
            Event::UserEvent(UserEvent::Error(msg)) => {
                log::error!("{msg}");
                last_error = Some(msg);
                let _ = proxy.send_event(UserEvent::Dirty);
            }
            Event::UserEvent(UserEvent::Menu(ev)) => {
                let action = menu::Action::parse(ev.id.as_ref());
                log::debug!("menu action {action:?}");
                match action {
                    menu::Action::Open => {
                        let _ = open::that(&url);
                    }
                    menu::Action::Refresh => {
                        last_error = None;
                        let core = core.clone();
                        let proxy = proxy.clone();
                        std::thread::spawn(move || {
                            let providers = core.status_all(true);
                            let autostart = core.autostart().map(|a| a.enabled).unwrap_or(false);
                            let usage = collect_usage(&core, &providers, true);
                            let update = core.update_status(true).ok().filter(|u| u.available);
                            let _ = proxy.send_event(UserEvent::Statuses { providers, autostart, usage, update });
                        });
                    }
                    menu::Action::Quit => {
                        tray.take();
                        *control_flow = ControlFlow::Exit;
                        std::process::exit(0);
                    }
                    menu::Action::Switch { provider, account } => {
                        last_error = None;
                        let core = core.clone();
                        let proxy = proxy.clone();
                        std::thread::spawn(move || match core.use_account(&provider, &account) {
                            Ok(out) => {
                                for w in
                                    out.warnings.iter().filter(|w| w.severity == crate::core::model::Severity::Warn)
                                {
                                    log::warn!("{}: {}", provider, w.message);
                                }
                                let _ = proxy.send_event(UserEvent::Dirty);
                            }
                            Err(e) => {
                                let _ = proxy.send_event(UserEvent::Error(format!("{provider}: {e:#}")));
                            }
                        });
                    }
                    menu::Action::Save { provider } => {
                        last_error = None;
                        let core = core.clone();
                        let proxy = proxy.clone();
                        std::thread::spawn(move || match core.save(&provider, None) {
                            Ok(_) => {
                                let _ = proxy.send_event(UserEvent::Dirty);
                            }
                            Err(e) => {
                                let _ = proxy.send_event(UserEvent::Error(format!("{provider}: {e:#}")));
                            }
                        });
                    }
                    menu::Action::Login { provider } => {
                        let core = core.clone();
                        let proxy = proxy.clone();
                        std::thread::spawn(move || match core.login_spawn(&provider) {
                            Ok((true, _)) => {}
                            Ok((false, cmd)) => {
                                let _ = proxy
                                    .send_event(UserEvent::Error(format!("could not open a terminal; run: {cmd}")));
                            }
                            Err(e) => {
                                let _ = proxy.send_event(UserEvent::Error(format!("{provider}: {e:#}")));
                            }
                        });
                    }
                    menu::Action::Update => {
                        last_error = None;
                        let core = core.clone();
                        let proxy = proxy.clone();
                        let server_file = core.dirs.server_file();
                        std::thread::spawn(move || match core.install_update() {
                            Ok(_) => {
                                let _ = crate::core::fsutil::remove_opt(&server_file);
                                crate::core::update::relaunch();
                            }
                            Err(e) => {
                                let _ = proxy.send_event(UserEvent::Error(format!("update: {e:#}")));
                            }
                        });
                    }
                    menu::Action::Autostart { enable } => {
                        last_error = None;
                        let core = core.clone();
                        let proxy = proxy.clone();
                        std::thread::spawn(move || match core.set_autostart(enable) {
                            Ok(_) => {
                                let _ = proxy.send_event(UserEvent::Dirty);
                            }
                            Err(e) => {
                                let _ = proxy.send_event(UserEvent::Error(format!("start at login: {e:#}")));
                            }
                        });
                    }
                    menu::Action::None => {}
                }
            }
            _ => {}
        }
    })
}

fn spawn_status(core: Arc<Core>, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let _ = core.reload_state();
        let providers = core.status_all(false);
        let autostart = core.autostart().map(|a| a.enabled).unwrap_or(false);
        let usage = collect_usage(&core, &providers, false);
        let update = core.update_status(false).ok().filter(|u| u.available);
        let _ = proxy.send_event(UserEvent::Statuses { providers, autostart, usage, update });
    });
}

/// Usage for signed-in providers that report it; cached by the core, failures omitted.
fn collect_usage(core: &Core, statuses: &[ProviderStatus], refresh: bool) -> std::collections::HashMap<String, Usage> {
    let mut out = std::collections::HashMap::new();
    for s in statuses {
        if s.live.is_none() || !s.info.supports_usage {
            continue;
        }
        let Ok(p) = core.provider(&s.info.id) else { continue };
        match core.usage(p, refresh) {
            Ok(Some(u)) => {
                out.insert(s.info.id.clone(), u);
            }
            Ok(None) => {}
            Err(e) => log::warn!("usage for {}: {e:#}", s.info.id),
        }
    }
    out
}
