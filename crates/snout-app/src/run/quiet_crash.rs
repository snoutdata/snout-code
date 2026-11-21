//! A crash that SnoutCode already reports, kept from macOS's crash reporter: no "quit
//! unexpectedly" window and no crash report for it.
//!
//! A Cypress older than the system it runs on crashes its browser as it closes, after the
//! results are in; the Tests window says so, with the fix. macOS would show its own window
//! for that crash on every run. So the run starts under this program again, as a small
//! wrapper: the wrapper takes its own crash exceptions on a port of its own, everything it
//! starts inherits that port, and a crash anywhere below it ends there, answered as handled,
//! instead of reaching ReportCrash. The process still dies of its signal and the run still
//! sees it; only the system's window and report are gone.
//!
//! macOS kills a system binary spawned with exception ports given to it directly, but not one
//! that inherits them; so the wrapper sets them on itself and its children inherit them.

/// Set on the wrapper: it runs its arguments (program first) with crashes kept quiet.
pub const ENV: &str = "SNOUT_QUIET_CRASH";

/// Whether this program can be the wrapper: it is the app (the tests' programs are not).
static APP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// This program is the app, so it can start itself as the wrapper.
pub fn enable() {
    APP.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// `cmd` started under the wrapper, when this Mac runs it; anything else as it is.
pub fn wrap(cmd: snout_host::Cmd, local: bool) -> snout_host::Cmd {
    if !cfg!(target_os = "macos") || !local || !APP.load(std::sync::atomic::Ordering::Relaxed) {
        return cmd;
    }
    let Ok(exe) = std::env::current_exe() else {
        return cmd;
    };
    let mut args = vec![cmd.program.clone()];
    args.extend(cmd.args.iter().cloned());
    let mut env = cmd.env.clone();
    env.push((ENV.into(), Some("1".into())));
    snout_host::Cmd { program: exe.into_os_string(), args, env, ..cmd }
}

/// The wrapper: run the arguments, catching the crashes below it, and end as they did.
#[cfg(target_os = "macos")]
pub fn run() -> ! {
    let mut args = std::env::args_os().skip(1);
    let Some(program) = args.next() else {
        std::process::exit(2);
    };
    mach::catch_crashes();
    let status = std::process::Command::new(&program).args(args).env_remove(ENV).status();
    match status {
        Ok(s) => {
            use std::os::unix::process::ExitStatusExt as _;
            std::process::exit(s.code().unwrap_or_else(|| 128 + s.signal().unwrap_or(0)))
        }
        Err(e) => {
            eprintln!("{}: {e}", std::path::Path::new(&program).display());
            std::process::exit(127)
        }
    }
}

#[cfg(target_os = "macos")]
mod mach {
    //! The few Mach calls the wrapper needs (`<mach/mach.h>`).

    type Port = u32;

    #[repr(C)]
    #[derive(Default)]
    struct Header {
        bits: u32,
        size: u32,
        remote: Port,
        local: Port,
        voucher: Port,
        id: i32,
    }

    #[repr(C)]
    struct Received {
        header: Header,
        body: [u8; 4096],
    }

    #[repr(C)]
    struct Reply {
        header: Header,
        ndr: [u8; 8],
        ret: i32,
    }

    const PORT_RIGHT_RECEIVE: u32 = 1;
    const MSG_TYPE_MAKE_SEND: u32 = 20;
    const EXC_MASK_CRASH: u32 = 1 << 10;
    const EXCEPTION_DEFAULT: u32 = 1;
    const MACH_EXCEPTION_CODES: u32 = 0x8000_0000;
    #[cfg(target_arch = "aarch64")]
    const THREAD_STATE_NONE: i32 = 5;
    #[cfg(target_arch = "x86_64")]
    const THREAD_STATE_NONE: i32 = 13;
    const RCV_MSG: i32 = 2;
    const SEND_MSG: i32 = 1;

    unsafe extern "C" {
        static NDR_record: [u8; 8];
        fn mach_port_allocate(task: Port, right: u32, name: *mut Port) -> i32;
        fn mach_port_insert_right(task: Port, name: Port, right: Port, kind: u32) -> i32;
        fn task_set_exception_ports(task: Port, mask: u32, port: Port, behavior: u32, flavor: i32) -> i32;
        fn mach_msg(msg: *mut Header, option: i32, send: u32, rcv: u32, name: Port, timeout: u32, notify: Port) -> i32;
        fn mach_msg_destroy(msg: *mut Header);
    }

    /// Take this process's crash exceptions, and so those of everything it starts from now
    /// on, on a port answered here. Nothing changes when that cannot be set up.
    pub fn catch_crashes() {
        // SAFETY: plain Mach calls on this task, with a port it owns.
        unsafe {
            #[allow(deprecated)]
            let task = libc::mach_task_self();
            let mut port: Port = 0;
            if mach_port_allocate(task, PORT_RIGHT_RECEIVE, &mut port) != 0
                || mach_port_insert_right(task, port, port, MSG_TYPE_MAKE_SEND) != 0
            {
                return;
            }
            let behavior = EXCEPTION_DEFAULT | MACH_EXCEPTION_CODES;
            if task_set_exception_ports(task, EXC_MASK_CRASH, port, behavior, THREAD_STATE_NONE) != 0 {
                return;
            }
            std::thread::spawn(move || serve(port));
        }
    }

    /// Answer each crash as handled, so the system's crash reporter is not asked.
    fn serve(port: Port) {
        loop {
            let mut msg = Received { header: Header::default(), body: [0; 4096] };
            // SAFETY: the buffer is as large as the size given; the reply goes to the reply
            // port the message carries, and the rest of what it carries is freed.
            unsafe {
                if mach_msg(&mut msg.header, RCV_MSG, 0, size_of::<Received>() as u32, port, 0, 0) != 0 {
                    continue;
                }
                let mut reply = Reply {
                    header: Header {
                        bits: msg.header.bits & 0x1f,
                        size: size_of::<Reply>() as u32,
                        remote: msg.header.remote,
                        local: 0,
                        voucher: 0,
                        id: msg.header.id + 100,
                    },
                    ndr: NDR_record,
                    ret: 0,
                };
                msg.header.remote = 0;
                mach_msg_destroy(&mut msg.header);
                mach_msg(&mut reply.header, SEND_MSG, size_of::<Reply>() as u32, 0, 0, 0, 0);
            }
        }
    }
}
