// How Claude Code and Codex reach her: the plugin, or hooks written into
// Claude Code's settings.json; hooks written into Codex's hooks.json (and
// Antigravity's, below); omp, an extension written into its agent folder.
// Read on demand (the person may change any of them at any time), and
// written only when they ask, from the settings.
//
// Every Claude Code event is an HTTP hook: Claude Code POSTs it to her port,
// no process at all. SessionStart also gets a command, run in the background
// (async), that starts her when she is not up, which no HTTP hook can do.
// That command is this program with ENSURE_FLAG, given as an argument list
// (no shell). Codex runs only commands: see "Codex" below.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

pub const ENSURE_FLAG: &str = "--wakuwaku-ensure-running";
pub const CODEX_FLAG: &str = "--wakuwaku-codex-hook";
pub const AGY_FLAG: &str = "--wakuwaku-agy-hook";
const URL_MARK: &str = "from=wakuwaku";
// What the app wrote before it was called Wakuwaku (claude-pets).
pub const LEGACY_ENSURE_FLAG: &str = "--claude-pets-ensure-running";
const LEGACY_URL_MARK: &str = "from=claude-pets";
pub const PLUGIN_ID: &str = "wakuwaku@wakuwaku";
pub const PLUGIN_COMMANDS: [&str; 2] = ["/plugin marketplace add zhoupengjie/wakuwaku", "/plugin install wakuwaku@wakuwaku"];

// Each event Claude Code reports, and whether it takes a matcher; the prompt
// waits for the person (up to 300 s) instead of the default 2.
const EVENTS: [(&str, bool, u64); 11] = [
    ("SessionStart", false, 2),
    ("SessionEnd", false, 2),
    ("UserPromptSubmit", false, 2),
    ("PreToolUse", true, 2),
    ("PostToolUse", true, 2),
    ("PostToolUseFailure", true, 2),
    ("PermissionRequest", true, 300),
    ("Elicitation", false, 2),
    ("TaskCompleted", false, 2),
    ("Stop", false, 2),
    ("StopFailure", false, 2),
];

pub fn claude_settings_file() -> PathBuf {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".claude"));
    dir.join("settings.json")
}

// Whether Claude Code is on this computer, as far as can be told: its
// folder there (~/.claude, or CLAUDE_CONFIG_DIR).
pub fn claude_here() -> bool {
    claude_settings_file().parent().is_some_and(Path::is_dir)
}

// The copy write_json keeps of a settings file (made once, beside it).
pub fn backup_of(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.wakuwaku.bak", file.display()))
}

// A path as the person reads it: ~ for the home folder when it is under it.
pub fn shown_path(file: &Path) -> String {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home.as_deref().filter(|h| !h.as_os_str().is_empty()).and_then(|h| file.strip_prefix(h).ok()) {
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => file.display().to_string(),
    }
}

// What kept a settings file from being written, for what to do about it:
// locked (another program has it open), invalid (not JSON), other. From
// the error's text: io's ends "(os error 32)" (or 33) whatever the
// language, serde_json's says where, "at line 3 column 5".
pub fn error_kind(error: &str) -> &'static str {
    if error.contains(AGY_NO_NAME) {
        "path"
    } else if error.contains("(os error 32)") || error.contains("(os error 33)") {
        "locked"
    } else if error.contains(" at line ") && error.contains(" column ") {
        "invalid"
    } else {
        "other"
    }
}

// A JSON settings file: {} when there is none, Err when unreadable.
// A file as last read, kept while its size and time stay (a look at the
// file's metadata, not its contents): the settings' snapshot asks for
// Claude Code's settings.json three times and Codex's hooks.json twice,
// up to four times a second while the settings are open (2026-10-13).
type Stamp = Option<(u64, std::time::SystemTime)>;
static READ: Mutex<Option<std::collections::HashMap<PathBuf, (Stamp, Result<Value, String>)>>> = Mutex::new(None);

fn read_json(file: &Path) -> Result<Value, String> {
    let stamp: Stamp = fs::metadata(file).ok().map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    let read = read.get_or_insert_with(std::collections::HashMap::new);
    if let Some((at, value)) = read.get(file) {
        if *at == stamp {
            return value.clone();
        }
    }
    let value = read_json_now(file);
    read.insert(file.to_path_buf(), (stamp, value.clone()));
    value
}

fn read_json_now(file: &Path) -> Result<Value, String> {
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

// Claude Code's settings.
fn read_settings() -> Result<Value, String> {
    read_json(&claude_settings_file())
}

// Write a settings file, backed up once beside itself.
fn write_json(file: &Path, value: &Value) -> Result<(), String> {
    let backup = backup_of(file);
    if file.exists() && !backup.exists() {
        fs::copy(file, &backup).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let written = fs::write(file, format!("{text}\n")).map_err(|e| e.to_string());
    // As last read no longer: read again next time, whatever its stamp.
    if let Some(read) = READ.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        read.remove(file);
    }
    written
}

// This program, as the hook command starts it.
pub fn launch_command() -> String {
    crate::data::exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn hook_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/hook?{URL_MARK}")
}

// A path as Windows compares them: one slash, any case.
fn same_path(p: &str) -> String {
    if cfg!(windows) { p.replace('\\', "/").to_lowercase() } else { p.to_string() }
}

// Ours: what this version installs, and what older ones did.
fn is_ours(hook: &Value) -> bool {
    let mut text = [hook.get("url"), hook.get("command")].iter().flatten().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
    for arg in hook.get("args").and_then(Value::as_array).into_iter().flatten() {
        text.push(' ');
        text.push_str(&arg.to_string());
    }
    [URL_MARK, ENSURE_FLAG, CODEX_FLAG, LEGACY_URL_MARK, LEGACY_ENSURE_FLAG, "claude-hook.js"].iter().any(|m| text.contains(m))
}

// Our hooks, event by event.
fn ours_by_event(settings: &Value) -> Map<String, Value> {
    let mut found = Map::new();
    for (event, groups) in settings.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        let ours: Vec<Value> = groups
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g.get("hooks").and_then(Value::as_array).cloned().unwrap_or_default())
            .filter(is_ours)
            .collect();
        if !ours.is_empty() {
            found.insert(event.clone(), Value::Array(ours));
        }
    }
    found
}

// Our entries dropped from every event, and events left empty.
fn strip(hooks: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(Value::Object(hooks)) = hooks else { return out };
    for (event, groups) in hooks {
        let Value::Array(groups) = groups else {
            out.insert(event.clone(), groups.clone());
            continue;
        };
        let kept: Vec<Value> = groups
            .iter()
            .filter_map(|group| {
                let mut group = group.clone();
                let hooks: Vec<Value> = group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| !is_ours(h)).cloned().collect();
                if hooks.is_empty() {
                    return None;
                }
                group["hooks"] = Value::Array(hooks);
                Some(group)
            })
            .collect();
        if !kept.is_empty() {
            out.insert(event.clone(), Value::Array(kept));
        }
    }
    out
}

// What we add, event by event. http_only: no command, nothing starts her.
fn entries(port: u16, http_only: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for (event, matcher, timeout) in EVENTS {
        // Claude Code runs no HTTP hook for SessionStart: the starter command
        // passes that one on.
        if event == "SessionStart" {
            continue;
        }
        let hooks = json!([{ "type": "http", "url": hook_url(port), "timeout": timeout }]);
        let group = if matcher { json!({ "matcher": "*", "hooks": hooks }) } else { json!({ "hooks": hooks }) };
        out.insert(event.into(), json!([group]));
    }
    if !http_only {
        out.insert(
            "SessionStart".into(),
            json!([{ "hooks": [{ "type": "command", "command": launch_command(), "args": [ENSURE_FLAG], "async": true, "timeout": 15 }] }]),
        );
    }
    out
}

pub fn install(settings: &Value, port: u16, http_only: bool) -> Value {
    let mut hooks = strip(settings.get("hooks"));
    for (event, groups) in entries(port, http_only) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = settings.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

pub fn uninstall(settings: &Value) -> Value {
    let mut out = settings.as_object().cloned().unwrap_or_default();
    let hooks = strip(settings.get("hooks"));
    if hooks.is_empty() {
        out.remove("hooks");
    } else {
        out.insert("hooks".into(), Value::Object(hooks));
    }
    Value::Object(out)
}

// How our entries stand against what this copy would install:
//   missing / ok / httpOnly (nothing starts her) / stale (another port or
//   copy, or an old version) / partial (some events lack them).
pub fn status_of(settings: &Value, port: u16) -> &'static str {
    let found = ours_by_event(settings);
    if !EVENTS.iter().any(|(name, ..)| found.contains_key(*name)) {
        return "missing";
    }
    let list = |name: &str| found.get(name).and_then(Value::as_array).cloned().unwrap_or_default();
    let has_flag = |h: &Value| h.get("args").and_then(Value::as_array).is_some_and(|a| a.iter().any(|x| x == ENSURE_FLAG));
    let is_old = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] != "http" && !has_flag(h)))
        || list("SessionStart").iter().any(|h| h["type"] == "http");
    let want_url = hook_url(port);
    let is_other_port = EVENTS.iter().any(|(name, ..)| list(name).iter().any(|h| h["type"] == "http" && h["url"] != want_url.as_str()));
    let starters: Vec<Value> = list("SessionStart").into_iter().filter(|h| h["type"] == "command").collect();
    let want = vec![same_path(&launch_command()), same_path(ENSURE_FLAG)];
    let is_other_copy = starters.iter().any(|h| {
        let mut got = vec![same_path(h["command"].as_str().unwrap_or(""))];
        got.extend(h.get("args").and_then(Value::as_array).into_iter().flatten().map(|a| same_path(a.as_str().unwrap_or(""))));
        got != want
    });
    if is_old || is_other_port || is_other_copy {
        return "stale";
    }
    if EVENTS.iter().any(|(name, ..)| *name != "SessionStart" && !found.contains_key(*name)) {
        return "partial";
    }
    if starters.is_empty() { "httpOnly" } else { "ok" }
}

pub fn hooks_status(port: u16) -> &'static str {
    match read_settings() {
        Ok(settings) => status_of(&settings, port),
        Err(_) => "unreadable",
    }
}

pub fn is_plugin_enabled() -> bool {
    read_settings().is_ok_and(|s| s.get("enabledPlugins").and_then(|p| p.get(PLUGIN_ID)) == Some(&Value::Bool(true)))
}

// 'plugin', 'hooks' (written into settings.json), 'both' (every event twice), or 'none'.
pub fn connection(port: u16) -> &'static str {
    let plugin = is_plugin_enabled();
    let hooks = !matches!(hooks_status(port), "missing" | "unreadable");
    match (plugin, hooks) {
        (true, true) => "both",
        (true, false) => "plugin",
        (false, true) => "hooks",
        (false, false) => "none",
    }
}

// Change Claude Code's settings.json, backed up once beside itself.
// action: install, install-http, remove.
pub fn write_hooks(action: &str, port: u16) -> Result<(), String> {
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&claude_settings_file())?;
    let after = match action {
        "remove" => uninstall(&before),
        "install" => install(&before, port, false),
        "install-http" => install(&before, port, true),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&claude_settings_file(), &after)
}


// --- Codex: hooks in ~/.codex/hooks.json ---------------------------------------------
//
// Codex runs hooks only as commands, through the session's shell (on Windows
// `pwsh -NoProfile -Command`, about 0.3 s to start): this program with
// CODEX_FLAG, which hands the event to her port and prints her answer to a
// prompt (main.rs). So few hooks make Codex wait: the turn starting and
// ending, a prompt, an edit (apply_patch) or a question, the session. Each
// tool call's step comes from hooks run in the background (async), which
// Codex has from 0.148 on; older ones skip async hooks with a warning, so
// those are left out for them.
//
// Codex runs a hook only once the person trusts it (/hooks in Codex), and
// again after it changes, such as this program moving: we never trust them
// on their behalf.

pub const CODEX_PROMPT_TIMEOUT: u64 = 300;
// A version: major, minor, patch.
pub type Version = (u64, u64, u64);
// The first Codex that runs async hooks.
const CODEX_ASYNC_SINCE: Version = (0, 148, 0);

// One hook we add: its event, the tools it is for (a regex, None for all),
// how long it may run (s), and whether in the background.
struct CodexHook {
    event: &'static str,
    matcher: Option<&'static str>,
    timeout: u64,
    is_async: bool,
}

// A prompt waits for the person; Codex gives Interrupt and SessionEnd 3 s at most.
const CODEX_HOOKS: [CodexHook; 10] = [
    CodexHook { event: "SessionStart", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "UserPromptSubmit", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "PreToolUse", matcher: Some("^request_user_input$"), timeout: 10, is_async: false },
    CodexHook { event: "PostToolUse", matcher: Some("^(apply_patch|request_user_input)$"), timeout: 10, is_async: false },
    CodexHook { event: "PermissionRequest", matcher: None, timeout: CODEX_PROMPT_TIMEOUT, is_async: false },
    CodexHook { event: "Stop", matcher: None, timeout: 10, is_async: false },
    CodexHook { event: "Interrupt", matcher: None, timeout: 3, is_async: false },
    CodexHook { event: "SessionEnd", matcher: None, timeout: 3, is_async: false },
    // Every step, in the background.
    CodexHook { event: "PreToolUse", matcher: None, timeout: 10, is_async: true },
    CodexHook { event: "PostToolUse", matcher: None, timeout: 10, is_async: true },
];

fn codex_hooks_for(with_async: bool) -> impl Iterator<Item = &'static CodexHook> {
    CODEX_HOOKS.iter().filter(move |h| with_async || !h.is_async)
}

pub fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".codex"))
}

pub fn codex_hooks_file() -> PathBuf {
    codex_home().join("hooks.json")
}

// "codex-cli 0.146.1" (or 0.149.0-alpha.1) → (0, 146, 1).
fn parse_version(text: &str) -> Option<Version> {
    text.split_whitespace().find_map(|word| {
        let mut parts = word.split('.').map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    })
}

fn ask_codex_version() -> Option<Version> {
    let mut command = std::process::Command::new("codex");
    command.arg("--version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    parse_version(&String::from_utf8_lossy(&command.output().ok()?.stdout))
}

// Codex's version, None when there is no `codex` to ask (the desktop app
// alone). Asked at most once a minute.
pub fn codex_version() -> Option<Version> {
    static KNOWN: Mutex<Option<(Instant, Option<Version>)>> = Mutex::new(None);
    let mut known = KNOWN.lock().unwrap();
    match *known {
        Some((at, version)) if at.elapsed() < Duration::from_secs(60) => version,
        _ => {
            let version = ask_codex_version();
            *known = Some((Instant::now(), version));
            version
        }
    }
}

// Whether Codex runs async hooks; one we cannot ask is taken as a current one.
pub fn codex_runs_async() -> bool {
    codex_version().is_none_or(|v| v >= CODEX_ASYNC_SINCE)
}

// The command line Codex's shell runs: the bare path when it needs no
// quoting (any shell takes it), else quoted for PowerShell (& '...') on
// Windows or for a POSIX shell.
pub fn codex_command(exe: &str) -> String {
    let is_plain = !exe.is_empty() && exe.chars().all(|c| c.is_ascii_alphanumeric() || "\\/:._-".contains(c));
    if is_plain {
        return format!("{exe} {CODEX_FLAG}");
    }
    if cfg!(windows) {
        format!("& '{}' {CODEX_FLAG}", exe.replace('\'', "''"))
    } else {
        format!("'{}' {CODEX_FLAG}", exe.replace('\'', r"'\''"))
    }
}

// What we add, event by event; `waiting` is what Codex shows while a prompt waits on her.
fn codex_entries(command: &str, waiting: &str, with_async: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for want in codex_hooks_for(with_async) {
        let mut hook = json!({ "type": "command", "command": command, "timeout": want.timeout });
        if want.is_async {
            hook["async"] = json!(true);
        }
        if want.event == "PermissionRequest" {
            hook["statusMessage"] = json!(waiting);
        }
        let mut group = json!({ "hooks": [hook] });
        if let Some(matcher) = want.matcher {
            group["matcher"] = json!(matcher);
        }
        let groups = out.entry(want.event).or_insert_with(|| json!([]));
        if let Value::Array(groups) = groups {
            groups.push(group);
        }
    }
    out
}

pub fn codex_install(file: &Value, command: &str, waiting: &str, with_async: bool) -> Value {
    let mut hooks = strip(file.get("hooks"));
    for (event, groups) in codex_entries(command, waiting, with_async) {
        let mut all = hooks.get(&event).and_then(Value::as_array).cloned().unwrap_or_default();
        all.extend(groups.as_array().cloned().unwrap_or_default());
        hooks.insert(event, Value::Array(all));
    }
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert("hooks".into(), Value::Object(hooks));
    Value::Object(out)
}

// Our hooks in a hooks.json: (event, matcher, hook).
fn codex_ours(file: &Value) -> Vec<(String, Option<String>, Value)> {
    let mut ours = Vec::new();
    for (event, groups) in file.get("hooks").and_then(Value::as_object).into_iter().flatten() {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group.get("matcher").and_then(Value::as_str).map(str::to_string);
            for hook in group.get("hooks").and_then(Value::as_array).into_iter().flatten().filter(|h| is_ours(h)) {
                ours.push((event.clone(), matcher.clone(), hook.clone()));
            }
        }
    }
    ours
}

fn is_codex_hook(found: &(String, Option<String>, Value), want: &CodexHook, command: &str) -> bool {
    let (event, matcher, hook) = found;
    event == want.event
        && matcher.as_deref() == want.matcher
        && hook["type"] == "command"
        && hook["command"] == command
        && hook["timeout"].as_u64() == Some(want.timeout)
        && hook.get("async").and_then(Value::as_bool).unwrap_or(false) == want.is_async
}

// How our entries stand against what this copy would install: missing / ok /
// stale (another copy of the program, or another version's hooks) /
// upgradable (only the background ones lack: Codex was updated since) /
// partial (others lack).
pub fn codex_status_of(file: &Value, command: &str, with_async: bool) -> &'static str {
    let ours = codex_ours(file);
    if ours.is_empty() {
        return "missing";
    }
    if !ours.iter().all(|found| CODEX_HOOKS.iter().any(|want| is_codex_hook(found, want, command))) {
        return "stale";
    }
    let lacking: Vec<&CodexHook> = codex_hooks_for(with_async).filter(|want| !ours.iter().any(|found| is_codex_hook(found, want, command))).collect();
    if lacking.is_empty() {
        "ok"
    } else if lacking.iter().all(|want| want.is_async) {
        "upgradable"
    } else {
        "partial"
    }
}

// absent (no Codex here), unreadable, or how our entries stand.
pub fn codex_hooks_status() -> &'static str {
    if !codex_home().is_dir() {
        return "absent";
    }
    match read_json(&codex_hooks_file()) {
        Ok(file) => codex_status_of(&file, &codex_command(&launch_command()), codex_runs_async()),
        Err(_) => "unreadable",
    }
}

pub fn is_codex_connected() -> bool {
    matches!(codex_hooks_status(), "ok" | "stale" | "upgradable" | "partial")
}

// Whether any of them is connected, as last looked (at start, as the
// settings read it, after connecting) or heard (any event from them): what
// the island asks for while it is not (island.js), without reading their
// settings files on every redraw. Connected until looked, so a first draw
// never asks wrongly.
static CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn note_connected(connected: bool) -> bool {
    CONNECTED.swap(connected, std::sync::atomic::Ordering::SeqCst) != connected
}

pub fn look_connected(port: u16) -> bool {
    let connected = connection(port) != "none" || is_codex_connected() || is_agy_connected() || is_omp_connected(port);
    note_connected(connected);
    connected
}

pub fn seems_connected() -> bool {
    CONNECTED.load(std::sync::atomic::Ordering::SeqCst)
}

// Change Codex's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_codex_hooks(action: &str, waiting: &str) -> Result<(), String> {
    let file = codex_hooks_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => uninstall(&before),
        "install" => codex_install(&before, &codex_command(&launch_command()), waiting, codex_runs_async()),
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}

// --- Antigravity: hooks in ~/.gemini/config/hooks.json --------------------------------
//
// Antigravity (Google's agy, and its 2.0 app, which reads the same file) runs
// hooks only as commands, each tool's in a block of its own by name: ours is
// "wakuwaku". Its events: SessionStart; PreInvocation, each call of the model
// (the first of a turn, invocationNum 0); PostToolUse; Stop. None for a prompt
// waiting on the person: she follows its work and its end, and asks nothing
// (events_agy.rs). Its standalone IDE runs none, as reported.
//
// Tried with agy 1.3.4 (2026-10-13): it runs a hook as `cmd /c "<command>"`,
// the command's own double quotes escaped as Go escapes them (\"), which cmd
// does not read, so a path with a space can not be quoted: its 8.3 name is
// used for one, and with none it is not installed. And a hook that fails is
// told back in cmd's code page, not UTF-8: agy's turn failed on it (invalid
// UTF-8). Ours answers at once whatever comes (main.rs agy_hook).

const AGY_BLOCK: &str = "wakuwaku";
const AGY_TIMEOUT: u64 = 10;

pub fn gemini_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default()).join(".gemini")
}

pub fn agy_hooks_file() -> PathBuf {
    gemini_dir().join("config").join("hooks.json")
}

// Whether Antigravity is on this computer: agy's folder, or the 2.0 app's
// (the standalone IDE's alone does not count: it runs no hooks).
pub fn agy_here() -> bool {
    let gemini = gemini_dir();
    gemini.join("antigravity-cli").is_dir() || gemini.join("antigravity").is_dir()
}

// A path cmd reads as one word with no quotes around it.
fn is_bare(path: &str) -> bool {
    !path.is_empty() && !path.chars().any(|c| c.is_whitespace() || "&|<>^()%!\"',;=`".contains(c))
}

#[cfg(windows)]
fn short_path(path: &str) -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetShortPathNameW(long: *const u16, short: *mut u16, size: u32) -> u32;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut out = vec![0u16; 1024];
    // SAFETY: both buffers ours, the second as long as said.
    let n = unsafe { GetShortPathNameW(wide.as_ptr(), out.as_mut_ptr(), out.len() as u32) } as usize;
    (n > 0 && n < out.len()).then(|| String::from_utf16_lossy(&out[..n]))
}

#[cfg(not(windows))]
fn short_path(_path: &str) -> Option<String> {
    None
}

// This program as agy's cmd reads it: as it is, or by its 8.3 name; None
// when neither will do (a space in it, and short names turned off there).
pub fn agy_program(exe: &str) -> Option<String> {
    if is_bare(exe) {
        return Some(exe.to_string());
    }
    short_path(exe).filter(|s| is_bare(s))
}

// Our block: the event's name after the flag (the payload has none).
fn agy_block(program: &str) -> Value {
    let hook = |event: &str| json!({ "type": "command", "command": format!("{program} {AGY_FLAG} {event}"), "timeout": AGY_TIMEOUT });
    json!({
        "SessionStart": [hook("SessionStart")],
        "PreInvocation": [hook("PreInvocation")],
        "PostToolUse": [{ "matcher": "", "hooks": [hook("PostToolUse")] }],
        "Stop": [hook("Stop")],
    })
}

pub fn agy_install(file: &Value, program: &str) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.insert(AGY_BLOCK.into(), agy_block(program));
    Value::Object(out)
}

pub fn agy_uninstall(file: &Value) -> Value {
    let mut out = file.as_object().cloned().unwrap_or_default();
    out.remove(AGY_BLOCK);
    Value::Object(out)
}

// How our block stands against what this copy would write: missing / ok /
// stale (another copy of the program, or another version's) / off (turned
// off in the file: enabled false).
pub fn agy_status_of(file: &Value, program: Option<&str>) -> &'static str {
    let Some(ours) = file.get(AGY_BLOCK) else { return "missing" };
    if ours.get("enabled").and_then(Value::as_bool) == Some(false) {
        return "off";
    }
    let mut ours = ours.clone();
    if let Some(m) = ours.as_object_mut() {
        m.remove("enabled");
    }
    match program {
        Some(p) if ours == agy_block(p) => "ok",
        _ => "stale",
    }
}

// absent (no Antigravity here), unreadable, or how our block stands.
pub fn agy_hooks_status() -> &'static str {
    if !agy_here() {
        return "absent";
    }
    match read_json(&agy_hooks_file()) {
        Ok(file) => agy_status_of(&file, agy_program(&launch_command()).as_deref()),
        Err(_) => "unreadable",
    }
}

// The tools on this computer by name, in their order (the island names
// them while none is connected): Claude Code by its folder, Codex,
// Antigravity and omp by theirs; none found, all but omp (it can not be told).
pub fn tools_here() -> Vec<&'static str> {
    let here: Vec<&str> = [("Claude Code", claude_here()), ("Codex", codex_home().is_dir()), ("Antigravity", agy_here()), ("omp", omp_here())]
        .into_iter()
        .filter(|(_, is)| *is)
        .map(|(name, _)| name)
        .collect();
    if here.is_empty() {
        vec!["Claude Code", "Codex", "Antigravity"]
    } else {
        here
    }
}

pub fn is_agy_connected() -> bool {
    matches!(agy_hooks_status(), "ok" | "stale")
}

// What keeps this program from agy's hooks, as write_agy_hooks says it
// (error_kind: "path").
const AGY_NO_NAME: &str = "no name for this program without quotes";

// Change Antigravity's hooks.json, backed up once beside itself. action: install, remove.
pub fn write_agy_hooks(action: &str) -> Result<(), String> {
    let file = agy_hooks_file();
    // Read from the disk, not as last read: nothing of someone else's is written back.
    let before = read_json_now(&file)?;
    let after = match action {
        "remove" if !file.exists() => return Ok(()),
        "remove" => agy_uninstall(&before),
        "install" => {
            let exe = launch_command();
            let Some(program) = agy_program(&exe) else { return Err(format!("{exe}: {AGY_NO_NAME} (a space in its path, and no short name for it)")) };
            agy_install(&before, &program)
        }
        _ => return Err(format!("unknown action {action}")),
    };
    write_json(&file, &after)
}

// --- oh-my-pi (omp): an extension in its agent folder ---------------------------------
//
// omp runs no hooks: what extends it is a module it loads (in its own Bun,
// inside omp.exe) from <agent dir>/extensions/ as it starts. Ours,
// integrations/omp/wakuwaku.ts, is written there with this copy's port, exe
// and data folder filled in (an omp started after that loads it), and taken
// out again; nothing else of omp's is touched. It reports to her port from
// inside omp (events_omp.rs). The agent folder as omp 18.11.1 finds it (its
// utils/src/dirs.ts): ~/.omp/agent, PI_CONFIG_DIR in place of .omp,
// PI_CODING_AGENT_DIR in place of the whole; and a profile (omp --profile
// <name>, or OMP_PROFILE) has one of its own, ~/.omp/profiles/<name>/agent,
// which she cannot tell from here: the extension goes into each one there is.

pub const OMP_FILE: &str = "wakuwaku.ts";
const OMP_SOURCE: &str = include_str!("../../integrations/omp/wakuwaku.ts");

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default())
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

// omp's own folder: ~/.omp, or what PI_CONFIG_DIR names under the home folder.
pub fn omp_root() -> PathBuf {
    home_dir().join(env_dir("PI_CONFIG_DIR").unwrap_or_else(|| PathBuf::from(".omp")))
}

// Its agent folders: the one it uses with no profile, then each profile's.
pub fn omp_agent_dirs() -> Vec<PathBuf> {
    agent_dirs_in(&omp_root(), env_dir("PI_CODING_AGENT_DIR"))
}

fn agent_dirs_in(root: &Path, agent_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = vec![agent_dir.unwrap_or_else(|| root.join("agent"))];
    let mut profiles: Vec<PathBuf> = fs::read_dir(root.join("profiles"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("agent"))
        .filter(|agent| agent.is_dir())
        .collect();
    profiles.sort();
    for dir in profiles {
        if !dirs.iter().any(|d| same_path(&d.to_string_lossy()) == same_path(&dir.to_string_lossy())) {
            dirs.push(dir);
        }
    }
    dirs
}

pub fn omp_files() -> Vec<PathBuf> {
    omp_agent_dirs().into_iter().map(|dir| dir.join("extensions").join(OMP_FILE)).collect()
}

// Whether omp is on this computer: its folder (made as it first runs), or
// its exe where its installer puts it.
pub fn omp_here() -> bool {
    let installed = env_dir("LOCALAPPDATA").is_some_and(|dir| dir.join("omp").join("omp.exe").is_file());
    omp_root().is_dir() || omp_agent_dirs()[0].is_dir() || installed
}

// The extension as a copy of her on this port, this exe and this data
// folder ("" for her own) writes it.
pub fn omp_extension(port: u16, exe: &str, data: &str) -> String {
    OMP_SOURCE
        .replace("\"__WAKUWAKU_PORT__\"", &format!("\"{port}\""))
        .replace("\"__WAKUWAKU_EXE__\"", &json!(exe).to_string())
        .replace("\"__WAKUWAKU_DATA__\"", &json!(data).to_string())
}

// What this copy writes: her data folder only when she was given one
// (WAKUWAKU_USER_DATA, a test's copy), for the one the extension starts.
fn omp_wanted(port: u16) -> String {
    omp_extension(port, &launch_command(), &std::env::var("WAKUWAKU_USER_DATA").unwrap_or_default())
}

// How the files stand against what this copy writes (None: no file in that
// folder): missing (in none) / ok / stale (another port or copy, an older
// version) / partial (a profile made since lacks it).
pub fn omp_status_of(found: &[Option<String>], want: &str) -> &'static str {
    if found.iter().all(Option::is_none) {
        "missing"
    } else if found.iter().flatten().any(|text| text != want) {
        "stale"
    } else if found.iter().all(Option::is_some) {
        "ok"
    } else {
        "partial"
    }
}

// absent (no omp here), unreadable, or how the extension stands.
pub fn omp_hooks_status(port: u16) -> &'static str {
    if !omp_here() {
        return "absent";
    }
    let mut found = Vec::new();
    for file in omp_files() {
        match fs::read_to_string(&file) {
            Ok(text) => found.push(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => found.push(None),
            Err(_) => return "unreadable",
        }
    }
    omp_status_of(&found, &omp_wanted(port))
}

pub fn is_omp_connected(port: u16) -> bool {
    matches!(omp_hooks_status(port), "ok" | "stale" | "partial")
}

// Write the extension into every agent folder, or take it out of each (its
// extensions folder too, once empty). action: install, remove.
pub fn write_omp_hooks(action: &str, port: u16) -> Result<(), String> {
    write_omp_files(&omp_files(), action, &omp_wanted(port))
}

fn write_omp_files(files: &[PathBuf], action: &str, want: &str) -> Result<(), String> {
    match action {
        "install" => {
            for file in files {
                if let Some(dir) = file.parent() {
                    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                fs::write(file, want).map_err(|e| format!("{}: {e}", file.display()))?;
            }
            Ok(())
        }
        "remove" => {
            for file in files {
                match fs::remove_file(file) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("{}: {e}", file.display())),
                    _ => {}
                }
                if let Some(dir) = file.parent() {
                    let _ = fs::remove_dir(dir);
                }
            }
            Ok(())
        }
        _ => Err(format!("unknown action {action}")),
    }
}

// --- Start at login: this program, from where it is now -------------------------------
//
// The switch is the Run value (HKCU ...\CurrentVersion\Run, Wakuwaku): this
// program with AT_LOGIN_FLAG and "run". Task Manager's Startup apps list it
// and may turn it off there (StartupApproved\Run, by the same name), which
// is off here too. In the taskbar home also a logon task (logon.rs, with
// "task"): Windows starts the Run items one by one once Explorer is up, late
// and held back (three minutes after a logon on the user's laptop,
// zhoupengjie/Wakuwaku#44), while the task starts her as the user logs on.
// The Run value stays for when the task does not run; whichever of the two
// comes second finds her up and leaves without a word (any other start
// brings her back into sight).
//
// Kept to where she is: as she starts on her own port (not a test's copy on
// another, nor one with a data folder of its own), a Run value naming
// another exe (gone: renamed or moved by hand, a download kept under its
// own name; or another copy of her) is written anew for this one, and the
// task made, repaired or removed to match. Never turned on where it was off.
// Run from the source tree (cargo, its target folder), she writes no build's
// path over another exe's; her own value she keeps up.

pub const AT_LOGIN_FLAG: &str = "--wakuwaku-at-login";
pub const BY_TASK: &str = "task";
const BY_RUN: &str = "run";

// Started at login: by her logon task (task) or the Run value (run); none
// when started any other way.
pub fn started_by(args: &[String]) -> Option<&'static str> {
    let at = args.iter().position(|a| a == AT_LOGIN_FLAG)?;
    Some(if args.get(at + 1).map(String::as_str) == Some(BY_TASK) { BY_TASK } else { BY_RUN })
}

// Start at login's decisions, plain (tested below; acted on, on Windows alone).
#[cfg_attr(not(windows), allow(dead_code))]
mod rules {
    use super::{same_path, AT_LOGIN_FLAG, BY_RUN, BY_TASK};
    use crate::logon::TaskSeen;

    // The Run value for this program, and what its logon task gives it.
    pub fn run_command(exe: &str) -> String {
        format!("\"{exe}\" {AT_LOGIN_FLAG} {BY_RUN}")
    }

    pub fn task_args() -> String {
        format!("{AT_LOGIN_FLAG} {BY_TASK}")
    }

    // The program a command line starts: its first word, in quotes or not
    // (a path with spaces and no quotes, up to its .exe).
    pub fn program_of(command: &str) -> &str {
        let command = command.trim();
        if let Some(rest) = command.strip_prefix('"') {
            return rest.split('"').next().unwrap_or("");
        }
        match command.to_ascii_lowercase().find(".exe") {
            Some(at) => &command[..at + 4],
            None => command.split_whitespace().next().unwrap_or(""),
        }
    }

    // Start at login as it stands: off (no Run value, or turned off in Task
    // Manager), on for this program, or on for another exe.
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub enum Login {
        Off,
        Mine,
        Theirs,
    }

    pub fn login_of(value: Option<&str>, turned_off: bool, exe: &str) -> Login {
        match value {
            None => Login::Off,
            Some(_) if turned_off => Login::Off,
            Some(v) if same_path(program_of(v)) == same_path(exe) => Login::Mine,
            Some(_) => Login::Theirs,
        }
    }

    // What her start does with the Run value: keeps it, or writes this
    // program's in its place, and why (the exe it named is gone, it is
    // another copy of her, or it is this one as an older version wrote it).
    // No value stays none: start at login turned off stays off. Run from
    // the source tree, she takes no other exe's.
    #[derive(Debug, PartialEq)]
    pub enum RunFix {
        Keep,
        Rewrite(&'static str),
    }

    pub fn run_fix(value: Option<&str>, exe: &str, from_source: bool, exists: impl Fn(&str) -> bool) -> RunFix {
        let Some(value) = value else { return RunFix::Keep };
        let was = program_of(value);
        if same_path(was) == same_path(exe) {
            return if same_path(value) == same_path(&run_command(exe)) { RunFix::Keep } else { RunFix::Rewrite("as an older version wrote it") };
        }
        if from_source {
            return RunFix::Keep;
        }
        RunFix::Rewrite(if exists(was) { "another copy of her" } else { "gone" })
    }

    // What becomes of her logon task: there while start at login is hers
    // and the taskbar her home, running this program; gone otherwise.
    #[derive(Debug, PartialEq)]
    pub enum TaskFix {
        Keep,
        Make,
        Remove,
    }

    pub fn task_fix(login: Login, taskbar: bool, task: Option<&TaskSeen>, exe: &str) -> TaskFix {
        match (login, task) {
            // Another exe's start at login: not this one's to change.
            (Login::Theirs, _) => TaskFix::Keep,
            (Login::Off, Some(_)) => TaskFix::Remove,
            (Login::Mine, Some(_)) if !taskbar => TaskFix::Remove,
            (Login::Mine, None) if taskbar => TaskFix::Make,
            // Turned off in the Task Scheduler: left as the person left it.
            (Login::Mine, Some(t)) if !t.enabled => TaskFix::Keep,
            (Login::Mine, Some(t)) if same_path(&t.program) != same_path(exe) || t.args != task_args() => TaskFix::Make,
            _ => TaskFix::Keep,
        }
    }
}

#[cfg(windows)]
mod login {
    use super::launch_command;
    use super::rules::{login_of, run_command, run_fix, task_args, task_fix, Login, RunFix, TaskFix};
    use crate::logon;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    // Task Manager's Startup apps: those turned off there, by name, their
    // first byte odd (03; turned on again, 02).
    const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    const NAME: &str = "Wakuwaku";

    fn value(name: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_READ).and_then(|k| k.get_value::<String, _>(name)).ok()
    }

    fn turned_off(name: &str) -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(APPROVED, KEY_READ)
            .and_then(|k| k.get_raw_value(name))
            .is_ok_and(|v| v.bytes.first().is_some_and(|b| b & 1 == 1))
    }

    fn write(name: &str, command: &str) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN, KEY_READ | KEY_WRITE).map_err(|e| e.to_string())?;
        key.set_value(name, &command.to_string()).map_err(|e| e.to_string())
    }

    fn delete(path: &str, name: &str) -> Result<(), String> {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(path, KEY_READ | KEY_WRITE) else { return Ok(()) };
        match key.delete_value(name) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    fn login(name: &str, exe: &str) -> Login {
        login_of(value(name).as_deref(), turned_off(name), exe)
    }

    // On for this program: a Run value naming it (written by an older
    // version or moved here as she started, too), not turned off in Task
    // Manager.
    pub fn is_on() -> bool {
        login(NAME, &launch_command()) == Login::Mine
    }

    // This user's logon task: the Run value's name, "at logon", their SID;
    // and the SID.
    fn task_of(name: &str) -> Result<(String, String), String> {
        let sid = logon::user_sid().ok_or("no SID for this user")?;
        Ok((format!("{name} at logon-{sid}"), sid))
    }

    // The logon task made, repaired or removed to match start at login and
    // her home; what was done, if anything.
    fn sync_task(name: &str, exe: &str, taskbar: bool) -> Result<Option<String>, String> {
        let login = login(name, exe);
        if login == Login::Theirs {
            return Ok(None);
        }
        let (task, sid) = task_of(name)?;
        let seen = logon::xml(&task)?.map(|xml| logon::seen(&xml));
        match task_fix(login, taskbar, seen.as_ref(), exe) {
            TaskFix::Keep => Ok(None),
            TaskFix::Make => {
                logon::register(&task, &logon::task_xml(&sid, exe, &task_args()))?;
                Ok(Some(match seen {
                    Some(was) => format!("her logon task pointed at {} {}; now {exe} {}", was.program, was.args, task_args()),
                    None => format!("her logon task made: {task}"),
                }))
            }
            TaskFix::Remove => {
                logon::remove(&task)?;
                Ok(Some(format!("her logon task removed ({})", if login == Login::Off { "start at login is off" } else { "the island is her home" })))
            }
        }
    }

    fn set_named(name: &str, exe: &str, on: bool, taskbar: bool) -> Result<(), String> {
        if on {
            write(name, &run_command(exe))?;
            // Turned off in Task Manager before: on there too, as asked here.
            delete(APPROVED, name)?;
        } else {
            delete(RUN, name)?;
        }
        sync_task(name, exe, taskbar).map(drop).map_err(|e| format!("her logon task: {e}"))
    }

    // The switch: the Run value and (taskbar) the logon task, or neither.
    pub fn set(on: bool, taskbar: bool) -> Result<(), String> {
        set_named(NAME, &launch_command(), on, taskbar)
    }

    fn at_start_named(name: &str, exe: &str, from_source: bool, taskbar: bool) -> Vec<String> {
        let mut said = Vec::new();
        let was = value(name);
        if let RunFix::Rewrite(why) = run_fix(was.as_deref(), exe, from_source, |p| std::path::Path::new(p).is_file()) {
            let now = run_command(exe);
            let was = was.unwrap_or_default();
            said.push(match write(name, &now) {
                Ok(()) => format!("start at login pointed at {was} ({why}); now {now}"),
                Err(e) => format!("start at login pointed at {was} ({why}); not written anew: {e}"),
            });
        }
        match sync_task(name, exe, taskbar) {
            Ok(done) => said.extend(done),
            Err(e) => said.push(format!("her logon task: {e}")),
        }
        said
    }

    // As she starts on her own port: the Run value moved to this program,
    // the task to match; what was done, a line each.
    pub fn at_start(taskbar: bool) -> Vec<String> {
        at_start_named(NAME, &launch_command(), crate::data::from_source(), taskbar)
    }

    // Her home changed: the task there for the taskbar alone.
    pub fn home_changed(taskbar: bool) -> Vec<String> {
        match sync_task(NAME, &launch_command(), taskbar) {
            Ok(done) => done.into_iter().collect(),
            Err(e) => vec![format!("her logon task: {e}")],
        }
    }

    // Started by her logon task while start at login is off (the Run value
    // taken away, or turned off in Task Manager): the task goes, and true
    // (she goes too).
    pub fn task_unwanted() -> bool {
        if login(NAME, &launch_command()) != Login::Off {
            return false;
        }
        if let Ok((task, _)) = task_of(NAME) {
            let _ = logon::remove(&task);
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Against this computer's own registry and Task Scheduler, under a
        // test's name (WakuwakuTest, its task WakuwakuTest at logon-<SID>),
        // all of it taken away at the end: cargo test -- --ignored
        // start_at_login_kept_on_this_computer --nocapture.
        #[test]
        #[ignore]
        fn start_at_login_kept_on_this_computer() {
            const TEST: &str = "WakuwakuTest";
            let exe = launch_command();
            let (task, _) = task_of(TEST).unwrap();
            let was = r#""C:\nowhere\wakuwaku-v0.2.5-windows-x64.exe""#;
            let windows_view = || {
                let out = std::process::Command::new("schtasks").args(["/Query", "/TN", &task, "/V", "/FO", "LIST"]).output().unwrap();
                println!("{}", String::from_utf8_lossy(&out.stdout));
            };
            let clean = || {
                let _ = delete(RUN, TEST);
                let _ = delete(APPROVED, TEST);
                let _ = logon::remove(&task);
            };
            clean();

            // Off: nothing made at the start.
            assert!(at_start_named(TEST, &exe, false, true).is_empty());
            assert_eq!(value(TEST), None);
            assert_eq!(logon::xml(&task).unwrap(), None);

            // A value for an exe gone: this one's, and the task made for it.
            write(TEST, was).unwrap();
            let said = at_start_named(TEST, &exe, false, true);
            println!("{said:#?}");
            assert!(said[0].contains("(gone)"), "{said:?}");
            assert_eq!(value(TEST), Some(run_command(&exe)));
            assert_eq!(login(TEST, &exe), Login::Mine);
            let seen = logon::seen(&logon::xml(&task).unwrap().unwrap());
            assert_eq!((seen.program.as_str(), seen.args.as_str(), seen.enabled), (exe.as_str(), task_args().as_str(), true));
            windows_view();
            // Again: nothing to do.
            assert!(at_start_named(TEST, &exe, false, true).is_empty());

            // From the source tree another exe's value stays as it is.
            write(TEST, was).unwrap();
            assert!(at_start_named(TEST, &exe, true, true).is_empty());
            assert_eq!(value(TEST).as_deref(), Some(was));
            write(TEST, &run_command(&exe)).unwrap();

            // The island her home: the task goes, the value stays.
            assert_eq!(sync_task(TEST, &exe, false).unwrap().as_deref(), Some("her logon task removed (the island is her home)"));
            assert_eq!(logon::xml(&task).unwrap(), None);
            assert_eq!(sync_task(TEST, &exe, true).unwrap(), Some(format!("her logon task made: {task}")));

            // Turned off in Task Manager: off here, the task gone; on again from the switch.
            let off = winreg::RegValue { bytes: vec![3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8].into(), vtype: winreg::enums::RegType::REG_BINARY };
            RegKey::predef(HKEY_CURRENT_USER).create_subkey(APPROVED).unwrap().0.set_raw_value(TEST, &off).unwrap();
            assert_eq!(login(TEST, &exe), Login::Off);
            assert_eq!(at_start_named(TEST, &exe, false, true), vec!["her logon task removed (start at login is off)".to_string()]);
            set_named(TEST, &exe, true, true).unwrap();
            assert!(!turned_off(TEST));
            assert_eq!(login(TEST, &exe), Login::Mine);
            assert!(logon::xml(&task).unwrap().is_some());

            // Off from the switch: both gone.
            set_named(TEST, &exe, false, true).unwrap();
            assert_eq!(value(TEST), None);
            assert_eq!(logon::xml(&task).unwrap(), None);
            clean();
        }
    }
}

#[cfg(not(windows))]
mod login {
    pub fn is_on() -> bool {
        false
    }

    pub fn set(_on: bool, _taskbar: bool) -> Result<(), String> {
        Err("not on this system".into())
    }

    pub fn at_start(_taskbar: bool) -> Vec<String> {
        Vec::new()
    }

    pub fn home_changed(_taskbar: bool) -> Vec<String> {
        Vec::new()
    }

    pub fn task_unwanted() -> bool {
        false
    }
}

pub use login::{at_start as login_at_start, home_changed as login_home_changed, is_on as is_open_at_login, set as set_open_at_login, task_unwanted as login_task_unwanted};

#[cfg(test)]
mod tests {
    #[test]
    fn agy_hooks_install_status_and_uninstall() {
        let program = r"D:\pet\wakuwaku.exe";
        let theirs = json!({ "my-linter-hook": { "PostToolUse": [{ "matcher": "run_command", "hooks": [{ "command": "./lint.sh" }] }] } });
        assert_eq!(agy_status_of(&theirs, Some(program)), "missing");
        let with = agy_install(&theirs, program);
        assert_eq!(agy_status_of(&with, Some(program)), "ok");
        assert_eq!(with["wakuwaku"]["Stop"][0]["command"], r"D:\pet\wakuwaku.exe --wakuwaku-agy-hook Stop");
        assert_eq!(with["wakuwaku"]["PostToolUse"][0]["matcher"], "");
        // Theirs kept, ours alone taken out.
        assert_eq!(with["my-linter-hook"], theirs["my-linter-hook"]);
        assert_eq!(agy_uninstall(&with), theirs);
        assert_eq!(agy_status_of(&with, Some(r"E:\elsewhere\wakuwaku.exe")), "stale");
        assert_eq!(agy_status_of(&with, None), "stale");
        let mut off = with.clone();
        off["wakuwaku"]["enabled"] = json!(false);
        assert_eq!(agy_status_of(&off, Some(program)), "off");
        off["wakuwaku"]["enabled"] = json!(true);
        assert_eq!(agy_status_of(&off, Some(program)), "ok");
    }

    #[test]
    fn the_omp_extension_is_written_for_this_copy() {
        let exe = r#"C:\Program Files\Wakuwaku\it's "her".exe"#;
        let text = omp_extension(47312, exe, r"C:\waku\omp\data");
        assert!(text.contains(r#"const PORT = Number("47312") || 47213"#), "the port filled in");
        assert!(text.contains(&format!("const EXE = {}", json!(exe))), "the exe as a JS string");
        assert!(text.contains(r#"const DATA = "C:\\waku\\omp\\data""#));
        assert!(!text.contains("\"__WAKUWAKU_"), "nothing left to fill in");
        assert!(omp_extension(47213, "", "").contains(r#"const DATA = """#));
        // Every agent folder has it as this copy writes it, or not.
        let (ours, old) = (Some(text.clone()), Some(omp_extension(47213, exe, "")));
        assert_eq!(omp_status_of(&[None, None], &text), "missing");
        assert_eq!(omp_status_of(&[ours.clone()], &text), "ok");
        assert_eq!(omp_status_of(&[ours.clone(), ours.clone()], &text), "ok");
        assert_eq!(omp_status_of(&[ours.clone(), None], &text), "partial");
        assert_eq!(omp_status_of(&[ours.clone(), old.clone()], &text), "stale");
        assert_eq!(omp_status_of(&[None, old], &text), "stale");
    }

    #[test]
    fn the_omp_extension_goes_into_each_agent_folder_and_out() {
        let root = std::env::temp_dir().join(format!("wakuwaku-omp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // No profiles: the agent folder alone, made or not; PI_CODING_AGENT_DIR in its place.
        assert_eq!(agent_dirs_in(&root, None), vec![root.join("agent")]);
        assert_eq!(agent_dirs_in(&root, Some(PathBuf::from(r"D:\omp-agent"))), vec![PathBuf::from(r"D:\omp-agent")]);
        // Each profile's (a folder with no agent folder yet is none).
        for p in ["work", "home"] {
            fs::create_dir_all(root.join("profiles").join(p).join("agent")).unwrap();
        }
        fs::create_dir_all(root.join("profiles").join("empty")).unwrap();
        let dirs = agent_dirs_in(&root, None);
        assert_eq!(dirs, vec![root.join("agent"), root.join("profiles").join("home").join("agent"), root.join("profiles").join("work").join("agent")]);

        let files: Vec<PathBuf> = dirs.iter().map(|d| d.join("extensions").join(OMP_FILE)).collect();
        let theirs = root.join("agent").join("extensions").join("theirs.ts");
        fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        fs::write(&theirs, "export default () => {}").unwrap();
        write_omp_files(&files, "install", "ours").unwrap();
        assert!(files.iter().all(|f| fs::read_to_string(f).unwrap() == "ours"));
        write_omp_files(&files, "install", "ours again").unwrap();
        assert_eq!(fs::read_to_string(&files[1]).unwrap(), "ours again");
        // Out: ours alone, and an extensions folder left empty.
        write_omp_files(&files, "remove", "").unwrap();
        assert!(files.iter().all(|f| !f.exists()));
        assert!(theirs.exists(), "another extension stays");
        assert!(!root.join("profiles").join("work").join("agent").join("extensions").exists());
        assert!(root.join("profiles").join("work").join("agent").is_dir());
        write_omp_files(&files, "remove", "").unwrap();
        assert!(write_omp_files(&files, "frob", "").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_cmd_reads_without_quotes() {
        assert!(is_bare(r"D:\pet\wakuwaku.exe"));
        assert!(is_bare(r"C:\Users\张三\wakuwaku.exe"));
        assert!(!is_bare(r"C:\Program Files\Wakuwaku\wakuwaku.exe"));
        assert!(!is_bare(r"C:\Program Files (x86)\w.exe"));
        assert!(!is_bare(r"C:\a&b\w.exe"));
        assert_eq!(agy_program(r"D:\pet\wakuwaku.exe").as_deref(), Some(r"D:\pet\wakuwaku.exe"));
        assert_eq!(super::error_kind(&format!("x: {AGY_NO_NAME} (a space)")), "path");
    }

    #[test]
    fn a_write_that_failed_says_why() {
        assert_eq!(super::error_kind("另一个程序正在使用此文件，进程无法访问。 (os error 32)"), "locked");
        assert_eq!(super::error_kind("expected `,` or `}` at line 3 column 5"), "invalid");
        assert_eq!(super::error_kind("拒绝访问。 (os error 5)"), "other");
    }

    use super::*;

    #[test]
    fn install_status_and_uninstall() {
        let mine = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "echo hi" }] }] }, "model": "x" });
        assert_eq!(status_of(&mine, 47213), "missing");

        let with = install(&mine, 47213, false);
        assert_eq!(status_of(&with, 47213), "ok");
        assert_eq!(status_of(&with, 47299), "stale");
        assert_eq!(with["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(with["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 300);
        assert_eq!(with["hooks"]["SessionStart"][0]["hooks"][0]["args"], json!([ENSURE_FLAG]));
        assert_eq!(status_of(&install(&mine, 47213, true), 47213), "httpOnly");

        // Installing twice replaces, never doubles.
        let again = install(&with, 47213, false);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);

        let without = uninstall(&with);
        assert_eq!(without, mine);
        assert_eq!(uninstall(&install(&json!({}), 47213, false)), json!({}));
    }

    #[test]
    fn some_events_missing_is_partial() {
        let mut with = install(&json!({}), 47213, false);
        with["hooks"].as_object_mut().unwrap().remove("Elicitation");
        assert_eq!(status_of(&with, 47213), "partial");
    }

    #[test]
    fn codex_hooks_install_status_and_uninstall() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        assert_eq!(cmd, r"D:\pet\wakuwaku.exe --wakuwaku-codex-hook");
        let mine = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "notify-me" }] }] }, "other": 1 });
        assert_eq!(codex_status_of(&mine, &cmd, true), "missing");

        let with = codex_install(&mine, &cmd, "waiting", true);
        assert_eq!(codex_status_of(&with, &cmd, true), "ok");
        assert_eq!(with["hooks"]["Stop"].as_array().unwrap().len(), 2);
        let ask = &with["hooks"]["PermissionRequest"][0]["hooks"][0];
        assert_eq!((ask["timeout"].as_u64(), ask.get("async"), ask["statusMessage"].as_str()), (Some(300), None, Some("waiting")));
        // A question waits in the foreground; every step in the background.
        let pre = with["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!((pre[0]["matcher"].as_str(), pre[0]["hooks"][0].get("async")), (Some("^request_user_input$"), None));
        assert_eq!((pre[1].get("matcher"), &pre[1]["hooks"][0]["async"]), (None, &json!(true)));
        assert_eq!(with["hooks"]["PostToolUse"][0]["matcher"], "^(apply_patch|request_user_input)$");
        // Another copy of the program: a repair.
        assert_eq!(codex_status_of(&with, &codex_command(r"E:\elsewhere\wakuwaku.exe"), true), "stale");

        let again = codex_install(&with, &cmd, "waiting", true);
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(again["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        let mut part = again.clone();
        part["hooks"].as_object_mut().unwrap().remove("Interrupt");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");

        assert_eq!(uninstall(&with), mine);
        assert_eq!(uninstall(&codex_install(&json!({}), &cmd, "w", true)), json!({}));
    }

    #[test]
    fn an_older_codex_gets_no_background_hooks() {
        let cmd = codex_command(r"D:\pet\wakuwaku.exe");
        let old = codex_install(&json!({}), &cmd, "w", false);
        let all: Vec<&Value> = old["hooks"].as_object().unwrap().values().flat_map(|g| g.as_array().unwrap()).collect();
        assert_eq!(all.len(), 8);
        assert!(all.iter().all(|g| g["hooks"][0].get("async").is_none()));
        assert_eq!(codex_status_of(&old, &cmd, false), "ok");
        // Codex updated since: only the background ones lack.
        assert_eq!(codex_status_of(&old, &cmd, true), "upgradable");
        let mut part = old.clone();
        part["hooks"].as_object_mut().unwrap().remove("Stop");
        assert_eq!(codex_status_of(&part, &cmd, true), "partial");
        // Installed for a newer Codex, read for an older one: still ours, nothing stale.
        assert_eq!(codex_status_of(&codex_install(&json!({}), &cmd, "w", true), &cmd, false), "ok");
    }

    #[test]
    fn codex_versions() {
        assert_eq!(parse_version("codex-cli 0.146.1\n"), Some((0, 146, 1)));
        assert_eq!(parse_version("codex-cli 0.149.0-alpha.1"), Some((0, 149, 0)));
        assert_eq!(parse_version("nothing here"), None);
        assert!((0, 146, 1) < CODEX_ASYNC_SINCE && (0, 148, 0) >= CODEX_ASYNC_SINCE && (1, 0, 0) >= CODEX_ASYNC_SINCE);
    }

    #[test]
    fn codex_commands_quote_what_needs_it() {
        if cfg!(windows) {
            assert_eq!(codex_command(r"C:\Program Files\Waku\wakuwaku.exe"), r"& 'C:\Program Files\Waku\wakuwaku.exe' --wakuwaku-codex-hook");
            assert_eq!(codex_command(r"D:\宠物\it's.exe"), r"& 'D:\宠物\it''s.exe' --wakuwaku-codex-hook");
        } else {
            assert_eq!(codex_command("/opt/my pets/wakuwaku"), "'/opt/my pets/wakuwaku' --wakuwaku-codex-hook");
        }
    }

    #[test]
    fn start_at_login_moves_with_her() {
        use super::rules::*;
        use crate::logon::TaskSeen;
        let exe = r"D:\Program Files\Wakuwaku\wakuwaku.exe";
        let ours = run_command(exe);
        assert_eq!(ours, r#""D:\Program Files\Wakuwaku\wakuwaku.exe" --wakuwaku-at-login run"#);
        let old = r#""D:\Program Files\Wakuwaku\wakuwaku-v0.2.5-windows-x64.exe""#;
        let before = format!("\"{exe}\"");
        let (gone, there) = (|_: &str| false, |_: &str| true);

        // Off stays off, whatever she is.
        assert_eq!(run_fix(None, exe, false, gone), RunFix::Keep);
        // Renamed, moved, or another copy: this one's in its place; from a
        // source tree no other exe's.
        assert_eq!(run_fix(Some(old), exe, false, gone), RunFix::Rewrite("gone"));
        assert_eq!(run_fix(Some(old), exe, false, there), RunFix::Rewrite("another copy of her"));
        assert_eq!(run_fix(Some(old), exe, true, gone), RunFix::Keep);
        // Hers: as this version writes it, or as 0.3.0 did (no flag), which
        // even a source tree's run brings up to date.
        assert_eq!(run_fix(Some(&ours), exe, false, there), RunFix::Keep);
        assert_eq!(run_fix(Some(&before), exe, true, there), RunFix::Rewrite("as an older version wrote it"));
        if cfg!(windows) {
            assert_eq!(run_fix(Some(&ours.to_uppercase()), exe, false, there), RunFix::Keep);
        }

        // The program a value starts, quoted or not.
        assert_eq!(program_of(&ours), exe);
        assert_eq!(program_of(r"C:\Program Files\W\w.EXE --x"), r"C:\Program Files\W\w.EXE");
        assert_eq!(program_of(r"C:\w\w --x"), r"C:\w\w");
        assert_eq!(program_of(""), "");

        // On: a value naming this program, in either form, not turned off in Task Manager.
        assert_eq!(login_of(None, false, exe), Login::Off);
        assert_eq!(login_of(Some(&ours), true, exe), Login::Off);
        assert_eq!(login_of(Some(&ours), false, exe), Login::Mine);
        assert_eq!(login_of(Some(&before), false, exe), Login::Mine);
        assert_eq!(login_of(Some(old), false, exe), Login::Theirs);

        // The logon task: there in the taskbar home, running this program;
        // left alone when turned off in the Task Scheduler or another exe's.
        let task = |program: &str, enabled| TaskSeen { program: program.into(), args: task_args(), enabled };
        let elsewhere = r"D:\old\wakuwaku.exe";
        assert_eq!(task_fix(Login::Mine, true, None, exe), TaskFix::Make);
        assert_eq!(task_fix(Login::Mine, true, Some(&task(exe, true)), exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Mine, true, Some(&task(elsewhere, true)), exe), TaskFix::Make);
        assert_eq!(task_fix(Login::Mine, true, Some(&TaskSeen { args: String::new(), ..task(exe, true) }), exe), TaskFix::Make);
        assert_eq!(task_fix(Login::Mine, true, Some(&task(elsewhere, false)), exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Mine, false, None, exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Mine, false, Some(&task(exe, true)), exe), TaskFix::Remove);
        assert_eq!(task_fix(Login::Off, true, Some(&task(exe, false)), exe), TaskFix::Remove);
        assert_eq!(task_fix(Login::Off, true, None, exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Theirs, true, Some(&task(elsewhere, true)), exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Theirs, false, Some(&task(elsewhere, true)), exe), TaskFix::Keep);

        // How she was started, as the two give it to her.
        let args = |line: &str| line.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(started_by(&args("wakuwaku.exe")), None);
        assert_eq!(started_by(&args(&format!("wakuwaku.exe {}", &ours[ours.find(" --").unwrap() + 1..]))), Some("run"));
        assert_eq!(started_by(&args(&format!("wakuwaku.exe {} --wakuwaku-out-of-job", task_args()))), Some("task"));
    }
}
