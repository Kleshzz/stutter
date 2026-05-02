mod config;
mod error;
mod hypr;
mod scheduler;

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!($($arg)*);
        }
    };
}

use tokio::io::AsyncBufReadExt;
use tokio::signal::unix::{Signal, SignalKind, signal};

use error::{Result, StutterError};
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

async fn handle_event(
    event: &str,
    cmd_socket_path: &std::path::Path,
    cfg: &config::Config,
    prev_pid: &mut Option<u32>,
    prev_addr: &mut Option<String>,
    dry_run: bool,
) {
    if event.starts_with("activewindow>>") {
        match hypr::get_active_window(cmd_socket_path).await {
            Ok((new_pid, new_addr)) => {
                if let Some(p) = *prev_pid {
                    if p != new_pid {
                        reset_prev(prev_pid, prev_addr, cfg.default_nice, "reset", dry_run);
                    }
                }
                match set_priority(new_pid, cfg.focused_nice, dry_run) {
                    Ok(()) if !dry_run => {
                        log!("[stutter] pid {new_pid} → nice {}", cfg.focused_nice);
                    }
                    Ok(()) => {}
                    Err(e) => log!("[stutter] failed to boost pid {new_pid}: {e}"),
                }
                *prev_pid = Some(new_pid);
                *prev_addr = Some(new_addr);
            }
            Err(StutterError::NoActiveWindow) => {
                // Tab switch or empty workspace — nothing to boost
            }
            Err(e) => {
                log!("[stutter] failed to get active window: {e}");
            }
        }
    } else if let Some(addr) = event.strip_prefix("closewindow>>") {
        if Some(addr) == prev_addr.as_deref() {
            prev_pid.take();
            prev_addr.take();
        }
    } else if event.starts_with("workspace>>") {
        reset_prev(
            prev_pid,
            prev_addr,
            cfg.default_nice,
            "reset on workspace switch",
            dry_run,
        );
    }
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
    let mut line = String::new();

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    let event_socket_path = hypr::get_socket_path(".socket2.sock")?;
    let cmd_socket_path = hypr::get_socket_path(".socket.sock")?;

    loop {
        let mut reader = match hypr::connect_events(&event_socket_path).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[stutter] failed to connect to event socket: {e}, retrying in 3s");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        };
        log!("[stutter] connected to event socket");

        loop {
            line.clear();

            tokio::select! {
                res = reader.read_line(&mut line) => {
                    let n = match res {
                        Ok(n) => n,
                        Err(e) => {
                            log!("[stutter] socket error: {e}, reconnecting in 3s");
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            break;
                        }
                    };

                    if n == 0 {
                        log!("[stutter] event socket closed, reconnecting in 3s");
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        break;
                    }

                    handle_event(
                        line.trim_end(),
                        &cmd_socket_path,
                        &cfg,
                        &mut prev_pid,
                        &mut prev_addr,
                        dry_run,
                    ).await;
                }
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
            }
        }
    }
}
