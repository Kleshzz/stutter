use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::{Result, StutterError};

#[derive(Debug, serde::Deserialize)]
pub struct ActiveWindow {
    pub pid: u32,
    pub address: String,
}

fn socket_path(name: &str) -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| StutterError::NoRuntimeDir)?;
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| StutterError::NoInstanceSignature)?;
    Ok(PathBuf::from(runtime_dir).join("hypr").join(sig).join(name))
}

// connect to the event socket (.socket2.sock), returns a BufReader — read events from it line by line.
pub async fn connect_events() -> Result<BufReader<UnixStream>> {
    let path = socket_path(".socket2.sock")?;
    let stream = UnixStream::connect(path).await?;
    Ok(BufReader::new(stream))
}

// query the PID and address of the active window via the command socket (.socket.sock)
pub async fn get_active_window() -> Result<(u32, String)> {
    let path = socket_path(".socket.sock")?;
    let mut stream = UnixStream::connect(path).await?;

    stream.write_all(b"j/activewindow").await?;
    stream.shutdown().await?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf).await?;

    let buf = buf.trim();
    crate::log!("[stutter] debug: raw response: '{buf}'");

    if buf.is_empty() || buf == "{}" || buf == "unknown request" {
        return Err(StutterError::NoActiveWindow);
    }

    let window: ActiveWindow = serde_json::from_str(buf)?;

    // pid = 0 means no real window (e.g. desktop, empty workspace)
    if window.pid == 0 {
        return Err(StutterError::NoActiveWindow);
    }

    Ok((window.pid, window.address))
}
