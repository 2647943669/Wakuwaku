// Antigravity hook events → pet messages, the same messages as events.rs makes
// (state.rs has the format), plus agent: "agy". The words for a task, a reply
// and the project are events.rs's.
//
// Antigravity (agy, and its 2.0 app) runs its hooks as commands: this program
// with connection::AGY_FLAG and the event's name, which its payload does not
// carry, hands each to /hook?agent=agy (main.rs agy_hook). What there is, as
// agy 1.3.4 sends it (2026-10-13): camelCase; conversationId the session,
// workspacePaths the folders it works in, transcriptPath its transcript.
// SessionStart; PreInvocation before each call of the model, the first of a
// turn invocationNum 0; PostToolUse with toolCall { name, args }, the args
// saying what was done in words of their own (toolAction: "Creating
// hello.txt"); Stop with fullyIdle, terminationReason and error. No prompt
// waiting on the person, no tool starting (PreToolUse would have to answer
// for every tool, and an answer missing denies it): she follows the work and
// its end. The person's words and the last reply come from the transcript
// (with_transcript), as the payload has neither.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use serde_json::{json, Value};

use crate::events::{clip, project_of, reply_of, step, task_of};

fn s<'a>(e: &'a Value, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

// What a tool did, in plain words: its own, else its name.
fn step_of_agy(tool: &str, args: &Value) -> Value {
    match [s(args, "toolAction"), s(args, "toolSummary")].into_iter().find(|w| !w.trim().is_empty()) {
        Some(words) => step("step.run", json!({ "what": clip(words.trim(), 60) })),
        None => step("step.tool", json!({ "tool": tool })),
    }
}

// The mood part of the message for an event, or None for one that changes nothing.
fn mood_of(e: &Value) -> Option<Value> {
    let call = e.get("toolCall").unwrap_or(&Value::Null);
    let tool = s(call, "name");
    let args = call.get("args").unwrap_or(&Value::Null);
    let m = match s(e, "hook_event_name") {
        "SessionStart" => json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" } }),
        // The turn's first call of the model: a turn begins.
        "PreInvocation" if e.get("invocationNum").and_then(Value::as_u64) == Some(0) => {
            let mut m = json!({ "mood": "working", "event": "turn-start" });
            if let Some(task) = task_of(s(e, "prompt")) {
                m["task"] = json!(task);
            }
            m
        }
        "PreInvocation" => json!({ "mood": "working" }),
        "PostToolUse" if !s(e, "error").is_empty() => json!({
            "mood": "working", "detail": tool, "event": "tool-failed", "react": "failed",
            "say": { "key": "say.toolFailed", "vars": { "tool": tool } },
        }),
        "PostToolUse" => {
            let mut m = json!({ "mood": "working", "detail": tool, "event": "tool-done", "step": step_of_agy(tool, args) });
            // A file it wrote (write_to_file's, the edits' TargetFile): what
            // the turn changed. (A file read is view_file's AbsolutePath.)
            if !s(args, "TargetFile").is_empty() {
                m["file"] = json!(s(args, "TargetFile"));
            }
            m
        }
        "Stop" if !s(e, "error").is_empty() => json!({ "mood": "error", "error": clip(s(e, "error").lines().next().unwrap_or(""), 80) }),
        // Stopped by the person: nothing to come and see.
        "Stop" if ["CANCEL", "INTERRUPT", "ABORT"].iter().any(|w| s(e, "terminationReason").to_uppercase().contains(w)) => json!({ "mood": "idle", "event": "turn-end" }),
        // Not done yet (something of it still runs).
        "Stop" if e.get("fullyIdle").and_then(Value::as_bool) == Some(false) => return None,
        "Stop" => {
            let mut m = json!({ "mood": "done", "event": "turn-end" });
            if let Some(reply) = reply_of(s(e, "last_assistant_message")) {
                m["reply"] = json!(reply);
            }
            m
        }
        _ => return None,
    };
    Some(m)
}

// The message for an event, or None for one that changes nothing.
pub fn to_message(e: &Value) -> Option<Value> {
    if !e.is_object() {
        return None;
    }
    let mut message = mood_of(e)?;
    message["agent"] = json!("agy");
    if !s(e, "conversationId").is_empty() {
        message["session"] = json!(s(e, "conversationId"));
    }
    if let Some(folder) = e.get("workspacePaths").and_then(Value::as_array).and_then(|w| w.first()).and_then(Value::as_str).filter(|w| !w.is_empty()) {
        message["project"] = json!(project_of(folder));
    }
    Some(message)
}

// So much of the transcript's end is read: a turn's lines are there.
const TAIL: u64 = 256 * 1024;

// The person's words for a turn starting (prompt), and the last reply for one
// ending (last_assistant_message), from the transcript's last lines, as
// agy 1.3.4 writes it: one step a line, the person's a USER_INPUT whose
// content holds them in <USER_REQUEST>, the model's answer a
// PLANNER_RESPONSE with content. Undocumented: nothing when it reads
// otherwise.
pub fn with_transcript(e: &mut Value) {
    let event = s(e, "hook_event_name");
    let wants = match event {
        "PreInvocation" if e.get("invocationNum").and_then(Value::as_u64) == Some(0) => "USER_INPUT",
        "Stop" => "PLANNER_RESPONSE",
        _ => return,
    };
    let Some(lines) = tail(s(e, "transcriptPath")) else { return };
    let last = lines
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|step| s(step, "type") == wants && !s(step, "content").trim().is_empty());
    let Some(content) = last.as_ref().map(|step| s(step, "content")) else { return };
    if wants == "USER_INPUT" {
        let said = content.split_once("<USER_REQUEST>").and_then(|(_, rest)| rest.split_once("</USER_REQUEST>")).map_or(content, |(said, _)| said);
        e["prompt"] = json!(said.trim());
    } else {
        e["last_assistant_message"] = json!(content.trim());
    }
}

fn tail(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // As agy 1.3.4 sends them (trimmed), the event's name added by main.rs.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({
            "hook_event_name": name,
            "conversationId": "5064aa2e",
            "modelName": "gemini-3.8-flash-high",
            "workspacePaths": ["Z:/nowhere/agyws"],
            "transcriptPath": "",
        });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn agy_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({}))).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "agy", "session": "5064aa2e", "project": "agyws" }));
        let m = to_message(&event("PreInvocation", json!({ "invocationNum": 0, "initialNumSteps": 1, "prompt": "Create a file named hello.txt" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("Create a file named hello.txt")));
        let m = to_message(&event("PreInvocation", json!({ "invocationNum": 1, "initialNumSteps": 3 }))).unwrap();
        assert_eq!((m["mood"].as_str(), m.get("event")), (Some("working"), None));
        let call = json!({ "name": "write_to_file", "args": { "CodeContent": "hi", "TargetFile": "Z:\\nowhere\\agyws\\hello.txt", "toolAction": "Creating hello.txt", "toolSummary": "Create hello.txt" } });
        let m = to_message(&event("PostToolUse", json!({ "stepIdx": 2, "error": "", "toolCall": call }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.run", "vars": { "what": "Creating hello.txt" } }));
        assert_eq!((m["event"].as_str(), m["file"].as_str()), (Some("tool-done"), Some("Z:\\nowhere\\agyws\\hello.txt")));
        let m = to_message(&event("PostToolUse", json!({ "error": "exit status 1", "toolCall": { "name": "run_command", "args": {} } }))).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str()), (Some("tool-failed"), Some("failed")));
        let m = to_message(&event("Stop", json!({ "executionNum": 0, "fullyIdle": true, "terminationReason": "NO_TOOL_CALL", "error": "", "last_assistant_message": "done" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["reply"].as_str()), (Some("done"), Some("turn-end"), Some("done")));
        let m = to_message(&event("Stop", json!({ "fullyIdle": true, "error": "quota exceeded" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["error"].as_str()), (Some("error"), Some("quota exceeded")));
        let m = to_message(&event("Stop", json!({ "fullyIdle": true, "terminationReason": "USER_CANCELLED", "error": "" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
    }

    #[test]
    fn agy_events_that_change_nothing() {
        assert!(to_message(&event("Stop", json!({ "fullyIdle": false, "error": "" }))).is_none());
        for name in ["PostInvocation", "PreToolUse", "Nope", ""] {
            assert!(to_message(&event(name, json!({}))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }

    #[test]
    fn the_person_s_words_and_the_reply_from_the_transcript() {
        let file = std::env::temp_dir().join(format!("wakuwaku-agy-{}.jsonl", std::process::id()));
        let lines = [
            json!({ "step_index": 0, "source": "USER_EXPLICIT", "type": "USER_INPUT", "status": "DONE", "content": "<USER_REQUEST>\nCreate a file named hello.txt\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nThe current local time is 09:47.\n</ADDITIONAL_METADATA>" }),
            json!({ "step_index": 1, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "thinking": "write it", "tool_calls": [] }),
            json!({ "step_index": 3, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "content": "done" }),
        ];
        std::fs::write(&file, lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
        let path = file.to_string_lossy().into_owned();
        let mut e = event("PreInvocation", json!({ "invocationNum": 0, "transcriptPath": path }));
        with_transcript(&mut e);
        assert_eq!(e["prompt"], "Create a file named hello.txt");
        let mut e = event("Stop", json!({ "fullyIdle": true, "transcriptPath": path }));
        with_transcript(&mut e);
        assert_eq!(e["last_assistant_message"], "done");
        // A later call of the model reads nothing.
        let mut e = event("PreInvocation", json!({ "invocationNum": 1, "transcriptPath": path }));
        with_transcript(&mut e);
        assert!(e.get("prompt").is_none());
        let _ = std::fs::remove_file(&file);
    }
}
