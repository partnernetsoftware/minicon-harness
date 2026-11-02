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
│   ├── [v] lists top-level windows: hwnd, pid, rect, minimized/maximized/focused, class, title
│   ├── [v] optional `window` filter (substring), refused when it matches nothing
│   ├── [v] deterministic order: focused first, then top-left to bottom-right
│   └── [v] `capture`: true → whole desktop, or a window title → that window;
│           one PNG published under .harness/capture, path composed by the tool
└── op=act [v] — exactly one operation per call, `why` required
    ├── [v] focus        window
    ├── [v] move         window, x, y, width, height   (verified by readback)
    ├── [v] click        x, y, button?, double?
    ├── [v] type         text
    ├── [v] key          keys ("ctrl+s", "win+left")
    └── [v] scroll       x, y, delta
```

Bounds, each a named constant beside the code that enforces it:

| Constant | Value | What it bounds |
| --- | --- | --- |
| `SCREEN_TOOL_MAX_WINDOWS` | 64 | windows in one observe result (it becomes a prompt) |
| `SCREEN_TOOL_MAX_CAPTURE_BYTES` | 16 MiB | one PNG on disk (it never enters the prompt) |
| `SCREEN_TOOL_MAX_TYPE_CHARS` | 4096 | characters one `act:type` delivers |
| `SCREEN_TOOL_MOVE_TOLERANCE_PX` | 4 | readback slack before a move counts as not performed |

Three invariants the tool keeps, all inherited from PRD 38's CU0..CU2:

1. **No second permission layer.** `--allow-screen` is the whole grant. There is
   no per-action allow-list, no policy engine, no prompt.
2. **The tool is always advertised.** An ungranted `screen` answers every call
   with a refusal naming the flag, so a model cannot tell "not granted yet" from
   "tool absent" by probing -- the same rule `exec` already follows with an
   empty `--allow-cmd`.
3. **A refusal is a tool result.** Nothing here aborts the run.

## Evidence

- `[v]` `cargo test`: 44 passed, 1 ignored (the real-desktop test, below).
- `[v]` `cargo test -- --ignored` on this Windows machine: observe listed the
  live desktop, wrote a 5,293,518-byte PNG (1680x1050) and moved a real window.
- `[v]` the PNG was opened by an **independent** decoder (Pillow): `PNG
  (1680, 1050) RGB`, and the crop renders real UI text -- so the store-mode
  encoder is not just structurally valid.
- `[v]` the move readback check fired against a real maximized window
  (`800x600` kept vs `720x520` asked) instead of reporting success.

## Gaps, stated rather than papered over

| Gap | Consequence today | Candidate decision |
| --- | --- | --- |
| `[-]` no accessibility tree | observation is pixel-level; a text-only model can list windows and capture pixels but cannot read a control's name, role or value | the only real options are (a) hand-write the UIA subset in this crate (~400-600 lines of `unsafe` COM), or (b) take `minicon-platform` as an optional feature on qualified cells only. Both change a stated invariant; owner decision required |
| `[-]` Windows only | macOS/Linux cells compile and refuse by name | qualify one journey per OS before claiming it, per PRD 38 |
| `[-]` no vision input path | a capture is a local artifact; the model reads its path, not its pixels | the wire codec is text-only by design; an image arm is a separate decision |
| `[_]` capture retention | captures accumulate under `.harness/capture` with no pruning | `--diff`/journal adjacency, or a per-invocation cap |

## Where this goes next

The reason to put computer use *inside* the worker rather than beside it is
that the loop, the bounds and the journal are already here: an observation is a
tool result, an operation is a tool result, and the same 16-turn / 64-call /
10-minute budget covers both. A super-agent that reads files, runs commands and
operates a desktop under one auditable budget is a smaller thing than three
agents under three budgets.

The ordering that follows from the gap table is: accessibility observation first
(it is what makes the tool useful to a text model), then per-OS qualification,
then an image arm in the wire codec.
