// What starting a PowerShell script well takes, for mail's agents
// (mail/agent.rs): its arguments quoted, and a job object that takes what
// she started down with her.
//
// The plugins she runs for you were scripts started from here once; they are
// done inside her now (plugins.rs). waku, the command wrapper for the
// terminal she wrote out beside them, is gone too.

// A PowerShell string: in single quotes, which PowerShell also takes in
// their curly forms, each doubled inside.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

// A job object that ends every process in it when she ends, however she ends.
#[cfg(windows)]
pub(crate) mod job {
    #[repr(C)]
    #[derive(Default)]
    struct Basic {
        per_process_user_time: i64,
        per_job_user_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_processes: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct Extended {
        basic: Basic,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *const u8, name: *const u16) -> isize;
        fn SetInformationJobObject(job: isize, class: i32, info: *const Extended, len: u32) -> i32;
        fn AssignProcessToJobObject(job: isize, process: isize) -> i32;
    }

    const EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const KILL_ON_JOB_CLOSE: u32 = 0x2000;

    // Its handle stays open while she runs; closed as she ends, it ends them.
    pub struct Job(isize);

    impl Job {
        pub fn new() -> Option<Job> {
            // SAFETY: no attributes, no name; the limits are a struct of the size given.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job == 0 {
                    return None;
                }
                let limits = Extended { basic: Basic { flags: KILL_ON_JOB_CLOSE, ..Basic::default() }, ..Extended::default() };
                (SetInformationJobObject(job, EXTENDED_LIMIT_INFORMATION, &limits, std::mem::size_of::<Extended>() as u32) != 0).then_some(Job(job))
            }
        }

        pub fn assign(&self, child: &std::process::Child) -> bool {
            use std::os::windows::io::AsRawHandle;
            // SAFETY: a process we just started and still hold.
            unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as isize) != 0 }
        }
    }

    #[cfg(test)]
    pub fn extended_size() -> usize {
        std::mem::size_of::<Extended>()
    }
}

#[cfg(not(windows))]
pub(crate) mod job {
    pub struct Job;
    impl Job {
        pub fn new() -> Option<Job> {
            None
        }
        pub fn assign(&self, _child: &std::process::Child) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_powershell_is_given_is_quoted() {
        assert_eq!(quote("o'neil/x’y"), "'o''neil/x’’y'");
        assert_eq!(quote("C:\\d\\ci.ps1"), "'C:\\d\\ci.ps1'");
    }

    #[cfg(windows)]
    #[test]
    fn the_job_limits_are_the_size_windows_wants() {
        assert_eq!(job::extended_size(), if cfg!(target_pointer_width = "64") { 144 } else { 112 });
    }
}
