use std::path::PathBuf;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use tracing::debug;

use super::{FocusEvent, WmBackend};
use crate::error::{Result, StutterError};

pub struct NiriBackend {
    reader: BufReader<UnixStream>,
    line: String,
}

impl NiriBackend {
    pub async fn connect() -> Result<Self> {
        let path = niri_socket_path()?;
        let mut stream = UnixStream::connect(&path).await?;
        stream.write_all(b"{\"EventStream\":null}\n").await?;

        let mut reader = BufReader::new(stream);
        // consume the handshake response
        let mut handshake = String::new();
        reader.read_line(&mut handshake).await?;
        debug!("niri handshake: {}", handshake.trim());

        Ok(Self {
            reader,
            line: String::new(),
        })
    }
}

fn niri_socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| StutterError::NoRuntimeDir)?;
    if let Ok(p) = std::env::var("NIRI_SOCKET") {
        return Ok(PathBuf::from(p));
    }
    Ok(PathBuf::from(runtime_dir).join("niri").join("socket"))
}

#[derive(serde::Deserialize)]
struct NiriEvent {
    #[serde(rename = "WindowFocusChanged")]
    window_focus_changed: Option<WindowFocusChanged>,
}

#[derive(serde::Deserialize)]
struct WindowFocusChanged {
    window: Option<NiriWindow>,
}

#[derive(serde::Deserialize)]
struct NiriWindow {
    id: u64,
    pid: Option<u32>,
    app_id: Option<String>,
}

impl WmBackend for NiriBackend {
    async fn next_focus_event(&mut self) -> Result<Option<FocusEvent>> {
        loop {
            self.line.clear();
            let n = self.reader.read_line(&mut self.line).await?;
            if n == 0 {
                return Ok(None);
            }
            let event = match serde_json::from_str::<NiriEvent>(self.line.trim_end()) {
                Ok(e) => e,
                Err(e) => {
                    debug!("failed to parse niri event: {e} (line: {})", self.line.trim_end());
                    continue;
                }
            };
            if let Some(WindowFocusChanged {
                window:
                    Some(NiriWindow {
                        pid: Some(pid),
                        id,
                        app_id,
                    }),
            }) = event.window_focus_changed
            {
                return Ok(Some(FocusEvent {
                    pid,
                    addr: id.to_string(),
                    class: app_id.unwrap_or_default(),
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn parse_event(json: &str) -> Option<FocusEvent> {
        let Ok(event) = serde_json::from_str::<NiriEvent>(json) else {
            return None;
        };
        if let Some(WindowFocusChanged {
            window:
                Some(NiriWindow {
                    pid: Some(pid),
                    id,
                    app_id,
                }),
        }) = event.window_focus_changed
        {
            Some(FocusEvent {
                pid,
                addr: id.to_string(),
                class: app_id.unwrap_or_default(),
            })
        } else {
            None
        }
    }

    #[test]
    fn parses_window_focus_changed() {
        let json = r#"{"WindowFocusChanged":{"window":{"id":42,"pid":1234,"title":"foo","app_id":"bar"}}}"#;
        let event = parse_event(json).unwrap();
        assert_eq!(event.pid, 1234);
        assert_eq!(event.addr, "42");
    }

    #[test]
    fn focus_lost_returns_none() {
        // window = null means focus lost (empty workspace)
        let json = r#"{"WindowFocusChanged":{"window":null}}"#;
        assert!(parse_event(json).is_none());
    }

    #[test]
    fn unrelated_event_returns_none() {
        let json = r#"{"WorkspaceActivated":{"id":1,"focused":true}}"#;
        assert!(parse_event(json).is_none());
    }

    #[test]
    fn malformed_json_returns_none() {
        assert!(parse_event("not json").is_none());
        assert!(parse_event("{}").is_none());
    }

    #[test]
    fn window_without_pid_returns_none() {
        let json = r#"{"WindowFocusChanged":{"window":{"id":42}}}"#;
        assert!(parse_event(json).is_none());
    }

    #[test]
    fn parses_app_id_as_class() {
        let json = r#"{"WindowFocusChanged":{"window":{"id":1,"pid":99,"app_id":"firefox"}}}"#;
        let event = parse_event(json).unwrap();
        assert_eq!(event.class, "firefox");
    }

    #[test]
    fn missing_app_id_gives_empty_class() {
        let json = r#"{"WindowFocusChanged":{"window":{"id":1,"pid":99}}}"#;
        let event = parse_event(json).unwrap();
        assert_eq!(event.class, "");
    }
}
