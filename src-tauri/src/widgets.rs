// Plugins in the island, the first kind: widgets. Each is a few words a
// script keeps up to date there (or the pet herself, for the built-in ones),
// shown only while no session needs you: sessions always come first.
//
//   POST /widget { id, label, value?, detail?, icon?, color?, ttl?, nudge? }
//     id     a-z 0-9 _ -, up to 40; the same id again replaces it
//     label  up to 40 characters; value up to 24
//     detail up to 24, shown after the value in the open island only (the
//            weather's range for the day): the compact one stays short
//     icon   one of ICONS, else a dot
//     color  #rgb or #rrggbb, for the value
//     ttl    how long (s) it stays without a new one: 300 by default, 10 to 86400
//     nudge  true, or a few words: the island opens for it once, if nothing
//            else wants you; once a minute at most, per widget
//     private  true: the label (and the nudge's words) are kept off screen
//            while the specifics are (the details setting off); the value,
//            like "3 封未读", still shows
//   POST /widget { id, remove: true }
//
// Built in (texts as { key, vars } for the page to say in her language):
//   monitor  the machine: CPU, memory, the network's pace, the battery, each
//            on or off ("monitor" setting), read every 1 to 10 seconds, and
//            the hardware's sensors picked from LibreHardwareMonitor (lhm.rs);
//            its value is { parts: [{ icon, text, kind? }] }, shown as icon
//            and number
//   today    turns today, how long the agents worked, approvals given on her
//            (Antigravity's never come to her)
//   tokens   today's tokens, Claude Code's, Codex's and Antigravity's (tokens.rs)
//
// Hers too, never expiring and never a script's: each mail account's
// (mail.rs, "inbox-<id>").
use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

pub const ICONS: [&str; 26] = [
    "cpu", "weather", "note", "calendar", "bell", "stock", "today", "clock", "mail", "music", "code", "star", "dot", "chart", "battery", "timer", "flag", "server",
    "check", "terminal", "coin", "gauge", "memory", "down", "up", "bolt",
];
// The built-in ones: id, icon, colour.
pub const BUILT_IN: [(&str, &str, &str); 3] = [("today", "today", "#34d27b"), ("tokens", "code", "#ff9f0a"), ("monitor", "gauge", "#5e9bff")];
const MAX_SCRIPTED: usize = 32;
const DEFAULT_TTL: u64 = 300;
const NUDGE_EVERY_MS: u64 = 60_000;

pub struct Widget {
    pub id: String,
    label: Value,
    value: Value,
    // A script's few words more, for the open island alone.
    detail: String,
    icon: String,
    color: String,
    // When it goes, if no new one comes (ms); never, for the built-in ones.
    until: Option<u64>,
    at: u64,
    private: bool,
}

#[derive(Default)]
pub struct Widgets {
    // In the order first seen; the built-in ones first.
    list: Vec<Widget>,
    nudged: HashMap<String, u64>,
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

pub fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn is_color(c: &str) -> bool {
    let hex = c.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

// What a script sent, checked.
pub struct Put {
    pub id: String,
    // The words to open the island with, when it asked to.
    pub nudge: Option<String>,
}

impl Widgets {
    // A widget from a script: Err with why, when it will not do.
    pub fn put(&mut self, v: &Value, now: u64) -> Result<Put, &'static str> {
        let s = |key: &str| v.get(key).and_then(Value::as_str).unwrap_or("");
        let id = s("id");
        if !is_id(id) {
            return Err("id: a-z 0-9 _ -, up to 40");
        }
        if BUILT_IN.iter().any(|b| b.0 == id) || self.list.iter().any(|w| w.id == id && w.until.is_none()) {
            return Err("id: taken by a built-in widget");
        }
        if v.get("remove").and_then(Value::as_bool) == Some(true) {
            self.list.retain(|w| w.id != id);
            return Ok(Put { id: id.into(), nudge: None });
        }
        let label = clip(s("label"), 40);
        if label.is_empty() {
            return Err("label: some words, up to 40");
        }
        let ttl = v.get("ttl").and_then(Value::as_u64).unwrap_or(DEFAULT_TTL).clamp(10, 86_400);
        let widget = Widget {
            id: id.into(),
            label: json!(label),
            value: json!(clip(s("value"), 24)),
            detail: clip(s("detail"), 24),
            icon: if ICONS.contains(&s("icon")) { s("icon").into() } else { "dot".into() },
            color: if is_color(s("color")) { s("color").into() } else { String::new() },
            until: Some(now + ttl * 1000),
            at: now,
            private: v.get("private").and_then(Value::as_bool) == Some(true),
        };
        let scripted = self.list.iter().filter(|w| w.until.is_some()).count();
        match self.list.iter_mut().find(|w| w.id == id) {
            Some(old) => *old = widget,
            None if scripted >= MAX_SCRIPTED => return Err("too many widgets: 32 at most"),
            None => self.list.push(widget),
        }
        let nudge = match v.get("nudge") {
            Some(Value::Bool(true)) => Some(String::new()),
            Some(Value::String(words)) if !words.trim().is_empty() => Some(clip(words, 60)),
            _ => None,
        };
        // Once a minute at most, per widget.
        let nudge = nudge.filter(|_| self.nudged.get(id).is_none_or(|&t| now.saturating_sub(t) >= NUDGE_EVERY_MS));
        if nudge.is_some() {
            self.nudged.insert(id.into(), now);
        }
        Ok(Put { id: id.into(), nudge })
    }

    // A built-in widget's words, now. True when they changed.
    pub fn set_built_in(&mut self, id: &str, value: Value, now: u64) -> bool {
        if let Some(w) = self.list.iter_mut().find(|w| w.id == id) {
            if w.value == value {
                return false;
            }
            w.value = value;
            w.at = now;
            return true;
        }
        let at = self.list.iter().take_while(|w| is_built_in(&w.id)).count();
        self.list.insert(at, Widget { value, at: now, ..built_in(id) });
        true
    }

    // One of hers that is not built in (a mail account's): its words now,
    // kept until she takes it away. True when they changed.
    #[allow(clippy::too_many_arguments)]
    pub fn put_owned(&mut self, id: &str, icon: &str, color: &str, label: Value, value: Value, private: bool, now: u64) -> bool {
        let widget = Widget { id: id.into(), label, value, detail: String::new(), icon: icon.into(), color: color.into(), until: None, at: now, private };
        match self.list.iter_mut().find(|w| w.id == id) {
            Some(old) if old.label == widget.label && old.value == widget.value && old.color == widget.color => false,
            Some(old) => {
                *old = widget;
                true
            }
            None => {
                self.list.push(widget);
                true
            }
        }
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.id != id);
        self.list.len() != before
    }

    pub fn remove_built_in(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.id != id);
        self.list.len() != before
    }

    // Those whose script stopped sending go. True when any did.
    pub fn expire(&mut self, now: u64) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.until.is_none_or(|t| t > now));
        self.list.len() != before
    }

    // A widget's words, and whether they are private.
    pub fn label_of(&self, id: &str) -> Option<(Value, Value, bool)> {
        self.list.iter().find(|w| w.id == id).map(|w| (w.label.clone(), w.value.clone(), w.private))
    }

    // A script's widgets that are gone with it. True when any were there.
    pub fn remove_scripted(&mut self, gone: impl Fn(&str) -> bool) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.until.is_none() || !gone(&w.id));
        self.list.len() != before
    }

    // All of them, in the order the person put them in (then as first seen),
    // each saying whether it is on and which plugin it comes from (`owner`:
    // one she runs, scripts.rs), which places it when the plugin was placed.
    // The built-in ones not measured (off, or nothing to read yet, like a
    // battery on a desktop) are there too, with no value, so they can be
    // turned on.
    pub fn view(&self, now: u64, off: &[String], order: &[String], owner: &dyn Fn(&str) -> Option<&'static str>) -> Value {
        let unread: Vec<Widget> = BUILT_IN.iter().filter(|b| !self.list.iter().any(|w| w.id == b.0)).map(|b| built_in(b.0)).collect();
        let measured = self.list.iter().take_while(|w| is_built_in(&w.id)).count();
        let mut all: Vec<&Widget> = self.list[..measured].iter().chain(&unread).chain(&self.list[measured..]).collect();
        let rank = |w: &Widget| order.iter().position(|o| *o == w.id || Some(o.as_str()) == owner(&w.id)).unwrap_or(usize::MAX);
        all.sort_by_key(|w| rank(w));
        Value::Array(
            all.into_iter()
                .map(|w| {
                    json!({
                        "id": w.id,
                        "label": w.label,
                        "value": w.value,
                        "detail": w.detail,
                        "icon": w.icon,
                        "color": w.color,
                        "builtIn": is_built_in(&w.id),
                        // builtIn, mail (hers, a mail account's), or script.
                        "from": if is_built_in(&w.id) { "builtIn" } else if w.until.is_none() { "mail" } else { "script" },
                        "plugin": owner(&w.id),
                        "on": !off.contains(&w.id),
                        "leftMs": w.until.map(|t| t.saturating_sub(now)),
                        "at": w.at,
                        "private": w.private,
                    })
                })
                .collect(),
        )
    }
}

fn is_built_in(id: &str) -> bool {
    BUILT_IN.iter().any(|b| b.0 == id)
}

// A built-in widget with nothing to say yet.
fn built_in(id: &str) -> Widget {
    let (icon, color) = BUILT_IN.iter().find(|b| b.0 == id).map_or(("dot", ""), |b| (b.1, b.2));
    Widget { id: id.into(), label: json!({ "key": format!("widget.{id}") }), value: Value::Null, detail: String::new(), icon: icon.into(), color: color.into(), until: None, at: 0, private: false }
}

// --- Today: what the day has seen ---------------------------------------------------

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Today {
    pub date: String,
    pub turns: u64,
    // Of those, Antigravity's: its approvals never come to her.
    pub agy_turns: u64,
    pub worked_ms: u64,
    pub approvals: u64,
}

impl Today {
    pub fn load(dir: &Path, date: &str) -> Today {
        let saved: Value = std::fs::read_to_string(dir.join("today.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        if saved["date"].as_str() != Some(date) {
            return Today { date: date.into(), ..Today::default() };
        }
        let n = |k: &str| saved[k].as_u64().unwrap_or(0);
        Today { date: date.into(), turns: n("turns"), agy_turns: n("agyTurns"), worked_ms: n("workedMs"), approvals: n("approvals") }
    }

    pub fn save(&self, dir: &Path) {
        let body = json!({ "date": self.date, "turns": self.turns, "agyTurns": self.agy_turns, "workedMs": self.worked_ms, "approvals": self.approvals });
        let _ = std::fs::write(dir.join("today.json"), body.to_string());
    }

    // A new day starts from nothing. True when it did.
    pub fn roll(&mut self, date: &str) -> bool {
        if self.date == date {
            return false;
        }
        *self = Today { date: date.into(), ..Today::default() };
        true
    }

    // Turns, the time as h:mm (under a minute, in seconds: a few short turns
    // are no 0:00), approvals; with Antigravity's turns alone, "—" for them.
    pub fn words(&self) -> Value {
        let minutes = self.worked_ms / 60_000;
        let time = match self.worked_ms {
            1..=59_999 => json!({ "key": "widget.todaySeconds", "vars": { "n": (self.worked_ms / 1000).max(1).to_string() } }),
            _ => json!(format!("{}:{:02}", minutes / 60, minutes % 60)),
        };
        let key = if self.turns > 0 && self.agy_turns == self.turns && self.approvals == 0 { "widget.todayValueAgy" } else { "widget.todayValue" };
        json!({ "key": key, "vars": { "turns": self.turns.to_string(), "time": time, "approvals": self.approvals.to_string() } })
    }
}

// The local date, YYYY-MM-DD.
pub fn local_date() -> String {
    let (y, m, d, _) = local_now();
    format!("{y:04}-{m:02}-{d:02}")
}

// Local midnight, in ms since 1970.
pub fn local_midnight() -> u64 {
    crate::now_ms().saturating_sub(local_now().3)
}

// The local year, month, day, and ms since midnight.
fn local_now() -> (u16, u16, u16, u64) {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            weekday: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            ms: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(time: *mut SystemTime);
        }
        let mut t = SystemTime::default();
        // SAFETY: fills our own struct.
        unsafe { GetLocalTime(&mut t) };
        let ms = ((u64::from(t.hour) * 60 + u64::from(t.minute)) * 60 + u64::from(t.second)) * 1000 + u64::from(t.ms);
        (t.year, t.month, t.day, ms)
    }
    #[cfg(unix)]
    {
        // The system's local time (its time zone, summer time and all).
        let now = crate::now_ms();
        let secs = (now / 1000) as libc::time_t;
        // SAFETY: localtime_r fills our own tm from our own time_t.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
            return utc_now(now);
        }
        let ms = ((tm.tm_hour as u64 * 60 + tm.tm_min as u64) * 60 + tm.tm_sec as u64) * 1000 + now % 1000;
        ((tm.tm_year + 1900) as u16, (tm.tm_mon + 1) as u16, tm.tm_mday as u16, ms)
    }
    #[cfg(not(any(windows, unix)))]
    {
        utc_now(crate::now_ms())
    }
}

// The day in UTC, when the local time is not to be had.
#[cfg(not(windows))]
fn utc_now(now: u64) -> (u16, u16, u16, u64) {
    // Days since 1970 to a date (Howard Hinnant's civil_from_days, as crash.rs).
    let z = (now / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as u16, m as u16, d as u16, now % 86_400_000)
}

// --- The machine: CPU and memory ----------------------------------------------------------

#[cfg(windows)]
mod sys {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct PowerStatus {
        ac_line: u8,
        battery_flag: u8,
        percent: u8,
        saver: u8,
        life_secs: u32,
        full_life_secs: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32;
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetIfTable(table: *mut u8, size: *mut u32, order: i32) -> u32;
    }

    // The battery as Windows' quick settings show it, or None without a
    // battery. Its state unknown (255), the level still counts.
    pub fn power() -> Option<super::Power> {
        let mut p = PowerStatus::default();
        // SAFETY: fills our own struct.
        if unsafe { GetSystemPowerStatus(&mut p) } == 0 || (p.battery_flag != 255 && p.battery_flag & 128 != 0) || p.percent > 100 {
            return None;
        }
        let plugged = p.ac_line == 1;
        let known = p.battery_flag != 255;
        Some(super::Power {
            percent: p.percent,
            plugged,
            charging: known && p.battery_flag & 8 != 0,
            saver: p.saver & 1 != 0,
            critical: known && p.battery_flag & 4 != 0,
            left: (p.life_secs != u32::MAX && !plugged).then_some(p.life_secs),
        })
    }

    // Windows' own battery glyphs (SysBatt Fluent Icons), the font its
    // taskbar draws the battery in, from where Windows keeps it; None before
    // Windows 11 has it.
    pub fn battery_font() -> Option<Vec<u8>> {
        let windows = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Windows".into());
        [r"SystemApps\MicrosoftWindows.Client.Core_cw5n1h2txyewy\SystemTray\Assets\Sysbatt.ttf", r"SystemResources\Windows.UI.ControlCenter\Assets\Sysbatt.ttf"]
            .iter()
            .find_map(|at| std::fs::read(windows.join(at)).ok())
    }

    // Windows' "Battery percentage" (Settings > System > Power & battery):
    // the percentage beside the battery on the taskbar.
    pub fn battery_percent_shown() -> bool {
        use winreg::enums::HKEY_CURRENT_USER;
        winreg::RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced")
            .and_then(|k| k.get_value::<u32, _>("IsBatteryPercentageEnabled"))
            .is_ok_and(|on| on != 0)
    }

    // (percent, charging, seconds left if known), or None without a battery.
    pub fn battery() -> Option<(u8, bool, Option<u32>)> {
        let mut p = PowerStatus::default();
        // SAFETY: fills our own struct.
        if unsafe { GetSystemPowerStatus(&mut p) } == 0 || p.battery_flag & 128 != 0 || p.battery_flag == 255 || p.percent > 100 {
            return None;
        }
        let is_charging = p.ac_line == 1;
        Some((p.percent, is_charging, (p.life_secs != u32::MAX && !is_charging).then_some(p.life_secs)))
    }

    // Bytes in and out so far, over the network adapters that are up: each
    // adapter once (Windows lists one several times, through its filters).
    pub fn network_bytes() -> Option<(u64, u64)> {
        // MIB_IFROW: 860 bytes; the type at 516, the MAC at 532 (8), the
        // state at 544, bytes in at 552, bytes out at 576.
        const ROW: usize = 860;
        let mut size = 0u32;
        // SAFETY: first the size, then a buffer of that size.
        unsafe { GetIfTable(std::ptr::null_mut(), &mut size, 0) };
        let mut buf = vec![0u8; size as usize + ROW];
        // SAFETY: the buffer is as big as the call asked for.
        if unsafe { GetIfTable(buf.as_mut_ptr(), &mut size, 0) } != 0 {
            return None;
        }
        let dword = |at: usize| u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]);
        let count = dword(0) as usize;
        let mut by_mac: std::collections::HashMap<[u8; 8], (u32, u32)> = std::collections::HashMap::new();
        for i in 0..count {
            let row = 4 + i * ROW;
            if row + ROW > buf.len() {
                break;
            }
            let (kind, state) = (dword(row + 516), dword(row + 544));
            let mut mac = [0u8; 8];
            mac.copy_from_slice(&buf[row + 532..row + 540]);
            // Ethernet or Wi-Fi, up, with an address of its own.
            if !(kind == 6 || kind == 71) || state != 5 || mac == [0; 8] {
                continue;
            }
            let (got, sent) = (dword(row + 552), dword(row + 576));
            let seen = by_mac.entry(mac).or_insert((got, sent));
            *seen = (seen.0.max(got), seen.1.max(sent));
        }
        Some(by_mac.values().fold((0, 0), |(a, b), &(got, sent)| (a + u64::from(got), b + u64::from(sent))))
    }

    fn n(t: FileTime) -> u64 {
        (u64::from(t.high) << 32) | u64::from(t.low)
    }

    // (idle, busy) time so far, all cores. Not what Task Manager shows on
    // every machine: on some Windows 11 22H2 and 23H2 ones the idle time it
    // is made of (NtQuerySystemInformation's SystemProcessorPerformanceInformation)
    // grows while the cores are busy, and a CPU at 45% read 1 to 3% (#51);
    // HWiNFO and MSI Afterburner moved off it for the same. Only where
    // Windows' own count (Counter) is not to be had.
    pub fn times() -> Option<(u64, u64)> {
        let (mut idle, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default());
        // SAFETY: fills our own structs.
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
        // Kernel time includes the idle time.
        ok.then(|| (n(idle), n(kernel) + n(user) - n(idle)))
    }

    pub fn memory_load() -> Option<u32> {
        // SAFETY: a struct of the size the call asks for.
        unsafe {
            let mut s: MemoryStatus = std::mem::zeroed();
            s.length = std::mem::size_of::<MemoryStatus>() as u32;
            (GlobalMemoryStatusEx(&mut s) != 0).then_some(s.load)
        }
    }

    // Windows' build (26100 for 24H2), or 0 when it cannot be read.
    pub fn windows_build() -> u32 {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber"))
            .ok()
            .and_then(|b| b.trim().parse().ok())
            .unwrap_or(0)
    }

    // PDH, Windows' performance counters (pdh.dll): loaded from System32 the
    // first time a counter is opened, not at her start, as the monitor is
    // off by default; None should it or one of its functions be missing.
    type OpenQuery = unsafe extern "system" fn(source: *const u16, user: usize, query: *mut isize) -> u32;
    type AddCounter = unsafe extern "system" fn(query: isize, path: *const u16, user: usize, counter: *mut isize) -> u32;
    type Collect = unsafe extern "system" fn(query: isize) -> u32;
    type Formatted = unsafe extern "system" fn(counter: isize, format: u32, kind: *mut u32, value: *mut CounterValue) -> u32;
    type CloseQuery = unsafe extern "system" fn(query: isize) -> u32;

    struct Pdh {
        open: OpenQuery,
        add: AddCounter,
        collect: Collect,
        formatted: Formatted,
        close: CloseQuery,
    }

    // PDH_FMT_COUNTERVALUE: its status, then the value (a union of 8 bytes).
    #[repr(C)]
    #[derive(Default)]
    struct CounterValue {
        status: u32,
        value: f64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const ();
    }

    fn pdh() -> Option<&'static Pdh> {
        static PDH: std::sync::OnceLock<Option<Pdh>> = std::sync::OnceLock::new();
        PDH.get_or_init(|| {
            const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x800;
            let name: Vec<u16> = "pdh.dll".encode_utf16().chain([0]).collect();
            // SAFETY: Windows' own library, from System32 alone; each
            // export by its name, cast to the shape PDH documents for it.
            unsafe {
                let module = LoadLibraryExW(name.as_ptr(), 0, LOAD_LIBRARY_SEARCH_SYSTEM32);
                if module == 0 {
                    return None;
                }
                let f = |name: &std::ffi::CStr| Some(GetProcAddress(module, name.as_ptr() as *const u8)).filter(|f| !f.is_null());
                Some(Pdh {
                    open: std::mem::transmute::<*const (), OpenQuery>(f(c"PdhOpenQueryW")?),
                    add: std::mem::transmute::<*const (), AddCounter>(f(c"PdhAddEnglishCounterW")?),
                    collect: std::mem::transmute::<*const (), Collect>(f(c"PdhCollectQueryData")?),
                    formatted: std::mem::transmute::<*const (), Formatted>(f(c"PdhGetFormattedCounterValue")?),
                    close: std::mem::transmute::<*const (), CloseQuery>(f(c"PdhCloseQuery")?),
                })
            }
        })
        .as_ref()
    }

    // One of Windows' counters, by its English path (the same whatever
    // Windows' language), in a query of its own: opened once (the counters'
    // names looked up then: ~0.3 s on this thread, ~4 MB that PDH keeps),
    // collected each turn (~0.2 ms).
    pub struct Counter {
        pdh: &'static Pdh,
        query: isize,
        counter: isize,
    }

    impl Counter {
        pub fn open(path: &str) -> Option<Counter> {
            let pdh = pdh()?;
            let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
            let (mut query, mut counter) = (0isize, 0isize);
            // SAFETY: PDH fills our own handles; the path ends in 0. A query
            // whose counter would not be added is closed again.
            unsafe {
                if (pdh.open)(std::ptr::null(), 0, &mut query) != 0 {
                    return None;
                }
                if (pdh.add)(query, path.as_ptr(), 0, &mut counter) != 0 {
                    (pdh.close)(query);
                    return None;
                }
                // The first sample: a rate needs two.
                (pdh.collect)(query);
            }
            Some(Counter { pdh, query, counter })
        }

        // Its value since the last time, or None when Windows has none to
        // give (too soon after the first sample, or the counter failed).
        pub fn value(&mut self) -> Option<f64> {
            const PDH_FMT_DOUBLE: u32 = 0x200;
            let (mut kind, mut v) = (0u32, CounterValue::default());
            // SAFETY: our own query and counter, open till dropped; the value
            // is written into our own struct.
            let ok = unsafe { (self.pdh.collect)(self.query) == 0 && (self.pdh.formatted)(self.counter, PDH_FMT_DOUBLE, &mut kind, &mut v) == 0 };
            // PDH_CSTATUS_VALID_DATA or PDH_CSTATUS_NEW_DATA.
            (ok && v.status <= 1).then_some(v.value)
        }
    }

    impl Drop for Counter {
        fn drop(&mut self) {
            // SAFETY: our own query, closed once.
            unsafe { (self.pdh.close)(self.query) };
        }
    }
}

#[cfg(not(windows))]
mod sys {
    pub fn times() -> Option<(u64, u64)> {
        None
    }
    pub fn memory_load() -> Option<u32> {
        None
    }
    pub fn battery() -> Option<(u8, bool, Option<u32>)> {
        None
    }
    pub fn power() -> Option<super::Power> {
        None
    }
    pub fn battery_font() -> Option<Vec<u8>> {
        None
    }
    pub fn battery_percent_shown() -> bool {
        false
    }
    pub fn network_bytes() -> Option<(u64, u64)> {
        None
    }
    pub fn windows_build() -> u32 {
        0
    }
    pub struct Counter;
    impl Counter {
        pub fn open(_path: &str) -> Option<Counter> {
            None
        }
        pub fn value(&mut self) -> Option<f64> {
            None
        }
    }
}

// The counter Task Manager's CPU figure is, by Windows' build. Its
// Performance and Processes pages showed "% Processor Utility" from
// Windows 8 on: the work done, against the cores' base speed, so past
// 100% (held at 100) while they run faster. Windows 11 24H2 (26100) went
// over to the share of time busy, "% Processor Time", in its May 2025
// update (KB5058411): tried on 26300 here, Task Manager 56% with two of
// four cores busy at 4.5 GHz, % Processor Time 55%, % Processor Utility 101%.
pub fn processor_counter(build: u32) -> &'static str {
    if build >= 26100 {
        r"\Processor Information(_Total)\% Processor Time"
    } else {
        r"\Processor Information(_Total)\% Processor Utility"
    }
}

// The processor's use, 0 to 100: Windows' count when one was read (Task
// Manager's figure; past 100 under Utility, held at 100), else the share
// of time busy between two readings of GetSystemTimes, (idle, busy) passed;
// None with neither.
fn cpu_share(counted: Option<f64>, between: Option<(u64, u64)>) -> Option<u64> {
    if let Some(p) = counted.filter(|p| p.is_finite()) {
        return Some(p.round().clamp(0.0, 100.0) as u64);
    }
    let (idle, busy) = between?;
    Some(if idle + busy == 0 { 0 } else { (busy * 100 + (idle + busy) / 2) / (idle + busy) })
}

// The battery: its level, plugged in, charging (a battery kept from charging
// while plugged in, as a laptop's battery care does at 80%, is plugged in and
// not charging), the battery saver on, critically low, the seconds left when
// Windows knows them.
pub struct Power {
    percent: u8,
    plugged: bool,
    charging: bool,
    saver: bool,
    critical: bool,
    left: Option<u32>,
}

// The battery for the taskbar's quick settings, beside the network and the
// volume as Windows' own shows it there on a laptop (taskbar.rs keys_loop,
// island.js showBattery), with whether Windows shows its percentage beside
// it; null without a battery (a desktop), and none is shown.
pub fn battery_state() -> Value {
    match sys::power() {
        Some(p) => json!({
            "percent": p.percent,
            "plugged": p.plugged,
            "charging": p.charging,
            "saver": p.saver,
            "critical": p.critical,
            "left": p.left,
            "shown": sys::battery_percent_shown(),
        }),
        None => Value::Null,
    }
}

// Windows' battery font (taskbar_battery_font), or nothing to have it from.
pub fn battery_font() -> Vec<u8> {
    sys::battery_font().unwrap_or_default()
}

// Bytes a second, in a few characters: 512B, 8.4K, 80K, 1.2M, 35M, 1.1G.
pub fn rate(bytes_per_sec: f64) -> String {
    let (n, unit) = match bytes_per_sec.max(0.0) {
        b if b >= 1e9 => (b / 1e9, "G"),
        b if b >= 1e6 => (b / 1e6, "M"),
        b if b >= 1e3 => (b / 1e3, "K"),
        b => return format!("{b:.0}B"),
    };
    if n < 10.0 { format!("{n:.1}{unit}") } else { format!("{n:.0}{unit}") }
}

// The monitor's settings: which parts are on, and how often it reads; the
// hardware's sensors, from LibreHardwareMonitor (lhm.rs); and whether
// nobody is at the keyboard (its thread says, main.rs).
pub struct Watching {
    pub cpu: bool,
    pub memory: bool,
    pub net: bool,
    pub battery: bool,
    pub every: u64,
    pub lhm: crate::lhm::Conf,
    pub quiet: bool,
}

impl Watching {
    pub fn from(v: &Value) -> Watching {
        let on = |k: &str| v[k] != false;
        Watching {
            cpu: on("cpu"),
            memory: on("mem"),
            net: on("net"),
            battery: on("battery"),
            every: v["every"].as_u64().unwrap_or(2).clamp(1, 10),
            lhm: crate::lhm::Conf::from(v),
            quiet: false,
        }
    }
}

// The machine as the monitor reads it: CPU use and the network's pace
// between two readings, memory in use, the battery.
pub struct Monitor {
    times: Option<(u64, u64)>,
    // Windows' own count of the CPU's use (processor_counter), while the CPU
    // part is on: opened at its first reading, tried again only once it is
    // turned off and on. Given up after three readings in a row without a
    // value; GetSystemTimes then, as where it could not be opened.
    counter: Option<sys::Counter>,
    counter_tried: bool,
    misses: u8,
    bytes: Option<(u64, u64, std::time::Instant)>,
    // The last CPU use and pace read: kept for a reading that fails once,
    // so a part on stays in sight (pinned, the island keeps its width).
    cpu: Option<String>,
    net: Option<(String, String)>,
    lhm: crate::lhm::Lhm,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor { times: None, counter: None, counter_tried: false, misses: 0, bytes: None, cpu: None, net: None, lhm: crate::lhm::Lhm::new() }
    }

    // The parts that are on, as icon and number: cpu 12%, memory 45%, down
    // 1.2M and up 80K a second, battery (a bolt while charging) 85%; then
    // the hardware's sensors picked, with their kind too: { icon: "gpu",
    // kind: "temp", text: "58°" } (lhm.rs). A part needing two readings
    // shows from its second, and keeps its last when a reading fails. None
    // with nothing to show.
    pub fn read(&mut self, w: &Watching) -> Option<Value> {
        let mut parts = Vec::new();
        let mut part = |icon: &str, text: String| parts.push(json!({ "icon": icon, "text": text }));
        if w.cpu && !self.counter_tried {
            self.counter_tried = true;
            self.counter = sys::Counter::open(processor_counter(sys::windows_build()));
        }
        let counted = self.counter.as_mut().and_then(sys::Counter::value);
        if self.counter.is_some() {
            self.misses = if counted.is_some() { 0 } else { self.misses + 1 };
            if self.misses >= 3 {
                self.counter = None;
            }
        }
        // GetSystemTimes each turn all the same (a call of nothing), so it
        // has a reading to go on from should the counter be given up.
        let times = if w.cpu { sys::times() } else { None };
        let between = times.zip(self.times).map(|(now, (idle, busy))| (now.0.saturating_sub(idle), now.1.saturating_sub(busy)));
        // With the counter open, its value or the last one kept (its first
        // sample has none): never a turn of the other way's.
        if let Some(share) = cpu_share(counted, if self.counter.is_some() { None } else { between }) {
            self.cpu = Some(format!("{share}%"));
        }
        if !w.cpu {
            self.cpu = None;
            self.counter = None;
            self.counter_tried = false;
            self.misses = 0;
        }
        if times.is_some() || !w.cpu {
            self.times = times;
        }
        if let Some(cpu) = &self.cpu {
            part("cpu", cpu.clone());
        }
        if let Some(memory) = w.memory.then(sys::memory_load).flatten() {
            part("memory", format!("{memory}%"));
        }
        let bytes = if w.net { sys::network_bytes().map(|(a, b)| (a, b, std::time::Instant::now())) } else { None };
        if let (Some((got, sent, now)), Some((g, s, at))) = (bytes, self.bytes) {
            let secs = now.duration_since(at).as_secs_f64().max(0.5);
            // The counters are 32 bits: they come round again after 4 GB.
            let d = |a: u64, b: u64| (a as u32).wrapping_sub(b as u32) as f64 / secs;
            self.net = Some((rate(d(got, g)), rate(d(sent, s))));
        }
        if !w.net {
            self.net = None;
        }
        if bytes.is_some() || !w.net {
            self.bytes = bytes;
        }
        if let Some((down, up)) = &self.net {
            part("down", down.clone());
            part("up", up.clone());
        }
        if let Some((percent, is_charging, _)) = w.battery.then(sys::battery).flatten() {
            part(if is_charging { "bolt" } else { "battery" }, format!("{percent}%"));
        }
        self.lhm.turn(&w.lhm, w.quiet);
        parts.extend(self.lhm.parts(&w.lhm));
        (!parts.is_empty()).then(|| json!({ "parts": parts }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The view without the built-in ones, which are always there.
    fn scripted(view: Value) -> Value {
        Value::Array(view.as_array().unwrap().iter().filter(|w| w["builtIn"] == false).cloned().collect())
    }

    #[test]
    fn the_local_date_is_a_real_date() {
        let date = local_date();
        let parts: Vec<u16> = date.split('-').map(|p| p.parse().unwrap()).collect();
        assert_eq!(parts.len(), 3, "{date}");
        assert!(parts[0] >= 2026 && (1..=12).contains(&parts[1]) && (1..=31).contains(&parts[2]), "{date}");
        assert!(crate::now_ms() - local_midnight() < 86_400_000);
    }

    #[cfg(not(windows))]
    #[test]
    fn the_utc_day_is_counted_from_1970() {
        assert_eq!(utc_now(0), (1970, 1, 1, 0));
        // 2026-10-13T11:20:00.250Z
        assert_eq!(utc_now(1_791_890_400_250), (2026, 10, 13, 40_800_250));
        // 2024-02-29T00:00:00Z, a leap day
        assert_eq!(utc_now(1_709_164_800_000), (2024, 2, 29, 0));
    }

    #[test]
    fn a_script_puts_replaces_and_removes_its_widget() {
        let mut w = Widgets::default();
        let put = w.put(&json!({ "id": "weather", "label": "上海 · 多云", "value": "22°", "icon": "weather", "color": "#ffb340" }), 1_000).unwrap();
        assert_eq!((put.id.as_str(), put.nudge), ("weather", None));
        assert_eq!(scripted(w.view(1_000, &[], &[], &|_| None))[0]["detail"], "", "no detail sent");
        w.put(&json!({ "id": "weather", "label": "上海 · 晴", "value": "24°", "detail": " 14°~22° ", "icon": "nope", "color": "red" }), 2_000).unwrap();
        let view = scripted(w.view(2_000, &[], &[], &|_| None));
        assert_eq!(view.as_array().unwrap().len(), 1);
        assert_eq!((view[0]["label"].as_str(), view[0]["icon"].as_str(), view[0]["color"].as_str()), (Some("上海 · 晴"), Some("dot"), Some("")));
        assert_eq!(view[0]["detail"], "14°~22°");
        assert_eq!(view[0]["leftMs"], 300_000);
        w.put(&json!({ "id": "weather", "label": "x", "detail": "一二三四五六七八九十一二三四五六七八九十一二三四五六" }), 2_500).unwrap();
        assert_eq!(scripted(w.view(2_500, &[], &[], &|_| None))[0]["detail"].as_str().unwrap().chars().count(), 24);
        w.put(&json!({ "id": "weather", "remove": true }), 3_000).unwrap();
        assert!(scripted(w.view(3_000, &[], &[], &|_| None)).as_array().unwrap().is_empty());
    }

    #[test]
    fn bad_widgets_say_why() {
        let mut w = Widgets::default();
        assert!(w.put(&json!({ "id": "Has Space", "label": "x" }), 0).is_err());
        assert!(w.put(&json!({ "id": "x" }), 0).is_err());
        assert!(w.put(&json!({ "id": "monitor", "label": "mine" }), 0).is_err());
        for n in 0..MAX_SCRIPTED {
            w.put(&json!({ "id": format!("w{n}"), "label": "x" }), 0).unwrap();
        }
        assert!(w.put(&json!({ "id": "one-more", "label": "x" }), 0).is_err());
    }

    #[test]
    fn widgets_go_when_their_script_stops_and_nudge_once_a_minute() {
        let mut w = Widgets::default();
        let put = w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": "5 分钟后开会" }), 0).unwrap();
        assert_eq!(put.nudge.as_deref(), Some("5 分钟后开会"));
        assert!(w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": true }), 30_000).unwrap().nudge.is_none());
        assert_eq!(w.put(&json!({ "id": "cal", "label": "15:00 设计评审", "ttl": 10, "nudge": true }), 61_000).unwrap().nudge.as_deref(), Some(""));
        assert!(!w.expire(70_000));
        assert!(w.expire(71_001));
    }

    #[test]
    fn built_ins_stay_first_and_the_order_is_the_persons() {
        let mut w = Widgets::default();
        w.put(&json!({ "id": "a", "label": "A" }), 0).unwrap();
        assert!(w.set_built_in("monitor", json!("1"), 0));
        assert!(!w.set_built_in("monitor", json!("1"), 5));
        let ids = |v: Value| v.as_array().unwrap().iter().map(|w| w["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        // The ones not measured are listed too, without a value.
        let none = |_: &str| None;
        let all = w.view(0, &[], &[], &none);
        assert_eq!(ids(all.clone()), ["monitor", "today", "tokens", "a"]);
        assert_eq!((all[0]["value"].clone(), all[1]["value"].clone(), all[1]["icon"].clone()), (json!("1"), Value::Null, json!("today")));
        assert_eq!(ids(w.view(0, &[], &["a".into()], &none))[0], "a");
        assert_eq!(w.view(0, &["tokens".into()], &[], &none)[2]["on"], false);
        // A plugin's widgets go where the plugin was put, and say whose they are.
        let mine = |id: &str| (id == "a").then_some("mine");
        let view = w.view(0, &[], &["mine".into()], &mine);
        assert_eq!((view[0]["id"].clone(), view[0]["plugin"].clone()), (json!("a"), json!("mine")));
        assert!(w.remove_scripted(|id| id == "a") && !w.remove_scripted(|id| id == "sys"));
        // A mail account's: hers, kept, not a script's to take over.
        assert!(w.put_owned("inbox-1", "mail", "", json!("王总：周五的方案"), json!("3"), true, 0));
        assert!(!w.put_owned("inbox-1", "mail", "", json!("王总：周五的方案"), json!("3"), true, 9));
        assert!(w.put(&json!({ "id": "inbox-1", "label": "mine" }), 0).is_err());
        assert_eq!(w.view(0, &[], &[], &none)[3]["from"], "mail");
        assert!(!w.expire(u64::MAX) && w.remove("inbox-1"));
        w.put(&json!({ "id": "a", "label": "A" }), 0).unwrap();
        // Built-in ones never go; a script's does.
        assert!(w.expire(u64::MAX));
        assert_eq!(ids(w.view(0, &[], &[], &none)), ["monitor", "today", "tokens"]);
    }

    #[test]
    fn private_widgets_say_so_and_rates_read_short() {
        let mut w = Widgets::default();
        w.put(&json!({ "id": "mail", "label": "王总：周五的方案", "value": "3 封未读", "private": true }), 0).unwrap();
        assert_eq!(scripted(w.view(0, &[], &[], &|_| None))[0]["private"], true);
        assert_eq!(w.label_of("mail").map(|l| l.2), Some(true));
        assert_eq!([rate(80.0), rate(8_400.0), rate(81_234.0), rate(1_234_567.0), rate(250_000_000.0)], ["80B", "8.4K", "81K", "1.2M", "250M"]);
        let all = Watching::from(&json!({ "every": 99 }));
        assert_eq!((all.cpu, all.battery, all.every), (true, true, 10));
    }

    // On this machine: the second reading has CPU use and the network's pace.
    // Windows' counters (sys): elsewhere there is nothing to read yet.
    #[cfg(windows)]
    #[test]
    fn the_monitor_reads_this_machine() {
        let all = Watching::from(&json!({ "every": 99, "lhm": false }));
        let mut m = Monitor::new();
        let _ = m.read(&all);
        let icons = |v: Option<Value>| v.map(|v| v["parts"].as_array().unwrap().iter().map(|p| p["icon"].as_str().unwrap().to_string()).collect::<Vec<_>>()).unwrap_or_default();
        let second = icons(m.read(&all));
        assert!(second.starts_with(&["cpu".into(), "memory".into()]), "{second:?}");
        let only = Watching::from(&json!({ "cpu": false, "net": false, "battery": false, "lhm": false }));
        assert_eq!(icons(m.read(&only)), ["memory"]);
    }

    // The CPU's use as Task Manager has it: Windows' count, rounded and held
    // at 100; without it, the time busy between two readings.
    #[test]
    fn the_cpu_is_windows_count_else_the_time_busy() {
        assert_eq!(cpu_share(Some(44.6), Some((990, 10))), Some(45));
        assert_eq!(cpu_share(Some(173.2), None), Some(100));
        assert_eq!(cpu_share(Some(-0.4), None), Some(0));
        // No count (none read yet, the counter given up, or not a number):
        // the time busy, rounded.
        assert_eq!(cpu_share(None, Some((550, 450))), Some(45));
        assert_eq!(cpu_share(Some(f64::NAN), Some((1, 2))), Some(67));
        assert_eq!(cpu_share(None, Some((0, 0))), Some(0));
        assert_eq!(cpu_share(None, None), None);
        // Utility before 24H2, the time busy from it on.
        assert!(processor_counter(22631).ends_with(r"\% Processor Utility"));
        assert!(processor_counter(19045).ends_with(r"\% Processor Utility"));
        assert!(processor_counter(26100).ends_with(r"\% Processor Time"));
        assert!(processor_counter(26300).starts_with(r"\Processor Information(_Total)\"));
    }

    // Windows' counter on this machine: opened (its first sample taken),
    // a share of 0 to 100 at its second; one Windows has not, never opened.
    #[cfg(windows)]
    #[test]
    fn windows_counts_this_machines_cpu() {
        assert!(sys::windows_build() >= 10240);
        let mut counter = sys::Counter::open(processor_counter(sys::windows_build())).expect("the processor's counter");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let share = cpu_share(counter.value(), None).expect("a value at the second sample");
        assert!(share <= 100, "{share}");
        assert!(sys::Counter::open(r"\Processor Information(_Total)\% No Such Counter").is_none());
    }

    #[test]
    fn today_counts_and_starts_again_each_day() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-today-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = Today::load(&dir, "2026-10-09");
        t.turns = 12;
        t.worked_ms = 103 * 60_000;
        t.approvals = 5;
        t.save(&dir);
        assert_eq!(Today::load(&dir, "2026-10-09"), t);
        assert_eq!(t.words(), json!({ "key": "widget.todayValue", "vars": { "turns": "12", "time": "1:43", "approvals": "5" } }));
        assert_eq!(Today::load(&dir, "2026-10-10").turns, 0);
        assert!(t.roll("2026-10-10") && t.turns == 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn today_under_a_minute_in_seconds_and_antigravity_s_approvals_never_counted() {
        let words = |turns, agy_turns, worked_ms, approvals| Today { date: String::new(), turns, agy_turns, worked_ms, approvals }.words();
        let seconds = |n: &str| json!({ "key": "widget.todaySeconds", "vars": { "n": n } });
        assert_eq!(words(3, 0, 45_600, 1)["vars"]["time"], seconds("45"));
        assert_eq!(words(1, 0, 400, 0)["vars"]["time"], seconds("1"));
        assert_eq!((words(0, 0, 0, 0)["vars"]["time"].clone(), words(9, 0, 60_000, 0)["vars"]["time"].clone()), (json!("0:00"), json!("0:01")));
        // Antigravity's turns alone: no approvals to count, "—".
        assert_eq!(words(2, 2, 45_000, 0)["key"], "widget.todayValueAgy");
        assert_eq!(words(3, 2, 45_000, 0)["key"], "widget.todayValue");
        assert_eq!(words(0, 0, 0, 0)["key"], "widget.todayValue");
        let dir = std::env::temp_dir().join(format!("wakuwaku-today-agy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = Today { date: "2026-10-14".into(), turns: 4, agy_turns: 3, worked_ms: 1, approvals: 0 };
        t.save(&dir);
        assert_eq!(Today::load(&dir, "2026-10-14"), t);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
