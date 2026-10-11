// Easy on the battery and the fans. Two things Windows is told:
//
// - This process may run on the efficient cores at lower clocks (the
//   "efficiency mode" Task Manager shows with a leaf), while nobody is at
//   the keyboard or mouse (main.rs check_quiet); and its background threads
//   always may (thread_eco), whatever the core layout: a CPU with small
//   cores (Intel's E-cores, AMD's dense cores) puts them there, one
//   without runs them slower. Windows keeps the choice itself; nothing is
//   pinned to a core.
// - How long since the keyboard or mouse was last touched (idle_ms), for
//   the quiet state: she holds still and the loops look less often.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    // PROCESS_POWER_THROTTLING_STATE and THREAD_POWER_THROTTLING_STATE: the
    // same three numbers.
    #[repr(C)]
    struct Throttling {
        version: u32,
        control: u32,
        state: u32,
    }

    const VERSION: u32 = 1;
    const EXECUTION_SPEED: u32 = 0x1;
    const PROCESS_POWER_THROTTLING: i32 = 4;
    const THREAD_POWER_THROTTLING: i32 = 2;

    #[repr(C)]
    struct LastInput {
        size: u32,
        time: u32,
    }

    // Handles as isize, as main.rs declares GetCurrentProcess too.
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetCurrentThread() -> isize;
        fn SetProcessInformation(process: isize, class: i32, info: *const c_void, size: u32) -> i32;
        fn SetThreadInformation(thread: isize, class: i32, info: *const c_void, size: u32) -> i32;
        fn GetTickCount() -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetLastInputInfo(info: *mut LastInput) -> i32;
    }

    fn throttling(on: bool) -> Throttling {
        // Control 0 leaves it to Windows again.
        Throttling { version: VERSION, control: if on { EXECUTION_SPEED } else { 0 }, state: if on { EXECUTION_SPEED } else { 0 } }
    }

    // This process easy on the power, or back to Windows' own choice.
    pub fn process_eco(on: bool) -> bool {
        let state = throttling(on);
        // SAFETY: our own process; the struct is ours, its size passed.
        unsafe { SetProcessInformation(GetCurrentProcess(), PROCESS_POWER_THROTTLING, &state as *const Throttling as *const c_void, std::mem::size_of::<Throttling>() as u32) != 0 }
    }

    // The calling thread easy on the power, for good: a loop that only
    // looks at things now and then.
    pub fn thread_eco() -> bool {
        let state = throttling(true);
        // SAFETY: our own thread; the struct is ours, its size passed.
        unsafe { SetThreadInformation(GetCurrentThread(), THREAD_POWER_THROTTLING, &state as *const Throttling as *const c_void, std::mem::size_of::<Throttling>() as u32) != 0 }
    }

    // Since the keyboard or mouse was last touched, ms; 0 when unknown.
    pub fn idle_ms() -> u64 {
        let mut last = LastInput { size: std::mem::size_of::<LastInput>() as u32, time: 0 };
        // SAFETY: our own struct, its size set.
        if unsafe { GetLastInputInfo(&mut last) } == 0 {
            return 0;
        }
        // SAFETY: a plain read.
        u64::from(unsafe { GetTickCount() }.wrapping_sub(last.time))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn process_eco(_on: bool) -> bool {
        false
    }
    pub fn thread_eco() -> bool {
        false
    }
    pub fn idle_ms() -> u64 {
        0
    }
}

pub use imp::{idle_ms, process_eco, thread_eco};
