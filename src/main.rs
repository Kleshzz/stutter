mod backend;
mod config;
mod error;
mod scheduler;

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!($($arg)*);
        }
    };
}

use tokio::signal::unix::{Signal, SignalKind, signal};

use backend::{Backend, WmBackend};
use error::Result;
use scheduler::set_priority;

async fn wait_shutdown(sigterm: &mut Signal, sigint: &mut Signal) {
    tokio::select! { _ = sigterm.recv() => {}, _ = sigint.recv() => {} }
}

fn reset_prev(
    prev_pid: &mut Option<u32>,
    prev_addr: &mut Option<String>,
    default_nice: i32,
    reason: &str,
    dry_run: bool,
) {
    if let Some(p) = prev_pid.take() {
        if let Err(e) = set_priority(p, default_nice, dry_run) {
            log!("[stutter] failed to reset priority for pid {p}: {e}");
        } else if !dry_run {
            log!("[stutter] pid {p} → nice {default_nice} ({reason})");
        }
    }
    prev_addr.take();
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let dry_run = std::env::args().any(|arg| arg == "--dry-run");
    if dry_run {
        println!("[stutter] starting in dry-run mode");
    } else {
        log!("[stutter] starting...");
    }

    let mut cfg = config::load();
    log!(
        "[stutter] focused_nice={} default_nice={}",
        cfg.focused_nice,
        cfg.default_nice
    );

    let mut prev_pid: Option<u32> = None;
    let mut prev_addr: Option<String> = None;

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    loop {
        let mut wm = match backend::detect().await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[stutter] failed to connect to WM: {e}, retrying in 3s");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        let wm_name = match &wm {
            Backend::Hyprland(_) => "Hyprland",
            Backend::Niri(_) => "niri",
        };
        log!("[stutter] connected to {wm_name}");

        loop {
            tokio::select! {
                () = wait_shutdown(&mut sigterm, &mut sigint) => {
                    log!("[stutter] received termination signal, exiting");
                    if let Some(p) = prev_pid {
                        let _ = set_priority(p, cfg.default_nice, dry_run);
                    }
                    return Ok(());
                }
                _ = sighup.recv() => {
                    log!("[stutter] received SIGHUP, reloading config");
                    cfg = config::load();
                    log!(
                        "[stutter] reloaded config: focused_nice={} default_nice={}",
                        cfg.focused_nice,
                        cfg.default_nice
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
                            if let Some(p) = prev_pid {
                                if p != event.pid {
                                    reset_prev(&mut prev_pid, &mut prev_addr, cfg.default_nice, "reset", dry_run);
                                }
                            }
                            match set_priority(event.pid, cfg.focused_nice, dry_run) {
                                Ok(()) if !dry_run => {
                                    log!("[stutter] pid {} → nice {}", event.pid, cfg.focused_nice);
                                }
                                Ok(()) => {}
                                Err(e) => log!("[stutter] failed to boost pid {}: {e}", event.pid),
                            }
                            prev_pid = Some(event.pid);
                            prev_addr = Some(event.addr);
                        }
                        Ok(None) => {
                            log!("[stutter] WM socket closed, reconnecting in 3s");
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            break;
                        }
                        Err(e) => {
                            log!("[stutter] socket error: {e}, reconnecting in 3s");
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            break;
                        }
                    }
                }
            }
        }
    }
}
