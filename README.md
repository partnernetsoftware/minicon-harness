# minicon-harness

Minimal bounded agent harness in Rust, in the spirit of pi-agent: the whole
agent is a model loop with exactly **two tools**, `file` and `exec`. No plugin
host, no permission-policy framework -- the two tools' own bounds (the `--root`
path, the `--allow-cmd` list) are the whole policy.

Migrated from the proven prototype in `minicon` (`src/harness.rs` +
`src/harness_wire.rs`), now a standalone std-first crate with no platform-crate
dependency.

## Layout

- `src/tools.rs` -- `FileTool` (root-confined read/write) and `ExecTool`
  (allow-listed argv execution, no shell).
- `src/wire.rs` -- one `Transport` seam, an OpenAI-compatible
  chat-completions codec, and the bounded turn loop.
- `src/lib.rs` -- invocation budget and the harness entry point.

## Bounds

- 16 model turns / 64 tool calls / 10-minute deadline per invocation.
- `file`: 256 KiB reads; relative paths only; `..`, absolute paths and
  symlinks refused, never rewritten.
- `exec`: one allow-listed basename per call, argv vector only, no shell;
  30 s per call, 1 MiB captured output.

## Usage

```bash
export HARNESS_API_KEY=sk-...
cargo run -- --root ./playground --task "write hello.txt and show it" \
    --allow-cmd ls --allow-cmd cat
```
