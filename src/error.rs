#[derive(Debug, thiserror::Error)]
pub enum StutterError {
    #[error("HYPRLAND_INSTANCE_SIGNATURE is not set")]
    NoInstanceSignature,

    #[error("XDG_RUNTIME_DIR is not set")]
    NoRuntimeDir,

    #[error("no active window")]
    NoActiveWindow,

    #[error("socket error: {0}")]
    Socket(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("setpriority failed for pid {pid}: errno {errno}")]
    Priority { pid: u32, errno: i32 },
}

pub type Result<T> = std::result::Result<T, StutterError>;
