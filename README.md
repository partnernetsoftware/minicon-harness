# minicon-harness

Minimal bounded agent harness in Rust, in the spirit of pi-agent: the whole
agent is a model loop with exactly **three tools** -- `file`, `exec` and
`screen`. No plugin host, no permission-policy framework -- the tools' own
bounds (the `--root` path, the `--allow-cmd` list, the `--allow-screen` grant)
are the whole policy.

`screen` is the computer-use half: `op=observe` reports the virtual desktop
geometry, every display (including negative-coordinate secondaries) and all
top-level windows, and can write one PNG under the task root -- whole desktop,
one window, or one explicit region, with automatic scale-down on large
screens. `op=act` performs exactly one operation (focus, move, click, hover,
drag, type, key, scroll) and reads the desktop back: pointer position, window
rectangle and foreground window are evidence, not echo. It is granted per
invocation and refuses by name when not granted or when the platform was never
qualified. Contract and migration notes:
[`prd/PRD_01_03_computer_use.md`](prd/PRD_01_03_computer_use.md).

Migrated from the proven prototype in `minicon` (`src/harness.rs` +
`src/harness_wire.rs`), now a standalone std-first crate with no platform-crate
dependency.

## Layout

- `src/tools.rs` -- `FileTool` (root-confined read/write) and `ExecTool`
  (allow-listed argv execution, no shell).
- `src/screen.rs` -- `ScreenTool`: display/window observation, store-mode PNG
  capture with box downsampling (hand-written encoder), and input/window
  operations through hand-written Windows FFI. No crate: `user32`/`gdi32`
  bindings only.
- `src/wire.rs` -- one `Transport` seam, an OpenAI-compatible
  chat-completions codec, and the bounded turn loop.
- `src/lib.rs` -- invocation budget and the harness entry point.

## Bounds

- 16 model turns / 64 tool calls / 10-minute deadline per invocation.
- `file`: 256 KiB reads; relative paths only; `..`, absolute paths and
  symlinks refused, never rewritten.
- `exec`: one allow-listed basename per call, argv vector only, no shell;
  30 s per call, 1 MiB captured output.
- `screen`: 64 windows per observation, 16 MiB per capture (scale 1..8, chosen
  automatically when 0), 12 retained captures, 8192 px per region side, 4096
  characters per `type`, 4 px of readback slack before a window move or a
  pointer action counts as not performed.

## Usage

```bash
export HARNESS_API_KEY=sk-...
cargo run -- --root ./playground --task "write hello.txt and show it" \
    --allow-cmd ls --allow-cmd cat

# computer use, explicitly granted for this invocation only
cargo run -- --root ./playground --allow-screen \
    --task "list the windows, capture the desktop, then put the Grok Bot window at 0,0 800x600"
```

Captures land under `<root>/.harness/capture/`; the model is given the relative
path, never a path it chose.
