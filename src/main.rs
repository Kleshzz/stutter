mod backend;
mod config;
mod error;
mod scheduler;

use backend::{Backend, WmBackend};
use error::Result;
use scheduler::set_priority;
use tokio::signal::unix::{Signal, SignalKind, signal};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

async fn wait_shutdown(sigterm: &mut Signal, sigint: &mut Signal) {
    tokio::select! { _ = sigterm.recv() => {}, _ = sigint.recv() => {} }
}

fn reset_prev(
    prev_pid: &mut Option<u32>,
    prev_addr: &mut Option<String>,
    current_boosted_nice: &mut Option<i32>,
    default_nice: i32,
    reason: &str,
    dry_run: bool,
) {
    if let Some(p) = prev_pid.take() {
        if let Err(e) = set_priority(p, default_nice, dry_run) {
            error!("failed to reset priority for pid {p}: {e}");
        } else if !dry_run {
            info!("pid {p} → nice {default_nice} ({reason})");
        }
    }
    prev_addr.take();
    current_boosted_nice.take();
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .without_time()
        .with_target(false)
        .init();

    let dry_run = std::env::args().any(|arg| arg == "--dry-run");
    if dry_run {
        info!("starting in dry-run mode");
    } else {
        info!("starting");
    }

    let mut cfg = config::load();
    info!(
        focused_nice = cfg.focused_nice,
        default_nice = cfg.default_nice,
        "config loaded"
    );

    let mut prev_pid: Option<u32> = None;
    let mut prev_addr: Option<String> = None;
    let mut current_boosted_nice: Option<i32> = None;

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    loop {
        let mut wm = match backend::detect().await {
            Ok(b) => b,
            Err(e) => {
                error!("failed to connect to WM: {e}, retrying in 3s");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        let wm_name = match &wm {
            Backend::Hyprland(_) => "Hyprland",
            Backend::Niri(_) => "niri",
        };
        info!("connected to {wm_name}");

        loop {
            tokio::select! {
                () = wait_shutdown(&mut sigterm, &mut sigint) => {
                    info!("received termination signal, exiting");
                    if let Some(p) = prev_pid {
                        let _ = set_priority(p, cfg.default_nice, dry_run);
                    }
                    return Ok(());
                }
                _ = sighup.recv() => {
                    info!("received SIGHUP, reloading config");
                    cfg = config::load();
                    info!(
                        focused_nice = cfg.focused_nice,
                        default_nice = cfg.default_nice,
                        "reloaded config"
                    );
                }
                result = async {
                    match &mut wm {
                        Backend::Hyprland(b) => b.next_focus_event().await,
                        Backend::Niri(b) => b.next_focus_event().await,
                    }
                } => {
                    match result {
                        Ok(Some(event)) => {
                            if prev_pid == Some(event.pid) && prev_addr.as_ref() == Some(&event.addr) {
                                continue;
                            }

                            if prev_pid != Some(event.pid) {
                                reset_prev(
                                    &mut prev_pid,
                                    &mut prev_addr,
                                    &mut current_boosted_nice,
                                    cfg.default_nice,
                                    "reset",
                                    dry_run,
                                );
                            } else {
                                info!("focus moved to another window of pid {} ({})", event.pid, event.class);
                            }

                            let focused_nice = cfg.focused_nice_for(&event.class);
                            if current_boosted_nice != Some(focused_nice) {
                                if let Err(e) = set_priority(event.pid, focused_nice, dry_run) {
                                    error!("failed to boost pid {}: {e}", event.pid);
                                } else if !dry_run {
                                    info!(
                                        "pid {} ({}) → nice {}",
                                        event.pid, event.class, focused_nice
                                    );
                                }
                                current_boosted_nice = Some(focused_nice);
                            }

                            prev_pid = Some(event.pid);
                            prev_addr = Some(event.addr);
                        }

                        Ok(None) => {
                            warn!("WM socket closed, reconnecting in 3s");
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            break;
                        }
                        Err(e) => {
                            warn!("socket error: {e}, reconnecting in 3s");
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            break;
                        }
                    }
                }
            }
        }
    }
}
