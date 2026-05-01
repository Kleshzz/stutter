mod error;
mod hypr;
mod scheduler;

use tokio::io::AsyncBufReadExt;

use error::{Result, StutterError};
use scheduler::{set_priority, DEFAULT_NICE, FOCUSED_NICE};

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("[stutter] starting...");

    let mut reader = hypr::connect_events().await?;
    eprintln!("[stutter] connected to event socket");

    let mut prev_pid: Option<u32> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;

        if n == 0 {
            eprintln!("[stutter] event socket closed, exiting");
            break;
        }

        let event = line.trim_end();

        if event.starts_with("activewindow>>") {
            match hypr::get_active_window_pid().await {
                Ok(new_pid) => {
                    if let Some(p) = prev_pid {
                        if p != new_pid {
                            if let Err(e) = set_priority(p, DEFAULT_NICE) {
                                eprintln!("[stutter] failed to reset priority for pid {p}: {e}");
                            }
                        }
                    }

                    match set_priority(new_pid, FOCUSED_NICE) {
                        Ok(()) => eprintln!("[stutter] pid {new_pid} → nice {FOCUSED_NICE}"),
                        Err(e) => eprintln!("[stutter] failed to boost pid {new_pid}: {e}"),
                    }

                    prev_pid = Some(new_pid);
                }
                Err(StutterError::NoActiveWindow) => {
                    // Tab switch or empty workspace — nothing to boost
                }
                Err(e) => {
                    eprintln!("[stutter] failed to get active window: {e}");
                }
            }
        }
    }

    if let Some(p) = prev_pid {
        let _ = set_priority(p, DEFAULT_NICE);
    }

    Ok(())
}