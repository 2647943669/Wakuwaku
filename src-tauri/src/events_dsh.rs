// DeepSeek Harness hook events → pet messages: Claude Code's (events.rs),
// its tools named as Claude Code's, plus agent: "dsh".
//
// DSH runs them through its Claude Code hook bridge
// (@deepseek-ai/dsh-hooks-claude-code: connection.rs), each as this program
// with connection::DSH_FLAG and the event's name, which hands it to
// /hook?agent=dsh (main.rs dsh_hook). What there is, as the desktop app
// 0.2.0-rc.2 and its CLI send it (2026-10-15): session_id, cwd,
// hook_event_name and an empty transcript_path; SessionStart with source,
// UserPromptSubmit with prompt, PreToolUse and PostToolUse with tool_name,
// tool_input, tool_use_id (and tool_response, as text: "Error: …" for one
// that failed, which gets no event of its own), Stop with nothing more: its
// last reply is read from its session log, zstd-compressed (last_reply); no
// prompt waiting on the person (its approvals stay in its own window). Its
// tools have names of their own (read, write, edit, str_replace_editor,
// pwsh or bash, glob, grep, web_search, web_fetch, todo_write,
// ask_user_question), told in Claude Code's words here (claude_named). The
// session's name is DSH's own, from its projection cache (title_of).
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

// A DSH tool and its input as Claude Code names them, for events.rs.
fn claude_named(tool: &str, input: &Value) -> (String, Value) {
    let file = |key: &str| json!({ "file_path": s(input, key) });
    match tool {
        "read" | "read_image" => ("Read".into(), file("file_path")),
        "write" => ("Write".into(), file("file_path")),
        "edit" => ("Edit".into(), file("file_path")),
        "str_replace_editor" => match s(input, "command") {
            "view" => ("Read".into(), file("path")),
            "create" => ("Write".into(), file("path")),
            _ => ("Edit".into(), file("path")),
        },
        "pwsh" | "bash" => ("Bash".into(), json!({ "command": s(input, "command"), "description": s(input, "description") })),
        "glob" => ("Glob".into(), input.clone()),
        "grep" => ("Grep".into(), input.clone()),
        "web_search" => ("WebSearch".into(), input.clone()),
        "web_fetch" => ("WebFetch".into(), input.clone()),
        "todo_write" => {
            let todos: Vec<Value> = input.get("todos").and_then(Value::as_array).into_iter().flatten().map(|t| json!({ "content": s(t, "content"), "status": s(t, "status") })).collect();
            ("TodoWrite".into(), json!({ "todos": todos }))
        }
        "ask_user_question" => ("AskUserQuestion".into(), input.clone()),
        _ => (tool.to_string(), input.clone()),
    }
}

// The session's name as DSH gave it (its first-prompt title, or one its
// model made up), from its projection cache:
// <DSH home>/storages/session_projcache/sessions/<session id>.json, whose
// rows.title.val it is. Undocumented: none when it reads otherwise.
pub fn title_of(home: &std::path::Path, session: &str) -> Option<String> {
    if session.is_empty() || !session.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    let file = home.join("storages").join("session_projcache").join("sessions").join(format!("{session}.json"));
    let cache: Value = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    let title = cache.pointer("/record/rows/title/val")?.as_str()?.trim();
    (!title.is_empty()).then(|| crate::events::clip(title, 80))
}

// --- Its last reply, from its session log ------------------------------------------
//
// DSH keeps each session's events as JSONL, compressed in Zstandard frames,
// one appended for each batch it writes (dsh-session-persistence-jsonl
// 0.2.0-rc.2): <home>/sessions/<the cwd's key>/<the session id>/
// session.v4.jsonl.zstd. A turn's last words are an assistant/message, its
// data.message.content's text parts, in one of the last frames: written
// with the Stop hook's hook/invoked, so there as the hook runs. Read from
// the end: only the file's tail (more of it while a turn needs it, up to
// MAX_WINDOW), each frame found back from the end by its magic number and
// taken only when it decodes whole into lines of JSON and ends where the one
// after it begins (a frame still being written at the end is passed over),
// newest first, up to the turn's turn/start; all of it in DEADLINE at most.
// Undocumented: no reply when anything reads otherwise.

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const FIRST_WINDOW: usize = 256 * 1024;
const MAX_WINDOW: usize = 4 * 1024 * 1024;
// One batch's events at most (a frame decoding to more is passed over).
const MAX_FRAME_TEXT: usize = 4 * 1024 * 1024;
// The most a frame's header may ask the decoder to hold (its window): DSH's
// level writes a few MB at most; what looks like a frame inside another's
// bytes may ask for anything (ruzstd's own bound is 128 MB).
const MAX_ZSTD_WINDOW: u64 = 16 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_millis(300);

// A name as DSH writes it into a path: [A-Za-z0-9._-] as they are, any other
// UTF-16 unit (~ too) as ~XXXX (encodeSegment).
fn escaped(c: char, out: &mut String) {
    if c != '~' && (c.is_ascii_alphanumeric() || "._-".contains(c)) {
        out.push(c);
    } else {
        let mut units = [0u16; 2];
        for unit in c.encode_utf16(&mut units) {
            out.push_str(&format!("~{unit:04X}"));
        }
    }
}

// The folder DSH keeps a project's sessions in, from its path (projectKey):
// each run of separators (/ \ :) one -, the rest escaped, leading -s gone,
// 251 at most, between --.
pub fn project_key(cwd: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in cwd.chars() {
        if matches!(c, '/' | '\\' | ':') {
            if !in_run {
                out.push('-');
            }
            in_run = true;
        } else {
            escaped(c, &mut out);
            in_run = false;
        }
    }
    let key: String = out.trim_start_matches('-').chars().take(251).collect();
    format!("--{}--", if key.is_empty() { "root" } else { &key })
}

// A session's log: the current generation's (v4), else the newest there is,
// compressed or not; a look into its own folder alone.
pub fn session_log(home: &Path, cwd: &str, session: &str) -> Option<PathBuf> {
    if cwd.is_empty() || session.is_empty() {
        return None;
    }
    let mut id = String::new();
    session.chars().for_each(|c| escaped(c, &mut id));
    let dir = home.join("sessions").join(project_key(cwd)).join(id);
    let current = dir.join("session.v4.jsonl.zstd");
    if current.is_file() {
        return Some(current);
    }
    let version = |name: &str| name.strip_prefix("session.v")?.split_once('.').filter(|(_, rest)| matches!(*rest, "jsonl" | "jsonl.zstd"))?.0.parse::<u32>().ok();
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|name| Some((version(&name)?, name)))
        .max()
        .map(|(_, name)| dir.join(name))
}

// What a line of the log says of the turn: its last words, or its start.
enum Seen {
    Reply(String),
    TurnStart,
}

fn seen(line: &str) -> Option<Seen> {
    if !line.contains("\"assistant/message\"") && !line.contains("\"turn/start\"") {
        return None;
    }
    let event: Value = serde_json::from_str(line).ok()?;
    match event["type"].as_str()? {
        "turn/start" => Some(Seen::TurnStart),
        "assistant/message" => {
            let parts = event.pointer("/data/message/content")?.as_array()?;
            let text: Vec<&str> = parts.iter().filter(|p| p["type"] == "text").filter_map(|p| p["text"].as_str()).collect();
            let text = text.join("\n");
            (!text.trim().is_empty()).then(|| Seen::Reply(text.trim().to_string()))
        }
        _ => None,
    }
}

// One whole frame from the start of these bytes: its text, and how many of
// the bytes it took. None when it does not decode, or not into lines of JSON.
fn decode_frame(bytes: &[u8]) -> Option<(String, usize)> {
    let mut source = bytes;
    let mut out = Vec::new();
    {
        let decoder = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(&mut source, MAX_ZSTD_WINDOW).ok()?;
        decoder.take(MAX_FRAME_TEXT as u64 + 1).read_to_end(&mut out).ok()?;
    }
    if out.len() > MAX_FRAME_TEXT {
        return None;
    }
    let text = String::from_utf8(out).ok()?;
    let is_jsonl = text.lines().map(str::trim).filter(|l| !l.is_empty()).all(|l| l.starts_with('{') && l.ends_with('}'));
    is_jsonl.then(|| (text, bytes.len() - source.len()))
}

// What a tail of the log gives: the turn's last words, none (the turn began
// without any, or the whole log was read), or more of the log is needed.
enum Scan {
    Found(String),
    NoReply,
    More,
}

fn scan_frames(tail: &[u8], is_whole: bool, until: Instant) -> Scan {
    let mut starts = Vec::new();
    let mut at = 0;
    while let Some(i) = tail[at..].windows(4).position(|w| w == ZSTD_MAGIC) {
        starts.push(at + i);
        at += i + 1;
    }
    // The start of the frame taken last (the one after this, in the log).
    let mut next: Option<usize> = None;
    for &start in starts.iter().rev() {
        if Instant::now() > until {
            return Scan::NoReply;
        }
        if next.is_some_and(|n| start >= n) {
            continue;
        }
        let Some((text, used)) = decode_frame(&tail[start..]) else { continue };
        let end = start + used;
        let fits = match next {
            Some(n) => end == n,
            // The newest whole frame: the end, or one being written after it.
            None => end == tail.len() || tail[end..].starts_with(&ZSTD_MAGIC),
        };
        if !fits {
            continue;
        }
        next = Some(start);
        for line in text.lines().rev() {
            match seen(line) {
                Some(Seen::Reply(reply)) => return Scan::Found(reply),
                Some(Seen::TurnStart) => return Scan::NoReply,
                None => {}
            }
        }
    }
    if is_whole { Scan::NoReply } else { Scan::More }
}

// The same for a log written as plain JSONL.
fn scan_lines(tail: &[u8], is_whole: bool) -> Scan {
    let text = String::from_utf8_lossy(tail);
    // A line cut by the window's start, or one still being written, is no line.
    let mut lines: Vec<&str> = text.split('\n').collect();
    lines.pop();
    for line in lines.iter().skip(usize::from(!is_whole)).rev() {
        match seen(line) {
            Some(Seen::Reply(reply)) => return Scan::Found(reply),
            Some(Seen::TurnStart) => return Scan::NoReply,
            None => {}
        }
    }
    if is_whole { Scan::NoReply } else { Scan::More }
}

// The last words of the session's turn just ended, from its log.
pub fn last_reply(log: &Path) -> Option<String> {
    last_reply_from(log, FIRST_WINDOW)
}

fn last_reply_from(log: &Path, first_window: usize) -> Option<String> {
    use std::io::{Seek, SeekFrom};
    let until = Instant::now() + DEADLINE;
    let compressed = log.extension().is_some_and(|e| e == "zstd");
    let mut file = std::fs::File::open(log).ok()?;
    let len = file.metadata().ok()?.len();
    let mut window = first_window.max(1);
    loop {
        let take = len.min(window as u64);
        file.seek(SeekFrom::Start(len - take)).ok()?;
        let mut tail = Vec::with_capacity(take as usize);
        (&mut file).take(take).read_to_end(&mut tail).ok()?;
        let is_whole = take == len;
        let scan = if compressed { scan_frames(&tail, is_whole, until) } else { scan_lines(&tail, is_whole) };
        match scan {
            Scan::Found(reply) => return Some(reply),
            Scan::NoReply => return None,
            Scan::More if is_whole || window >= MAX_WINDOW || Instant::now() > until => return None,
            Scan::More => window = (window * 4).min(MAX_WINDOW),
        }
    }
}

// The message for an event, or None for one that changes nothing.
pub fn to_message(e: &Value, home: &std::path::Path) -> Option<Value> {
    let fields: &Map<String, Value> = e.as_object()?;
    let mut claude = fields.clone();
    // A turn's end: its last words, as Claude Code gives them.
    if s(e, "hook_event_name") == "Stop" && s(e, "last_assistant_message").is_empty() {
        if let Some(reply) = session_log(home, s(e, "cwd"), s(e, "session_id")).and_then(|log| last_reply(&log)) {
            claude.insert("last_assistant_message".into(), json!(reply));
        }
    }
    // A tool that failed (an edit the person refused in DSH's own dialog
    // too) still ends in PostToolUse, its result an error's words: a
    // failure, which changed nothing.
    if s(e, "hook_event_name") == "PostToolUse" && s(e, "tool_response").trim_start().starts_with("Error") {
        claude.insert("hook_event_name".into(), json!("PostToolUseFailure"));
    }
    if let Some(tool) = fields.get("tool_name").and_then(Value::as_str) {
        let (name, input) = claude_named(tool, fields.get("tool_input").unwrap_or(&Value::Null));
        claude.insert("tool_name".into(), json!(name));
        claude.insert("tool_input".into(), input);
    }
    let claude = Value::Object(claude);
    let mut message = crate::events::to_message(&claude)?;
    message["agent"] = json!("dsh");
    // The to-do list as its tool ends: the one hook it has (connection.rs).
    if s(e, "hook_event_name") == "PostToolUse" {
        if let Some(todo) = crate::events::todo_of(s(&claude, "tool_name"), claude.get("tool_input").unwrap_or(&Value::Null)) {
            message["todo"] = todo;
        }
    }
    if matches!(s(e, "hook_event_name"), "SessionStart" | "UserPromptSubmit" | "Stop") {
        if let Some(title) = title_of(home, s(e, "session_id")) {
            message["title"] = json!(title);
        }
    }
    Some(message)
}

// A line for her log as a session starts or a turn begins or ends: what came,
// and whether she found the app it runs in (server.rs chain_for), for
// telling why its window can't be reached.
pub fn log_line(e: &Value, found: Option<u32>) -> Option<String> {
    let name = s(e, "hook_event_name");
    if !matches!(name, "SessionStart" | "UserPromptSubmit" | "Stop") {
        return None;
    }
    let app = found.map_or("no app found".to_string(), |pid| format!("its app found (pid {pid})"));
    Some(format!("dsh {name} ({}){}", s(e, "session_id").chars().skip(8).take(8).collect::<String>(), if name == "Stop" { String::new() } else { format!(", {app}") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // As the CLI 0.2.0-rc.2's bridge sends them (from the test VM), in a
    // folder that is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": "session-c961", "transcript_path": "", "cwd": "Z:\\nowhere\\dsh", "hook_event_name": name });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    fn nowhere() -> std::path::PathBuf {
        std::path::PathBuf::from("Z:\\nowhere\\.dsh")
    }

    #[test]
    fn dsh_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({ "source": "startup" })), &nowhere()).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "dsh", "session": "session-c961", "project": "dsh" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Read note.txt, then create copy.txt" })), &nowhere()).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("Read note.txt, then create copy.txt")));
        let write = json!({ "tool_name": "write", "tool_input": { "file_path": "Z:\\nowhere\\dsh\\copy.txt", "content": "hi\n" }, "tool_use_id": "call_00_CKTB", "tool_response": "<path>…</path>" });
        let m = to_message(&event("PostToolUse", write), &nowhere()).unwrap();
        assert_eq!((m["event"].as_str(), m["detail"].as_str(), m["file"].as_str()), (Some("tool-done"), Some("Write"), Some("Z:\\nowhere\\dsh\\copy.txt")));
        // Refused in DSH's own dialog: no edit.
        let refused = json!({ "tool_name": "write", "tool_input": { "file_path": "C:\\waku\\agents\\outside.txt" }, "tool_response": "Error: the user rejected escalating this operation to \"danger-full-access\"" });
        let m = to_message(&event("PostToolUse", refused), &nowhere()).unwrap();
        assert_eq!((m["event"].as_str(), m.get("file")), (Some("tool-failed"), None));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "pwsh", "tool_input": { "command": "Get-ChildItem", "description": "List files" } })), &nowhere()).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.run", "vars": { "what": "List files" } }));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "str_replace_editor", "tool_input": { "command": "str_replace", "path": "Z:\\a\\b.rs" } })), &nowhere()).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "b.rs" } }));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "ask_user_question", "tool_input": { "questions": [] } })), &nowhere()).unwrap();
        assert_eq!(m["mood"], "waiting");
        let todos = json!({ "todos": [{ "content": "Read note.txt", "status": "completed" }, { "content": "Write copy.txt", "status": "in_progress" }] });
        let m = to_message(&event("PostToolUse", json!({ "tool_name": "todo_write", "tool_input": todos })), &nowhere()).unwrap();
        assert_eq!(m["todo"]["set"][1], json!({ "text": "Write copy.txt", "active": "", "status": "in_progress" }));
        let m = to_message(&event("Stop", json!({ "stop_hook_active": false })), &nowhere()).unwrap();
        assert_eq!((m["mood"].as_str(), m.get("reply"), m["agent"].as_str()), (Some("done"), None, Some("dsh")));
        assert!(to_message(&event("SubagentStop", json!({ "agent_id": "a" })), &nowhere()).is_none());
        let mut start = event("UserPromptSubmit", json!({ "prompt": "x" }));
        start["session_id"] = json!("session-6aa5a96d-33fc");
        assert_eq!(log_line(&start, Some(3684)).as_deref(), Some("dsh UserPromptSubmit (6aa5a96d), its app found (pid 3684)"));
        start["hook_event_name"] = json!("Stop");
        assert_eq!(log_line(&start, None).as_deref(), Some("dsh Stop (6aa5a96d)"));
        assert!(log_line(&event("PostToolUse", json!({})), None).is_none());
        assert!(to_message(&json!("nope"), &nowhere()).is_none());
    }

    // Real session logs from the test VM (DSH 0.2.0-rc.2, test prompts only):
    // c96167f1 the CLI reading note.txt and writing copy.txt, one turn;
    // 6aa5a96d the desktop app, a question then a write it was refused, two
    // turns; b7df6078 a one-word answer.
    const C96: &[u8] = include_bytes!("../../test/fixtures/dsh/session-c96167f1.v4.jsonl.zstd");
    const A6A: &[u8] = include_bytes!("../../test/fixtures/dsh/session-6aa5a96d.v4.jsonl.zstd");
    const B7D: &[u8] = include_bytes!("../../test/fixtures/dsh/session-b7df6078.v4.jsonl.zstd");
    const C96_REPLY: &str = "Done — note.txt contained `hi`, and copy.txt now has the same text.";

    // A log written into a folder of the test's own, by this name.
    fn log_file(tag: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wakuwaku-dsh-log-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session.v4.jsonl.zstd");
        std::fs::write(&file, bytes).unwrap();
        file
    }

    #[test]
    fn where_dsh_keeps_a_session_s_log() {
        assert_eq!(project_key("C:\\Users\\waku\\Documents\\deepseek-harness\\default-workspace"), "--C-Users-waku-Documents-deepseek-harness-default-workspace--");
        assert_eq!(project_key("C:\\waku\\agents\\dsh"), "--C-waku-agents-dsh--");
        assert_eq!(project_key("/home/me/my proj~1"), "--home-me-my~0020proj~007E1--");
        assert_eq!(project_key("D:\\宠物\\a.b_c-d"), "--D-~5BA0~7269-a.b_c-d--");
        assert_eq!(project_key("/"), "--root--");
        assert_eq!(project_key(&"x".repeat(300)).len(), 255);

        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-home-{}", std::process::id()));
        let dir = home.join("sessions").join("--C-waku-agents-dsh--").join("session-c96167f1-eb93");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(session_log(&home, "C:\\waku\\agents\\dsh", "session-c96167f1-eb93"), None);
        // A later generation alone, or plain JSONL: the newest; v4 when there.
        std::fs::write(dir.join("session.v3.jsonl"), b"").unwrap();
        std::fs::write(dir.join("session.v5.jsonl.zstd"), b"").unwrap();
        std::fs::write(dir.join("session.v5.jsonl.zstd.tmp"), b"").unwrap();
        assert_eq!(session_log(&home, "C:\\waku\\agents\\dsh", "session-c96167f1-eb93"), Some(dir.join("session.v5.jsonl.zstd")));
        std::fs::write(dir.join("session.v4.jsonl.zstd"), C96).unwrap();
        let log = session_log(&home, "C:\\waku\\agents\\dsh", "session-c96167f1-eb93").unwrap();
        assert_eq!(log, dir.join("session.v4.jsonl.zstd"));
        assert_eq!(session_log(&home, "", "session-c96167f1-eb93"), None);

        // A turn's end, its last words on the island as Claude Code's are.
        let stop = json!({ "session_id": "session-c96167f1-eb93", "transcript_path": "", "cwd": "C:\\waku\\agents\\dsh", "hook_event_name": "Stop", "stop_hook_active": false });
        let m = to_message(&stop, &home).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str()), (Some("done"), Some("Done — note.txt contained hi, and copy.txt now has the same text.")));
        // Another session's, or none: no words, nothing else changed.
        let mut other = stop.clone();
        other["session_id"] = json!("session-none");
        assert_eq!(to_message(&other, &home).unwrap(), json!({ "mood": "done", "agent": "dsh", "session": "session-none", "project": "dsh" }));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn the_last_words_of_a_turn_from_its_log() {
        // A frame decodes whole and says where it ends (the session line, 156 bytes).
        let (text, used) = decode_frame(C96).unwrap();
        assert_eq!(used, 156);
        assert!(text.starts_with("{\"type\":\"session\""), "{text}");
        assert!(decode_frame(&C96[1..]).is_none());

        assert_eq!(last_reply(&log_file("c96", C96)).as_deref(), Some(C96_REPLY));
        assert_eq!(last_reply(&log_file("b7d", B7D)).as_deref(), Some("hello"));
        // Two turns: the second's, all its lines.
        let second = last_reply(&log_file("6aa", A6A)).unwrap();
        assert!(second.starts_with("I couldn't create the file — you declined the escalation request") && second.ends_with("with the content `hi`."), "{second}");
        // A window smaller than the frames: more of the log, as needed.
        assert_eq!(last_reply_from(&log_file("6aa-small", A6A), 600).as_deref(), Some(second.as_str()));
        assert_eq!(last_reply_from(&log_file("c96-small", C96), 100).as_deref(), Some(C96_REPLY));
    }

    #[test]
    fn a_log_still_being_written() {
        // Its last frame cut short (turn/end on its way): passed over.
        assert_eq!(last_reply(&log_file("cut-last", &C96[..14900])).as_deref(), Some(C96_REPLY));
        // The frame with the last words cut short: the turn's words before them.
        assert_eq!(last_reply(&log_file("cut-reply", &C96[..14500])).as_deref(), Some("I'll read note.txt first."));
        // The second turn begun, none of its words yet: none, not the first turn's.
        assert_eq!(last_reply(&log_file("cut-turn", &A6A[..16700])), None);
        // Nothing of a log, or not one.
        assert_eq!(last_reply(&log_file("empty", b"")), None);
        assert_eq!(last_reply(&log_file("junk", b"\x28\xb5\x2f\xfdjunk that is no frame")), None);
        assert_eq!(last_reply(Path::new("Z:\\nowhere\\session.v4.jsonl.zstd")), None);
    }

    #[test]
    fn a_log_in_plain_jsonl() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-dsh-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session.v4.jsonl");
        let line = |v: Value| format!("{v}\n");
        let text = [
            line(json!({ "type": "turn/start", "seq": 1, "data": { "turn": 1 } })),
            line(json!({ "type": "assistant/message", "seq": 2, "data": { "turn": 1, "message": { "role": "assistant", "content": [{ "type": "text", "text": "All done." }, { "type": "tool-call" }] } } })),
            line(json!({ "type": "turn/end", "seq": 3, "data": { "turn": 1 } })),
        ]
        .concat();
        std::fs::write(&file, &text).unwrap();
        assert_eq!(last_reply(&file).as_deref(), Some("All done."));
        assert_eq!(last_reply_from(&file, 40).as_deref(), Some("All done."));
        std::fs::write(&file, format!("{text}{}", line(json!({ "type": "turn/start", "seq": 4 })))).unwrap();
        assert_eq!(last_reply(&file), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_session_s_name_from_dsh_s_cache() {
        let home = std::env::temp_dir().join(format!("wakuwaku-dsh-{}", std::process::id()));
        let dir = home.join("storages").join("session_projcache").join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let cache = json!({ "version": 7, "record": { "rows": { "title": { "ver": 1, "seq": 44, "val": "Read note.txt and copy it" } } } });
        std::fs::write(dir.join("session-c961.json"), cache.to_string()).unwrap();
        assert_eq!(title_of(&home, "session-c961").as_deref(), Some("Read note.txt and copy it"));
        let m = to_message(&event("Stop", json!({})), &home).unwrap();
        assert_eq!(m["title"], "Read note.txt and copy it");
        assert_eq!(title_of(&home, "session-none"), None);
        assert_eq!(title_of(&home, "..\\..\\x"), None);
        let _ = std::fs::remove_dir_all(&home);
    }
}
