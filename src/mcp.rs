//! 0.0.19 H-line: the MCP adapter (W2 spike, test-list first).
//!
//! A **translation layer beside the core**, never a plugin host inside it:
//! newline-delimited JSON-RPC 2.0 over stdio, one resident core instance,
//! the `cu-abi` wire names verbatim. Every bound, grant, receipt and refusal
//! of the core passes through unchanged (equivalence invariant E1-E7,
//! `plan/v0.0.19.md`); the adapter adds no buffer it does not pass through
//! and no policy of its own.
//!
//! Grants are the whole policy and come from the mounting client's argv:
//! `--root` (required), `--allow-cmd NAME` (repeatable), `--allow-screen`.
//! Missing grants refuse identically to the core (E4).

use crate::cu::CuHost;
use crate::screen::ScreenTool;
use crate::tools::{ExecTool, FileTool};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// The MCP protocol revision this adapter was verified against (W0 pass 11).
/// Unknown client versions are answered with this one; the client decides.
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

/// One in-flight request at a time; a frame larger than this is a protocol
/// error, never a buffer (W0 measured the worst legal observe at 11.5 KiB).
pub const MCP_MAX_FRAME_BYTES: usize = 1_048_576;

/// The adapter's own error vocabulary: protocol errors only. Application
/// refusals travel as `result.isError` (E1).
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;

pub struct Adapter {
    root: String,
    allow_cmd: Vec<String>,
    allow_screen: bool,
}

impl Adapter {
    pub fn new(root: String, allow_cmd: Vec<String>, allow_screen: bool) -> Self {
        Self { root, allow_cmd, allow_screen }
    }

    /// The whole adapter: read a line, route, write a line. Notifications
    /// (no `id`) get no response. EOF ends the loop; the core's Drop runs.
    pub fn serve(&self) {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => return,
            };
            if line.len() > MCP_MAX_FRAME_BYTES {
                let _ = writeln!(stdout, "{}", error_body(Value::Null, PARSE_ERROR, "frame too large"));
                let _ = stdout.flush();
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            let frame: Value = match serde_json::from_str(&line) {
                Ok(frame) => frame,
                Err(_) => {
                    let _ = writeln!(stdout, "{}", error_body(Value::Null, PARSE_ERROR, "parse error"));
                    let _ = stdout.flush();
                    continue;
                }
            };
            if let Some(response) = self.route(&frame) {
                let _ = writeln!(stdout, "{}", response);
                let _ = stdout.flush();
            }
        }
    }

    /// Route one JSON-RPC frame. `None` means notification (no response).
    fn route(&self, frame: &Value) -> Option<String> {
        let id = frame.get("id").cloned();
        let method = frame.get("method").and_then(Value::as_str);
        // E7: unknown extra fields are ignored, never errors.
        let method = match method {
            Some(method) => method,
            None => {
                return id.map(|id| {
                    error_body(id, INVALID_REQUEST, "missing method")
                });
            }
        };
        let id = match id {
            Some(id) => id,
            // A notification: initialized, cancelled, logs — nothing to do.
            None => return None,
        };
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "minicon-harness-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions":
                    "Bounded local computer-use / execution backend. Tools: \
                     file, exec, screen — one bounded operation per call.",
            })),
            // E7: ping is answered at any time, before or after initialized.
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": crate::wire::tool_definitions(),
            })),
            "tools/call" => self.call_tool(frame.get("params")),
            other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
        };
        Some(match result {
            Ok(result) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result,
            })
            .to_string(),
            Err((code, message)) => error_body(id, code, &message),
        })
    }

    /// One tools/call -> one bounded core invocation. Application refusals
    /// are results with isError=true and the core's reason text verbatim.
    fn call_tool(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let params = params.ok_or_else(|| {
            (INVALID_REQUEST, "tools/call needs params".to_owned())
        })?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (INVALID_REQUEST, "tools/call needs a name".to_owned()))?;
        // E4: the adapter adds no grant of its own. Tools are constructed
        // per call with exactly the mounting client's flags.
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let (is_error, text) = match name {
            "file" => self.file_call(&arguments),
            "exec" => self.exec_call(&arguments),
            "screen" => self.screen_call(&arguments),
            // The model invented a tool: named refusal, not a silent drop.
            other => (
                true,
                format!(
                    "mcp: refused: unknown tool {other:?}; this server exposes exactly \
                     file, exec and screen"
                ),
            ),
        };
        Ok(json!({
            "content": [{ "type": "text", "text": text }],
            "isError": is_error,
        }))
    }

    fn file_call(&self, arguments: &Value) -> (bool, String) {
        let file_tool = match FileTool::new(&self.root) {
            Ok(tool) => tool,
            Err(reason) => return (true, reason),
        };
        let path = match arguments.get("path").and_then(Value::as_str) {
            Some(path) => path,
            None => return (true, "file: refused: no `path` argument".to_owned()),
        };
        match arguments.get("op").and_then(Value::as_str) {
            Some("read") => match file_tool.read(path) {
                Ok(contents) => (false, contents),
                Err(reason) => (true, reason),
            },
            Some("write") => {
                let contents = match arguments.get("contents").and_then(Value::as_str) {
                    Some(contents) => contents,
                    None => {
                        return (true, "file: refused: op=write needs a `contents` argument".to_owned())
                    }
                };
                match file_tool.write(path, contents) {
                    Ok(written) => (false, format!("wrote {written} bytes to {path}")),
                    Err(reason) => (true, reason),
                }
            }
            Some("edit") => {
                let (old, new) = match (
                    arguments.get("old").and_then(Value::as_str),
                    arguments.get("new").and_then(Value::as_str),
                ) {
                    (Some(old), Some(new)) => (old, new),
                    _ => {
                        return (true, "file: refused: op=edit needs `old` and `new`".to_owned())
                    }
                };
                match file_tool.edit(path, old, new) {
                    Ok(count) => (false, format!("replaced {count} bytes in {path}")),
                    Err(reason) => (true, reason),
                }
            }
            Some(other) => (
                true,
                format!("file: refused: unknown op {other:?}; use read, write or edit"),
            ),
            None => (true, "file: refused: no `op` argument".to_owned()),
        }
    }

    fn exec_call(&self, arguments: &Value) -> (bool, String) {
        let exec_tool = ExecTool::new(
            std::path::Path::new(&self.root),
            &self.allow_cmd,
        );
        // The wire schema's own spelling: one `argv` vector, program first
        // (E1 — the adapter must not invent a second parameter shape).
        let argv: Vec<String> = match arguments.get("argv").and_then(Value::as_array) {
            Some(rows) => rows
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            None => return (true, "exec: refused: no `argv` argument".to_owned()),
        };
        let (program, args) = match argv.split_first() {
            Some((program, args)) => (program.as_str(), args),
            None => return (true, "exec: refused: `argv` is empty".to_owned()),
        };
        match exec_tool.run(program, args) {
            Ok(outcome) => {
                let status = if outcome.timed_out { "timed out" } else { "finished" };
                let ok = outcome.exit_code == Some(0) && !outcome.timed_out;
                (
                    !ok,
                    format!(
                        "{status}; exit={:?}; stdout:\n{}\nstderr:\n{}",
                        outcome.exit_code, outcome.stdout, outcome.stderr
                    ),
                )
            }
            Err(reason) => (true, reason),
        }
    }

    fn screen_call(&self, arguments: &Value) -> (bool, String) {
        // E2: a non-granted or non-Windows cell refuses identically through
        // the adapter; the cell matrix never widens or narrows here.
        let host = CuHost::new(ScreenTool::new(std::path::Path::new(&self.root), self.allow_screen));
        let result = host.invoke_json(arguments);
        let is_error = !matches!(result.verdict, crate::cu::CuVerdict::Performed);
        (is_error, result.text)
    }
}

fn error_body(id: Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(allow_screen: bool) -> Adapter {
        let root = std::env::temp_dir().join("mh-mcp-equiv-test");
        std::fs::create_dir_all(&root).expect("the adapter root must exist");
        Adapter::new(root.to_string_lossy().into_owned(), Vec::new(), allow_screen)
    }

    fn route(adapter: &Adapter, frame: Value) -> Value {
        serde_json::from_str(&adapter.route(&frame).expect("a request must answer"))
            .expect("a response is one JSON object")
    }

    fn call(adapter: &Adapter, name: &str, arguments: Value) -> (bool, String) {
        let response = route(
            adapter,
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": { "name": name, "arguments": arguments },
            }),
        );
        let result = &response["result"];
        (
            result["isError"].as_bool().unwrap_or(true),
            result["content"][0]["text"].as_str().unwrap_or_default().to_owned(),
        )
    }

    #[test]
    fn e1_core_refusals_keep_their_reason_text_verbatim() {
        let a = adapter(false);
        let (is_error, text) = call(&a, "file", json!({ "op": "read", "path": "../escape.txt" }));
        assert!(is_error);
        assert!(text.contains("refused"), "{text}");
        assert!(text.contains(".."), "the core's own refusal text must pass through: {text}");
        let (is_error, text) = call(
            &a,
            "screen",
            json!({ "op": "observe", "wait_ms": 2001 }),
        );
        assert!(is_error);
        assert!(text.contains("1..=2000"), "the D4 ceiling refusal must be verbatim: {text}");
    }

    #[test]
    fn e2_no_grant_means_the_same_named_refusal_not_a_hidden_tool() {
        let a = adapter(false);
        let (is_error, text) = call(&a, "screen", json!({ "op": "observe" }));
        assert!(is_error);
        assert!(
            text.contains("--allow-screen"),
            "the grant refusal must name the missing flag: {text}"
        );
    }

    #[test]
    fn e3_initialize_ping_and_static_tools_list_follow_the_spec() {
        let a = adapter(true);
        let init = route(
            &a,
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": { "elicitation": {}, "tasks": {} },
                    "clientInfo": { "name": "equiv", "version": "0" },
                }
            }),
        );
        assert_eq!(init["result"]["protocolVersion"], json!(MCP_PROTOCOL_VERSION));
        assert_eq!(init["result"]["capabilities"]["tools"], json!({}));
        let ping = route(&a, json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }));
        assert_eq!(ping["result"], json!({}));
        let list = route(&a, json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }));
        let tools = list["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["file", "exec", "screen"]);
    }

    #[test]
    fn e4_the_adapter_grants_nothing_of_its_own() {
        let a = adapter(false);
        let (is_error, text) =
            call(&a, "exec", json!({ "why": "x", "argv": ["git", "status"] }));
        assert!(is_error);
        assert!(text.contains("allow-list") || text.contains("--allow-cmd"), "{text}");
        let (is_error, text) =
            call(&a, "file", json!({ "op": "read", "path": "does-not-exist.txt" }));
        assert!(is_error);
        assert!(text.contains("does-not-exist"), "{text}");
    }

    #[test]
    fn e5_and_e6_invented_tools_and_methods_are_named_never_silent() {
        let a = adapter(true);
        let (is_error, text) = call(&a, "invented", json!({}));
        assert!(is_error);
        assert!(text.contains("exactly"), "the refusal must name the tool set: {text}");
        let response = route(&a, json!({ "jsonrpc": "2.0", "id": 9, "method": "bogus" }));
        assert_eq!(response["error"]["code"], json!(-32601));
        let parsed: Value =
            serde_json::from_str(&error_body(Value::Null, PARSE_ERROR, "parse error")).unwrap();
        assert_eq!(parsed["error"]["code"], json!(PARSE_ERROR));
    }

    #[test]
    fn notifications_produce_no_response() {
        let a = adapter(true);
        assert!(a
            .route(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .is_none());
        assert!(a
            .route(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {} }))
            .is_none());
    }
}
