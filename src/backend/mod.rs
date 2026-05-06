pub mod hyprland;
pub mod niri;

use crate::error::Result;

pub struct FocusEvent {
    pub pid: u32,
    pub addr: String,
    pub class: String,
}

pub enum FocusChange {
    Focused(FocusEvent),
    Unfocused,
}

pub trait WmBackend: Send {
    async fn next_focus_event(&mut self) -> Result<Option<FocusChange>>;
}

pub enum Backend {
    Hyprland(hyprland::HyprlandBackend),
    Niri(niri::NiriBackend),
}

pub async fn detect() -> Result<Backend> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Ok(Backend::Hyprland(hyprland::HyprlandBackend::connect().await?));
    }
    if std::env::var("NIRI_SOCKET").is_ok()
        || std::env::var("XDG_RUNTIME_DIR")
            .is_ok_and(|d| std::path::Path::new(&d).join("niri/socket").exists())
    {
        return Ok(Backend::Niri(niri::NiriBackend::connect().await?));
    }
    Err(crate::error::StutterError::NoWmDetected)
}
