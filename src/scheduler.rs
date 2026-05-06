#![allow(unsafe_code)]

use tracing::{debug, warn};

use crate::error::{Result, StutterError};

static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn reset_warned() {
    WARNED.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn handle_errno(pid: u32, errno: i32, _action: &str) -> Result<()> {
    // ESRCH = process no longer exists, skip silently
    if errno == libc::ESRCH {
        debug!("pid {pid} not found (ESRCH), skipping");
        return Ok(());
    }

    if errno == libc::EPERM {
        if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            warn!(
                "pid {pid}: permission denied (EPERM). \
                 Run stutter as root or grant CAP_SYS_NICE to set priority for other users' processes."
            );
        }
        return Ok(());
    }

    Err(StutterError::Priority { pid, errno })
}

pub fn set_priority(pid: u32, nice: i32, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("[stutter] [DRY RUN] would set pid {pid} to nice {nice}");
        return Ok(());
    }

    unsafe { *libc::__errno_location() = 0 };
    let ret = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) };

    if ret == -1 {
        let errno = unsafe { *libc::__errno_location() };
        return handle_errno(pid, errno, "setting");
    }

    Ok(())
}
