use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::{Result, StutterError};

#[derive(Debug, serde::Deserialize)]
pub struct ActiveWindow {
    pub pid: u32,
    pub address: String,
}

pub fn get_socket_path(name: &str) -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| StutterError::NoRuntimeDir)?;
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| StutterError::NoInstanceSignature)?;
    Ok(PathBuf::from(runtime_dir).join("hypr").join(sig).join(name))
}

// connect to the event socket (.socket2.sock), returns a BufReader — read events from it line by line.
pub async fn connect_events(path: &std::path::Path) -> Result<BufReader<UnixStream>> {
    let stream = UnixStream::connect(path).await?;
    Ok(BufReader::new(stream))
}

// query the PID and address of the active window via the command socket (.socket.sock)
pub async fn get_active_window(path: &std::path::Path) -> Result<(u32, String)> {
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

    Ok((
        window.pid,
        window.address.trim_start_matches("0x").to_owned(),
    ))
}

use super::{FocusEvent, WmBackend};

pub struct HyprlandBackend {
    reader: tokio::io::BufReader<tokio::net::UnixStream>,
    cmd_socket_path: std::path::PathBuf,
    line: String,
}

impl HyprlandBackend {
    pub async fn connect() -> crate::error::Result<Self> {
        let event_path = get_socket_path(".socket2.sock")?;
        let cmd_socket_path = get_socket_path(".socket.sock")?;
        let reader = connect_events(&event_path).await?;
        Ok(Self {
            reader,
            cmd_socket_path,
            line: String::new(),
        })
    }
}

impl WmBackend for HyprlandBackend {
    async fn next_focus_event(&mut self) -> crate::error::Result<Option<FocusEvent>> {
        use tokio::io::AsyncBufReadExt;
        loop {
            self.line.clear();
            let n = self.reader.read_line(&mut self.line).await?;
            if n == 0 {
                return Ok(None); // сокет закрылся
            }
            let event = self.line.trim_end();
            if event.starts_with("activewindow>>") {
                match get_active_window(&self.cmd_socket_path).await {
                    Ok((pid, addr)) => return Ok(Some(FocusEvent { pid, addr })),
                    Err(crate::error::StutterError::NoActiveWindow) => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_window(json: &str) -> Result<(u32, String)> {
        let buf = json.trim();
        if buf.is_empty() || buf == "{}" || buf == "unknown request" {
            return Err(StutterError::NoActiveWindow);
        }
        let window: ActiveWindow = serde_json::from_str(buf)?;
        if window.pid == 0 {
            return Err(StutterError::NoActiveWindow);
        }
        Ok((window.pid, window.address.trim_start_matches("0x").to_owned()))
    }

    #[test]
    fn parses_active_window() {
        let json = r#"{"pid":1234,"address":"0xdeadbeef","class":"kitty","title":"~"}"#;
        let (pid, addr) = parse_window(json).unwrap();
        assert_eq!(pid, 1234);
        assert_eq!(addr, "deadbeef");
    }

    #[test]
    fn empty_response_is_no_active_window() {
        assert!(matches!(parse_window("{}"), Err(StutterError::NoActiveWindow)));
        assert!(matches!(parse_window(""), Err(StutterError::NoActiveWindow)));
        assert!(matches!(parse_window("unknown request"), Err(StutterError::NoActiveWindow)));
    }

    #[test]
    fn pid_zero_is_no_active_window() {
        let json = r#"{"pid":0,"address":"0x0","class":"","title":""}"#;
        assert!(matches!(parse_window(json), Err(StutterError::NoActiveWindow)));
    }

    #[test]
    fn address_strips_0x_prefix() {
        let json = r#"{"pid":99,"address":"0xABCD","class":"x","title":"y"}"#;
        let (_, addr) = parse_window(json).unwrap();
        assert_eq!(addr, "ABCD");
    }
}