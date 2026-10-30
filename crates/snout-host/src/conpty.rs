//! Terminals on Windows: a process on a pseudo console (ConPTY) of its own.
//!
//! The console is made with no flags. In particular not with "inherit the cursor", which has
//! the console ask the terminal where its cursor is (`ESC[6n`) and hold the program back until
//! the answer comes: a terminal starts empty, so there is nothing to inherit, and a reader
//! that does not answer would wait forever. Nor with the resize quirk, which tells the console
//! that the terminal reflows its own text on a resize and so needs no repaint: SnoutCode's
//! terminal engines do not reflow, so the console repaints.
//!
//! The process is in a job object of its own, as the processes of [`crate::LocalHost::spawn`]
//! are, so stopping it stops what it started. Once it has ended, the console is closed: the
//! console keeps its output open otherwise, and the reader would never see the end.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::{FromRawHandle as _, RawHandle};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Console::{COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
};

use crate::win::{Job, wide};
use crate::{Cmd, ExitStatus, PtyProcess};

/// A handle this module owns.
struct Owned(HANDLE);

// SAFETY: process and pipe handles may be used from any thread.
unsafe impl Send for Owned {}
// SAFETY: the calls made on them are thread-safe.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle is ours.
        unsafe { CloseHandle(self.0) };
    }
}

impl Owned {
    /// The handle as a file, which then owns it.
    fn into_file(self) -> File {
        let h = self.0;
        std::mem::forget(self);
        // SAFETY: an open pipe handle, now owned by the file alone.
        unsafe { File::from_raw_handle(h as RawHandle) }
    }
}

/// The pseudo console, until it is closed.
struct Console(Mutex<Option<HPCON>>);

impl Console {
    fn close(&self) {
        // Taken out first: closing waits for the console to end, which resizes need not wait for.
        let con = self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(con) = con {
            // SAFETY: a console we made, closed once.
            unsafe { ClosePseudoConsole(con) };
        }
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        self.close();
    }
}

/// A process on a pseudo console of its own.
pub struct ConPty {
    // Dropped in this order: the output's reading end before the console, so closing the
    // console never waits for output nobody reads.
    reader: Option<File>,
    writer: Option<File>,
    console: Arc<Console>,
    process: Arc<Owned>,
    pid: u32,
    job: Option<Job>,
}

fn pipe() -> io::Result<(Owned, Owned)> {
    let (mut r, mut w): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: two places for the handles; no security attributes (not inheritable).
    if unsafe { CreatePipe(&mut r, &mut w, std::ptr::null(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((Owned(r), Owned(w)))
}

fn size(rows: u16, cols: u16) -> COORD {
    COORD { X: cols.clamp(1, i16::MAX as u16) as i16, Y: rows.clamp(1, i16::MAX as u16) as i16 }
}

/// Start `cmd` on a new pseudo console of `rows` × `cols`.
pub fn spawn(cmd: &Cmd, rows: u16, cols: u16) -> io::Result<ConPty> {
    let env = environment(cmd);
    let program = resolve(&cmd.program, env.get(OsStr::new("PATH")).map(|(_, v)| v)).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("{} was not found", cmd.program.to_string_lossy()))
    })?;
    let (application, mut line) = command_line(&program, &cmd.args);

    let (in_read, in_write) = pipe()?;
    let (out_read, out_write) = pipe()?;
    let mut con: HPCON = 0;
    // SAFETY: the pipe ends the console reads from and writes to, and a place for it.
    let hr = unsafe { CreatePseudoConsole(size(rows, cols), in_read.0, out_write.0, 0, &mut con) };
    if hr != 0 {
        return Err(io::Error::other(format!("the pseudo console could not be made (HRESULT {hr:#x})")));
    }
    let console = Arc::new(Console(Mutex::new(Some(con))));
    // The console has its own copies of these.
    drop((in_read, out_write));

    // The attribute that puts the process on the console.
    let mut bytes = 0usize;
    // SAFETY: asks for the size only (fails with ERROR_INSUFFICIENT_BUFFER, as it should).
    unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut bytes) };
    let mut list = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    let attrs = list.as_mut_ptr().cast::<c_void>();
    // SAFETY: a buffer of the size asked for.
    if unsafe { InitializeProcThreadAttributeList(attrs, 1, 0, &mut bytes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the console's handle is the attribute's value itself, as the API wants.
    let ok = unsafe {
        UpdateProcThreadAttribute(
            attrs,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            con as *const c_void,
            size_of::<HPCON>(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        let e = io::Error::last_os_error();
        // SAFETY: an initialized list.
        unsafe { DeleteProcThreadAttributeList(attrs) };
        return Err(e);
    }

    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    // No standard handles of the app's (its log, when its output goes to a file): the
    // program's are the console's.
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
    si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
    si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
    si.lpAttributeList = attrs;
    let mut block = environment_block(&env);
    let application = wide(application.as_os_str());
    let cwd = cmd.cwd.as_ref().map(|d| wide(d.as_os_str()));
    let mut pi = PROCESS_INFORMATION::default();
    // SAFETY: NUL-terminated strings, an environment block ending in two NULs, the start-up
    // information with its attribute list, and a place for the process's handles.
    let ok = unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED,
            block.as_mut_ptr().cast::<c_void>(),
            cwd.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
            &si.StartupInfo,
            &mut pi,
        )
    };
    let made = if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) };
    // SAFETY: an initialized list, no longer needed once the process is made.
    unsafe { DeleteProcThreadAttributeList(attrs) };
    made.map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", program.display())))?;

    let process = Arc::new(Owned(pi.hProcess));
    let thread = Owned(pi.hThread);
    let job = Job::new(false).ok().filter(|j| j.assign(process.0 as RawHandle).is_ok());
    // SAFETY: the process's first thread, made suspended.
    unsafe { ResumeThread(thread.0) };
    drop(thread);
    Ok(ConPty {
        reader: Some(out_read.into_file()),
        writer: Some(in_write.into_file()),
        console,
        process,
        pid: pi.dwProcessId,
        job,
    })
}

impl PtyProcess for ConPty {
    fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }

    fn take_reader(&mut self) -> io::Result<Box<dyn Read + Send>> {
        let reader = self.reader.take().ok_or_else(|| io::Error::other("the terminal's output is taken"))?;
        // Once the process has ended, the console closes, and the reader sees the end.
        let (process, console) = (self.process.clone(), self.console.clone());
        std::thread::Builder::new().name("pty-exit".into()).spawn(move || {
            // SAFETY: a process handle of ours.
            unsafe { WaitForSingleObject(process.0, INFINITE) };
            console.close();
        })?;
        Ok(Box::new(reader))
    }

    fn take_writer(&mut self) -> io::Result<Box<dyn Write + Send>> {
        let writer = self.writer.take().ok_or_else(|| io::Error::other("the terminal's input is taken"))?;
        Ok(Box::new(writer))
    }

    fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        let con = self.console.0.lock().unwrap_or_else(|e| e.into_inner());
        // Closed: the process has ended, there is nothing to resize.
        let Some(con) = *con else { return Ok(()) };
        // SAFETY: an open console.
        let hr = unsafe { ResizePseudoConsole(con, size(rows, cols)) };
        if hr == 0 { Ok(()) } else { Err(io::Error::other(format!("the terminal could not be resized ({hr:#x})"))) }
    }

    fn kill(&mut self) -> io::Result<()> {
        if let Some(job) = &self.job
            && job.terminate(1).is_ok()
        {
            return Ok(());
        }
        // SAFETY: a process handle of ours.
        if unsafe { TerminateProcess(self.process.0, 1) } == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        // SAFETY: a process handle of ours.
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_OBJECT_0 {
            return Ok(None);
        }
        self.status().map(Some)
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        // SAFETY: a process handle of ours.
        unsafe { WaitForSingleObject(self.process.0, INFINITE) };
        self.status()
    }
}

impl ConPty {
    fn status(&self) -> io::Result<ExitStatus> {
        let mut code = 0u32;
        // SAFETY: a process handle of ours and a place for the code.
        if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ExitStatus { code: Some(code as i32), signal: None })
    }
}

/// The program's environment: this process's, with `cmd`'s changes. Keyed by the upper-case
/// name (Windows's names ignore case), each with its name as written.
fn environment(cmd: &Cmd) -> BTreeMap<OsString, (OsString, OsString)> {
    let key = |k: &OsStr| OsString::from(k.to_string_lossy().to_uppercase());
    let mut env: BTreeMap<OsString, (OsString, OsString)> =
        std::env::vars_os().map(|(k, v)| (key(&k), (k, v))).collect();
    for (k, v) in &cmd.env {
        match v {
            Some(v) => env.insert(key(k), (k.clone(), v.clone())),
            None => env.remove(&key(k)),
        };
    }
    env
}

/// `NAME=value` NUL-separated, ending in two NULs, sorted by name as Windows wants.
fn environment_block(env: &BTreeMap<OsString, (OsString, OsString)>) -> Vec<u16> {
    let mut block = Vec::new();
    for (k, v) in env.values() {
        block.extend(k.encode_wide());
        block.push(u16::from(b'='));
        block.extend(v.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

/// The file to run for `program`: as given when it names a folder, else the first match on
/// `path`, either as named or with one of the extensions of `PATHEXT` (`pwsh` is `pwsh.exe`,
/// `claude` is npm's `claude.cmd`).
fn resolve(program: &OsStr, path: Option<&OsString>) -> Option<PathBuf> {
    let given = Path::new(program);
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    let candidates = |base: &Path| -> Vec<PathBuf> {
        if base.extension().is_some() {
            return vec![base.to_owned()];
        }
        let mut v: Vec<PathBuf> = exts
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| {
                let mut s = base.as_os_str().to_owned();
                s.push(e.to_ascii_lowercase());
                PathBuf::from(s)
            })
            .collect();
        v.push(base.to_owned());
        v
    };
    if given.components().count() > 1 || given.is_absolute() {
        return candidates(given).into_iter().find(|p| p.is_file());
    }
    let path = path.cloned().or_else(|| std::env::var_os("PATH")).unwrap_or_default();
    std::env::split_paths(&path)
        .filter(|d| !d.as_os_str().is_empty())
        .flat_map(|d| candidates(&d.join(given)))
        .find(|p| p.is_file())
}

fn is_batch_file(program: &Path) -> bool {
    program.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// The application to start and its command line (NUL-terminated). A batch file runs through
/// the Command Prompt, which takes no line break in an argument: each becomes a space.
fn command_line(program: &Path, args: &[OsString]) -> (PathBuf, Vec<u16>) {
    let mut line = Vec::new();
    let application = if is_batch_file(program) {
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        let cmd = Path::new(&system).join("System32").join("cmd.exe");
        line.extend(OsStr::new("cmd.exe /e:ON /v:OFF /d /c \"").encode_wide());
        push_program(program, &mut line);
        for a in args {
            line.push(u16::from(b' '));
            let a = a.to_string_lossy().replace("\r\n", " ").replace(['\r', '\n'], " ");
            push_arg(OsStr::new(&a), &mut line);
        }
        line.push(u16::from(b'"'));
        cmd
    } else {
        push_program(program, &mut line);
        for a in args {
            line.push(u16::from(b' '));
            push_arg(a, &mut line);
        }
        program.to_owned()
    };
    line.push(0);
    (application, line)
}

/// The program, first on the command line: quoted, with no escapes (the C runtime reads it
/// up to the next quote).
fn push_program(program: &Path, line: &mut Vec<u16>) {
    line.push(u16::from(b'"'));
    line.extend(program.as_os_str().encode_wide());
    line.push(u16::from(b'"'));
}

/// An argument as the C runtime's parser reads it back: quoted when it has a blank or a quote,
/// backslashes doubled before a quote.
fn push_arg(arg: &OsStr, line: &mut Vec<u16>) {
    let (quote, backslash) = (u16::from(b'"'), u16::from(b'\\'));
    let a: Vec<u16> = arg.encode_wide().collect();
    if !a.is_empty() && !a.iter().any(|&c| c == u16::from(b' ') || c == u16::from(b'\t') || c == quote) {
        line.extend(a);
        return;
    }
    line.push(quote);
    let mut backslashes = 0;
    for c in a {
        if c == backslash {
            backslashes += 1;
        } else {
            if c == quote {
                line.extend(std::iter::repeat_n(backslash, backslashes + 1));
            }
            backslashes = 0;
        }
        line.push(c);
    }
    line.extend(std::iter::repeat_n(backslash, backslashes));
    line.push(quote);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(program: &str, args: &[&str]) -> String {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        let (_, mut l) = command_line(Path::new(program), &args);
        l.pop();
        String::from_utf16(&l).unwrap()
    }

    #[test]
    fn quotes_arguments_as_the_c_runtime_reads_them() {
        assert_eq!(
            line(r"C:\Program Files\Git\bin\bash.exe", &["-l", "a b", "", r#"say "hi""#, r"C:\dir\", r"x\\"]),
            r#""C:\Program Files\Git\bin\bash.exe" -l "a b" "" "say \"hi\"" C:\dir\ x\\"#
        );
        assert_eq!(line(r"C:\x\y.exe", &[r"a b\"]), r#""C:\x\y.exe" "a b\\""#);
        assert!(
            line(r"C:\npm\claude.cmd", &["--x", "two\nlines"])
                .ends_with(r#"/d /c ""C:\npm\claude.cmd" --x "two lines"""#)
        );
    }

    #[test]
    fn finds_programs_on_the_path_with_their_extensions() {
        let found = resolve(OsStr::new("cmd"), None).expect("cmd is found");
        assert!(found.to_string_lossy().to_ascii_lowercase().ends_with(r"\cmd.exe"), "{}", found.display());
        assert!(resolve(OsStr::new("no-such-program-here"), None).is_none());
    }
}
