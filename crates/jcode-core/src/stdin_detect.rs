#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinState {
    Reading,
    NotReading,
    Unknown,
}

pub fn is_waiting_for_stdin(pid: u32) -> StdinState {
    #[cfg(target_os = "linux")]
    return linux::check(pid);

    #[cfg(target_os = "macos")]
    return macos::check(pid);

    #[cfg(target_os = "windows")]
    return windows::check(pid);

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return StdinState::Unknown;
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;

    pub fn check(pid: u32) -> StdinState {
        check_inner(pid, false)
    }

    fn check_inner(pid: u32, strict: bool) -> StdinState {
        // First try /proc/PID/syscall (most accurate - shows exact syscall + fd)
        if let Ok(contents) = std::fs::read_to_string(format!("/proc/{}/syscall", pid)) {
            // Format: "syscall_nr fd ..."
            // read = 0 on x86_64, 63 on aarch64
            // We want: read(0, ...) i.e. syscall read on fd 0 (stdin)
            let parts: Vec<&str> = contents.split_whitespace().collect();
            if parts.len() >= 2 {
                let syscall_nr = parts[0];
                let fd = parts[1];
                // read syscall: 0 on x86_64, 63 on aarch64
                let is_read = syscall_nr == "0" || syscall_nr == "63";
                let is_stdin = fd == "0x0";
                if is_read && is_stdin {
                    return StdinState::Reading;
                }
            }
        }

        // Fallback: /proc/PID/wchan (no special permissions needed).
        // This is less exact than /proc/PID/syscall, so pair it with an fd 0
        // pipe/pty check. For child processes, check_process_tree also verifies
        // the child shares the parent's stdin pipe before calling strict mode.
        if let Ok(wchan) = std::fs::read_to_string(format!("/proc/{}/wchan", pid)) {
            let wchan = wchan.trim();
            if (wchan == "n_tty_read"
                || wchan == "wait_woken"
                || wchan == "pipe_read"
                || wchan == "pipe_wait_readable"
                || wchan == "unix_stream_read_generic")
                && stdin_is_pipe_or_pty(pid)
            {
                return StdinState::Reading;
            }
            return StdinState::NotReading;
        }

        if strict {
            StdinState::NotReading
        } else {
            StdinState::Unknown
        }
    }

    fn stdin_is_pipe_or_pty(pid: u32) -> bool {
        if let Ok(link) = std::fs::read_link(format!("/proc/{}/fd/0", pid)) {
            let path = link.to_string_lossy();
            return path.contains("pipe") || path.contains("pts") || path.contains("ptmx");
        }
        false
    }

    /// Check all threads in a process group (for cases where a child is the one reading)
    pub fn check_process_tree(pid: u32) -> StdinState {
        // Check the process itself
        let result = check(pid);
        if result == StdinState::Reading {
            return result;
        }

        // Get the parent's stdin fd link target so we can verify children
        // share the same pipe (not just any pipe on fd 0)
        let parent_stdin_link = std::fs::read_link(format!("/proc/{}/fd/0", pid))
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        // Check child processes
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string()
                    && let Ok(child_pid) = name.parse::<u32>()
                    && let Ok(status) =
                        std::fs::read_to_string(format!("/proc/{}/status", child_pid))
                {
                    for line in status.lines() {
                        if let Some(ppid_str) = line.strip_prefix("PPid:\t")
                            && ppid_str.trim().parse::<u32>().ok() == Some(pid)
                        {
                            if let Some(ref parent_link) = parent_stdin_link {
                                let child_link =
                                    std::fs::read_link(format!("/proc/{}/fd/0", child_pid))
                                        .ok()
                                        .map(|p| p.to_string_lossy().to_string());
                                if child_link.as_deref() != Some(parent_link) {
                                    continue;
                                }
                            }
                            let child_result = check_inner(child_pid, true);
                            if child_result == StdinState::Reading {
                                return StdinState::Reading;
                            }
                        }
                    }
                }
            }
        }

        result
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::mem;

    // libproc bindings
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: i32,
        ) -> i32;
        fn proc_pidfdinfo(
            pid: i32,
            fd: i32,
            flavor: i32,
            buffer: *mut libc::c_void,
            buffersize: i32,
        ) -> i32;
        fn proc_listpids(ty: u32, typeinfo: u32, buffer: *mut libc::c_void, buffersize: i32)
        -> i32;
    }

    // proc_pidinfo flavors (sys/proc_info.h)
    const PROC_PIDT_SHORTBSDINFO: i32 = 13;
    const PROC_ALL_PIDS: u32 = 1;

    // proc_pidfdinfo flavors
    const PROC_PIDFDPIPEINFO: i32 = 6;

    // sys/pipe.h pipe_status flags
    const PIPE_WANTR: i32 = 0x008;

    // sys/proc_info.h
    #[repr(C)]
    struct proc_fileinfo {
        fi_openflags: u32,
        fi_status: u32,
        fi_offset: i64,
        fi_type: i32,
        fi_guardflags: u32,
    }

    #[repr(C)]
    struct vinfo_stat {
        vst_dev: u32,
        vst_mode: u16,
        vst_nlink: u16,
        vst_ino: u64,
        vst_uid: u32,
        vst_gid: u32,
        vst_atime: i64,
        vst_atimensec: i64,
        vst_mtime: i64,
        vst_mtimensec: i64,
        vst_ctime: i64,
        vst_ctimensec: i64,
        vst_birthtime: i64,
        vst_birthtimensec: i64,
        vst_size: i64,
        vst_blocks: i64,
        vst_blksize: i32,
        vst_flags: u32,
        vst_gen: u32,
        vst_rdev: u32,
        vst_qspare: [i64; 2],
    }

    #[repr(C)]
    struct pipe_info {
        pipe_stat: vinfo_stat,
        pipe_handle: u64,
        pipe_peerhandle: u64,
        pipe_status: i32,
        rfu_1: i32,
    }

    #[repr(C)]
    struct pipe_fdinfo {
        pfi: proc_fileinfo,
        pipeinfo: pipe_info,
    }

    #[repr(C)]
    struct proc_bsdshortinfo {
        pbsi_pid: u32,
        pbsi_ppid: u32,
        pbsi_pgid: u32,
        pbsi_status: u32,
        pbsi_comm: [u8; 16],
        pbsi_flags: u32,
        pbsi_uid: u32,
        pbsi_gid: u32,
        pbsi_ruid: u32,
        pbsi_rgid: u32,
        pbsi_svuid: u32,
        pbsi_svgid: u32,
        pbsi_rfu1: u32,
    }

    /// Detect whether a process is currently blocked reading from its stdin
    /// pipe.
    ///
    /// Approach: macOS exposes the kernel's pipe object state through
    /// `proc_pidfdinfo(.., PROC_PIDFDPIPEINFO)`. The `pipe_status` field
    /// contains `PIPE_WANTR` (`0x008`) whenever any reader is currently
    /// blocked waiting for data on that pipe. Because the kernel sets this
    /// bit on the shared pipe object, it's true whether the immediate
    /// process is the one calling `read(2)` or whether a forked descendant
    /// inherited the same fd 0 and is the actual reader (e.g. `bash -c head`
    /// where bash forks and head is the blocked reader).
    ///
    /// We also walk children once to handle the rare case where the parent
    /// has closed its end of the original stdin pipe but a child still
    /// holds a copy and is reading.
    pub fn check(pid: u32) -> StdinState {
        if !process_exists(pid as i32) {
            // Caller passed a stale pid; treat as Unknown so we don't
            // confidently report NotReading for a process that may already
            // be gone.
            return StdinState::Unknown;
        }

        match fd0_pipe_wants_reader(pid as i32) {
            Some(true) => return StdinState::Reading,
            Some(false) => {
                // Direct stdin is a pipe but no reader is blocked. Still
                // check children in case the parent forked and dup'd
                // stdin to a child that is the actual reader.
            }
            None => {
                // fd 0 is not a pipe (closed, /dev/null, regular file,
                // tty/pty, ...). For pipes we have a definitive answer;
                // for everything else fall back to "not reading" which
                // matches the Linux fallback semantics.
            }
        }

        if any_child_fd0_pipe_wants_reader(pid as i32) {
            return StdinState::Reading;
        }
        StdinState::NotReading
    }

    /// Convenience name matching the linux module so call sites can be
    /// platform-agnostic.
    pub fn check_process_tree(pid: u32) -> StdinState {
        check(pid)
    }

    fn process_exists(pid: i32) -> bool {
        let mut info: proc_bsdshortinfo = unsafe { mem::zeroed() };
        let r = unsafe {
            proc_pidinfo(
                pid,
                PROC_PIDT_SHORTBSDINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                mem::size_of::<proc_bsdshortinfo>() as i32,
            )
        };
        r > 0
    }

    /// Returns:
    /// - `Some(true)`  if fd 0 exists, is a pipe, and `PIPE_WANTR` is set
    /// - `Some(false)` if fd 0 exists and is a pipe but no reader is waiting
    /// - `None`        if fd 0 is not a pipe (closed, regular file, tty, ...)
    fn fd0_pipe_wants_reader(pid: i32) -> Option<bool> {
        let mut info: pipe_fdinfo = unsafe { mem::zeroed() };
        let r = unsafe {
            proc_pidfdinfo(
                pid,
                0,
                PROC_PIDFDPIPEINFO,
                &mut info as *mut _ as *mut libc::c_void,
                mem::size_of::<pipe_fdinfo>() as i32,
            )
        };
        if r > 0 {
            Some((info.pipeinfo.pipe_status & PIPE_WANTR) != 0)
        } else {
            None
        }
    }

    fn any_child_fd0_pipe_wants_reader(parent_pid: i32) -> bool {
        // Enumerate all PIDs and filter to children of parent_pid. We
        // intentionally do this rather than maintaining a process tree:
        // tool invocations are short-lived, the system process count is
        // bounded, and the cost is dominated by the fd-status syscall we
        // already need to make on each candidate.
        let pids = match list_all_pids() {
            Some(p) => p,
            None => return false,
        };
        for candidate in pids {
            if candidate == parent_pid || candidate <= 0 {
                continue;
            }
            if parent_of(candidate) != parent_pid {
                continue;
            }
            if matches!(fd0_pipe_wants_reader(candidate), Some(true)) {
                return true;
            }
        }
        false
    }

    fn list_all_pids() -> Option<Vec<i32>> {
        let need = unsafe { proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
        if need <= 0 {
            return None;
        }
        // Allow some headroom because pids may be created between the two
        // calls.
        let cap = (need as usize / mem::size_of::<i32>()) + 64;
        let mut buf = vec![0i32; cap];
        let bytes = (buf.len() * mem::size_of::<i32>()) as i32;
        let got = unsafe {
            proc_listpids(
                PROC_ALL_PIDS,
                0,
                buf.as_mut_ptr() as *mut libc::c_void,
                bytes,
            )
        };
        if got <= 0 {
            return None;
        }
        let n = got as usize / mem::size_of::<i32>();
        buf.truncate(n);
        Some(buf.into_iter().filter(|p| *p > 0).collect())
    }

    fn parent_of(pid: i32) -> i32 {
        let mut info: proc_bsdshortinfo = unsafe { mem::zeroed() };
        let r = unsafe {
            proc_pidinfo(
                pid,
                PROC_PIDT_SHORTBSDINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                mem::size_of::<proc_bsdshortinfo>() as i32,
            )
        };
        if r > 0 { info.pbsi_ppid as i32 } else { -1 }
    }

    // The pre-fix implementation walked threads looking for `pth_run_state
    // == TH_STATE_WAITING` and assumed that fd 0 being a pipe/vnode was
    // enough to call it Reading. That approach had two bugs:
    //   1. The constant was wrongly set to 2 instead of 3 (2 is
    //      TH_STATE_STOPPED), so the thread loop never matched and
    //      `check` always returned `NotReading` on macOS.
    //   2. Even with the right constant, every well-behaved blocked
    //      process (e.g. `sleep` waiting on a timer) reports WAITING
    //      while still holding a pipe on fd 0 inherited from its parent,
    //      which would falsely flag it as reading.
    // Both are fixed by reading the pipe object's own `PIPE_WANTR` bit
    // above: the kernel only sets that bit while a thread is actually
    // blocked inside the pipe read path.
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    pub fn check(pid: u32) -> StdinState {
        // Windows: use NtQueryInformationThread to check thread state
        // A process blocked on ReadFile/ReadConsole on stdin will have
        // its thread in a Wait state with a wait reason of UserRequest
        //
        // For now, use the simpler approach: check if the process has
        // a console handle and its thread is in a wait state via
        // WaitForSingleObject with zero timeout on the process handle

        // TODO(TASK-47): implement with windows-sys crate
        // Tracked in: TASK-47 (Implement Windows stdin detection using windows-sys)
        // - OpenProcess(PROCESS_QUERY_INFORMATION, pid)
        // - NtQuerySystemInformation for thread states
        // - Check for KWAIT_REASON::WrUserRequest on stdin handle
        StdinState::Unknown
    }
}

#[cfg(test)]
#[path = "stdin_detect_tests.rs"]
mod stdin_detect_tests;
