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
use tokio::signal::unix::{SignalKind, signal};

use error::{Result, StutterError};
use scheduler::set_priority;

#[tokio::main]
async fn main() -> Result<()> {
    log!("[stutter] starting...");

    let mut reader = hypr::connect_events().await?;
    log!("[stutter] connected to event socket");

    let cfg = config::load();
    log!(
        "[stutter] focused_nice={} default_nice={}",
        cfg.focused_nice,
        cfg.default_nice
    );

    let mut prev_pid: Option<u32> = None;
    let mut line = String::new();

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    loop {
        line.clear();

        tokio::select! {
            n = reader.read_line(&mut line) => {
                let n = n?;
                if n == 0 {
                    log!("[stutter] event socket closed, exiting");
                    break;
                }

                let event = line.trim_end();

                if event.starts_with("activewindow>>") {
                    match hypr::get_active_window_pid().await {
                        Ok(new_pid) => {
                            if let Some(p) = prev_pid {
                                if p != new_pid {
                                    if let Err(e) = set_priority(p, cfg.default_nice) {
                                        log!("[stutter] failed to reset priority for pid {p}: {e}");
                                    }
                                }
                            }

                            match set_priority(new_pid, cfg.focused_nice) {
                                Ok(()) => log!("[stutter] pid {new_pid} → nice {}", cfg.focused_nice),
                                Err(e) => log!("[stutter] failed to boost pid {new_pid}: {e}"),
                            }

                            prev_pid = Some(new_pid);
                        }
                        Err(StutterError::NoActiveWindow) => {
                            // Tab switch or empty workspace — nothing to boost
                        }
                        Err(e) => {
                            log!("[stutter] failed to get active window: {e}");
                        }
                    }
                }
            }
            _ = sigterm.recv() => {
                log!("[stutter] received SIGTERM, exiting");
                break;
            }
            _ = sigint.recv() => {
                log!("[stutter] received SIGINT, exiting");
                break;
            }
        }
    }

    if let Some(p) = prev_pid {
        let _ = set_priority(p, cfg.default_nice);
    }

    Ok(())
}
