//! Cross-platform process liveness helpers shared by low-level crates.

/// What can be established about a process from an operating-system probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLiveness {
    Live,
    Dead,
    Unknown,
}

/// Probe whether `pid` identifies a process that is still running.
///
/// `Unknown` is distinct from `Dead` so callers that clean up ownership state
/// never mistake a permissions or transient OS error for a terminated process.
pub fn liveness(pid: u32) -> ProcessLiveness {
    if pid == 0 {
        return ProcessLiveness::Dead;
    }

    #[cfg(unix)]
    {
        let Ok(pid) = libc::pid_t::try_from(pid) else {
            return ProcessLiveness::Dead;
        };
        let result = unsafe { libc::kill(pid, 0) };
        if result == 0 {
            return ProcessLiveness::Live;
        }

        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => ProcessLiveness::Dead,
            Some(libc::EPERM) => ProcessLiveness::Live,
            _ => ProcessLiveness::Unknown,
        }
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                classify_windows_open_failure(GetLastError())
            } else {
                let mut exit_code = 0u32;
                let query_succeeded = GetExitCodeProcess(handle, &mut exit_code) != 0;
                CloseHandle(handle);
                classify_windows_exit_code(query_succeeded, exit_code)
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        ProcessLiveness::Unknown
    }
}

/// Return whether `pid` may still identify a running process.
///
/// Ambiguous probes are intentionally treated as live. False positives leave a
/// stale marker for a later sweep; false negatives can delete a live owner's
/// marker and are therefore the more dangerous outcome.
pub fn is_running(pid: u32) -> bool {
    !matches!(liveness(pid), ProcessLiveness::Dead)
}

#[cfg(any(windows, test))]
const WINDOWS_ERROR_INVALID_PARAMETER: u32 = 87;
#[cfg(any(windows, test))]
const WINDOWS_STILL_ACTIVE: u32 = 259;

#[cfg(any(windows, test))]
fn classify_windows_open_failure(error_code: u32) -> ProcessLiveness {
    // OpenProcess documents ERROR_INVALID_PARAMETER for a nonexistent PID.
    // Access denied and every other error leave liveness indeterminate.
    if error_code == WINDOWS_ERROR_INVALID_PARAMETER {
        ProcessLiveness::Dead
    } else {
        ProcessLiveness::Unknown
    }
}

#[cfg(any(windows, test))]
fn classify_windows_exit_code(query_succeeded: bool, exit_code: u32) -> ProcessLiveness {
    if !query_succeeded {
        ProcessLiveness::Unknown
    } else if exit_code == WINDOWS_STILL_ACTIVE {
        ProcessLiveness::Live
    } else {
        ProcessLiveness::Dead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_running() {
        assert_eq!(liveness(std::process::id()), ProcessLiveness::Live);
        assert!(is_running(std::process::id()));
    }

    #[cfg(unix)]
    #[test]
    fn pid_larger_than_pid_t_is_not_running() {
        assert_eq!(liveness(u32::MAX), ProcessLiveness::Dead);
        assert!(!is_running(u32::MAX));
    }

    #[test]
    fn windows_open_failure_is_dead_only_for_nonexistent_pid() {
        assert_eq!(
            classify_windows_open_failure(WINDOWS_ERROR_INVALID_PARAMETER),
            ProcessLiveness::Dead
        );
        assert_eq!(
            classify_windows_open_failure(5), // ERROR_ACCESS_DENIED
            ProcessLiveness::Unknown
        );
        assert_eq!(
            classify_windows_open_failure(1234),
            ProcessLiveness::Unknown
        );
    }

    #[test]
    fn windows_exit_code_requires_a_successful_query() {
        assert_eq!(
            classify_windows_exit_code(false, WINDOWS_STILL_ACTIVE),
            ProcessLiveness::Unknown
        );
        assert_eq!(
            classify_windows_exit_code(true, WINDOWS_STILL_ACTIVE),
            ProcessLiveness::Live
        );
        assert_eq!(classify_windows_exit_code(true, 0), ProcessLiveness::Dead);
    }
}
