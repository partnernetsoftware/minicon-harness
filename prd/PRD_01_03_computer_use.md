# PRD 01.03 — computer use, as the third tool

Owner of how this repository carries computer use: what the `screen` tool is
allowed to do, which parts of the existing company computer-use work were
migrated and which were deliberately left behind, and what is still not
delivered. Product framing: this worker is the bounded core of a harness that
ships computer use itself, rather than a harness that needs a desktop agent
next to it.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Migration assessment (2026-11-02)

Three sibling trees were read before a line was written here. The conclusion is
that **the contracts migrate and the implementations do not**, and the reason is
structural rather than a judgement about quality.

| Source | What it owns | Size | Verdict |
| --- | --- | --- | --- |
| `agenterm/crates/agenterm-cu` | the richest CU surface: verbs, targets (current/ssh/vnc/rdp), grants, receipts, browser bridge, PTY snapshots | 153,343 lines of Rust, 13 direct dependencies incl. `minicon-platform` (git), `libloading`, `regex`, `objc2`, `windows-sys`, `x11rb` | `[no]` not migratable as code |
| `minicon/prd/PRD_02_38_computer_use.md` + `plan/plan-minicon-cu.md` | the CU0..CU4 outcome tree: observe/act separated, target identity, staleness, readback, explicit safe failures | design only, every leaf `[_]` | `[v]` migrated as the invariant set |
| `minicon/crates/minicon-platform` | the actual native mechanisms (`input-inject`, `a11y-tree`, `device-capture`) that agenterm's ABI exports | large multi-OS platform crate, feature-gated | `[-]` pointed at, not linked |

Why `agenterm-cu`'s code stays out:

1. **It is a service, this is one process.** `agenterm-cu` owns a persistent
   runtime coordinator (`runtime_coordinator.rs`, 1,278 lines), a grant store,
   receipts and transports (SSH/VNC/RDP: 3,000+ lines). Its model is "a daemon
   holds authority, a caller asks it". This worker is "start, do one bounded
   task, print an answer, exit". Importing the coordinator would import a
   lifecycle the harness does not have.
2. **It loads its mechanisms from `libagenterm` at run time** (`dynlib.rs`,
   `LoadLibrary`/`dlopen`). That means a second artifact that must be versioned
   *and colocated* with the worker -- and `docs/cu-gaps-analysis.md` in agenterm
   records exactly that failure: a published CU binary whose library was
   missing reported `agt_input_send_keys missing`, which reads like a code
   defect and is a packaging one. This repository's whole premise is one file.
3. **Its dependency graph is incompatible with the delivery court.** The `.com`
   pack requires every cell to cross-compile from std plus `serde_json` and
   `ureq` (`PRD_01_01`). A `minicon-platform` git dependency, `objc2`,
   `x11rb` and `windows-sys` end that property for all six cells at once.

What was migrated from it, and is now in `src/screen.rs`:

- **observe and act are separate.** `op=observe` has no side effect; `op=act`
  performs exactly one operation and reads the desktop back.
- **an operation names its target.** `find` accepts an exact title, or a
  substring only when it matches exactly one window; ambiguous input is refused
  with the candidates named, never resolved by guess.
- **success is readback, not a return value.** A move is compared against the
  rectangle the desktop reports afterwards; outside
  `SCREEN_TOOL_MOVE_TOLERANCE_PX` it is a **failure** that names both
  rectangles. (This caught a real fake success during qualification: a
  maximized Electron window kept `800x600` and the first implementation said
  `ok`.)
- **an unqualified path refuses by name.** Non-Windows builds, and ungranted
  invocations, return a typed refusal naming the platform or the flag, never an
  empty "success".

What was deliberately not migrated, and why:

- the **grant store and receipts** (agenterm): the harness has one grant --
  `--allow-screen` -- and the journal already carries the invocation's record.
- the **remote targets** (ssh/vnc/rdp): a bounded worker with a 10-minute
  deadline is the wrong shape for a desktop bridge.
- the **accessibility tree** (see the gap table): it is the one capability
  whose absence limits the tool the most, and it needs a decision (below)
  rather than a quiet import.

## The `screen` tool

```text
screen
├── op=observe [v]
│   ├── [v] virtual desktop rect + one line per display (primary flag, negative
│   │       coordinates allowed), from SM_*VIRTUALSCREEN / EnumDisplayMonitors
│   ├── [v] lists top-level windows: hwnd, pid, rect, minimized/maximized/focused, class, title
│   ├── [v] optional `window` filter (substring), refused when it matches nothing
│   ├── [v] deterministic order: focused first, then top-left to bottom-right
│   └── [v] `capture`: true → whole virtual desktop, a window title → that window,
│           or {x,y,width,height} → one region; one PNG under .harness/capture
│           with path composed by the tool and the resulting size/scale reported
└── op=act [v] — exactly one operation per call, `why` required
    ├── [v] focus        window                          (live-handle + foreground readback)
    ├── [v] move         window, x, y, width, height      (rect readback, 4 px tolerance)
    ├── [v] click        x, y, button?, double?           (pointer readback)
    ├── [v] hover        x, y                             (pointer readback)
    ├── [v] drag         x, y, to_x, to_y, button?        (12 stepped moves + pointer readback)
    ├── [v] type         text                             (foreground before/after)
    ├── [v] key          keys ("ctrl+s", "win+left")     (foreground before/after)
    └── [v] scroll       x, y, delta                      (pointer readback)
```

Bounds, each a named constant beside the code that enforces it:

| Constant | Value | What it bounds |
| --- | --- | --- |
| `SCREEN_TOOL_MAX_WINDOWS` | 64 | windows in one observe result (it becomes a prompt) |
| `SCREEN_TOOL_MAX_CAPTURE_BYTES` | 16 MiB | one PNG on disk (it never enters the prompt) |
| `SCREEN_TOOL_MAX_CAPTURE_SCALE` | 8 | largest downsample factor; 0 = choose automatically |
| `SCREEN_TOOL_MAX_CAPTURE_FILES` | 12 | PNGs retained per task root |
| `SCREEN_TOOL_MAX_REGION_PIXELS` | 8192 | one side of a capture region |
| `SCREEN_TOOL_MAX_TYPE_CHARS` | 4096 | characters one `act:type` delivers |
| `SCREEN_TOOL_MOVE_TOLERANCE_PX` | 4 | readback slack before a move counts as not performed |
| `SCREEN_TOOL_POINTER_TOLERANCE_PX` | 4 | readback slack before a pointer action counts as not performed |

Four invariants the tool keeps, all inherited from PRD 38's CU0..CU2:

1. **No second permission layer.** `--allow-screen` is the whole grant. There is
   no per-action allow-list, no policy engine, no prompt.
2. **The tool is always advertised.** An ungranted `screen` answers every call
   with a refusal naming the flag, so a model cannot tell "not granted yet" from
   "tool absent" by probing -- the same rule `exec` already follows with an
   empty `--allow-cmd`.
3. **A refusal is a tool result.** Nothing here aborts the run.
4. **A handle is never trusted across calls.** Every operation re-resolves the
   title, then re-checks `IsWindow` and the live title before touching the
   handle; a closed or reused window is refused rather than acted on.

Readback covers the whole surface now, not only window moves:

| Operation | Evidence | Failure when |
| --- | --- | --- |
| move | `GetWindowRect` after `SetWindowPos` | requested vs reported rect differs by > 4 px |
| click / hover / drag / scroll | `GetCursorPos` after `SetCursorPos` | requested vs reported pointer differs by > 4 px |
| type / key | foreground title before and after `SendInput` | foreground disappeared; a changed title is reported, not refused (Enter can open a dialog) |
| focus | re-enumerated foreground window | window manager refuses the foreground request |
| capture | IHDR dimensions and scale in the result | PNG over the byte bound at the chosen scale |

Two consequences worth stating: a pointer request outside the virtual desktop
is refused *before* the OS can clamp it to a different point; and a requested
capture region is never silently clipped -- outside means refused, with the
desktop geometry named.

## Evidence

- `[v]` `cargo test --offline`: 52 passed, 1 ignored (the real-desktop test).
- `[v]` `cargo test --offline real_desktop -- --ignored --nocapture` on this
  Windows machine (2026-11-04): `desktop=0,0 1680x1050 displays=1`, 17 windows,
  a 5,293,518-byte desktop PNG, a 100x60 region PNG at scale 2, a hover pointer
  readback, an out-of-bounds click refusal and a move readback.
- `[v]` both PNGs were opened by an **independent** decoder (Pillow): `PNG
  (1680, 1050) RGB` with 9,386 distinct colours and `PNG (100, 60) RGB` with
  734 -- so the store-mode encoder and the box downsampler produce real pixels,
  not just structurally valid files.
- `[v]` the move readback check fired against a real maximized window
  (`800x600` kept vs `720x520` asked) instead of reporting success.
- `[v]` pure functions (`capture_scale`, `downsample`, `foreground_evidence`,
  retention pruning) are unit-tested on every cell, including the ones where
  computer use is unqualified. The whole production file was additionally
  type-checked in its non-Windows configuration: the six cells that refuse by
  name still compile against the same `native` surface.

## Gaps, stated rather than papered over

| Gap | Consequence today | Candidate decision |
| --- | --- | --- |
| `[-]` no accessibility tree | observation is pixel-level; a text-only model can list windows and capture pixels but cannot read a control's name, role or value | the only real options are (a) hand-write the UIA subset in this crate (~400-600 lines of `unsafe` COM), or (b) take `minicon-platform` as an optional feature on qualified cells only. Both change a stated invariant; owner decision required |
| `[-]` Windows only | macOS/Linux cells compile and refuse by name | qualify one journey per OS before claiming it, per PRD 38 |
| `[-]` no vision input path | a capture is a local artifact; the model reads its path, not its pixels | the wire codec is text-only by design; an image arm is a separate decision |
| `[v]` capture retention | -- | newest 12 tool-produced PNGs are kept per root; only names the tool composes are considered, other files are never touched |
| `[_]` pointer-path qualification for drag/middle | hover and the shared pointer readback are qualified on a real desktop; drag and middle-button press are not yet exercised against one | add them to the ignored real-desktop journey |

## Where this goes next

The reason to put computer use *inside* the worker rather than beside it is
that the loop, the bounds and the journal are already here: an observation is a
tool result, an operation is a tool result, and the same 16-turn / 64-call /
10-minute budget covers both. A super-agent that reads files, runs commands and
operates a desktop under one auditable budget is a smaller thing than three
agents under three budgets.

The pixel-side surface is now as complete as a single-file, std-only tool can
make it: multi-display geometry, three capture targets with bounded scaling,
eight actions and readback on every one of them. The ordering that follows from
the gap table is unchanged: accessibility observation first (it is what makes
the tool useful to a text model), then per-OS qualification, then an image arm
in the wire codec. The a11y route still needs the owner's A/B decision.
