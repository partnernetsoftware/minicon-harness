//! # minicon-harness
//!
//! A minimal bounded agent loop with exactly two tools, `file` and `exec`.
//! Not a plugin host, not a permission-policy framework -- the two tools' own
//! bounds (the `--root` path, the `--allow-cmd` list) are the whole policy.
//!
//! In the spirit of pi-agent: the smallest complete thing that can run a
//! model against a filesystem and a command line, with every bound stated
//! as a constant next to the code that enforces it.

pub mod tools;
pub mod wire;

pub use tools::{ExecOutcome, ExecTool, FileTool};
pub use wire::{ModelReply, Transport, WireToolCall};

/// Most model round trips one invocation may take.
pub const INVOCATION_MAX_MODEL_TURNS: usize = 16;

/// Most tool calls one invocation may dispatch, across all turns.
pub const INVOCATION_MAX_TOOL_CALLS: usize = 64;

/// How long one whole invocation may run.
pub const INVOCATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// A fatal invocation boundary, separate from a recoverable tool refusal.
///
/// A tool refusal (path outside the root, unlisted command) is an ordinary
/// tool result the model reads and corrects; a `Stop` is the run ending.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stop {
    ModelTurns,
    ToolCalls,
    Deadline,
    Cancelled,
}

impl std::fmt::Display for Stop {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::ModelTurns => "invocation model-turn budget exhausted (16)",
            Self::ToolCalls => "invocation tool-call budget exhausted (64)",
            Self::Deadline => "invocation deadline exhausted (10 minutes)",
            Self::Cancelled => "invocation cancelled",
        };
        write!(formatter, "harness: {reason}; the task did not finish")
    }
}

/// One owner spans the whole invocation. Session resumption starts a fresh
/// invocation; counters never reset inside a backend call.
pub struct InvocationBudget {
    model_turns: std::cell::Cell<usize>,
    tool_calls: std::cell::Cell<usize>,
    pub deadline: std::time::Instant,
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl InvocationBudget {
    pub fn new(cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        Self {
            model_turns: std::cell::Cell::new(0),
            tool_calls: std::cell::Cell::new(0),
            deadline: std::time::Instant::now() + INVOCATION_TIMEOUT,
            cancelled,
        }
    }

    pub fn check(&self) -> Result<std::time::Duration, Stop> {
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Stop::Cancelled);
        }
        self.deadline
            .checked_duration_since(std::time::Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(Stop::Deadline)
    }

    pub fn take_model_turn(&self) -> Result<std::time::Duration, Stop> {
        let remaining = self.check()?;
        if self.model_turns.get() >= INVOCATION_MAX_MODEL_TURNS {
            return Err(Stop::ModelTurns);
        }
        self.model_turns.set(self.model_turns.get() + 1);
        Ok(remaining)
    }

    pub fn take_tool_call(&self) -> Result<(), Stop> {
        self.check()?;
        if self.tool_calls.get() >= INVOCATION_MAX_TOOL_CALLS {
            return Err(Stop::ToolCalls);
        }
        self.tool_calls.set(self.tool_calls.get() + 1);
        Ok(())
    }
}

/// Why one invocation ended. Either a fatal budget `Stop`, or a transport /
/// codec error string.
#[derive(Debug)]
pub enum Failure {
    Stopped(Stop),
    Backend(String),
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self::Backend(error)
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped(stop) => stop.fmt(formatter),
            Self::Backend(error) => formatter.write_str(error),
        }
    }
}

/// What it takes to run one task end to end.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub root: String,
    pub task: String,
    pub base_url: String,
    pub model: String,
    /// Permitted executable basenames for the `exec` tool. Empty does **not**
    /// disable the tool -- it still gets advertised, and every call is refused
    /// naming the missing allow-list, so a model cannot tell "not allowed yet"
    /// from "tool absent" by probing.
    pub allow_cmd: Vec<String>,
    /// The environment variable read for the API bearer. Defaults to
    /// `HARNESS_API_KEY`.
    pub key_env: String,
}

/// Parses the CLI form: `--root PATH --task TEXT [--base-url URL]
/// [--model NAME] [--allow-cmd NAME]... [--key-env NAME]`.
pub fn parse_args(args: &[String]) -> Result<Request, String> {
    let usage = "usage: minicon-harness --root PATH --task TEXT [--base-url URL] \
                 [--model NAME] [--allow-cmd NAME]... [--key-env NAME]"
        .to_owned();
    let mut root = None;
    let mut task = None;
    let mut base_url = OPENAI_COMPATIBLE_CHAT_URL.to_owned();
    let mut model = DEFAULT_MODEL.to_owned();
    let mut allow_cmd = Vec::new();
    let mut key_env = "HARNESS_API_KEY".to_owned();
    let mut position = 0usize;
    while let Some(flag) = args.get(position).map(String::as_str) {
        position += 1;
        let mut value = || -> Result<String, String> {
            let value = args
                .get(position)
                .ok_or_else(|| format!("{flag} requires a value"))?;
            position += 1;
            Ok(value.clone())
        };
        match flag {
            "--root" => root = Some(value()?),
            "--task" => task = Some(value()?),
            "--base-url" => base_url = value()?,
            "--model" => model = value()?,
            "--allow-cmd" => allow_cmd.push(value()?),
            "--key-env" => key_env = value()?,
            other => {
                return Err(format!("unexpected argument {other:?}\n{usage}"));
            }
        }
    }
    Ok(Request {
        root: root.ok_or_else(|| {
            format!(
                "--root PATH is required (there is deliberately no implicit current-directory \
                 default, which would widen the file tool's bound silently)\n{usage}"
            )
        })?,
        task: task.ok_or_else(|| format!("--task TEXT is required\n{usage}"))?,
        base_url,
        model,
        allow_cmd,
        key_env,
    })
}

/// Default OpenAI-compatible chat-completions endpoint. Overridable with
/// `--base-url`; the wire codec speaks one format, so any server that accepts
/// the documented request shape works.
pub const OPENAI_COMPATIBLE_CHAT_URL: &str = "https://api.deepseek.com/chat/completions";
pub const DEFAULT_MODEL: &str = "deepseek-chat";

/// The instruction the task is wrapped in, so the model knows what the two
/// tools' bounds are before it trips over them.
pub fn system_prompt(root: &std::path::Path) -> String {
    format!(
        "You are a bounded harness agent. You have exactly two tools, `file` and `exec`, and \
         no others. Every path is relative to the task root {root}; paths outside it are \
         refused, not rewritten. `exec` runs one allow-listed command per call with no shell. \
         A refusal is information: read it, adjust, and continue. Only after all requested \
         work has succeeded, answer with final plain text and no tool call.",
        root = root.display(),
    )
}

/// Runs one bounded task to a final assistant message.
///
/// The loop stops for exactly three reasons: a final text reply (success), a
/// transport or codec error, or one of the invocation bounds. A **tool refusal
/// is none of them** -- a path outside the root or an unlisted command comes
/// back to the model as an ordinary tool result so it can correct itself, and
/// it neither aborts the run nor buys another turn.
pub fn run(transport: &dyn Transport, request: &Request, key: &str) -> Result<String, Failure> {
    let budget = InvocationBudget::new(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        false,
    )));
    run_with_budget(transport, request, key, &budget)
}

pub fn run_with_budget(
    transport: &dyn Transport,
    request: &Request,
    key: &str,
    budget: &InvocationBudget,
) -> Result<String, Failure> {
    let file_tool = FileTool::new(&request.root)?;
    let exec_tool = ExecTool::new(file_tool.root_path(), &request.allow_cmd)
        .with_deadline(budget.deadline)
        .with_cancellation(budget.cancelled.clone());
    let mut messages = vec![
        serde_json::json!({ "role": "system", "content": system_prompt(file_tool.root_path()) }),
        serde_json::json!({ "role": "user", "content": request.task }),
    ];
    let mut tool_calls_spent = 0usize;

    for _turn in 0..INVOCATION_MAX_MODEL_TURNS {
        let body = wire::request_body(&request.model, &messages);
        let remaining = budget.take_model_turn()?;
        let reply = transport.post_json_with_timeout(&request.base_url, key, &body, &[], remaining);
        budget.check()?;
        let reply = reply?;
        match wire::parse_reply(&reply)? {
            ModelReply::Text(text) => return Ok(text),
            ModelReply::ToolCalls(calls) => {
                if tool_calls_spent + calls.len() > INVOCATION_MAX_TOOL_CALLS {
                    return Err(format!(
                        "harness: stopped after {tool_calls_spent} tool calls: this turn's {} \
                         more would pass the bound of {INVOCATION_MAX_TOOL_CALLS}; the task \
                         did not finish",
                        calls.len()
                    )
                    .into());
                }
                // Echo the assistant turn back before its results: the wire
                // format matches each result to the call it answers by id, and
                // a result whose call is absent from the history is rejected.
                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": serde_json::Value::Null,
                    "tool_calls": calls
                        .iter()
                        .map(|call| serde_json::json!({
                            "id": call.id,
                            "type": "function",
                            "function": { "name": call.name, "arguments": call.arguments },
                        }))
                        .collect::<Vec<_>>(),
                }));
                for call in &calls {
                    budget.take_tool_call()?;
                    tool_calls_spent += 1;
                    // A refusal is a result, not an early return.
                    let result = dispatch_tool_call(&file_tool, &exec_tool, call)
                        .unwrap_or_else(|refusal| refusal);
                    budget.check()?;
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": result,
                    }));
                }
            }
        }
    }
    Err(format!(
        "harness: stopped after the bound of {INVOCATION_MAX_MODEL_TURNS} model turns without \
         a final answer; the task did not finish"
    )
    .into())
}

/// Runs one tool call. `Err` is still something to tell the model, not a
/// reason to abandon the run.
fn dispatch_tool_call(
    file_tool: &FileTool,
    exec_tool: &ExecTool,
    call: &WireToolCall,
) -> Result<String, String> {
    let arguments: serde_json::Value = serde_json::from_str(&call.arguments).map_err(|error| {
        format!(
            "{}: refused: the arguments are not a JSON object: {error}",
            call.name
        )
    })?;
    match call.name.as_str() {
        "file" => {
            let path = arguments
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "file: refused: no `path` argument".to_owned())?;
            match arguments.get("op").and_then(serde_json::Value::as_str) {
                Some("read") => file_tool.read(path),
                Some("write") => {
                    let contents = arguments
                        .get("contents")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            "file: refused: op=write needs a `contents` argument".to_owned()
                        })?;
                    let written = file_tool.write(path, contents)?;
                    Ok(format!("file: wrote {written} bytes to {path}"))
                }
                other => Err(format!(
                    "file: refused: op must be \"read\" or \"write\", got {other:?}"
                )),
            }
        }
        "exec" => {
            let argv = arguments
                .get("argv")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| "exec: refused: no `argv` array argument".to_owned())?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        "exec: refused: every argv entry must be a string".to_owned()
                    })
                })
                .collect::<Result<Vec<String>, String>>()?;
            let (program, args) = argv
                .split_first()
                .ok_or_else(|| "exec: refused: argv is empty".to_owned())?;
            let outcome = exec_tool.run(program, args)?;
            Ok(format!(
                "exec: exit={} timed_out={}\nstdout:\n{}\nstderr:\n{}",
                outcome
                    .exit_code
                    .map_or_else(|| "terminated".to_owned(), |code| code.to_string()),
                outcome.timed_out,
                outcome.stdout,
                outcome.stderr
            ))
        }
        // Refused, and named: the model invented a tool, and telling it so is
        // how it stops trying.
        other => Err(format!(
            "{other}: refused: there is no such tool; this harness has exactly `file` and `exec`"
        )),
    }
}

/// CLI entry: resolve the key, run the task, print the answer.
pub fn main_entry() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let request = parse_args(&args)?;
    let key = std::env::var(&request.key_env)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "harness: {} is unset or empty; the harness reads its key from that environment \
                 variable",
                request.key_env
            )
        })?;
    let answer = run(&wire::HttpTransport, &request, &key).map_err(|error| error.to_string())?;
    println!("{answer}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    fn fixture_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "minicon-harness-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        root.canonicalize().expect("canonical fixture root")
    }

    fn text_reply(text: &str) -> String {
        serde_json::json!({"choices":[{"message":{"role":"assistant","content":text}}]}).to_string()
    }

    /// A `Transport` that answers from a script and keeps every request body.
    struct ScriptedTransport {
        replies: RefCell<VecDeque<String>>,
        sent: RefCell<Vec<String>>,
    }

    impl ScriptedTransport {
        fn new(replies: &[String]) -> Self {
            Self {
                replies: RefCell::new(replies.iter().cloned().collect()),
                sent: RefCell::new(Vec::new()),
            }
        }
    }

    impl Transport for ScriptedTransport {
        fn post_json(
            &self,
            _url: &str,
            _bearer: &str,
            body: &str,
            _extra_headers: &[(&str, &str)],
        ) -> Result<String, String> {
            self.sent.borrow_mut().push(body.to_owned());
            self.replies
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| "script exhausted".to_owned())
        }
    }

    #[test]
    fn parse_args_requires_root_and_task() {
        let error = parse_args(&[]).unwrap_err();
        assert!(error.contains("--root"));
        let error = parse_args(&["--root".to_owned(), ".".to_owned()]).unwrap_err();
        assert!(error.contains("--task"));
        let request =
            parse_args(&["--root".into(), "r".into(), "--task".into(), "t".into()]).unwrap();
        assert_eq!(request.root, "r");
        assert_eq!(request.task, "t");
        assert_eq!(request.key_env, "HARNESS_API_KEY");
    }

    #[test]
    fn parse_args_rejects_unknown_flags() {
        let error = parse_args(&["--bogus".into()]).unwrap_err();
        assert!(error.contains("--bogus"));
    }

    #[test]
    fn a_text_only_reply_finishes_without_tools() {
        let root = fixture_root("text-only");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "say something".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let model = ScriptedTransport::new(&[text_reply("done")]);
        let answer = run(&model, &request, "synthetic").expect("one turn");
        assert_eq!(answer, "done");
        let sent = model.sent.borrow();
        assert_eq!(sent.len(), 1);
        let body: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
        assert_eq!(body["messages"][1]["content"], "say something");
        assert_eq!(body["tools"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn tool_calls_round_trip_until_a_final_answer() {
        let root = fixture_root("round-trip");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "write note.txt".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let call = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":[{
                "id":"call-0", "type":"function", "function":{
                    "name":"file", "arguments":"{\"op\":\"write\",\"path\":\"note.txt\",\"contents\":\"hi\"}"
                }
            }]
        }}]})
        .to_string();
        let model = ScriptedTransport::new(&[call, text_reply("written")]);
        let answer = run(&model, &request, "synthetic").expect("two turns");
        assert_eq!(answer, "written");
        let sent = model.sent.borrow();
        let second: serde_json::Value = serde_json::from_str(&sent[1]).unwrap();
        let messages = second["messages"].as_array().unwrap();
        assert_eq!(messages.last().unwrap()["role"], "tool");
        assert_eq!(
            messages.last().unwrap()["content"],
            "file: wrote 2 bytes to note.txt"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("note.txt")).unwrap(),
            "hi"
        );
    }

    #[test]
    fn a_tool_refusal_is_a_result_not_an_abort() {
        let root = fixture_root("refusal");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "escape".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let call = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":[{
                "id":"call-0", "type":"function", "function":{
                    "name":"file", "arguments":"{\"op\":\"read\",\"path\":\"../escape.txt\"}"
                }
            }]
        }}]})
        .to_string();
        let model = ScriptedTransport::new(&[call, text_reply("understood")]);
        let answer = run(&model, &request, "synthetic").expect("refusal must not abort");
        assert_eq!(answer, "understood");
        let sent = model.sent.borrow();
        let second: serde_json::Value = serde_json::from_str(&sent[1]).unwrap();
        let content = second["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(content.contains("refused"), "{content}");
    }

    #[test]
    fn an_unknown_tool_is_named_not_silently_dropped() {
        let root = fixture_root("unknown-tool");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "invent".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let call = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":[{
                "id":"call-0", "type":"function", "function":{
                    "name":"web_search", "arguments":"{}"
                }
            }]
        }}]})
        .to_string();
        let model = ScriptedTransport::new(&[call, text_reply("ok")]);
        run(&model, &request, "synthetic").expect("unknown tool must not abort");
        let sent = model.sent.borrow();
        let second: serde_json::Value = serde_json::from_str(&sent[1]).unwrap();
        let content = second["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(content.contains("exactly `file` and `exec`"), "{content}");
    }

    #[test]
    fn the_turn_bound_ends_a_model_that_never_answers() {
        let root = fixture_root("turn-bound");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "loop forever".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let call = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":[{
                "id":"call-x", "type":"function", "function":{
                    "name":"file", "arguments":"{\"op\":\"read\",\"path\":\"x\"}"
                }
            }]
        }}]})
        .to_string();
        // One scripted call is replayed forever: the model never answers.
        struct LoopTransport(String);
        impl Transport for LoopTransport {
            fn post_json(
                &self,
                _url: &str,
                _bearer: &str,
                _body: &str,
                _extra_headers: &[(&str, &str)],
            ) -> Result<String, String> {
                Ok(self.0.clone())
            }
        }
        let model = LoopTransport(call);
        let error = run(&model, &request, "synthetic").unwrap_err();
        assert!(matches!(error, Failure::Backend(ref text) if text.contains("16 model turns")));
    }

    #[test]
    fn the_tool_call_bound_blocks_the_sixty_fifth_effect() {
        let root = fixture_root("tool-bound");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "write many".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let calls: Vec<_> = (0..100)
            .map(|index| {
                serde_json::json!({
                    "id": format!("call-{index}"), "type":"function", "function":{
                        "name":"file",
                        "arguments": serde_json::json!({
                            "op":"write", "path":format!("effect-{index}"), "contents":"x"
                        })
                        .to_string()
                    }
                })
            })
            .collect();
        let reply = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":calls
        }}]})
        .to_string();
        struct LoopTransport(String);
        impl Transport for LoopTransport {
            fn post_json(
                &self,
                _url: &str,
                _bearer: &str,
                _body: &str,
                _extra_headers: &[(&str, &str)],
            ) -> Result<String, String> {
                Ok(self.0.clone())
            }
        }
        let model = LoopTransport(reply);
        let error = run(&model, &request, "synthetic").unwrap_err();
        // A single turn asking for more than the whole invocation may spend is
        // refused wholesale, before any call in it dispatches.
        assert!(matches!(error, Failure::Backend(ref text) if text.contains("64")));
        assert!(
            !root.join("effect-0").exists(),
            "the oversized batch ran despite the bound"
        );
    }

    #[test]
    fn exactly_at_the_tool_call_bound_the_last_call_still_runs() {
        let root = fixture_root("tool-bound-edge");
        let request = Request {
            root: root.to_str().unwrap().to_owned(),
            task: "write many".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
        };
        let calls: Vec<_> = (0..64)
            .map(|index| {
                serde_json::json!({
                    "id": format!("call-{index}"), "type":"function", "function":{
                        "name":"file",
                        "arguments": serde_json::json!({
                            "op":"write", "path":format!("effect-{index}"), "contents":"x"
                        })
                        .to_string()
                    }
                })
            })
            .collect();
        let reply = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":calls
        }}]})
        .to_string();
        struct LoopTransport(String);
        impl Transport for LoopTransport {
            fn post_json(
                &self,
                _url: &str,
                _bearer: &str,
                _body: &str,
                _extra_headers: &[(&str, &str)],
            ) -> Result<String, String> {
                Ok(self.0.clone())
            }
        }
        let model = LoopTransport(reply);
        // The turn spends the whole budget; the *next* model turn is refused.
        let error = run(&model, &request, "synthetic").unwrap_err();
        assert!(
            matches!(error, Failure::Backend(ref text) if text.contains("64 tool calls")),
            "{error}"
        );
        for index in 0..64 {
            assert!(
                root.join(format!("effect-{index}")).exists(),
                "effect-{index} missing"
            );
        }
        assert!(!root.join("effect-64").exists());
    }
}
