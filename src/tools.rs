//! The two tools: `file` (root-confined read/write) and `exec` (allow-listed
//! argv execution). Their own bounds are the whole policy.

// ---------------------------------------------------------------------------
// the `file` tool
// ---------------------------------------------------------------------------

/// Largest file the `file` tool will read in one call. A tool result is
/// eventually going into a model prompt, so the bound is the read's, not the
/// caller's: an unbounded read would be an unbounded prompt.
pub const FILE_TOOL_MAX_READ_BYTES: usize = 256 * 1024;

/// Read and write, confined to one directory.
///
/// The bound is enforced by construction rather than by comparing resolved
/// prefixes, because a prefix comparison has to be right about symlinks at
/// every component and about paths that do not exist yet. Instead a relative
/// path is admitted only when it is made entirely of plain components and no
/// component that already exists is a symlink. Such a path cannot leave the
/// canonical root, so there is nothing left to clamp -- anything else is
/// refused.
#[derive(Clone, Debug)]
pub struct FileTool {
    root: std::path::PathBuf,
}

impl FileTool {
    /// Canonicalizes the root once. A root that is missing or is not a
    /// directory is refused here, so no later call has to wonder whether its
    /// bound exists.
    pub fn new(root: &str) -> Result<Self, String> {
        let root = std::fs::canonicalize(root)
            .map_err(|error| format!("file: --root {root:?} cannot be resolved: {error}"))?;
        if !root.is_dir() {
            return Err(format!(
                "file: --root {} is not a directory",
                root.display()
            ));
        }
        Ok(Self { root })
    }

    /// The canonical directory every path this tool accepts stays inside.
    pub fn root_path(&self) -> &std::path::Path {
        &self.root
    }

    /// Resolves one model-supplied relative path against the root, refusing
    /// anything that could denote a file outside it.
    ///
    /// Refused, never rewritten: an absolute path (including a Windows drive
    /// prefix), an empty path, any `..` or `.` component, and any existing
    /// component -- intermediate or final -- that is a symlink. The symlink
    /// check walks the path from the root downwards, so a symlinked directory
    /// halfway along cannot be traversed, and a leaf that does not exist yet
    /// (an ordinary create) is accepted only once every one of its existing
    /// ancestors has been checked.
    fn resolve(&self, relative: &str) -> Result<std::path::PathBuf, String> {
        use std::path::Component;

        if relative.is_empty() {
            return Err("file: path is empty; refused: give a path relative to --root".to_owned());
        }
        let candidate = std::path::Path::new(relative);
        let mut resolved = self.root.clone();
        for component in candidate.components() {
            match component {
                Component::Normal(part) => resolved.push(part),
                Component::ParentDir => {
                    return Err(format!(
                        "file: path {relative:?} is refused: a `..` component would escape \
                         --root {}; paths are not clamped into the root",
                        self.root.display()
                    ));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(format!(
                        "file: path {relative:?} is refused: only paths relative to --root {} \
                         are accepted, never absolute ones",
                        self.root.display()
                    ));
                }
                // `.` carries no authority but also no meaning here; refusing
                // it keeps "every component is a plain name" true, which is
                // the property the symlink walk below relies on.
                Component::CurDir => {
                    return Err(format!(
                        "file: path {relative:?} is refused: remove the `.` component"
                    ));
                }
            }
            match std::fs::symlink_metadata(&resolved) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(format!(
                        "file: path {relative:?} is refused: {} is a symlink, and the file \
                         tool never follows one -- a link could point outside --root {}",
                        resolved.display(),
                        self.root.display()
                    ));
                }
                // Absent is fine: a create names a leaf that does not exist,
                // and every ancestor it does have was checked on an earlier
                // turn of this loop.
                _ => {}
            }
        }
        Ok(resolved)
    }

    /// Reads one bounded UTF-8 file from inside the root.
    pub fn read(&self, relative: &str) -> Result<String, String> {
        let path = self.resolve(relative)?;
        let bytes = read_bounded(&path, FILE_TOOL_MAX_READ_BYTES)
            .map_err(|error| format!("file: read {relative:?} failed: {error}"))?;
        String::from_utf8(bytes)
            .map_err(|_| format!("file: read {relative:?} failed: file is not valid UTF-8"))
    }

    /// Writes one file inside the root, atomically: the new contents land in
    /// a sibling and are renamed over the destination, so a failure mid-write
    /// leaves the destination at its previous contents.
    pub fn write(&self, relative: &str, contents: &str) -> Result<usize, String> {
        use std::io::Write as _;

        let path = self.resolve(relative)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("file: write {relative:?} failed: {error}"))?;
        }
        let sibling = sibling_temporary_path(&path);
        {
            let mut file = std::fs::File::create(&sibling)
                .map_err(|error| format!("file: write {relative:?} failed: {error}"))?;
            file.write_all(contents.as_bytes())
                .map_err(|error| format!("file: write {relative:?} failed: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("file: write {relative:?} failed: {error}"))?;
        }
        std::fs::rename(&sibling, &path).map_err(|error| {
            let _ = std::fs::remove_file(&sibling);
            format!("file: write {relative:?} failed: {error}")
        })?;
        Ok(contents.len())
    }
}

/// A unique sibling path for the atomic-publish rename. The `.tmp-` prefix
/// plus the process id plus a counter keeps concurrent calls from colliding.
fn sibling_temporary_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let file_name = path
        .file_name()
        .map(std::ffi::OsStr::to_string_lossy)
        .unwrap_or_default();
    let mut sibling = path.to_path_buf();
    sibling.set_file_name(format!(
        ".tmp-{file_name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    sibling
}

/// Reads at most `max` bytes. A file longer than that is refused, never
/// silently truncated -- a silent truncation would be read by the model as
/// the whole file.
fn read_bounded(path: &std::path::Path, max: usize) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > max as u64 {
        return Err(format!(
            "file is {} bytes, over the read bound of {max}",
            metadata.len()
        ));
    }
    std::fs::read(path).map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// the `exec` tool
// ---------------------------------------------------------------------------

/// How long one `exec` call may run before it is terminated.
pub const EXEC_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub const EXEC_TOOL_MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const EXEC_TOOL_TRUNCATION_MARKER: &str = "\n[exec: combined output truncated]\n";

/// Exact inherited names permitted for exec. Provider keys, proxies and shell
/// startup variables are deliberately absent. HOME/USERPROFILE locate ordinary
/// native user configuration; this is an environment boundary, not filesystem
/// isolation from the current user.
const EXEC_TOOL_ENVIRONMENT: &[&str] = &[
    "PATH",
    "SystemRoot",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "USERPROFILE",
    "HOME",
    "TMP",
    "TEMP",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
];

/// What one finished `exec` call produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecOutcome {
    /// `None` when the process was terminated rather than exiting on its own
    /// (a signal, or this tool's own timeout).
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Runs exactly one allow-listed command per call, as an argv vector.
///
/// There is no shell anywhere in this path: the program is executed directly,
/// so `;`, `&&`, `|`, `$(...)` and friends reach the process as ordinary
/// argument bytes and cannot become a second command.
#[derive(Clone, Debug)]
pub struct ExecTool {
    root: std::path::PathBuf,
    allow_cmd: Vec<String>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    deadline: Option<std::time::Instant>,
}

impl ExecTool {
    pub fn new(root: &std::path::Path, allow_cmd: &[String]) -> Self {
        Self {
            root: root.to_path_buf(),
            allow_cmd: allow_cmd.to_vec(),
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            deadline: None,
        }
    }

    /// Shares an invocation owner's cancellation signal. Cancellation is
    /// terminal for an exec call and never returns a successful partial tool
    /// result.
    pub fn with_cancellation(
        mut self,
        cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        self.cancelled = cancelled;
        self
    }

    pub fn with_deadline(mut self, deadline: std::time::Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Decides whether `program` may run, without running it.
    ///
    /// An empty allow-list does not make the tool disappear; it makes every
    /// call fail with this one error, naming the flag that would grant it. A
    /// model probing the tool therefore cannot distinguish "not allowed yet"
    /// from "tool absent".
    fn admit(&self, program: &str) -> Result<(), String> {
        if self.allow_cmd.is_empty() {
            return Err(
                "exec: refused: the allow-list is empty, so no command may run; pass \
                 --allow-cmd NAME (repeatable) to permit an executable basename"
                    .to_owned(),
            );
        }
        if program.is_empty() {
            return Err("exec: refused: no command given".to_owned());
        }
        // A basename, not a path: without this, `--allow-cmd ls` would also
        // admit `./ls` or `/tmp/evil/ls`, whose basename matches but whose
        // bytes name a different executable.
        if std::path::Path::new(program).components().count() != 1
            || program.contains('/')
            || program.contains('\\')
        {
            return Err(format!(
                "exec: refused: {program:?} is a path, not an executable basename; \
                 --allow-cmd names basenames and exec resolves them on PATH"
            ));
        }
        if !self.allow_cmd.iter().any(|allowed| allowed == program) {
            return Err(format!(
                "exec: refused: {program:?} is not in the allow-list ({}); pass \
                 --allow-cmd {program} to permit it",
                self.allow_cmd.join(", ")
            ));
        }
        Ok(())
    }

    /// Runs one command with its arguments, capturing output.
    pub fn run(&self, program: &str, args: &[String]) -> Result<ExecOutcome, String> {
        self.admit(program)?;
        self.spawn_with_timeout(program, args, EXEC_TOOL_TIMEOUT)
    }

    fn spawn_with_timeout(
        &self,
        program: &str,
        args: &[String],
        timeout: std::time::Duration,
    ) -> Result<ExecOutcome, String> {
        use std::io::Read as _;
        use std::process::Stdio;

        if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err("exec: cancelled before spawn".to_owned());
        }
        let exec_deadline = std::time::Instant::now() + timeout;
        let deadline = self
            .deadline
            .map_or(exec_deadline, |deadline| deadline.min(exec_deadline));
        if std::time::Instant::now() >= deadline {
            return Err("exec: invocation deadline exhausted before spawn".to_owned());
        }

        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in EXEC_TOOL_ENVIRONMENT {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }

        let mut child = command
            .spawn()
            .map_err(|error| format!("exec: {program:?} could not be started: {error}"))?;

        // std pipes are blocking, so the deadline cannot be polled from the
        // same thread that reads them. Each pipe gets one reader thread that
        // appends into shared bounded capture state and ends at EOF -- which
        // the kill below forces -- while this thread owns cancellation,
        // deadline and reaping. Output beyond the combined bound is dropped
        // and flagged, never allowed to deadlock a child on a full pipe.
        let capture = std::sync::Arc::new(std::sync::Mutex::new(ExecCapture::default()));
        let mut readers = Vec::new();
        let pipes: Vec<(Option<Box<dyn std::io::Read + Send>>, bool)> = vec![
            (child.stdout.take().map(|pipe| Box::new(pipe) as _), false),
            (child.stderr.take().map(|pipe| Box::new(pipe) as _), true),
        ];
        for (pipe, is_stderr) in pipes {
            let Some(pipe) = pipe else { continue };
            let capture = std::sync::Arc::clone(&capture);
            readers.push(std::thread::spawn(move || {
                let mut pipe = pipe;
                loop {
                    let mut chunk = [0; 8192];
                    match pipe.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            if let Ok(mut guard) = capture.lock() {
                                guard.append(&chunk[..count], is_stderr);
                            } else {
                                break;
                            }
                        }
                    }
                }
            }));
        }

        let mut timed_out = false;
        loop {
            if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
                let _ = child.kill();
                let _ = child.wait();
                for reader in readers {
                    let _ = reader.join();
                }
                return Err("exec: cancelled".to_owned());
            }
            match child.try_wait() {
                Err(error) => {
                    let _ = child.kill();
                    for reader in readers {
                        let _ = reader.join();
                    }
                    return Err(format!("exec: wait failed: {error}"));
                }
                Ok(status) => {
                    if std::time::Instant::now() >= deadline {
                        timed_out = true;
                        // Forcing EOF is what ends the reader threads.
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    if status.is_some() && !timed_out {
                        break;
                    }
                    if timed_out {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
        // On the timed-out path the child has been killed; reap it so the
        // pipe EOFs and the readers finish. On the exit path this is a no-op.
        let status = child
            .wait()
            .map_err(|error| format!("exec: final wait failed: {error}"))?;
        for reader in readers {
            // Bounded join: the kill above guarantees EOF, so a reader can
            // only be arriving at that EOF. A plain join cannot hang here.
            let _ = reader.join();
        }
        let (stdout, stderr) = capture
            .lock()
            .map(|guard| guard.finish())
            .map_err(|_| "exec: output capture state was poisoned by a reader thread".to_owned())?;
        Ok(ExecOutcome {
            exit_code: if timed_out { None } else { status.code() },
            stdout,
            stderr,
            timed_out,
        })
    }
}

#[derive(Default)]
struct ExecCapture {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    truncated: bool,
}

impl ExecCapture {
    fn append(&mut self, bytes: &[u8], is_stderr: bool) {
        let remaining =
            EXEC_TOOL_MAX_OUTPUT_BYTES.saturating_sub(self.stdout.len() + self.stderr.len());
        let keep = remaining.min(bytes.len());
        self.truncated |= keep < bytes.len();
        let target = if is_stderr {
            &mut self.stderr
        } else {
            &mut self.stdout
        };
        target.extend_from_slice(&bytes[..keep]);
    }

    fn finish(&self) -> (String, String) {
        let mut stdout = String::from_utf8_lossy(&self.stdout).into_owned();
        let mut stderr = String::from_utf8_lossy(&self.stderr).into_owned();
        if self.truncated || stdout.len() + stderr.len() > EXEC_TOOL_MAX_OUTPUT_BYTES {
            // Account for UTF-8 replacement expansion and the marker itself.
            let available = EXEC_TOOL_MAX_OUTPUT_BYTES - EXEC_TOOL_TRUNCATION_MARKER.len();
            truncate_utf8(&mut stdout, available);
            truncate_utf8(&mut stderr, available - stdout.len());
            stderr.push_str(EXEC_TOOL_TRUNCATION_MARKER);
        }
        (stdout, stderr)
    }
}

fn truncate_utf8(text: &mut String, max_bytes: usize) {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "minicon-harness-tools-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        root.canonicalize().expect("canonical fixture root")
    }

    #[cfg(unix)]
    fn exec_tool(root: &std::path::Path) -> ExecTool {
        ExecTool::new(
            root,
            &[
                "sh".to_owned(),
                "echo".to_owned(),
                "cat".to_owned(),
                "mkdir".to_owned(),
            ],
        )
    }

    #[cfg(windows)]
    fn exec_tool(root: &std::path::Path) -> ExecTool {
        ExecTool::new(
            root,
            &[
                "cmd".to_owned(),
                "powershell".to_owned(),
                "findstr".to_owned(),
            ],
        )
    }

    #[test]
    fn file_refuses_absolute_parent_and_dot_paths() {
        let root = fixture_root("refuse");
        let tool = FileTool::new(root.to_str().unwrap()).unwrap();
        for path in ["/etc/passwd", "../escape", "a/../../b", "./here", ""] {
            let error = tool.read(path).unwrap_err();
            assert!(error.contains("refused"), "{path}: {error}");
        }
        // Windows drive prefixes are absolute paths on Windows.
        #[cfg(windows)]
        assert!(tool
            .read("C:\\Windows\\win.ini")
            .unwrap_err()
            .contains("refused"));
    }

    #[test]
    fn file_write_creates_parents_and_reads_back() {
        let root = fixture_root("write-read");
        let tool = FileTool::new(root.to_str().unwrap()).unwrap();
        let written = tool.write("a/b/note.txt", "hello").unwrap();
        assert_eq!(written, 5);
        assert_eq!(tool.read("a/b/note.txt").unwrap(), "hello");
    }

    #[test]
    fn file_write_is_atomic_on_failure() {
        let root = fixture_root("atomic");
        let tool = FileTool::new(root.to_str().unwrap()).unwrap();
        tool.write("keep.txt", "original").unwrap();
        // An unwritable target: make the parent a *file*, so creating a
        // sibling inside it fails.
        std::fs::write(root.join("blocker"), "").unwrap();
        let error = tool.write("blocker/inner.txt", "nope").unwrap_err();
        assert!(error.contains("failed"), "{error}");
        assert_eq!(tool.read("keep.txt").unwrap(), "original");
        assert!(!root.join("blocker/inner.txt").exists());
    }

    #[test]
    fn file_read_refuses_oversize_files_instead_of_truncating() {
        let root = fixture_root("oversize");
        let tool = FileTool::new(root.to_str().unwrap()).unwrap();
        let big = "x".repeat(FILE_TOOL_MAX_READ_BYTES + 1);
        tool.write("big.txt", &big).unwrap();
        let error = tool.read("big.txt").unwrap_err();
        assert!(error.contains("over the read bound"), "{error}");
    }

    #[test]
    fn exec_empty_allow_list_refuses_without_hiding_the_tool() {
        let root = fixture_root("empty-allow");
        let tool = ExecTool::new(&root, &[]);
        let error = tool.run("ls", &[]).unwrap_err();
        assert!(error.contains("allow-list is empty"), "{error}");
    }

    #[test]
    fn exec_refuses_paths_and_unlisted_programs() {
        let root = fixture_root("admit");
        let tool = exec_tool(&root);
        for program in ["./echo", "/bin/echo", "sub/dir/echo", "python3"] {
            let error = tool.run(program, &[]).unwrap_err();
            assert!(error.contains("refused"), "{program}: {error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn exec_runs_argv_without_a_shell_and_captures_output() {
        let root = fixture_root("argv");
        let tool = exec_tool(&root);
        let outcome = tool
            .run(
                "sh",
                &["-c".to_owned(), "echo hello > shown.txt".to_owned()],
            )
            .unwrap();
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.timed_out);
        // The redirect only worked because `sh` was the named program -- the
        // tool itself never interprets `>` for anyone.
        assert_eq!(
            std::fs::read_to_string(root.join("shown.txt")).unwrap(),
            "hello\n"
        );
    }

    #[cfg(windows)]
    #[test]
    fn exec_runs_argv_and_captures_output() {
        let root = fixture_root("argv-win");
        let tool = exec_tool(&root);
        let outcome = tool
            .run(
                "cmd",
                &["/C".to_owned(), "echo hello> shown.txt".to_owned()],
            )
            .unwrap();
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.timed_out);
        assert!(std::fs::read_to_string(root.join("shown.txt"))
            .unwrap()
            .contains("hello"));
    }

    #[cfg(unix)]
    #[test]
    fn exec_env_is_the_named_allow_list_only() {
        let root = fixture_root("env");
        let tool = exec_tool(&root);
        let outcome = tool
            .run(
                "sh",
                &[
                    "-c".to_owned(),
                    "echo \"${HARNESS_API_KEY-unset}\"".to_owned(),
                ],
            )
            .unwrap();
        assert!(outcome.stdout.contains("unset"), "{}", outcome.stdout);
    }

    #[cfg(unix)]
    #[test]
    fn exec_timeout_kills_a_runaway_child() {
        let root = fixture_root("timeout");
        // Inject a near deadline instead of waiting out EXEC_TOOL_TIMEOUT.
        let tool = ExecTool::new(&root, &["sleep".to_owned()])
            .with_deadline(std::time::Instant::now() + std::time::Duration::from_secs(2));
        let start = std::time::Instant::now();
        let outcome = tool.run("sleep", &["60".to_owned()]).unwrap();
        assert!(outcome.timed_out);
        assert_eq!(outcome.exit_code, None);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
    }

    #[cfg(windows)]
    #[test]
    fn exec_timeout_kills_a_runaway_child() {
        let root = fixture_root("timeout-win");
        // Inject a near deadline instead of waiting out EXEC_TOOL_TIMEOUT.
        let tool = ExecTool::new(&root, &["ping".to_owned()])
            .with_deadline(std::time::Instant::now() + std::time::Duration::from_secs(2));
        let start = std::time::Instant::now();
        let outcome = tool
            .run(
                "ping",
                &["-n".to_owned(), "60".to_owned(), "127.0.0.1".to_owned()],
            )
            .unwrap();
        assert!(outcome.timed_out);
        assert_eq!(outcome.exit_code, None);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
    }

    #[test]
    fn exec_output_is_bounded_and_marked() {
        // Direct unit check of the capture bound, without spawning a flood.
        let mut capture = ExecCapture::default();
        let chunk = vec![b'x'; 8192];
        let mut total = 0usize;
        while total < EXEC_TOOL_MAX_OUTPUT_BYTES + 64 * 1024 {
            capture.append(&chunk, false);
            total += chunk.len();
        }
        let (stdout, stderr) = capture.finish();
        assert!(stdout.len() + stderr.len() <= EXEC_TOOL_MAX_OUTPUT_BYTES);
        assert!(stderr.contains("truncated"));
    }
}
