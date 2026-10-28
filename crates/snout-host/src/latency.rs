//! A host made slow on purpose: every call waits before it goes on, as a call to a remote
//! host waits for the network. Calls made on the UI thread are reported on stderr, so the
//! ones left there can be found and moved off it.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;
use std::time::Duration;

use crate::{
    Cmd, DirEntry, Host, HostInfo, HostRef, Metadata, Output, ProcInfo, Process, PtyProcess, Stream, TextHit,
    WalkEntry, WalkOptions, Watcher,
};

pub struct LatencyHost {
    inner: HostRef,
    delay: Duration,
    ui: Option<ThreadId>,
    /// Calls made on the UI thread so far.
    ui_calls: AtomicU64,
}

impl LatencyHost {
    pub fn new(inner: HostRef, delay: Duration) -> Self {
        Self { inner, delay, ui: None, ui_calls: AtomicU64::new(0) }
    }

    /// Take the calling thread as the UI thread, whose calls are reported.
    pub fn ui_thread_here(mut self) -> Self {
        self.ui = Some(std::thread::current().id());
        self
    }

    /// How many calls the UI thread made.
    pub fn ui_calls(&self) -> u64 {
        self.ui_calls.load(Ordering::Relaxed)
    }

    fn wait(&self, op: &str, path: &dyn std::fmt::Debug) {
        if self.ui == Some(std::thread::current().id()) {
            self.ui_calls.fetch_add(1, Ordering::Relaxed);
            eprintln!("[host] {op} {path:?} on the UI thread");
        }
        std::thread::sleep(self.delay);
    }
}

impl Host for LatencyHost {
    fn info(&self) -> &HostInfo {
        self.inner.info()
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.wait("read", &path);
        self.inner.read(path)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.wait("read_to_string", &path);
        self.inner.read_to_string(path)
    }

    fn read_range(&self, path: &Path, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        self.wait("read_range", &path);
        self.inner.read_range(path, offset, len)
    }

    fn write_mode(&self, path: &Path, data: &[u8], mode: Option<u32>) -> io::Result<()> {
        self.wait("write", &path);
        self.inner.write_mode(path, data, mode)
    }

    fn create_dir_all_mode(&self, path: &Path, mode: Option<u32>) -> io::Result<()> {
        self.wait("create_dir_all", &path);
        self.inner.create_dir_all_mode(path, mode)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.wait("remove_file", &path);
        self.inner.remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        self.wait("remove_dir", &path);
        self.inner.remove_dir(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.wait("remove_dir_all", &path);
        self.inner.remove_dir_all(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.wait("rename", &from);
        self.inner.rename(from, to)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        self.wait("copy", &from);
        self.inner.copy(from, to)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        self.wait("metadata", &path);
        self.inner.metadata(path)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata> {
        self.wait("symlink_metadata", &path);
        self.inner.symlink_metadata(path)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        self.wait("read_dir", &path);
        self.inner.read_dir(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.wait("canonicalize", &path);
        self.inner.canonicalize(path)
    }

    fn trash(&self, paths: &[PathBuf]) -> io::Result<()> {
        self.wait("trash", &paths);
        self.inner.trash(paths)
    }

    fn walk(&self, root: &Path, options: &WalkOptions) -> io::Result<Vec<WalkEntry>> {
        self.wait("walk", &root);
        self.inner.walk(root, options)
    }

    fn find_text(&self, root: &Path, files: &[String], needle: &str, limit: usize) -> io::Result<Vec<TextHit>> {
        self.wait("find_text", &root);
        self.inner.find_text(root, files, needle, limit)
    }

    fn watch(&self, root: &Path) -> io::Result<Watcher> {
        self.wait("watch", &root);
        self.inner.watch(root)
    }

    fn spawn(&self, cmd: &Cmd) -> io::Result<Box<dyn Process>> {
        self.wait("spawn", &cmd.display());
        self.inner.spawn(cmd)
    }

    fn run(&self, cmd: &Cmd, input: Option<&[u8]>) -> io::Result<Output> {
        self.wait("run", &cmd.display());
        self.inner.run(cmd, input)
    }

    fn spawn_pty(&self, cmd: &Cmd, rows: u16, cols: u16) -> io::Result<Box<dyn PtyProcess>> {
        self.wait("spawn_pty", &cmd.display());
        self.inner.spawn_pty(cmd, rows, cols)
    }

    fn connect_tcp(&self, port: u16, timeout: Duration) -> io::Result<Box<dyn Stream>> {
        self.wait("connect_tcp", &port);
        self.inner.connect_tcp(port, timeout)
    }

    fn processes(&self, cpu: bool) -> io::Result<Vec<ProcInfo>> {
        self.wait("processes", &cpu);
        self.inner.processes(cpu)
    }

    fn stop_processes(&self, targets: &[(u32, u64)], grace: Duration) -> usize {
        self.wait("stop_processes", &targets);
        self.inner.stop_processes(targets, grace)
    }

    fn process_alive(&self, pid: u32) -> bool {
        self.wait("process_alive", &pid);
        self.inner.process_alive(pid)
    }
}
