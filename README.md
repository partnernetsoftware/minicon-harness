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

**Grant and environment disclosure (0.0.9).** `op=disclose` now answers "why
did that click not land" before any click is attempted. `evidence["environment"]`
reports `platform`, `session` (`gui` / `remote` / `headless` / `unknown`),
`display_count`, `virtual_desktop` and `foreground_present`.
`evidence["grants"]` is one typed row per probe -- `name`, `status`, optional
`reason` -- with `status` drawn from a four-value whitelist: `granted`,
`denied`, `unknown`, `not_applicable`. `unknown` and `not_applicable` always
carry a non-empty `reason`; rows are sorted by `name` and unique, and the probe
set is capped (8 rows, 160 characters per reason). Every probe is a **read-only
preflight** -- `CGPreflightScreenCaptureAccess`, `CGPreflightPostEventAccess`,
`AXIsProcessTrusted` on macOS, `XOpenDisplay` / `libXtst` resolution on Linux,
the foreground window and input-desktop state on Windows. It never requests a
permission, never raises a TCC prompt, never elevates, and the `…WithOptions`
variants are deliberately not called. This is additive evidence on the existing
`disclose` capability: no new capability row, `cu-abi 1.0` and the 12/10/8/0
matrix unchanged, and it works **even when computer use was not granted**. A
malformed probe set publishes no partial disclosure at all (`not_performed`).
Contract: [`prd/PRD_01_11_grant_disclosure.md`](prd/PRD_01_11_grant_disclosure.md).

**Display scale / coordinate-basis disclosure (0.0.10).** `op=disclose` now
publishes `evidence["environment"]["display_scales"]`, one row per display
with `index`, `logical_rect`, `physical_rect`, `scale` and an optional
`reason`, and every `observe` display carries the same `scale`. `scale` is a
bounded typed set -- `1`, `1.25`, `1.5`, `2`, `3` or `unknown` -- and a cell
that cannot probe safely publishes `unknown` with a non-empty `reason` naming
the OS and the failed symbol, never a plausible `1`. For a known scale
`physical_rect` equals `logical_rect * scale` in exact integer arithmetic;
rows are bounded (16 rows, 160-character reasons), indexed from zero and
unique. Every probe is **read-only preflight**: `GetDpiForSystem` /
`GetDpiForWindow` / `GetSystemMetricsForDpi` on Windows, `backingScaleFactor`
on macOS, a read-only `Xft.dpi` / `XRRScreenResources` query on Linux, and one
named `unknown` row on the fallback. No display mode, DPI setting, DPI
awareness or OS grant is changed, no permission is requested and no dialog
appears. This is additive evidence on the existing `disclose` capability: no
new capability row, `cu-abi 1.0` and the 12/10/8/0 matrix unchanged, and it
works **even when computer use was not granted**. A malformed scale set
publishes no partial disclosure at all (`not_performed`). Contract:
[`prd/PRD_01_13_display_scale_disclosure.md`](prd/PRD_01_13_display_scale_disclosure.md).

**Structured control disclosure (0.0.11).** `op=observe` gains an optional
`a11y` arm: `true` scans the foreground window, an integer scans that live
`hwnd`. `evidence["a11y_controls"]` publishes a bounded, read-only tree with
`name`, MSAA `role`, coarse `role_name`, `rect` in virtual-desktop
coordinates, `depth` and `child_id`, plus `a11y_source`, `a11y_reason`,
`a11y_truncated` and `a11y_role_basis`. This exists because the typed Win32
`controls[]` arm reports only one `Chrome_RenderWidgetHostHWND` for a
Chromium window, forcing a screenshot-and-vision fallback. Windows uses
hand-written MSAA/IAccessible COM (no new crate, no OS grant, no elevation);
Linux, macOS and the fallback publish an empty tree with a named reason until
AT-SPI / AX trees have their own grants and fixtures. Additive evidence on
`observe`: `cu-abi 1.0` and the 12/10/8/0 matrix are unchanged. Contract:
[`prd/PRD_01_15_structured_control_disclosure.md`](prd/PRD_01_15_structured_control_disclosure.md).
`v0.0.11` is a public signed Release accepted from the owner's view.

**Handle-addressed focus (0.0.12).** `act:focus` gains an optional `hwnd`
from `observe.windows[]`, mutually exclusive with `window`; a stale handle is
a named refusal. Focus evidence and readback publish `target_hwnd` read from
the live foreground window, and `actuation` stays `null` because no button
event was posted. Four backends expose `window_by_handle`; `cu-abi 1.0` and
the 12/10/8/0 matrix are unchanged. `v0.0.12` is a public signed Release
accepted from the owner's view. Contract:
[`prd/PRD_01_14_handle_addressed_targeting.md`](prd/PRD_01_14_handle_addressed_targeting.md).

**Bounded observation wait (0.0.13).** `op=observe` gains an optional
`wait_ms` (1..=2000): one bounded sleep before a fresh snapshot, then
additive `evidence["wait"]` with requested, monotonic elapsed, cumulative
and call-count values plus the new generation. Ceilings are 2000 ms per
call, 8000 ms cumulative and 8 calls per `CuHost`; every over-bound request
is a named refusal before any sleep. `disclose` publishes the remaining
budget. It is one sleep, not a polling loop or a model turn. This closes the
measured act/observation timing gap from the L1 trial. `cu-abi 1.0`, the
12/10/8/0 matrix and the G boundary are unchanged. `v0.0.13` is a public
signed Release accepted from the owner's view. Contract:
[`prd/PRD_01_16_bounded_wait.md`](prd/PRD_01_16_bounded_wait.md).
The 0.0.15 UIA probe verdict (kept verbatim as a W0 diagnostic record): every
COM slot and `RECT` conversion were verified on a real Windows desktop, but
the target WorkBuddy provider exposes no addressable (non-Pane) controls and a
cold-start zero-rectangle window, so the mandatory downgrade rule stopped the
UIA-first route before any shipped capability
([`plan/plan-0.0.15-uia-probe-and-routing.md`](plan/plan-0.0.15-uia-probe-and-routing.md),
[`examples/cu_probe_uia.rs`](examples/cu_probe_uia.rs)).

**A11y-addressed actuation (0.0.15, public signed Release).** W0 pass 7 then
measured the *MSAA* arm on that same client: 43 nodes, 8 buttons, 11 named
controls and 42 non-zero rectangles, while the Win32 `controls` scan is
refused. `act:control` gains optional `source` (`"controls"` default /
`"a11y"`) and `role`. The a11y arm resolves a normalised node by exact name
then unique substring, vetoes `unavailable`/`invisible`/`offscreen` states,
refuses ambiguity, unknown roles and an empty tree, pins the observation
generation, and requires the pointer readback to land **inside the resolved
node's rectangle**. Evidence publishes `source`, `a11y_node` (the 0.0.14
schema) and `a11y_node_schema`; the independent journey reader recomputes
`state_names`, the centre and containment. `press` still defaults to false,
so resolving a target cannot click. No new capability row, verb, grant,
crate, ABI bump, matrix cell or native signature. Tree:
[`prd/PRD_01_25_semantic_addressing_subtree.md`](prd/PRD_01_25_semantic_addressing_subtree.md),
plan: [`plan/plan-0.0.15-a11y-addressed-actuation.md`](plan/plan-0.0.15-a11y-addressed-actuation.md).
The UIA W2 stays stopped; the route switch is a dated controller decision
(`PRD_01_24`). `v0.0.15` is the latest public signed Release.

**A11y effect readback (0.0.16, public signed Release).**
[`plan/plan-0.0.16-a11y-effect-readback.md`](plan/plan-0.0.16-a11y-effect-readback.md)
is the 6/6-complete plan this version implements. `observe` gains an additive
`effect` object: a **pre-declared** `expect` whitelist is compared with a
recomputed `observed` set from the same a11y node in two published frames,
yielding `confirmed` / `unchanged` / `mismatch` / `vanished` or a named
refusal. The `before` frame is read *before* `state.frame` is overwritten and
bound to the dispatch generation (TOCTOU); identity is name + role + child_id
+ depth, never a coordinate. W0 pass 8 plus a real-desktop X-grade probe
falsified the v1 "a non-empty `changed` means success" contract (43
identity-stable nodes, zero differences, no editable `accValue` on the
WorkBuddy window), so `unchanged` and `mismatch` are first-class honest
verdicts. No native API, OS grant, event subscription, image arm, new verb,
capability row, `cu-abi` bump or matrix cell is added. W1/W2: 133 unit tests,
the four-configuration type gate, a real-desktop probe (`unchanged` gen 1→2,
`mismatch` gen 2→3) and two three-OS journey steps whose independent reader
recomputes `state_names`, `observed` and the verdict. All four version sources
read `0.0.16`. `v0.0.16` is a public signed Release accepted from the owner's view
(2026-11-07); the pack's size exceeded the old 6 MiB stamp after honest trims
and the ceiling was raised to 6.25 MiB by the dated owner decision in
[`prd/PRD_01_01_com_delivery.md`](prd/PRD_01_01_com_delivery.md).

**Native default action (0.0.17, public signed Release).**
[`plan/plan-0.0.17-native-default-action.md`](plan/plan-0.0.17-native-default-action.md):
`act:control` gains `invoke:true` (a11y arm only, mutually exclusive with
`press`) — the resolved node's MSAA `accDoDefaultAction` is dispatched **by
identity** to the object located in the live tree, with no pointer and no
window activation; evidence publishes the raw `hresult` and no actuation
receipt. The 0.0.16 effect whitelist gains the **`tree`** dimension: a
recomputable identity-set diff of the two frames' trees, because the W1
probe showed an Electron invoke's real effect (a panel opening, 43 → 48
nodes) is observable in the tree, not in the target node's own state. A
real-desktop X-grade loop verified `对话内搜索` → S_OK → tree 43→48 →
`confirmed observed:["tree"]` → Esc restore. `v0.0.17` is the latest public
signed Release, accepted from the owner's view (2026-11-08).

**A11y text write (0.0.18, public signed Release).**
[`plan/plan-0.0.18-a11y-text-write.md`](plan/plan-0.0.18-a11y-text-write.md):
`act:control` gains `set_value` (a11y arm only, exclusive with
`press`/`invoke`) — the resolved node's value is written via MSAA
`put_accValue` **by identity**, with readonly/protected refusals before any
native call and no pointer involvement. The W1 probe proved the write is
**asynchronous on Chromium**, so verification is the 0.0.16 effect readback
with `expect:["value"]` on a later observation, never the HRESULT alone; the
real-desktop loop confirmed both the honest subset `mismatch` and the
full-set `confirmed`. `v0.0.18` is the latest public signed Release,
accepted from the owner's view (2026-11-08); with it the CU-only line freeze
is satisfied and the harness line (omp reference) becomes eligible.
**Line freeze (owner direction 2026-11-07):** computer use owns versions
through `v0.0.18`; the harness branch may be improved with omp (oh-my-Pi) as a
reference only after `v0.0.18` is published and accepted from the owner's view.

The PRD is now a multi-dimensional tree with per-branch subtree documents and
a dated W0 learning ledger: see the *Document tree* section of
[`PRD.md`](PRD.md) and
[`prd/PRD_01_17_frontier_radar.md`](prd/PRD_01_17_frontier_radar.md).
`v0.0.15` is a public signed Release accepted from the owner's view
([`prd/PRD_01_25_semantic_addressing_subtree.md`](prd/PRD_01_25_semantic_addressing_subtree.md)).

**Continuous frontier learning (W0).** Each version cycle starts by learning
current computer-use and resource-domain technology (K, V, M, W, S plus
process / CPU / memory / GPU / device / network) and **keeping dated learning
notes in the PRD**. Findings are folded into multi-dimensional trees (T1-T9)
whose large branches have their own indexed subtree documents.
**Implementation starts only after the plan is complete** (三思后行 /
分而治之); both 0.0.15 plans passed their 6/6 checklists before any code ran,
and the UIA probe's downgrade rule stopped that route without an owner choice.
W0 pass 7 then supplied X-grade evidence for the shipped route. A research
finding is not authority: no `[no]`, `cu-abi`, matrix row or G boundary
changes because an article exists. Ledger, evidence grades and candidate
table:
[`prd/PRD_01_17_frontier_radar.md`](prd/PRD_01_17_frontier_radar.md).
**Decision authority (T8)**: under `/goal` the controller decides version
topics, gates, ordering and the G boundary, and reports the reason; the owner
keeps a veto, not an approval queue
([`prd/PRD_01_24_decision_authority_subtree.md`](prd/PRD_01_24_decision_authority_subtree.md)).

Above the mechanisms sits a **versioned host capability contract**
(`src/cu.rs`, `prd/PRD_01_04_cu_base.md`): cu-abi 1.0, a discoverable
capability manifest, typed inputs/outputs, observation generations with
pinned `as_of`, six verdicts (`performed` / `not_performed` / `unknown` /
`refused` / `unsupported` / `stale`) and an evidence receipt per call.
`op=disclose` reports the ABI, grant, capabilities and receipts, plus the
read-only `environment` probe, the typed `grants[]` rows and the
`environment.display_scales[]` rows (with `observe.displays[].scale`) -- and
works even when computer use was not granted. It is the host-side seam for later
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
