//! The invocation journal: one jsonl per `--root`, recording every file
//! change (for undo and diff) and every completed turn (for resume).
//!
//! Bounds: the journal is line-oriented jsonl, one record per line, nothing
//! else reads or writes it. The API key never enters a record. A corrupt
//! line fails the resume with a named error rather than starting fresh.

use std::io::Write as _;

/// Most undo steps one journal retains. Oldest dropped first, like csih's
/// 20-step bound.
pub const UNDO_MAX_STEPS: usize = 20;

/// One reversible file change.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoStep {
    /// Path relative to the root, as the model spelled it.
    pub path: String,
    /// Full previous contents; `None` means the file did not exist before.
    pub previous: Option<String>,
}

/// What one journal line records.
#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    /// A file write that succeeded: path and the contents BEFORE the write
    /// (`None` = the file did not exist).
    Wrote {
        path: String,
        previous: Option<String>,
    },
    /// A file edit that succeeded: path and the contents BEFORE the edit.
    Edited { path: String, previous: String },
    /// The task handed to one invocation.
    Task(String),
    /// The final answer one invocation produced.
    Answer(String),
}

impl Record {
    fn to_line(&self) -> String {
        match self {
            Self::Wrote { path, previous } => serde_json::json!({
                "kind": "wrote", "path": path, "previous": previous,
            })
            .to_string(),
            Self::Edited { path, previous } => serde_json::json!({
                "kind": "edited", "path": path, "previous": previous,
            })
            .to_string(),
            Self::Task(text) => serde_json::json!({"kind": "task", "text": text}).to_string(),
            Self::Answer(text) => serde_json::json!({"kind": "answer", "text": text}).to_string(),
        }
    }

    fn from_line(line: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("journal line is not JSON: {error}"))?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "journal line has no kind".to_owned())?;
        match kind {
            "wrote" => Ok(Self::Wrote {
                path: string_field(&value, "path")?,
                previous: match value.get("previous") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(v) => Some(
                        v.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| "previous must be a string or null".to_owned())?,
                    ),
                },
            }),
            "edited" => Ok(Self::Edited {
                path: string_field(&value, "path")?,
                previous: string_field(&value, "previous")?,
            }),
            "task" => Ok(Self::Task(string_field(&value, "text")?)),
            "answer" => Ok(Self::Answer(string_field(&value, "text")?)),
            other => Err(format!("journal line kind {other:?} is unknown")),
        }
    }
}

fn string_field(value: &serde_json::Value, name: &str) -> Result<String, String> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("journal line field {name:?} must be a string"))
}

/// One changed file: path, its state BEFORE the first change, its state
/// BEFORE the last change (the "after" of the change before it).
pub type ChangedFile = (String, Option<String>, Option<String>);

/// The journal file for one root: `<root>/.minicon-harness/journal.jsonl`.
pub struct Journal {
    path: std::path::PathBuf,
}

impl Journal {
    /// Locates the journal under an already-canonical root.
    pub fn for_root(root: &std::path::Path) -> Self {
        Self {
            path: root.join(".minicon-harness").join("journal.jsonl"),
        }
    }

    /// Appends one record, creating the directory on first use.
    pub fn append(&self, record: &Record) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("journal: {error}"))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| format!("journal: {}", error))?;
        writeln!(file, "{}", record.to_line()).map_err(|error| format!("journal: {}", error))?;
        Ok(())
    }

    /// Reads every record. A corrupt line is a named error, never a silent
    /// fresh start -- `--resume` against a broken journal must stop.
    pub fn read_all(&self) -> Result<Vec<Record>, String> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("journal: {}", error)),
        };
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(Record::from_line)
            .collect()
    }

    /// The undo steps, oldest first, bounded to the most recent
    /// [`UNDO_MAX_STEPS`]. Only file writes and edits participate.
    pub fn undo_steps(&self) -> Result<Vec<UndoStep>, String> {
        let mut steps: Vec<UndoStep> = Vec::new();
        for record in self.read_all()? {
            match record {
                Record::Wrote { path, previous } => steps.push(UndoStep { path, previous }),
                Record::Edited { path, previous } => steps.push(UndoStep {
                    path,
                    previous: Some(previous),
                }),
                Record::Task(_) | Record::Answer(_) => {}
            }
        }
        if steps.len() > UNDO_MAX_STEPS {
            steps.drain(..steps.len() - UNDO_MAX_STEPS);
        }
        Ok(steps)
    }

    /// The task/answer pairs, oldest first, flattened into the text a
    /// `--resume` run puts ahead of its new task.
    pub fn resume_context(&self) -> Result<String, String> {
        let mut context = String::new();
        for record in self.read_all()? {
            match record {
                Record::Task(text) => {
                    context.push_str("Previous task: ");
                    context.push_str(&text);
                    context.push('\n');
                }
                Record::Answer(text) => {
                    context.push_str("Previous answer: ");
                    context.push_str(&text);
                    context.push('\n');
                }
                Record::Wrote { .. } | Record::Edited { .. } => {}
            }
        }
        Ok(context)
    }

    /// The paths this journal's file changes touched, once each, plus the
    /// before/after contents of the last known state of each -- the material
    /// `--diff` renders. Order: first-touched first.
    pub fn changed_files(&self) -> Result<Vec<ChangedFile>, String> {
        // Walk oldest to newest, keeping the FIRST previous state and the
        // LAST new state per path. The "new" state of a step is the next
        // step's previous, or unknown for the newest step (the live file).
        let steps = self.undo_steps()?;
        let mut order: Vec<String> = Vec::new();
        let mut first_previous: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();
        let mut last_previous: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();
        for step in &steps {
            if !first_previous.contains_key(&step.path) {
                order.push(step.path.clone());
                first_previous.insert(step.path.clone(), step.previous.clone());
            }
            last_previous.insert(step.path.clone(), step.previous.clone());
        }
        Ok(order
            .into_iter()
            .map(|path| {
                let first = first_previous[&path].clone();
                let last = last_previous[&path].clone();
                (path, first, last)
            })
            .collect())
    }
}

/// Renders one changed file as a unified-ish before/after diff, bounded per
/// file. `current` is the live file contents, read by the caller.
pub fn render_diff(path: &str, previous: &Option<String>, current: Option<&str>) -> String {
    let before = previous.as_deref().unwrap_or("(file did not exist)");
    let after = current.unwrap_or("(unreadable)");
    let mut out = format!("--- {path} (before)\n+++ {path} (after)\n");
    let mut lines = 0usize;
    for line in before.lines() {
        if lines >= 40 {
            out.push_str("... (before truncated at 40 lines)\n");
            break;
        }
        out.push_str(&format!("-{line}\n"));
        lines += 1;
    }
    lines = 0;
    for line in after.lines() {
        if lines >= 40 {
            out.push_str("... (after truncated at 40 lines)\n");
            break;
        }
        out.push_str(&format!("+{line}\n"));
        lines += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "minicon-harness-journal-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        root.canonicalize().expect("canonical fixture root")
    }

    #[test]
    fn append_and_read_round_trip() {
        let root = fixture_root("round-trip");
        let journal = Journal::for_root(&root);
        journal.append(&Record::Task("build it".into())).unwrap();
        journal
            .append(&Record::Wrote {
                path: "a.txt".into(),
                previous: None,
            })
            .unwrap();
        journal.append(&Record::Answer("built".into())).unwrap();
        let records = journal.read_all().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0], Record::Task("build it".into()));
        assert_eq!(
            records[1],
            Record::Wrote {
                path: "a.txt".into(),
                previous: None
            }
        );
    }

    #[test]
    fn corrupt_line_is_a_named_error_not_a_fresh_start() {
        let root = fixture_root("corrupt");
        let journal = Journal::for_root(&root);
        journal.append(&Record::Task("fine".into())).unwrap();
        let mut raw = std::fs::read_to_string(&journal.path).unwrap();
        raw.push_str("{not json}\n");
        std::fs::write(&journal.path, raw).unwrap();
        let error = journal.read_all().unwrap_err();
        assert!(error.contains("not JSON"), "{error}");
    }

    #[test]
    fn undo_steps_bounded_to_twenty() {
        let root = fixture_root("bound");
        let journal = Journal::for_root(&root);
        for index in 0..30 {
            journal
                .append(&Record::Wrote {
                    path: format!("f{index}"),
                    previous: None,
                })
                .unwrap();
        }
        let steps = journal.undo_steps().unwrap();
        assert_eq!(steps.len(), UNDO_MAX_STEPS);
        assert_eq!(steps[0].path, "f10", "oldest dropped first");
        assert_eq!(steps.last().unwrap().path, "f29");
    }

    #[test]
    fn resume_context_carries_task_answer_pairs() {
        let root = fixture_root("resume");
        let journal = Journal::for_root(&root);
        journal.append(&Record::Task("t1".into())).unwrap();
        journal.append(&Record::Answer("a1".into())).unwrap();
        journal
            .append(&Record::Wrote {
                path: "x".into(),
                previous: None,
            })
            .unwrap();
        journal.append(&Record::Task("t2".into())).unwrap();
        let context = journal.resume_context().unwrap();
        assert!(context.contains("Previous task: t1"));
        assert!(context.contains("Previous answer: a1"));
        assert!(context.contains("Previous task: t2"));
        assert!(!context.contains("x"), "file records stay out of context");
    }

    #[test]
    fn diff_renders_before_after_with_bound() {
        let diff = render_diff(
            "f.txt",
            &Some("one\ntwo\n".into()),
            Some("one\nTWO\nthree\n"),
        );
        assert!(diff.contains("--- f.txt (before)"));
        assert!(diff.contains("+++ f.txt (after)"));
        assert!(diff.contains("-two"));
        assert!(diff.contains("+TWO"));
        let long = "x\n".repeat(60);
        let diff = render_diff("big", &Some(long.clone()), Some(&long));
        assert!(diff.contains("(before truncated at 40 lines)"));
    }

    #[test]
    fn changed_files_first_previous_last_previous() {
        let root = fixture_root("changed");
        let journal = Journal::for_root(&root);
        journal
            .append(&Record::Wrote {
                path: "a".into(),
                previous: None,
            })
            .unwrap();
        journal
            .append(&Record::Edited {
                path: "a".into(),
                previous: "v1".into(),
            })
            .unwrap();
        journal
            .append(&Record::Edited {
                path: "a".into(),
                previous: "v2".into(),
            })
            .unwrap();
        journal
            .append(&Record::Wrote {
                path: "b".into(),
                previous: None,
            })
            .unwrap();
        let changed = journal.changed_files().unwrap();
        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].0, "a");
        assert_eq!(changed[0].1, None, "first previous of a");
        assert_eq!(changed[0].2, Some("v2".into()), "last previous of a");
        assert_eq!(changed[1].0, "b");
    }
}
