// oh-my-pi's events, from her extension inside omp (integrations/omp/
// wakuwaku.ts, written by connection.rs): agent-core reads them (omp.rs
// to_event), events.rs tells them in her words (an omp call's step,
// step_of_omp: ours for the tools Claude Code has, else the model's intent).
// Here, what she makes of them.
#[cfg(test)]
use serde_json::{json, Value};

#[cfg(test)]
fn to_message(e: &Value) -> Option<Value> {
    agent_core::omp::to_event(e).map(|ev| crate::events::message_of(&ev))
}
#[cfg(test)]
mod tests {
    use super::*;

    // As the extension sends them from omp 18.11.1 (trimmed); the folder is no git repository.
    fn event(name: &str, more: Value) -> Value {
        let mut e = json!({ "session_id": "01a14146", "cwd": "Z:\\nowhere\\proj", "hook_event_name": name });
        for (k, v) in more.as_object().unwrap() {
            e[k] = v.clone();
        }
        e
    }

    fn tool(name: &str, args: Value, intent: &str) -> Value {
        to_message(&event("PreToolUse", json!({ "tool_name": name, "tool_input": args, "tool_use_id": "call_1", "intent": intent }))).unwrap()
    }

    #[test]
    fn omp_events_become_messages() {
        let m = to_message(&event("SessionStart", json!({ "source": "startup" }))).unwrap();
        assert_eq!(m, json!({ "mood": "idle", "react": "wave", "say": { "key": "say.hello" }, "session": "01a14146", "project": "proj", "agent": "omp" }));
        let m = to_message(&event("UserPromptSubmit", json!({ "prompt": "Run the shell command echo hi" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str(), m["task"].as_str()), (Some("working"), Some("turn-start"), Some("Run the shell command echo hi")));
        let m = tool("bash", json!({ "command": "echo hi", "timeout": 30, "cwd": "C:/waku" }), "Running echo hi");
        assert_eq!(
            m,
            json!({ "mood": "working", "detail": "Bash", "event": "tool-start", "step": { "key": "step.run", "vars": { "what": "Running echo hi" } },
                    "toolId": "call_1", "session": "01a14146", "project": "proj", "agent": "omp" })
        );
        let m = to_message(&event("Stop", json!({ "last_assistant_message": "Done.\nMore." }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["reply"].as_str(), m["agent"].as_str()), (Some("done"), Some("Done. More."), Some("omp")));
        let m = to_message(&event("StopFailure", json!({ "error": "", "error_details": "Connection error.\nretry later" }))).unwrap();
        assert_eq!((m["mood"].as_str(), m["error"].as_str()), (Some("error"), Some("Connection error.")));
        let m = to_message(&event("Interrupt", json!({}))).unwrap();
        assert_eq!((m["mood"].as_str(), m["event"].as_str()), (Some("idle"), Some("turn-end")));
        assert_eq!(to_message(&event("SessionEnd", json!({ "reason": "exit" }))).unwrap()["event"], "session-end");
        let m = to_message(&event("Stop", json!({ "session_title": "Echo a greeting" }))).unwrap();
        assert_eq!(m["title"], "Echo a greeting");
    }

    #[test]
    fn omp_tools_say_what_they_do() {
        assert_eq!(tool("read", json!({ "path": "src/a.txt" }), "Reading a.txt")["step"], json!({ "key": "step.read", "vars": { "file": "a.txt" } }));
        assert_eq!(tool("write", json!({ "path": "a.txt", "content": "hi\n" }), "")["step"], json!({ "key": "step.write", "vars": { "file": "a.txt" } }));
        let m = tool("edit", json!({ "path": "a.txt", "old_string": "hi", "new_string": "bye" }), "Changing hi to bye");
        assert_eq!((m["detail"].as_str(), &m["step"]), (Some("Edit"), &json!({ "key": "step.edit", "vars": { "file": "a.txt" } })));
        assert_eq!(tool("grep", json!({ "pattern": "bye", "path": "." }), "")["step"], json!({ "key": "step.search", "vars": { "what": "bye" } }));
        assert_eq!(tool("glob", json!({ "path": "*.txt" }), "Listing *.txt files")["step"], json!({ "key": "step.find", "vars": { "what": "*.txt" } }));
        assert_eq!(tool("bash", json!({ "command": "cd x && cargo test" }), "")["step"], json!({ "key": "step.command", "vars": { "what": "cargo test" } }));
        let task = json!({ "context": "…", "tasks": [{ "name": "SayHi", "agent": "sonic", "task": "Reply hi" }] });
        assert_eq!(tool("task", task.clone(), "Starting one subagent to reply hi")["step"], json!({ "key": "step.agent", "vars": { "what": "Starting one subagent to reply hi" } }));
        assert_eq!(tool("task", task, "")["step"]["vars"]["what"], "SayHi");
        assert_eq!(tool("web_search", json!({ "query": "omp extensions" }), "")["step"], json!({ "key": "step.webSearch", "vars": { "what": "omp extensions" } }));
        assert_eq!(tool("todo", json!({ "ops": [] }), "")["step"]["key"], "step.plan");
        // Its own tools: the model's words for them, else the name.
        assert_eq!(tool("lsp", json!({ "action": "hover" }), "Checking the type")["step"], json!({ "key": "step.run", "vars": { "what": "Checking the type" } }));
        assert_eq!(tool("lsp", json!({}), "")["step"], json!({ "key": "step.tool", "vars": { "tool": "lsp" } }));
        assert_eq!(tool("mcp__github__get_issue", json!({}), "Reading the issue")["step"], json!({ "key": "step.mcp", "vars": { "server": "github", "tool": "get_issue" } }));
        // One of ours with nothing to name: its intent.
        assert_eq!(tool("edit", json!({ "ops": [] }), "Fixing the import")["step"], json!({ "key": "step.run", "vars": { "what": "Fixing the import" } }));
        // A question waits on the person.
        assert_eq!(tool("ask", json!({ "questions": [] }), "")["mood"], "waiting");
    }

    #[test]
    fn an_edit_brings_its_file_and_a_failure_says_so() {
        let done = json!({ "tool_name": "edit", "tool_input": { "path": "a.txt" }, "tool_use_id": "c2", "file": "C:\\waku\\omp\\proj\\a.txt" });
        let m = to_message(&event("PostToolUse", done)).unwrap();
        assert_eq!((m["detail"].as_str(), m["event"].as_str(), m["file"].as_str(), m["toolId"].as_str()), (Some("Edit"), Some("tool-done"), Some("C:\\waku\\omp\\proj\\a.txt"), Some("c2")));
        // A read's path is no edit.
        let m = to_message(&event("PostToolUse", json!({ "tool_name": "read", "tool_input": { "path": "a.txt" }, "file": "C:\\a.txt" }))).unwrap();
        assert!(m.get("file").is_none());
        let m = to_message(&event("PostToolUseFailure", json!({ "tool_name": "bash", "tool_input": { "command": "false" }, "error": "exit 1" }))).unwrap();
        assert_eq!((m["event"].as_str(), m["react"].as_str(), &m["say"]["vars"]["tool"]), (Some("tool-failed"), Some("failed"), &json!("Bash")));
        // The edit made the turn's done a review.
        let mut pet = crate::state::Pet::default();
        for (e, at) in [(event("UserPromptSubmit", json!({ "prompt": "fix it" })), 1), (event("PostToolUse", json!({ "tool_name": "write", "tool_input": { "path": "b.txt" } })), 2), (event("Stop", json!({})), 3)] {
            pet.apply(&agent_core::omp::to_event(&e).unwrap(), None, at, crate::state::Hold::Seen);
        }
        assert_eq!((pet.get(4)["mood"].as_str(), pet.get(4)["agent"].as_str()), (Some("review"), Some("omp")));
    }

    #[test]
    fn approvals_wait_in_omp_and_plans_on_her() {
        let m = to_message(&event("ApprovalRequested", json!({ "tool_name": "bash", "tool_input": { "command": "rm -rf build" }, "intent": "" }))).unwrap();
        assert_eq!(m["mood"], "waiting");
        assert_eq!(m["detail"], json!({ "key": "detail.approveInTerminal", "vars": { "what": { "key": "step.command", "vars": { "what": "rm -rf build" } } } }));
        assert_eq!(to_message(&event("ApprovalResolved", json!({ "approved": true }))).unwrap()["mood"], "working");
        // A plan review: waiting on her, as Claude Code's ExitPlanMode.
        let plan = json!({ "tool_name": "ExitPlanMode", "tool_input": { "plan": "1. Write a.txt" }, "tool_use_id": "r1" });
        let m = to_message(&event("PermissionRequest", plan.clone())).unwrap();
        assert_eq!((m["mood"].as_str(), &m["detail"]), (Some("waiting"), &json!({ "key": "detail.planToConfirm" })));
        let mut answered = plan.clone();
        answered["choice"] = json!("execute");
        assert_eq!(to_message(&event("PostToolUse", answered.clone())).unwrap()["mood"], "working");
        answered["choice"] = json!("save");
        assert_eq!(to_message(&event("PostToolUse", answered.clone())).unwrap()["mood"], "idle");
        answered["choice"] = json!("");
        assert_eq!(to_message(&event("PostToolUse", answered)).unwrap()["mood"], "idle");
    }

    #[test]
    fn omp_events_that_change_nothing() {
        for name in ["Nope", ""] {
            assert!(to_message(&event(name, json!({}))).is_none(), "{name}");
        }
        assert!(to_message(&json!("nope")).is_none());
    }
}
