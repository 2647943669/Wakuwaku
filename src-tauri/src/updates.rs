// Her updates, for the exe downloaded from GitHub (a build run from the
// source tree updates by a build, and is never looked for): Wakuwaku's
// latest release looked at as she starts, then once a day (settings:
// autoUpdateCheck, on unless turned off; "check now" any time). A newer one
// is told on the island once (updateTold). On the person's word (Update, on
// the General page): its exe downloaded into her data folder, checked
// against the SHA-256 GitHub gives for it (none given, or another: not
// installed), put where hers is, hers kept beside it as <name>.old.exe, and
// started with --after-update <her pid>: it waits for her to be gone (the
// port, the taskbar), and she quits as the menu does. A prompt waiting for
// an answer is answered first. The old exe goes once the new one has run a
// minute; should the new one not start, the old one is still there to be
// renamed back (DEVELOPMENT.md).
//
// For tests: WAKUWAKU_UPDATE_URL, the release's JSON from elsewhere (its
// exe from anywhere then); WAKUWAKU_UPDATE_FORCE=1, a build from the source
// tree looks and updates too.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::Shared;

const LATEST: &str = "https://api.github.com/repos/zhoupengjie/Wakuwaku/releases/latest";
pub const AFTER_UPDATE: &str = "--after-update";
const DAY_MS: u64 = 86_400_000;
const HOUR_MS: u64 = 3_600_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub page: String,
    pub exe: Option<String>,
    pub digest: Option<String>,
    pub size: u64,
}

struct State {
    // When last looked (ms), and when to look again.
    checked_at: u64,
    next_at: u64,
    checking: bool,
    latest: Option<Release>,
    // What went wrong last: { code, detail }.
    error: Option<Value>,
    // An update under way: download, verify, waiting (a prompt open), restart.
    stage: Option<&'static str>,
    got: u64,
    total: u64,
}

static STATE: Mutex<State> = Mutex::new(State { checked_at: 0, next_at: 0, checking: false, latest: None, error: None, stage: None, got: 0, total: 0 });
// Started by an update, after this pid: said in the log once she is up.
static AFTER: Mutex<Option<u32>> = Mutex::new(None);

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// A downloaded exe (or one told to, for tests): only those update.
pub fn may_update() -> bool {
    std::env::var("WAKUWAKU_UPDATE_FORCE").as_deref() == Ok("1") || !crate::data::from_source()
}

fn test_url() -> Option<String> {
    std::env::var("WAKUWAKU_UPDATE_URL").ok().filter(|u| !u.trim().is_empty())
}

// Whether one version is newer than another: their numbers one by one (so
// 0.10.0 is after 0.9.0), a missing one 0; one with a part after "-" (a
// prerelease) before the same without.
pub fn newer(theirs: &str, ours: &str) -> bool {
    fn parts(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let v = v.split('+').next().unwrap_or(v);
        let (core, pre) = match v.split_once('-') {
            Some((core, _)) => (core, true),
            None => (v, false),
        };
        (core.split('.').map(|n| n.trim().parse().unwrap_or(0)).collect(), pre)
    }
    let ((a, a_pre), (b, b_pre)) = (parts(theirs), parts(ours));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    !a_pre && b_pre
}

// The release GitHub's JSON says, if it counts (no draft, no prerelease):
// its version, its page, and its exe by name with the SHA-256 given for it.
pub fn release_of(v: &Value) -> Option<Release> {
    if v["draft"] == true || v["prerelease"] == true {
        return None;
    }
    let version = v["tag_name"].as_str()?.trim().trim_start_matches(['v', 'V']).to_string();
    if version.is_empty() {
        return None;
    }
    let name = format!("wakuwaku-v{version}-windows-x64.exe");
    let asset = v["assets"].as_array().and_then(|all| all.iter().find(|a| a["name"].as_str() == Some(name.as_str())));
    let digest = asset
        .and_then(|a| a["digest"].as_str())
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase);
    Some(Release {
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        exe: asset.and_then(|a| a["browser_download_url"].as_str()).map(str::to_string),
        digest,
        size: asset.and_then(|a| a["size"].as_u64()).unwrap_or(0),
        version,
    })
}

// From GitHub itself (its release pages, and the downloads they redirect
// to), unless a test gave the release from elsewhere.
fn from_github(url: &str) -> bool {
    if test_url().is_some() {
        return true;
    }
    url.strip_prefix("https://github.com/zhoupengjie/Wakuwaku/").is_some()
}

fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(seconds))).build().new_agent()
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let url = test_url().unwrap_or_else(|| LATEST.to_string());
    let body: Value = agent(20)
        .get(&url)
        .header("user-agent", &format!("wakuwaku/{}", current()))
        .header("accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    Ok(release_of(&body))
}

fn error(code: &str, detail: impl Into<String>) -> Value {
    json!({ "code": code, "detail": detail.into() })
}

// Looked at now (one look at a time); told on the island if newer.
pub fn check_now(sh: &Arc<Shared>) {
    {
        let mut s = STATE.lock().unwrap();
        if s.checking {
            return;
        }
        s.checking = true;
    }
    sh.push_settings();
    let got = fetch_latest();
    let now = crate::now_ms();
    {
        let mut s = STATE.lock().unwrap();
        s.checking = false;
        s.checked_at = now;
        match &got {
            Ok(release) => {
                s.latest = release.clone();
                s.next_at = now + DAY_MS;
                if s.stage.is_none() {
                    s.error = None;
                }
            }
            // Quietly again in an hour.
            Err(e) => {
                s.next_at = now + HOUR_MS;
                s.error = Some(error("check", e.clone()));
            }
        }
    }
    match &got {
        Ok(Some(r)) if newer(&r.version, current()) => sh.log(&format!("update: v{} out, newer than v{}", r.version, current())),
        Ok(Some(r)) => sh.log(&format!("update: v{} is the latest; this is v{}", r.version, current())),
        Ok(None) => sh.log("update: no release to go by (none, a draft or a prerelease)"),
        Err(e) => sh.log(&format!("update: could not look ({e}); again in an hour")),
    }
    tell(sh);
    sh.push_settings();
}

// A newer one told on the island, once for each: only while she is in
// sight and no session needs you (else at a later look).
fn tell(sh: &Arc<Shared>) {
    let Some(r) = STATE.lock().unwrap().latest.clone().filter(|r| newer(&r.version, current())) else { return };
    if sh.setting("updateTold").as_str() == Some(r.version.as_str()) || !sh.is_visible() || sh.pet.lock().unwrap().wants_you() {
        return;
    }
    let lang = sh.lang();
    let words = crate::i18n::t(lang, "update.out").replace("{version}", &r.version);
    sh.to_pages("pet:nudge", json!({ "id": "update", "icon": "star", "words": words, "label": crate::i18n::t(lang, "update.tap"), "value": "", "private": false, "open": { "tab": "update" } }));
    sh.change(json!({ "updateTold": r.version }));
    sh.log(&format!("update: v{} told on the island", r.version));
}

// As she starts: the old exe gone after a minute's run, and the looks
// (twenty seconds in, then as due; every ten minutes it is seen whether a
// look is due, or a newer one waits to be told).
pub fn start(sh: &Arc<Shared>) {
    if let Some(pid) = *AFTER.lock().unwrap() {
        sh.log(&format!("update: v{} up, after pid {pid} ended", current()));
    }
    let tidy = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        remove_old(&tidy);
    });
    if !may_update() {
        sh.log("update: run from the source tree: not looked for");
        return;
    }
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        loop {
            let due = crate::now_ms() >= STATE.lock().unwrap().next_at;
            if due && sh.setting("autoUpdateCheck") != false {
                check_now(&sh);
            } else {
                tell(&sh);
            }
            std::thread::sleep(Duration::from_secs(600));
        }
    });
}

// For the settings: this version, the latest, and how an update goes.
pub fn view() -> Value {
    let s = STATE.lock().unwrap();
    let latest = s.latest.as_ref().map(|r| json!({ "version": r.version, "page": r.page, "newer": newer(&r.version, current()), "canInstall": r.exe.is_some() && r.digest.is_some() }));
    json!({
        "current": current(),
        "may": may_update(),
        "source": crate::data::from_source(),
        "checking": s.checking,
        "checkedAt": s.checked_at,
        "latest": latest,
        "error": s.error,
        "stage": s.stage,
        "got": s.got,
        "total": s.total,
    })
}

// The latest release's page, opened (only GitHub's, unless a test's).
pub fn open_page(sh: &Shared) {
    let page = STATE.lock().unwrap().latest.as_ref().map(|r| r.page.clone()).unwrap_or_default();
    if page.is_empty() || !from_github(&page) {
        return;
    }
    if !crate::jump::open_url(&page) {
        sh.log(&format!("update: could not open {page}"));
    }
}

// The person said Update: off on its own thread.
pub fn update_now(sh: &Arc<Shared>) {
    let release = {
        let mut s = STATE.lock().unwrap();
        if s.stage.is_some() {
            return;
        }
        let Some(r) = s.latest.clone().filter(|r| newer(&r.version, current())) else { return };
        s.stage = Some("download");
        s.got = 0;
        s.total = r.size;
        s.error = None;
        r
    };
    sh.push_settings();
    let sh = sh.clone();
    std::thread::spawn(move || {
        if let Err(e) = install(&sh, &release) {
            sh.log(&format!("update: v{} not installed: {e}", release.version));
            let mut s = STATE.lock().unwrap();
            s.stage = None;
            s.error = Some(e);
            drop(s);
            sh.push_settings();
        }
    });
}

fn stage(sh: &Shared, stage: &'static str) {
    STATE.lock().unwrap().stage = Some(stage);
    sh.push_settings();
}

fn install(sh: &Arc<Shared>, r: &Release) -> Result<(), Value> {
    if !may_update() {
        return Err(error("source", ""));
    }
    let url = r.exe.as_deref().ok_or_else(|| error("noExe", ""))?;
    let digest = r.digest.as_deref().ok_or_else(|| error("noDigest", ""))?;
    if !from_github(url) {
        return Err(error("elsewhere", url));
    }
    let part = sh.dir.join(format!("update-v{}.exe.part", r.version));
    let got = download(sh, url, &part).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        error("download", e)
    })?;
    stage(sh, "verify");
    if got != digest {
        let _ = std::fs::remove_file(&part);
        return Err(error("digest", format!("{got} ≠ {digest}")));
    }
    sh.log(&format!("update: v{} downloaded, its SHA-256 as GitHub gives it", r.version));
    // A prompt waiting for an answer: answered first (the new one would
    // not have it).
    if !sh.asks.lock().unwrap().is_empty() {
        stage(sh, "waiting");
        sh.log("update: a prompt is open: waiting for its answer");
        while !sh.asks.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    stage(sh, "restart");
    let exe = replace(&part).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        error("replace", e)
    })?;
    if let Err(e) = start_new(&exe) {
        // Hers back where it was.
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::rename(old_path(&exe), &exe);
        return Err(error("start", e));
    }
    sh.log(&format!("update: v{} in place of v{} ({}), started; she quits for it", r.version, current(), exe.display()));
    sh.quit();
    Ok(())
}

// Downloaded to `to`, its SHA-256 as it comes (lowercase hex); the progress
// for the settings.
fn download(sh: &Shared, url: &str, to: &Path) -> Result<String, String> {
    let mut res = agent(600).get(url).header("user-agent", &format!("wakuwaku/{}", current())).call().map_err(|e| e.to_string())?;
    let total = res.body().content_length().unwrap_or(0);
    if total > 0 {
        STATE.lock().unwrap().total = total;
    }
    let mut reader = res.body_mut().with_config().limit(512 * 1024 * 1024).reader();
    let mut file = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hash.update(&buf[..n]);
        got += n as u64;
        STATE.lock().unwrap().got = got;
        sh.push_settings();
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

// Beside hers, its name kept (she may have been renamed): <name>.old.exe.
fn old_path(exe: &Path) -> PathBuf {
    let stem = exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "wakuwaku".into());
    exe.with_file_name(format!("{stem}.old.exe"))
}

// The new exe where hers is (a running exe can be renamed, not removed),
// hers kept as <name>.old.exe. Its path.
fn replace(part: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = old_path(&exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| format!("{} → {}: {e}", exe.display(), old.display()))?;
    let put = std::fs::rename(part, &exe).or_else(|_| std::fs::copy(part, &exe).map(|_| ()));
    if let Err(e) = put {
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("{}: {e}", exe.display()));
    }
    let _ = std::fs::remove_file(part);
    Ok(exe)
}

// Started on its own (out of her process tree and job, as the taskbar's
// guard), to wait for her.
fn start_new(exe: &Path) -> Result<(), String> {
    let mut command = std::process::Command::new(exe);
    command.arg(AFTER_UPDATE).arg(std::process::id().to_string());
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        if command.creation_flags(flags | CREATE_BREAKAWAY_FROM_JOB).spawn().is_ok() {
            return Ok(());
        }
        command.creation_flags(flags);
    }
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

// After a minute's run, the exe an update left: gone (tried a few times:
// the old one's taskbar guard may still have it open a moment).
fn remove_old(sh: &Shared) {
    let Ok(exe) = std::env::current_exe() else { return };
    let old = old_path(&exe);
    for _ in 0..6 {
        if !old.exists() {
            return;
        }
        if std::fs::remove_file(&old).is_ok() {
            sh.log(&format!("update: {} removed, the new one having run a minute", old.display()));
            return;
        }
        std::thread::sleep(Duration::from_secs(10));
    }
    sh.log(&format!("update: could not remove {}", old.display()));
}

// Started by an update (--after-update <pid>): that one waited for, at most
// a minute, before she takes the port and the taskbar.
pub fn wait_after_update(args: &[String]) {
    let Some(pid) = args.iter().position(|a| a == AFTER_UPDATE).and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u32>().ok()) else { return };
    *AFTER.lock().unwrap() = Some(pid);
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn WaitForSingleObject(handle: isize, ms: u32) -> u32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const SYNCHRONIZE: u32 = 0x0010_0000;
        // SAFETY: a handle we close.
        unsafe {
            let process = OpenProcess(SYNCHRONIZE, 0, pid);
            if process != 0 {
                WaitForSingleObject(process, 60_000);
                CloseHandle(process);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{newer, release_of};

    #[test]
    fn versions_compared() {
        assert!(newer("v0.2.1", "0.2.0"));
        assert!(newer("0.10.0", "0.9.0"));
        assert!(newer("1.0", "0.99.9"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("v0.2.0", "0.2.0"));
        assert!(!newer("0.1.9", "0.2.0"));
        assert!(!newer("0.9.0", "0.10.0"));
        assert!(newer("0.2.0", "0.2.0-beta"));
        assert!(!newer("0.2.0-beta", "0.2.0"));
    }

    #[test]
    fn releases_read() {
        let asset = |name: &str, digest: &str| json!({ "name": name, "size": 7, "digest": digest, "browser_download_url": format!("https://github.com/zhoupengjie/Wakuwaku/releases/download/x/{name}") });
        let good = json!({
            "tag_name": "v0.2.1", "draft": false, "prerelease": false,
            "html_url": "https://github.com/zhoupengjie/Wakuwaku/releases/tag/v0.2.1",
            "assets": [asset("notes.txt", ""), asset("wakuwaku-v0.2.1-windows-x64.exe", &format!("sha256:{}", "AB".repeat(32)))],
        });
        let r = release_of(&good).unwrap();
        assert_eq!(r.version, "0.2.1");
        assert_eq!(r.digest.as_deref(), Some("ab".repeat(32).as_str()));
        assert!(r.exe.unwrap().ends_with("wakuwaku-v0.2.1-windows-x64.exe"));
        let mut draft = good.clone();
        draft["draft"] = json!(true);
        assert!(release_of(&draft).is_none());
        let mut pre = good.clone();
        pre["prerelease"] = json!(true);
        assert!(release_of(&pre).is_none());
        // No SHA-256 given, or not one: none to check by.
        let mut bare = good.clone();
        bare["assets"] = json!([asset("wakuwaku-v0.2.1-windows-x64.exe", "md5:xyz")]);
        assert_eq!(release_of(&bare).unwrap().digest, None);
        // The exe by another name: none to install.
        let mut other = good;
        other["assets"] = json!([asset("wakuwaku.exe", "")]);
        assert_eq!(release_of(&other).unwrap().exe, None);
    }
}
