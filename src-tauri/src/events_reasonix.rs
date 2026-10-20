// Reasonix's hook events (zhoupengjie/Wakuwaku#60), handed to her port by
// this program with connection::REASONIX_FLAG (main.rs reasonix_hook):
// agent-core reads them (reasonix.rs to_event_at: in Claude Code's words, a
// subagent's steps as its session's, a failure told once, its approvals
// answered in Reasonix), events.rs tells them in her words. Here, what she
// makes of them.
#[cfg(test)]
use serde_json::{json, Value};

#[cfg(test)]
use agent_core::reasonix::SAME_ASK_MS;

#[cfg(test)]
fn to_message_at(e: &Value, now: u64) -> Option<Value> {
    agent_core::reasonix::to_event_at(e, now).map(|ev| crate::events::message_of(&ev))
}

#[cfg(test)]
fn to_message(e: &Value) -> Option<Value> {
    to_message_at(e, crate::now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    // As Reasonix's hook package writes them (its Payload), in a folder that
    // is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "event": name, "sessionId": "rx-7f3a", "cwd": "Z:\\nowhere\\rx" });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    #[test]
    fn reasonix_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({ "source": "startup" }))).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "agent": "reasonix", "session": "rx-7f3a", "project": "rx" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Create a file named note.txt containing hi", "turn": 1 }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["task"].as_str()), (Some("working"), Some("turn-start"), Some("Create a file named note.txt containing hi")));
        // Its tools by Claude Code's names, their arguments too: the step says the file.
        let write = json!({ "path": "note.txt", "content": "hi\n" });
        let m = to_message(&event("PreToolUse", json!({ "toolName": "write_file", "toolArgs": write }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["detail"].as_str()), (Some("working"), Some("Write")));
        assert_eq!(m["step"], json!({ "key": "step.write", "vars": { "file": "note.txt" } }));
        // Arguments as a string of JSON read the same.
        let m = to_message(&event("PreToolUse", json!({ "toolName": "edit_file", "toolArgs": "{\"path\":\"src\\\\a.rs\"}" }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.edit", "vars": { "file": "a.rs" } }));
        let m = to_message(&event("PreToolUse", json!({ "toolName": "bash", "toolArgs": { "command": "cd x && cargo test" } }))).unwrap();
        assert_eq!(m["step"], json!({ "key": "step.command", "vars": { "what": "cargo test" } }));
        // The file it changed, made whole from the session's folder; one whole already as it is.
        let m = to_message(&event("PostToolUse", json!({ "toolName": "write_file", "toolArgs": write, "toolResult": "wrote 3 bytes" }))).unwrap();
        let whole = std::path::Path::new("Z:\\nowhere\\rx").join("note.txt").to_string_lossy().into_owned();
        assert_eq!((m["event"].as_str(), m["file"].as_str()), (Some("tool-done"), Some(whole.as_str())));
        if cfg!(windows) {
            let m = to_message(&event("PostToolUse", json!({ "toolName": "edit_file", "toolArgs": { "path": "D:\\w\\b.rs" } }))).unwrap();
            assert_eq!(m["file"], "D:\\w\\b.rs");
        }
        let m = to_message(&event("PostToolUseFailure", json!({ "toolName": "bash", "toolArgs": { "command": "false" }, "error": "exit status 1" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str()), (Some("tool-failed"), Some("failed")));
        // A subagent's starter, by any of its names.
        for tool in ["task", "explore", "parallel_tasks"] {
            let m = to_message(&event("PreToolUse", json!({ "toolName": tool, "toolArgs": { "description": "Explore the code" } }))).unwrap();
            assert_eq!(m["step"], json!({ "key": "step.agent", "vars": { "what": "Explore the code" } }), "{tool}");
        }
        // Its question to the person, and its to-do list.
        assert_eq!(to_message(&event("PreToolUse", json!({ "toolName": "ask", "toolArgs": { "questions": [] } }))).unwrap()["mood"], "waiting");
        let todo = to_message(&event("PreToolUse", json!({ "toolName": "todo_write", "toolArgs": { "todos": [{ "content": "Run tests", "activeForm": "Running tests", "status": "in_progress" }] } }))).unwrap();
        assert_eq!(todo["todo"], json!({ "set": [{ "text": "Run tests", "active": "Running tests", "status": "in_progress" }] }));
        // A tool of its own, and an MCP one.
        assert_eq!(to_message(&event("PreToolUse", json!({ "toolName": "code_index" }))).unwrap()["step"], json!({ "key": "step.tool", "vars": { "tool": "code_index" } }));
        assert_eq!(to_message(&event("PreToolUse", json!({ "toolName": "mcp__github__create_issue" }))).unwrap()["step"]["key"], "step.mcp");
        // A prompt for its approval: waiting, answered in Reasonix.
        let m = to_message(&event("PermissionRequest", json!({ "toolName": "bash", "toolArgs": { "command": "git push" } }))).unwrap();
        assert_eq!(m["mood"], "waiting");
        assert_eq!(m["detail"], json!({ "key": "detail.approveIn", "vars": { "app": "Reasonix", "what": { "key": "step.command", "vars": { "what": "git push" } } } }));
        // Its end, with its last words; a failure, with why.
        let m = to_message(&event("Stop", json!({ "lastAssistantText": "## Done\nCreated **note.txt** with `hi`.", "turn": 1 }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str(), m["agent"].as_str()), (Some("done"), Some("Done: Created note.txt with hi."), Some("reasonix")));
        let m = to_message(&event("StopFailure", json!({ "error": "deepseek: 402 Insufficient Balance" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["error"].as_str()), (Some("error"), Some("deepseek: 402 Insufficient Balance")));
        let m = to_message(&event("SessionEnd", json!({ "reason": "exit" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("session-end")));
    }

    #[test]
    fn the_person_s_words_past_reasonix_s_own_blocks() {
        // As Reasonix 2.44 composes it (trimmed): its blocks, then the person's words.
        // (agent-core's reasonix.rs has the rest of how they are found.)
        let prompt = "<current-date>\nToday's date is 2026-10-18 (Sunday); the local time zone is UTC+02:00.\n</current-date>\n\n<available-skills>\n# Skills\n- explore [🧬 subagent] — Explore…\n</available-skills>\n\nList the files in this folder and reply in one short sentence.";
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": prompt, "turn": 1 }))).unwrap();
        assert_eq!(m["task"], "List the files in this folder and reply in one short sentence.");
    }

    #[test]
    fn a_failure_is_told_once_and_a_stop_by_the_person_is_no_failure() {
        // A failed turn: StopFailure, then its Stop hooks again with the error.
        let m = to_message(&event("StopFailure", json!({ "error": "context deadline exceeded", "turn": 2 }))).unwrap();
        assert_eq!(m["mood"], "error");
        assert!(to_message(&event("Stop", json!({ "error": "context deadline exceeded", "turn": 2 }))).is_none());
        // A failed tool: PostToolUseFailure, then PostToolUse again with the error.
        let failed = json!({ "toolName": "edit_file", "toolArgs": { "path": "a.rs" }, "error": "old_string not found" });
        assert_eq!(to_message(&event("PostToolUseFailure", failed.clone())).unwrap()["event"], "tool-failed");
        assert!(to_message(&event("PostToolUse", failed)).is_none());
        // The person stopping the turn: at rest, no error; its Stop again changes nothing.
        let stopped = json!({ "error": "context canceled", "isInterrupt": true });
        let m = to_message(&event("StopFailure", stopped.clone())).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m.get("error")), (Some("idle"), Some("turn-end"), None));
        assert!(to_message(&event("Stop", stopped)).is_none());
    }

    #[test]
    fn subagents_work_for_their_session() {
        // A subagent's step: the session's, within a turn of it.
        let m = to_message(&event("PreToolUse", json!({ "sessionId": "rx-7f3a:subagent:call_7", "toolName": "grep", "toolArgs": { "pattern": "TODO" } }))).unwrap();
        assert_eq!((m["session"].as_str(), m["within"].as_bool(), m["mood"].as_str()), (Some("rx-7f3a"), Some(true), Some("working")));
        assert_eq!(m["step"], json!({ "key": "step.search", "vars": { "what": "TODO" } }));
        // Its edits count as the session's; a failure of its tool is said by no reaction of its own.
        let m = to_message(&event("PostToolUse", json!({ "sessionId": "rx-7f3a:subagent:call_7", "toolName": "write_file", "toolArgs": { "path": "C:\\w\\n.md" } }))).unwrap();
        assert_eq!((m["file"].as_str(), m["within"].as_bool()), (Some("C:\\w\\n.md"), Some(true)));
        let m = to_message(&event("PostToolUseFailure", json!({ "sessionId": "rx-7f3a:guardian", "toolName": "bash", "error": "exit 1" }))).unwrap();
        assert!(m.get("react").is_none() && m.get("say").is_none());
        // Never a turn, an ending or a session of its own.
        for (name, more) in [("UserPromptSubmit", json!({ "prompt": "Explore the code for TODOs" })), ("Stop", json!({ "lastAssistantText": "Found 3." })), ("SessionStart", json!({})), ("SessionEnd", json!({}))] {
            let mut more = more;
            more["sessionId"] = json!("rx-7f3a:subagent:call_7");
            assert!(to_message(&event(name, more)).is_none(), "{name}");
        }
    }

    #[test]
    fn an_approval_is_told_once_with_what_waits() {
        let ask = |name: &str, more: Value, now: u64| to_message_at(&event(name, more), now);
        let session = json!({ "sessionId": "rx-ask-1" });
        let mut pr = session.clone();
        pr["toolName"] = json!("bash");
        pr["toolArgs"] = json!({ "command": "git push" });
        pr["subject"] = json!("git push");
        assert_eq!(ask("PermissionRequest", pr, 1_000).unwrap()["detail"]["key"], "detail.approveIn");
        // Its Notification as the dialog shows: said already.
        let mut note = session.clone();
        note["notificationType"] = json!("permission_prompt");
        note["message"] = json!("Approval needed: bash (git push)");
        assert!(ask("Notification", note.clone(), 1_050).is_none());
        // One with no PermissionRequest before it (a plan's, Auto Guard's): in its words.
        let m = ask("Notification", note.clone(), 1_000 + SAME_ASK_MS + 1).unwrap();
        assert_eq!(m["detail"], json!({ "key": "detail.approveIn", "vars": { "app": "Reasonix", "what": "Approval needed: bash (git push)" } }));
        let mut other = note.clone();
        other["sessionId"] = json!("rx-ask-2");
        other["message"] = json!("Auto Guard: confirm the next action");
        assert_eq!(ask("Notification", other, 1_100).unwrap()["mood"], "waiting");
        // A question: as the ask tool's start said it.
        let mut question = session.clone();
        question["notificationType"] = json!("question_prompt");
        assert_eq!(ask("Notification", question, 2_000).unwrap()["detail"], json!({ "key": "detail.asksQuestion" }));
        // Anything else: in its words, or that it needs the person.
        let mut else_ = session;
        else_["notificationType"] = json!("idle");
        assert_eq!(ask("Notification", else_.clone(), 3_000).unwrap()["detail"], json!({ "key": "detail.needsYou" }));
        else_["message"] = json!("Waiting for your input");
        assert_eq!(ask("Notification", else_, 3_000).unwrap()["detail"], "Waiting for your input");
    }

    #[test]
    fn reasonix_events_that_change_nothing() {
        // The person stopping a tool is no failure of it.
        assert!(to_message(&event("PostToolUseFailure", json!({ "toolName": "bash", "isInterrupt": true }))).is_none());
        for name in ["SubagentStart", "SubagentStop", "PostLLMCall", "PreCompact", "Nope"] {
            assert!(to_message(&event(name, json!({ "callId": "c1" }))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }
}
