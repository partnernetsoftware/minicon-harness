//! # minicon-harness
//!
//! A minimal bounded agent loop with exactly two tools, `file` and `exec`.
//! Not a plugin host, not a permission-policy framework -- the two tools' own
//! bounds (the `--root` path, the `--allow-cmd` list) are the whole policy.
//!
//! In the spirit of pi-agent: the smallest complete thing that can run a
//! model against a filesystem and a command line, with every bound stated
//! as a constant next to the code that enforces it.

pub mod journal;
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

/// The one canonical version banner. LF-terminated on purpose: the delivery
/// court compares these exact bytes, so a CRLF or a missing newline is a
/// failure, not a formatting choice. `env!` binds it to `Cargo.toml` at
/// compile time -- a Cargo.toml version bump without a matching court
/// expectation fails the release, never drifts silently.
pub const VERSION_BANNER: &str = concat!("minicon-harness ", env!("CARGO_PKG_VERSION"), "\n");

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
    /// Resume: replay this journal as prior context ahead of the new task.
    /// `None` starts a fresh journal; `Some` appends to the existing one.
    pub resume: bool,
    /// Undo every file write/edit this invocation recorded, then stop. No
    /// model round trip happens.
    pub undo: bool,
    /// Print the diff of what the journal's last invocation changed, then
    /// stop. No model round trip happens.
    pub diff: bool,
}

/// What flag parsing produced. `Version` short-circuits the run: printing
/// the version never requires a task, a root or an API key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Parsed {
    Version,
    Run(Request),
}

/// Parses the CLI form: `--root PATH --task TEXT [--base-url URL]
/// [--model NAME] [--allow-cmd NAME]... [--key-env NAME] [--resume]
/// [--undo] [--diff]`.
pub fn parse_args(args: &[String]) -> Result<Parsed, String> {
    if args.len() == 1 && args[0] == "--version" {
        return Ok(Parsed::Version);
    }
    let usage = "usage: minicon-harness --root PATH --task TEXT [--base-url URL] \
                 [--model NAME] [--allow-cmd NAME]... [--key-env NAME] \
                 [--resume] [--undo] [--diff]"
        .to_owned();
    let mut root = None;
    let mut task = None;
    let mut base_url = OPENAI_COMPATIBLE_CHAT_URL.to_owned();
    let mut model = DEFAULT_MODEL.to_owned();
    let mut allow_cmd = Vec::new();
    let mut key_env = "HARNESS_API_KEY".to_owned();
    let mut resume = false;
    let mut undo = false;
    let mut diff = false;
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
            "--resume" => resume = true,
            "--undo" => undo = true,
            "--diff" => diff = true,
            other => {
                return Err(format!("unexpected argument {other:?}\n{usage}"));
            }
        }
    }
    if undo && diff {
        return Err("--undo and --diff are mutually exclusive".to_owned());
    }
    if task.is_none() && !undo && !diff {
        return Err(format!(
            "--task TEXT is required
{usage}"
        ));
    }
    Ok(Parsed::Run(Request {
        root: root.ok_or_else(|| {
            format!(
                "--root PATH is required (there is deliberately no implicit current-directory \
                 default, which would widen the file tool's bound silently)\n{usage}"
            )
        })?,
        task: task.unwrap_or_default(),
        base_url,
        model,
        allow_cmd,
        key_env,
        resume,
        undo,
        diff,
    }))
}

/// Default OpenAI-compatible chat-completions endpoint. Overridable with
/// `--base-url`; the wire codec speaks one format, so any server that accepts
/// the documented request shape works.
pub const OPENAI_COMPATIBLE_CHAT_URL: &str = "https://api.deepseek.com/chat/completions";
pub const DEFAULT_MODEL: &str = "deepseek-chat";

/// The instruction the task is wrapped in, so the model knows what the tools'
/// bounds are before it trips over them, plus the project's own conventions
/// from `HARNESS.md` (fallback `AGENTS.md`) at the root when present.
pub fn system_prompt(root: &std::path::Path) -> String {
    let mut prompt = format!(
        "You are a bounded harness agent. You have exactly two tools, `file` and `exec`, and \
         no others. Every path is relative to the task root {root}; paths outside it are \
         refused, not rewritten. `exec` runs one allow-listed command per call with no shell. \
         A refusal is information: read it, adjust, and continue. Only after all requested \
         work has succeeded, answer with final plain text and no tool call.",
        root = root.display(),
    );
    if let Some(project) = project_conventions(root) {
        prompt.push_str("\n\nProject conventions (from the task root):\n");
        prompt.push_str(&project);
    }
    prompt
}

/// Reads `HARNESS.md` (fallback `AGENTS.md`) at the root, capped at 8KB on a
/// codepoint boundary. Absent file → nothing; the cap never splits a char.
fn project_conventions(root: &std::path::Path) -> Option<String> {
    const CAP: usize = 8 * 1024;
    for name in ["HARNESS.md", "AGENTS.md"] {
        let path = root.join(name);
        if let Ok(raw) = std::fs::read(&path) {
            let capped = &raw[..raw.len().min(CAP)];
            let mut text = String::from_utf8_lossy(capped).into_owned();
            while !text.is_char_boundary(text.len()) {
                text.pop();
            }
            if raw.len() > CAP {
                text.push_str("\n... (truncated at 8KB)");
            }
            return Some(text);
        }
    }
    None
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
    let journal = journal::Journal::for_root(file_tool.root_path());
    let mut messages = vec![
        serde_json::json!({ "role": "system", "content": system_prompt(file_tool.root_path()) }),
        serde_json::json!({ "role": "user", "content": request.task }),
    ];
    // Resume: replay the journal's task/answer pairs ahead of the new task,
    // so the model continues where the last invocation stopped. The turn and
    // tool-call bounds stay per-invocation, never accumulated.
    if request.resume {
        let context = journal.resume_context().map_err(Failure::Backend)?;
        if !context.is_empty() {
            messages.insert(1, serde_json::json!({ "role": "user", "content": context }));
        }
        if request.task.is_empty() {
            return Err(Failure::Backend(
                "--resume with no --task has nothing to continue; give a task".to_owned(),
            ));
        }
    }
    if !request.task.is_empty() {
        journal
            .append(&journal::Record::Task(request.task.clone()))
            .map_err(Failure::Backend)?;
    }
    let mut tool_calls_spent = 0usize;

    for _turn in 0..INVOCATION_MAX_MODEL_TURNS {
        let body = wire::request_body(&request.model, &messages);
        budget.take_model_turn()?;
        // Retry with backoff stays inside one budget turn: the 10-minute
        // deadline still bounds the whole invocation.
        let reply =
            wire::post_with_retry(transport, &request.base_url, key, &body, std::thread::sleep);
        budget.check()?;
        let reply = reply?;
        match wire::parse_reply(&reply)? {
            ModelReply::Text(text) => {
                journal
                    .append(&journal::Record::Answer(text.clone()))
                    .map_err(Failure::Backend)?;
                return Ok(text);
            }
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
                    let dispatched = dispatch_tool_call(&file_tool, &exec_tool, call)
                        .unwrap_or_else(|refusal| Dispatched {
                            result: refusal,
                            change: None,
                        });
                    if let Some(change) = &dispatched.change {
                        // Only successful changes carry a record; appending
                        // it is what makes --undo and --diff real evidence.
                        journal.append(change).map_err(Failure::Backend)?;
                    }
                    budget.check()?;
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "content": dispatched.result,
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

/// The machine-readable status of one dispatched tool call, prefixed to the
/// tool result so the model reads structured truth before prose. Success
/// comes from the tool's own evidence -- an exec's exit code, a write that
/// really landed, an edit that replaced exactly once -- never from a
/// function having returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolStatus {
    Ok,
    Failed,
    Refused,
}

impl ToolStatus {
    fn tag(self) -> &'static str {
        match self {
            Self::Ok => "status=ok",
            Self::Failed => "status=failed",
            Self::Refused => "status=refused",
        }
    }
}

/// Formats one tool result: `status=... ` then the body. `Err` maps to
/// `Failed` or `Refused` by the caller's classification; both are results,
/// not aborts.
fn tool_result(status: ToolStatus, body: String) -> String {
    format!("{} {body}", status.tag())
}

/// The outcome of one dispatched call: the model-facing result text plus,
/// for a successful file change, the journal record that makes it reversible.
struct Dispatched {
    result: String,
    change: Option<journal::Record>,
}

/// Runs one tool call. `Err` is still something to tell the model, not a
/// reason to abandon the run; it means the call itself failed or was
/// refused.
fn dispatch_tool_call(
    file_tool: &FileTool,
    exec_tool: &ExecTool,
    call: &WireToolCall,
) -> Result<Dispatched, String> {
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
                Some("read") => file_tool.read(path).map(|contents| Dispatched {
                    result: tool_result(ToolStatus::Ok, contents),
                    change: None,
                }),
                Some("write") => {
                    let contents = arguments
                        .get("contents")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            "file: refused: op=write needs a `contents` argument".to_owned()
                        })?;
                    // A zero-byte write is a legal empty file, not a failure:
                    // success here means the bytes really landed on disk.
                    let previous = file_tool.read(path).ok();
                    let written = file_tool.write(path, contents)?;
                    Ok(Dispatched {
                        result: tool_result(
                            ToolStatus::Ok,
                            format!("wrote {written} bytes to {path}"),
                        ),
                        change: Some(journal::Record::Wrote {
                            path: path.to_owned(),
                            previous,
                        }),
                    })
                }
                Some("edit") => {
                    let old = arguments
                        .get("old")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            "file: refused: op=edit needs an `old` argument".to_owned()
                        })?;
                    let new = arguments
                        .get("new")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            "file: refused: op=edit needs a `new` argument".to_owned()
                        })?;
                    let previous = file_tool.read(path).ok();
                    let size_after = file_tool.edit(path, old, new)?;
                    // previous is the true before-state only when the edit
                    // succeeded; on failure the record is not appended.
                    Ok(Dispatched {
                        result: tool_result(
                            ToolStatus::Ok,
                            format!("edited {path}; file is now {size_after} bytes"),
                        ),
                        change: Some(journal::Record::Edited {
                            path: path.to_owned(),
                            previous: previous.unwrap_or_default(),
                        }),
                    })
                }
                other => Err(format!(
                    "file: refused: op must be \"read\", \"write\" or \"edit\", got {other:?}"
                )),
            }
        }
        "exec" => {
            let why = arguments
                .get("why")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    "exec: refused: the `why` argument is required: one short line saying what \
                     this command is for"
                        .to_owned()
                })?;
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
            let status = if outcome.timed_out || outcome.exit_code != Some(0) {
                ToolStatus::Failed
            } else {
                ToolStatus::Ok
            };
            Ok(Dispatched {
                result: tool_result(
                    status,
                    format!(
                        "why: {why}\nexec: exit={} timed_out={}\nstdout:\n{}\nstderr:\n{}",
                        outcome
                            .exit_code
                            .map_or_else(|| "terminated".to_owned(), |code| code.to_string()),
                        outcome.timed_out,
                        outcome.stdout,
                        outcome.stderr
                    ),
                ),
                change: None,
            })
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
    match parse_args(&args)? {
        Parsed::Version => {
            print!("{VERSION_BANNER}");
            Ok(())
        }
        Parsed::Run(request) => {
            // --undo and --diff are local journal operations: no key, no
            // model round trip, they work offline on any previous root.
            if request.undo || request.diff {
                let file_tool = FileTool::new(&request.root)?;
                let journal = journal::Journal::for_root(file_tool.root_path());
                if request.undo {
                    let steps = journal.undo_steps().map_err(|error| error.to_string())?;
                    let last = steps.last().ok_or_else(|| {
                        "harness: nothing to undo; the journal has no file changes".to_owned()
                    })?;
                    match &last.previous {
                        Some(previous) => {
                            file_tool.write(&last.path, previous)?;
                            println!("undid {} (restored {} bytes)", last.path, previous.len());
                        }
                        None => {
                            let path = file_tool.root_path().join(&last.path);
                            std::fs::remove_file(&path)
                                .map_err(|error| format!("harness: {}", error))?;
                            println!("undid {} (created file removed)", last.path);
                        }
                    }
                    return Ok(());
                }
                // --diff
                let changed = journal.changed_files().map_err(|error| error.to_string())?;
                if changed.is_empty() {
                    println!("no file changes recorded");
                    return Ok(());
                }
                for (path, first_previous, _last_previous) in changed {
                    let live = file_tool.read(&path).ok();
                    // Show the FIRST previous against the LIVE file: that is
                    // what "what did this journal change" means to a person.
                    println!(
                        "{}",
                        journal::render_diff(&path, &first_previous, live.as_deref())
                    );
                }
                return Ok(());
            }
            let key = std::env::var(&request.key_env)
                .ok()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "harness: {} is unset or empty; the harness reads its key from that \
                         environment variable",
                        request.key_env
                    )
                })?;
            let answer =
                run(&wire::HttpTransport, &request, &key).map_err(|error| error.to_string())?;
            println!("{answer}");
            Ok(())
        }
    }
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
        match parse_args(&["--root".into(), "r".into(), "--task".into(), "t".into()]).unwrap() {
            Parsed::Run(request) => {
                assert_eq!(request.root, "r");
                assert_eq!(request.task, "t");
                assert_eq!(request.key_env, "HARNESS_API_KEY");
                assert!(!request.resume && !request.undo && !request.diff);
            }
            other => panic!("expected a run request, got {other:?}"),
        }
    }

    #[test]
    fn parse_args_carries_undo_diff_resume_flags() {
        let parsed =
            parse_args(&["--root".into(), "r".into(), "--undo".into()]).expect("undo alone");
        match parsed {
            Parsed::Run(request) => {
                assert!(request.undo);
                assert!(!request.diff && !request.resume);
                assert!(request.task.is_empty());
            }
            other => panic!("expected a run request, got {other:?}"),
        }
        let error = parse_args(&[
            "--root".into(),
            "r".into(),
            "--undo".into(),
            "--diff".into(),
        ])
        .unwrap_err();
        assert!(error.contains("mutually exclusive"));
    }

    #[test]
    fn parse_args_rejects_unknown_flags() {
        let error = parse_args(&["--bogus".into()]).unwrap_err();
        assert!(error.contains("--bogus"));
    }

    #[test]
    fn version_short_circuits_without_a_task_root_or_key() {
        assert_eq!(parse_args(&["--version".into()]).unwrap(), Parsed::Version);
        // Exact bytes, including the LF and the version, for the court.
        assert_eq!(
            VERSION_BANNER,
            format!(
                "minicon-harness {}
",
                env!("CARGO_PKG_VERSION")
            )
        );
        // Any other spelling is not a version query.
        assert!(parse_args(&["--version".into(), "extra".into()]).is_err());
        assert!(parse_args(&["-V".into()]).is_err());
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
            resume: false,
            undo: false,
            diff: false,
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
            resume: false,
            undo: false,
            diff: false,
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
            "status=ok wrote 2 bytes to note.txt"
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
            resume: false,
            undo: false,
            diff: false,
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
            resume: false,
            undo: false,
            diff: false,
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
            resume: false,
            undo: false,
            diff: false,
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
            resume: false,
            undo: false,
            diff: false,
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
            resume: false,
            undo: false,
            diff: false,
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

    #[test]
    fn resume_replays_prior_task_and_answer() {
        let root = fixture_root("resume-e2e");
        let root_str = root.to_str().unwrap().to_owned();
        let first = Request {
            root: root_str.clone(),
            task: "first task".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
            resume: false,
            undo: false,
            diff: false,
        };
        let model = ScriptedTransport::new(&[text_reply("first answer")]);
        run(&model, &first, "synthetic").expect("first run");
        let second = Request {
            resume: true,
            task: "second task".to_owned(),
            ..first
        };
        let model2 = ScriptedTransport::new(&[text_reply("second answer")]);
        run(&model2, &second, "synthetic").expect("resumed run");
        let sent = model2.sent.borrow();
        let body: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
        let messages = body["messages"].as_array().unwrap();
        let replay = messages[1]["content"].as_str().unwrap();
        assert!(replay.contains("Previous task: first task"), "{replay}");
        assert!(replay.contains("Previous answer: first answer"), "{replay}");
        assert_eq!(messages[2]["content"], "second task");
    }

    #[test]
    fn undo_journal_records_the_write() {
        let root = fixture_root("undo-e2e");
        let root_str = root.to_str().unwrap().to_owned();
        let request = Request {
            root: root_str.clone(),
            task: "write and undo".to_owned(),
            base_url: "http://127.0.0.1/fixture".to_owned(),
            model: "fixture".to_owned(),
            allow_cmd: Vec::new(),
            key_env: "HARNESS_API_KEY".to_owned(),
            resume: false,
            undo: false,
            diff: false,
        };
        let call = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null, "tool_calls":[{
                "id":"c", "type":"function", "function":{"name":"file","arguments":
                "{\"op\":\"write\",\"path\":\"u.txt\",\"contents\":\"new\"}"}
            }]
        }}]})
        .to_string();
        let model = ScriptedTransport::new(&[call, text_reply("done")]);
        run(&model, &request, "synthetic").unwrap();
        assert_eq!(std::fs::read_to_string(root.join("u.txt")).unwrap(), "new");
        let file_tool = FileTool::new(&root_str).unwrap();
        let journal = journal::Journal::for_root(file_tool.root_path());
        let steps = journal.undo_steps().unwrap();
        let last = steps.last().unwrap();
        assert_eq!(last.path, "u.txt");
        assert_eq!(last.previous, None);
    }

    #[test]
    fn project_file_enters_the_system_prompt() {
        let root = fixture_root("project-file");
        std::fs::write(root.join("HARNESS.md"), "always answer in haiku").unwrap();
        let prompt = system_prompt(&root);
        assert!(prompt.contains("always answer in haiku"), "{prompt}");
        assert!(prompt.starts_with("You are a bounded harness agent."));
        let root2 = fixture_root("project-file-fallback");
        std::fs::write(root2.join("AGENTS.md"), "agents fallback").unwrap();
        assert!(system_prompt(&root2).contains("agents fallback"));
        let root3 = fixture_root("project-file-none");
        assert!(!system_prompt(&root3).contains("Project conventions"));
    }
}
