#![allow(unsafe_code)]

use crate::error::{Result, StutterError};

pub fn set_priority(pid: u32, nice: i32) -> Result<()> {
    let ret = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };

    if ret == -1 {
        let errno = unsafe { *libc::__errno_location() };

        // ESRCH = process no longer exists, skip silently
        if errno == libc::ESRCH {
            crate::log!("[stutter] pid {pid} not found (ESRCH), skipping");
            return Ok(());
        }

        if errno == libc::EPERM || errno == libc::EACCES {
            static WARNED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!(
                    "[stutter] error: Permission denied when setting priority for pid {pid}. Please ensure the binary has CAP_SYS_NICE capability or is run as root."
                );
            }
            return Err(StutterError::Priority { pid, errno });
        }

        return Err(StutterError::Priority { pid, errno });
    }

    Ok(())
}
