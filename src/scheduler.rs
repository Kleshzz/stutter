#![allow(unsafe_code)]

use crate::error::{Result, StutterError};

static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn handle_errno(pid: u32, errno: i32, action: &str) -> Result<()> {
    // ESRCH = process no longer exists, skip silently
    if errno == libc::ESRCH {
        crate::log!("[stutter] pid {pid} not found (ESRCH), skipping");
        return Ok(());
    }

    if (errno == libc::EPERM || errno == libc::EACCES)
        && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        eprintln!(
            "[stutter] error: Permission denied when {action} priority for pid {pid}. Please ensure the binary has CAP_SYS_NICE capability or is run as root."
        );
    }

    Err(StutterError::Priority { pid, errno })
}

pub fn set_priority(pid: u32, nice: i32, dry_run: bool) -> Result<()> {
    unsafe { *libc::__errno_location() = 0 };
    let current_nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid) };

    if current_nice == -1 {
        let errno = unsafe { *libc::__errno_location() };
        if errno != 0 {
            return handle_errno(pid, errno, "getting");
        }
    }

    if current_nice == nice {
        return Ok(());
    }

    if dry_run {
        println!("[stutter] [DRY RUN] would set pid {pid} to nice {nice} (current: {current_nice})");
        return Ok(());
    }

    let ret = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };

    if ret == -1 {
        let errno = unsafe { *libc::__errno_location() };
        return handle_errno(pid, errno, "setting");
    }

    Ok(())
}
