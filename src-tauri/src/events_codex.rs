// Codex's hook events: agent-core reads them (codex.rs to_event), events.rs
// tells them in her words. Here, what she makes of them.
#[cfg(test)]
use serde_json::{json, Value};

#[cfg(test)]
fn to_message(e: &Value) -> Option<Value> {
    agent_core::codex::to_event(e).map(|ev| crate::events::message_of(&ev))
}
#[cfg(test)]
mod tests {
    use super::*;

    // As Codex 0.146 sends them (trimmed); the folder is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": "s", "turn_id": "t1", "cwd": "Z:\\nowhere\\pet", "hook_event_name": name, "model": "gpt-5.6-luna" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn codex_events_become_messages() {
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "cd x && cargo test" }, "tool_use_id": "u1" }))).unwrap();
        assert_eq!(
            m,
            json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.command", "vars": { "what": "cargo test" } },
                    "toolId": "u1", "agent": "codex", "session": "s", "turn": "t1", "project": "pet" })
        );
        let patch = "*** Begin Patch\n*** Add File: D:\\w\\note.txt\n+x\n*** End Patch";
        let m = to_message(&event("PostToolUse", json!({ "tool_name": "apply_patch", "tool_input": { "command": patch } }))).unwrap();
        assert_eq!((m["event"].as_str(), m["file"].as_str()), (Some("tool-done"), Some("D:\\w\\note.txt")));
        let m = to_message(&event("PreToolUse", json!({ "tool_name": "apply_patch", "tool_input": { "command": patch } }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "note.txt" } }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "fix the login page" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("fix the login page")));
        let m = to_message(&event("Stop", json!({ "stop_hook_active": false, "last_assistant_message": "All set." }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["reply"].as_str()), (Some("done"), Some("turn-end"), Some("All set.")));
        let m = to_message(&event("Interrupt", json!({}))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
        let m = to_message(&event("PermissionRequest", json!({ "tool_name": "Bash", "tool_input": { "command": "rm -rf build" } }))).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "rm -rf build" } } } }));
        assert_eq!(to_message(&event("PreToolUse", json!({ "tool_name": "request_user_input" }))).unwrap()["mood"], "waiting");
        assert_eq!(to_message(&event("SessionStart", json!({ "source": "startup" }))).unwrap()["react"], "wave");
        assert_eq!(to_message(&event("SessionEnd", json!({ "reason": "other" }))).unwrap()["event"], "session-end");
    }

    // The desktop app's own request (a suggested next message) is no task of
    // the person's: its session gone, its ending nothing; with an image, the
    // person's words after the app's headings are the task (#74).
    #[test]
    fn codex_desktop_asking_of_its_own_accord() {
        let mine = |name: &str, turn: &str, more: Value| {
            let mut e = event(name, more);
            e["session_id"] = json!("desk-74");
            e["turn_id"] = json!(turn);
            to_message(&e)
        };
        let ask = "This is an app-generated request for a suggested next user message. Reply with one short message.";
        assert_eq!(mine("UserPromptSubmit", "a1", json!({ "prompt": ask })).unwrap()["event"], "session-end");
        assert!(mine("Stop", "a1", json!({ "last_assistant_message": "Thanks!" })).is_none());
        let image = "# Files mentioned by the user:\n\n## shot.png: C:\\w\\shot.png\nImage attachment: true\n\n## My request for Codex:\nwhy is this button grey?\n";
        assert_eq!(mine("UserPromptSubmit", "p1", json!({ "prompt": image })).unwrap()["task"], "why is this button grey?");
    }

    #[test]
    fn codex_events_that_change_nothing() {
        assert!(to_message(&event("SessionStart", json!({ "source": "compact" }))).is_none());
        for name in ["PreCompact", "PostCompact", "SubagentStart", "SubagentStop", "Nope"] {
            assert!(to_message(&event(name, json!({}))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }
}
