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
screens. With `controls` it also lists one window's typed child controls
(label, class-derived role, text, rectangle, visibility, enabled state) -- see
[`prd/PRD_01_05_semantic_controls.md`](prd/PRD_01_05_semantic_controls.md);
this is Win32 child-window enumeration, **not** a UIA accessibility tree, and
a self-drawn UI reports zero controls and says why. `op=act` performs exactly
one operation (focus, move, click, hover, drag, type, key, scroll, control)
and reads the desktop back: pointer position, window rectangle and foreground
window are evidence, not echo. `action=control` addresses a control **by name**
from the last observation instead of by coordinate; the pointer readback must
land inside that control's rectangle. It is granted per invocation and refuses
by name when not granted or when the platform was never qualified. Contract
and migration notes:
[`prd/PRD_01_03_computer_use.md`](prd/PRD_01_03_computer_use.md).

**Scope.** Here computer use means KVM plus the window/control target that
binds an effect: one local interactive session, one effect per call, readback
that proves desktop state. Process, memory, device, network and storage
management are **not** part of `screen`: processes belong to `exec`, storage
to `file`, window-manager product behavior to `minicon-winmgr`, and the rest
is an explicit non-goal. Domain model and boundary:
[`prd/PRD_01_08_computer_use_scope.md`](prd/PRD_01_08_computer_use_scope.md).

**Richer actuation (0.0.7).** `action=drag` accepts a bounded `path[]` of
2..16 points: each segment is interpolated, the pointer readback must land at
the last point, and `evidence["actuation"]["path_points"]` reports how many
points the native backend actually traversed. `action=scroll` accepts
`delta_x`/`delta_y` within +-1440 and publishes them as
`evidence["scroll"]`; `delta` stays accepted as the vertical alias. Both are
additive wire parameters, not new capabilities: `cu-abi 1.0` and the 12-row
matrix are unchanged. Contract:
[`prd/PRD_01_09_richer_actuation.md`](prd/PRD_01_09_richer_actuation.md).

**Held-key modifiers (0.0.8).** `action=key` accepts `hold: true` with one
whitelisted modifier (`ctrl`, `shift`, `alt`, `win`/`cmd`) and `release: true`
to let the set go. `CuHost` owns the held set, `op=disclose` publishes
`evidence["held_keys"]`, and each flagged call publishes
`evidence["keyboard"]` with the state after the call. A foreground change, a
lost grant or host teardown releases everything before any further effect;
keyboard steps never carry a button actuation receipt. Contract:
[`prd/PRD_01_10_held_key_modifiers.md`](prd/PRD_01_10_held_key_modifiers.md).

Above the mechanisms sits a **versioned host capability contract**
(`src/cu.rs`, `prd/PRD_01_04_cu_base.md`): cu-abi 1.0, a discoverable
capability manifest, typed inputs/outputs, observation generations with
pinned `as_of`, six verdicts (`performed` / `not_performed` / `unknown` /
`refused` / `unsupported` / `stale`) and an evidence receipt per call.
`op=disclose` reports the ABI, grant, capabilities and receipts -- and works
even when computer use was not granted. It is the host-side seam for later
`unisa + ujs` dynamic logic; no script runtime is embedded.

Migrated from the proven prototype in `minicon` (`src/harness.rs` +
`src/harness_wire.rs`), now a standalone std-first crate with no platform-crate
dependency.

## Layout

- `src/cu.rs` -- `CuHost`: cu-abi 1.0, capability manifest, typed requests,
  generations, verdicts and evidence receipts. The only entrance to a desktop
  effect.
- `src/tools.rs` -- `FileTool` (root-confined read/write) and `ExecTool`
  (allow-listed argv execution, no shell).
- `src/screen.rs` -- `ScreenTool`: display/window observation, a typed child
  control inventory, store-mode PNG capture with box downsampling (hand-written
  encoder), the shared bounds/readback/actuation-receipt layer, and the
  compiled-cell backend selection. No crate.
- `src/screen_linux.rs`, `src/screen_macos.rs`, `src/screen_dynlib.rs` -- the
  three native backends: Win32 `user32`/`gdi32` in `screen.rs`, macOS
  `CoreGraphics`/`CoreFoundation` and Linux `X11`/`XTest` resolved at runtime
  with `dlopen`/`dlsym`; no link-time platform library. Each compiled cell
  declares its own capability matrix and refuses missing verbs by name.
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
  characters per `type`, 128 controls per scan (truncation reported), 256
  characters per control text, 200 ms per cross-process `WM_GETTEXT`, 4 px of
  readback slack before a window move or a pointer action counts as not
  performed, 32 retained CU receipts.
- `screen` actuation receipt: every button event reports `button`, `downs`,
  `ups`, `double`, a non-empty `target` and `path_points`; a missing receipt
  is `unknown`, a mismatched one is `not_performed`
  (`prd/PRD_01_07_interaction_closure.md`).

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
