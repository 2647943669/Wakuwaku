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
// that failed, which gets no event of its own), Stop with nothing
// more: no last reply (DSH keeps its session log zstd-compressed), no
// prompt waiting on the person (its approvals stay in its own window). Its
// tools have names of their own (read, write, edit, str_replace_editor,
// pwsh or bash, glob, grep, web_search, web_fetch, todo_write,
// ask_user_question), told in Claude Code's words here (claude_named). The
// session's name is DSH's own, from its projection cache (title_of).
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

// The message for an event, or None for one that changes nothing.
pub fn to_message(e: &Value, home: &std::path::Path) -> Option<Value> {
    let fields: &Map<String, Value> = e.as_object()?;
    let mut claude = fields.clone();
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
