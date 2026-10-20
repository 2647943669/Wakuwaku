// OpenCode's events, from her plugin in OpenCode's background service
// (integrations/opencode/wakuwaku.js, installed by connection.rs): agent-core
// reads them (opencode.rs: normalize, by the names Claude Code's hooks have,
// for the prompts and the jump too; to_event), events.rs tells them in her
// words. Here, what she makes of them.
#[cfg(test)]
use serde_json::{json, Value};

#[cfg(test)]
use agent_core::opencode::normalize;

#[cfg(test)]
fn to_message(e: &Value) -> Option<Value> {
    agent_core::opencode::to_event(e).map(|ev| crate::events::message_of(&ev))
}
#[cfg(test)]
mod tests {
    use super::*;

    // As her plugin sends them from OpenCode 2.0.29 (trimmed); the folder is no git repository.
    fn event(kind: &str, more: Value) -> Value {
        let mut e = json!({ "type": kind, "sessionID": "ses_1", "directory": "Z:\\nowhere\\oc3", "title": "Shell echo command" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    fn message(kind: &str, more: Value) -> Option<Value> {
        to_message(&normalize(&event(kind, more)))
    }

    #[test]
    fn opencode_events_become_messages() {
        let m = message("session.created", json!({})).unwrap();
        assert_eq!(
            m,
            json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "opencode", "session": "ses_1", "project": "oc3", "title": "Shell echo command" })
        );
        let m = message("session.execution.started", json!({ "prompt": "Run the shell command: echo hi" })).unwrap();
        assert_eq!((m["event"].as_str(), m["task"].as_str()), (Some("turn-start"), Some("Run the shell command: echo hi")));
        let call = json!({ "id": "call_00_E9yQ", "tool": "shell", "input": { "command": "cd x && echo hi" } });
        let m = message("session.tool.called", call.clone()).unwrap();
        assert_eq!(
            (m["detail"].as_str(), m["event"].as_str(), m["toolId"].as_str(), &m["step"]),
            (Some("Bash"), Some("tool-start"), Some("call_00_E9yQ"), &json!({ "key": "step.command", "vars": { "what": "echo hi" } }))
        );
        let m = message("session.tool.success", call.clone()).unwrap();
        assert_eq!((m["event"].as_str(), m["toolId"].as_str()), (Some("tool-done"), Some("call_00_E9yQ")));
        let m = message("session.tool.failed", json!({ "tool": "shell", "input": {}, "error": { "type": "tool.execution", "message": "exit 1" } })).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str(), &m["say"]["vars"]["tool"]), (Some("tool-failed"), Some("failed"), &json!("shell")));
        let m = message("session.execution.succeeded", json!({ "text": "Done — the command output `hi`." })).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str()), (Some("done"), Some("Done — the command output hi.")));
        let m = message("session.execution.failed", json!({ "error": { "type": "provider.rate-limit", "message": "429" } })).unwrap();
        assert_eq!((m["mood"].as_str(), &m["error"]), (Some("error"), &json!({ "key": "error.rate_limit", "vars": {} })));
        let m = message("session.execution.failed", json!({ "error": { "type": "unknown", "message": "Something broke\nmore" } })).unwrap();
        assert_eq!(m["error"], "Something broke");
        let m = message("session.execution.interrupted", json!({ "reason": "user" })).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
        assert_eq!(message("session.deleted", json!({})).unwrap()["event"], "session-end");
        assert_eq!(message("form.created", json!({})).unwrap()["mood"], "waiting");
        assert_eq!(message("form.replied", json!({})).unwrap()["mood"], "working");
    }

    #[test]
    fn edits_name_their_files() {
        let m = message("session.tool.success", json!({ "tool": "edit", "input": { "path": "Z:\\p\\src\\a.rs", "oldString": "a", "newString": "b" } })).unwrap();
        assert_eq!((m["detail"].as_str(), m["file"].as_str()), (Some("Edit"), Some("Z:\\p\\src\\a.rs")));
        let patch = "*** Begin Patch\n*** Add File: hello.txt\n+hi\n*** Update File: b.txt\n@@\n-a\n+b\n*** End Patch";
        let m = message("session.tool.called", json!({ "tool": "patch", "input": { "patchText": patch } })).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "hello.txt +1" } }));
        let m = message("session.tool.success", json!({ "tool": "patch", "input": { "patchText": patch } })).unwrap();
        // Its first file by its whole path (agent-core keeps every one: zhoupengjie/agent-hub#2).
        assert_eq!((m["detail"].as_str(), m["file"].as_str()), (Some("apply_patch"), Some("Z:\\nowhere\\oc3\\hello.txt")));
        let m = message("session.tool.called", json!({ "tool": "read", "input": { "path": "/x/notes.md" } })).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.read", "vars": { "file": "notes.md" } }));
        // A tool of its own (an MCP server's): by its name.
        let m = message("session.tool.called", json!({ "tool": "github_create_issue", "input": {} })).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.tool", "vars": { "tool": "github_create_issue" } }));
    }

    #[test]
    fn a_permission_and_its_reply() {
        let asked = normalize(&event(
            "permission.asked",
            json!({ "id": "per_1", "action": "shell", "resources": ["echo hi"], "save": ["echo *"], "source": { "type": "tool", "id": "call_1" }, "tool": "shell", "input": { "command": "echo hi" } }),
        ));
        assert_eq!(
            (asked["hook_event_name"].as_str(), asked["tool_name"].as_str(), asked["permission_id"].as_str(), asked["tool_use_id"].as_str()),
            (Some("PermissionRequest"), Some("shell"), Some("per_1"), Some("call_1"))
        );
        assert_eq!(asked["tool_input"], json!({ "command": "echo hi", "description": "" }));
        let m = to_message(&asked).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approve", "vars": { "what": { "key": "step.command", "vars": { "what": "echo hi" } } } }));
        // One not of a call (a folder outside the project): by its action.
        let m = message("permission.asked", json!({ "id": "per_2", "action": "external_directory", "resources": ["C:\\elsewhere\\*"] })).unwrap();
        assert_eq!(m["detail"]["vars"]["what"], json!({ "key": "step.tool", "vars": { "tool": "external_directory" } }));
        let replied = normalize(&event("permission.replied", json!({ "requestID": "per_1", "reply": "once" })));
        assert_eq!((replied["hook_event_name"].as_str(), replied["permission_id"].as_str()), (Some("PermissionReplied"), Some("per_1")));
        assert_eq!(to_message(&replied).unwrap()["mood"], "working");
        // Turned down: no failure of the tool's, and the turn it ends is stopped.
        let m = message("session.tool.failed", json!({ "tool": "shell", "error": { "type": "aborted", "message": "The user declined this tool call" } })).unwrap();
        assert_eq!((m["event"].as_str(), m.get("react")), (Some("tool-done"), None));
        let m = message("session.tool.failed", json!({ "tool": "shell", "error": { "type": "permission.rejected", "message": "no" } })).unwrap();
        assert_eq!((m["event"].as_str(), m.get("react")), (Some("tool-done"), None));
        let m = message("session.execution.failed", json!({ "error": { "type": "permission.rejected", "message": "no" } })).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
    }

    #[test]
    fn opencode_events_that_change_nothing() {
        for kind in ["session.renamed", "session.step.started", "message.updated", "", "session.execution.interrupted"] {
            let reason = if kind == "session.execution.interrupted" { json!({ "reason": "superseded" }) } else { json!({}) };
            assert!(normalize(&event(kind, reason)).is_null(), "{kind}");
        }
        assert!(to_message(&Value::Null).is_none());
    }

    // Each event as the server takes it, applied at a time (ms): the turns it
    // began and the time it counted.
    #[test]
    fn an_opencode_round_counts_its_turn_and_its_time() {
        let mut pet = crate::state::Pet::default();
        let mut feed = |kind: &str, more: Value, at: u64| {
            agent_core::opencode::to_event(&normalize(&event(kind, more))).and_then(|ev| pet.apply(&ev, None, at, crate::state::Hold::Seen)).map_or((0, 0), |out| (out.turns, out.worked_ms))
        };
        assert_eq!(feed("session.created", json!({}), 0), (0, 0));
        assert_eq!(feed("session.execution.started", json!({ "prompt": "Run the shell command: echo hi" }), 1_000), (1, 0));
        assert_eq!(feed("session.tool.called", json!({ "id": "c1", "tool": "shell", "input": { "command": "echo hi" } }), 2_000), (0, 0));
        assert_eq!(feed("permission.asked", json!({ "id": "per_1", "action": "shell", "resources": ["echo hi"], "tool": "shell" }), 2_100), (0, 0));
        assert_eq!(feed("permission.replied", json!({ "requestID": "per_1", "reply": "once" }), 9_000), (0, 0));
        assert_eq!(feed("session.tool.success", json!({ "id": "c1", "tool": "shell", "input": { "command": "echo hi" } }), 9_300), (0, 0));
        assert_eq!(feed("session.execution.succeeded", json!({ "text": "Done." }), 10_000), (0, 9_000));
    }
}
