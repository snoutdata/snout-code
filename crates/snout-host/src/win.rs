//! Windows: processes started without a console window, job objects (a process and every
//! process it starts, stopped together), whether a process runs, and the Recycle Bin.
//!
//! Windows has no signals and no process groups. A job object takes their place: every
//! process the app starts is put in a job of its own, so stopping it stops what it started
//! too (`gradlew.bat` runs `cmd.exe`, which runs Java); and the app itself runs in a job that
//! ends every process left in it when the app's last handle to it closes, crash or not.

use std::ffi::{OsStr, c_void};
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::{AsRawHandle as _, RawHandle};
use std::path::{Path, PathBuf};
use std::process::Command;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, HANDLE_FLAG_INHERIT, STILL_ACTIVE, SetHandleInformation,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS, GetCurrentProcess,
    GetExitCodeProcess, INFINITE, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW,
};

/// Start `c` without a console window: the app has none, so a console program it starts
/// (git, java, cmd) would otherwise open one of its own.
pub fn no_console(c: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt as _;
    c.creation_flags(CREATE_NO_WINDOW)
}

/// Start `c` on its own: no console window, out of the app's job (where the system allows
/// it), in a process group of its own, so it outlives the app.
pub fn detached(c: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt as _;
    c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
}

/// Spawn `c` detached (see [`detached`]); where the job the app runs in forbids leaving it,
/// spawn it inside it instead.
pub fn spawn_detached(c: &mut Command) -> io::Result<std::process::Child> {
    use std::os::windows::process::CommandExt as _;
    match detached(c).spawn() {
        Ok(child) => Ok(child),
        // ERROR_ACCESS_DENIED: a job that does not allow breaking away.
        Err(e) if e.raw_os_error() == Some(5) => c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP).spawn(),
        Err(e) => Err(e),
    }
}

/// A job object: the processes put in it, and those they start.
#[derive(Debug)]
pub struct Job(HANDLE);

// SAFETY: a job handle may be used from any thread.
unsafe impl Send for Job {}
// SAFETY: the calls made on it are thread-safe.
unsafe impl Sync for Job {}

impl Job {
    /// A new job. With `kill_on_close`, its processes end when its last handle closes. A
    /// process in it may start one outside it ([`detached`]).
    pub fn new(kill_on_close: bool) -> io::Result<Self> {
        // SAFETY: no name, no security attributes.
        let h = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self(h);
        // SAFETY: plain data, valid all zeros.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_BREAKAWAY_OK | if kill_on_close { JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE } else { 0 };
        // SAFETY: `info` is the structure the class names, of its size.
        let ok = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast::<c_void>(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Put the process `process` (a handle with PROCESS_SET_QUOTA and PROCESS_TERMINATE) in it.
    pub fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: both handles are valid for the call.
        if unsafe { AssignProcessToJobObject(self.0, process as HANDLE) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Put `child` in it.
    pub fn assign_child(&self, child: &std::process::Child) -> io::Result<()> {
        self.assign(child.as_raw_handle())
    }

    /// End every process in it.
    pub fn terminate(&self, code: u32) -> io::Result<()> {
        // SAFETY: a valid job handle.
        if unsafe { TerminateJobObject(self.0, code) } == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    /// Let processes started from now on inherit the job's handle (or not): a process that
    /// holds it keeps the job, and so its processes, alive (the quit reaper).
    pub fn set_inheritable(&self, inherit: bool) -> io::Result<()> {
        let flag = if inherit { HANDLE_FLAG_INHERIT } else { 0 };
        // SAFETY: a valid handle.
        if unsafe { SetHandleInformation(self.0, HANDLE_FLAG_INHERIT, flag) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is ours.
        unsafe { CloseHandle(self.0) };
    }
}

/// The app's own job (see [`make_app_job`]).
static APP_JOB: std::sync::OnceLock<Job> = std::sync::OnceLock::new();

/// Put this process in a job that ends every process it starts when the process ends,
/// however it ends. The job lives as long as the process. Called once, by the app at its
/// start (not by its tests); the processes meant to outlive it leave the job
/// ([`spawn_detached`]).
pub fn make_app_job() -> io::Result<&'static Job> {
    if let Some(job) = APP_JOB.get() {
        return Ok(job);
    }
    let job = Job::new(true)?;
    // SAFETY: the pseudo handle of this process.
    job.assign(unsafe { GetCurrentProcess() } as RawHandle)?;
    Ok(APP_JOB.get_or_init(|| job))
}

/// The app's job, once [`make_app_job`] made it.
pub fn app_job() -> Option<&'static Job> {
    APP_JOB.get()
}

/// An open handle to a process.
struct Proc(HANDLE);

impl Proc {
    fn open(pid: u32, access: u32) -> Option<Self> {
        // SAFETY: a plain call; a null handle means it failed.
        let h = unsafe { OpenProcess(access, 0, pid) };
        (!h.is_null()).then_some(Self(h))
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        // SAFETY: the handle is ours.
        unsafe { CloseHandle(self.0) };
    }
}

/// Whether a process with this id runs.
pub fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Some(p) = Proc::open(pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
        // ERROR_ACCESS_DENIED: it runs, as someone we may not ask about.
        // SAFETY: a plain call.
        return unsafe { GetLastError() } == 5;
    };
    let mut code = 0u32;
    // SAFETY: a valid handle and a place for the code.
    let ok = unsafe { GetExitCodeProcess(p.0, &mut code) } != 0;
    // Still running, or not to be asked: taken as running.
    !ok || code == STILL_ACTIVE as u32
}

/// Wait until the process `pid` has ended (at once if it has, or cannot be asked about).
pub fn wait_for_exit(pid: u32) {
    if let Some(p) = Proc::open(pid, PROCESS_SYNCHRONIZE) {
        // SAFETY: a valid handle with SYNCHRONIZE.
        unsafe { WaitForSingleObject(p.0, INFINITE) };
    }
}

/// End the process `pid` now.
pub fn kill_pid(pid: u32) -> io::Result<()> {
    let p = Proc::open(pid, PROCESS_TERMINATE).ok_or_else(io::Error::last_os_error)?;
    // SAFETY: a valid handle.
    if unsafe { TerminateProcess(p.0, 1) } == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

/// `s` as a NUL-terminated UTF-16 string.
pub fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// Move `paths` to the Recycle Bin. A file on a drive without one (a network share) is
/// deleted for good, as Explorer does.
pub fn recycle(paths: &[PathBuf]) -> io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    // The paths, absolute, each ended by a NUL and the list by another.
    let mut from: Vec<u16> = Vec::new();
    for p in paths {
        let abs = std::path::absolute(p)?;
        from.extend(abs.as_os_str().encode_wide());
        from.push(0);
    }
    from.push(0);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: `op` and the list it points to live through the call.
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 {
        return Err(io::Error::other(format!("could not move to the Recycle Bin (error {r:#x})")));
    }
    if op.fAnyOperationsAborted != 0 {
        return Err(io::Error::other("moving to the Recycle Bin was cancelled"));
    }
    Ok(())
}

/// `path` with the `\\?\` prefix Rust's `canonicalize` gives taken off where the path does
/// not need it (`\\?\C:\x` is `C:\x`, `\\?\UNC\server\share` is `\\server\share`).
pub fn simplify(path: &Path) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.len() < 260 && rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_runs_and_a_made_up_one_does_not() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(0));
    }

    #[test]
    fn a_job_stops_what_it_holds() {
        let job = Job::new(false).unwrap();
        let mut child = no_console(Command::new("cmd.exe").args(["/c", "ping -n 30 127.0.0.1 >NUL"])).spawn().unwrap();
        job.assign_child(&child).unwrap();
        job.terminate(1).unwrap();
        let status = child.wait().unwrap();
        assert_eq!(status.code(), Some(1));
    }

    #[test]
    fn verbatim_paths_are_simplified() {
        assert_eq!(simplify(Path::new(r"\\?\C:\Users\ann")), Path::new(r"C:\Users\ann"));
        assert_eq!(simplify(Path::new(r"\\?\UNC\srv\share\x")), Path::new(r"\\srv\share\x"));
        assert_eq!(simplify(Path::new(r"C:\x")), Path::new(r"C:\x"));
    }
}
