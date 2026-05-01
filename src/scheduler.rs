#![allow(unsafe_code)]

use crate::error::{Result, StutterError};

pub const FOCUSED_NICE: i32 = -5;
pub const DEFAULT_NICE: i32 = 0;

pub fn set_priority(pid: u32, nice: i32) -> Result<()> {
    let ret = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid, nice) };

    if ret == -1 {
        let errno = unsafe { *libc::__errno_location() };

        // ESRCH = process no longer exists, skip silently
        if errno == libc::ESRCH {
            crate::log!("[stutter] pid {pid} not found (ESRCH), skipping");
            return Ok(());
        }

        return Err(StutterError::Priority { pid, errno });
    }

    Ok(())
}
