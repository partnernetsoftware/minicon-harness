# minicon-harness product requirements

minicon-harness is a bounded agent worker in one portable file: exactly three
tools, `file`, `exec` and `screen`, an OpenAI-compatible model loop, and no
installer, no bundled runtime and no permission-policy engine. `screen` is the
computer-use half: it observes the desktop or performs one bounded operation on
it, under the same rule as the other two -- the tool's own bounds are the whole
policy. Its contract, and the assessment of which sibling CU work was migrated,
is owned by `prd/PRD_01_03_computer_use.md`. Its packaging model
follows the minicon.com method of the sibling `partnernetsoftware/minicon`
repository: native cells per OS/ISA, a Cosmopolitan fat-trampoline
`minicon-harness.com` with a ZipOS overlay, and a six-cell native execution
court before anything is published.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Product outcome

A script or another program hands one narrowly scoped local file/exec task to
a model and gets the final answer on stdout, in one bounded invocation, from
a single executable that runs on Windows/macOS/Linux x86_64 and ARM64.

```text
minicon-harness — one-file bounded agent worker
├── Worker core (this repo, crates-free Rust)
│   ├── [v] file tool: root-confined read/write, no .. or symlink escape,
│   │   256 KiB bounded reads, atomic publish-by-rename
│   ├── [v] exec tool: allow-listed basename argv, no shell, env allow-list,
│   │   30 s kill, 1 MiB bounded combined capture with marker
│   ├── [v] screen tool: observe (window inventory + one PNG under the root)
│   │   or act (focus/move/click/type/key/scroll, one per call, `why`
│   │   required), granted per invocation by --allow-screen, Windows
│   │   qualified, other platforms refuse by name
│   ├── [v] bounded loop: 16 model turns, 64 tool calls, 10-minute deadline;
│   │   refusals are tool results, never aborts
│   └── [v] OpenAI-compatible codec over ureq; no redirects, 4 MiB bounded body
├── [v] CLI: --root --task --base-url --model --allow-cmd --key-env
│   └── [v] --version banner (exact LF literal, court-verified)
├── 0.0.2 frontier (shipped, learned from unisacc/csih)
│   ├── [v] file gains an edit op: exact-match replace, 0 hits fail /
│   │   2+ hits fail / exactly 1 replaces, atomic write-back (edit.cx rule)
│   ├── [v] exec gains a required `why` field, echoed as the one-line
│   │   tool summary the caller reads first
│   ├── [v] structured tool status: success comes from the tool's own rc
│   │   (exec exit code, write really landed), never from a returned ok
│   ├── [v] retry with backoff on 429/5xx (2s/5s, max 2), 401 never retried,
│   │   in-flight cancel stays possible
│   ├── [v] session journal: one jsonl per --root, /resume replays it as
│   │   prior context (csih owned-session, minus the takeover protocol)
│   ├── [v] /undo for file writes+edits (bounded 20 steps), /diff to show
│   │   what this invocation changed
│   └── [v] CSIH.md-style project file: HARNESS.md at --root joined to the
│       system prompt (8KB codepoint-safe cap)
├── Delivery: minicon-harness.com → prd/PRD_01_01_com_delivery.md
├── Windows signing: company Authenticode → prd/PRD_01_02_windows_signing.md
├── Computer use: the `screen` tool → prd/PRD_01_03_computer_use.md
│   └── domain model (KVM + window target) and resource boundary
│       → prd/PRD_01_08_computer_use_scope.md
├── 0.0.3 frontier (signed releases)
│   ├── [-] S1 signing qualification on this repository (owner fills 3
│   │   secrets, one qualification_only run, receipt read back)
│   ├── [-] S2 release-policy signing.mode → required (owner flip)
│   ├── [-] S3 release.yml consumes the signed chain: mode=required
│   │   publishes only receipt-verified signed bytes; mode=off keeps the
│   │   unsigned path
│   ├── [-] S4 Defender court scans the exact published bytes, clean
│   │   verdict or the release blocks
│   ├── [-] S5 release automation repaired: v0.0.3 published by CI alone
│   │   (403 diagnosed or routed via a PAT secret)
│   └── [_] S6 docs court: README/README.zh cover 0.0.2 flags + signing
│   ├── [v] six native cells (win/osx/lnx × x86_64/aarch64), release profile
│   ├── [v] cosmocc APE trampoline + ZipOS /zip/cells overlay (5.62 MB packed)
│   ├── [v] Authenticode signability padding (unsigned 0.0.1; slot reserved)
│   └── [v] GitHub CI pack + six-cell execute court + public Release v0.0.1
└── [no] plugin host, permission policy, streaming, MCP, a scripting surface
    over the desktop, a second model loop
    (sessions enter 0.0.2 as plain journals; the takeover/owner-lock
    protocol stays out; `screen` exposes eight bounded operations, not an
    automation language)
    authority. The model API cannot execute tools; the tools cannot reach the
    model API. Scope extensions need a new owner decision here.
```

## Governing invariants

- The three tools' own bounds are the whole policy. There is no second
  permission layer, and a refusal is information returned to the model.
- An operation on the desktop is performed once and read back; a system that
  keeps its own geometry produces a failure that names both rectangles, never a
  success that reports the request.
- Every bound is a named constant next to the code that enforces it.
- One invocation, one bounded task, one final answer on stdout. The process
  exits; no service identity, no listener.
- The `.com` artifact is a pack of already-qualified native cells. It never
  recompiles the worker against cosmopolitan libc, and cells are never
  rebuilt on a test runner.
- Computer use operates one local interactive session: keyboard, video, mouse
  and the window/control target that binds them. Host resources -- processes,
  memory, devices, network, storage -- are not computer use here; they belong
  to `exec`/`file` or are explicit non-goals (`prd/PRD_01_08_computer_use_scope.md`).

## What computer use means here

Added 2026-11-05. The owner's model is **KVM plus resource management**
(window, process, memory, hardware). This repository carries a deliberately
narrow slice of it: the bounded operation of one local interactive session --
read its video output, send keyboard and mouse input, and bind those effects
to a verified window/control target. Process, memory, device and host-resource
management are **outside** computer use here: `exec` owns processes, `file`
owns storage, `minicon-winmgr` owns window-manager product behavior, and the
rest is `[no]`. The domain model, the resource-boundary table and the CU
document branch are owned by `prd/PRD_01_08_computer_use_scope.md`; the tree
below remains the single index of feature status.

How to read the tree: **A and B are the domain axes** (what the desktop can
observe and what it can do); **C, D, E and F are cross-cutting layers** (how a
call is admitted, looped, compiled and evidenced); **G is the boundary** (what
computer use is not). Counting C-F as domain coverage would inflate the
product's scope on paper.

## CU capability tree (benchmarked against `prd/prd-cu-openai.md`)

Added 2026-11-05. This is the **single index of computer-use features**: where
a capability lives, who owns its contract, and which version may implement it.
Legend as above: `[v]` delivered with evidence, `[-]` partial/open, `[_]`
planned, `[no]` explicit non-goal. A `[_]` means a chapter exists, not that
code exists. The external baseline is `prd/prd-cu-openai.md` (OpenAI Responses
API `computer` tool, Codex, ChatGPT agent/Work); raw evidence is kept outside
the repo in a temporary corpus. No OpenAI field name is imported by this tree.

```text
minicon-harness computer use
├── A Observation
│   ├── [v] A1 pixel arm: virtual screen, displays, window inventory, PNG
│   │        captured under the task root; desktop/window/region, scale
│   │        owner PRD 01.03 / 01.06
│   ├── [v] A2 typed Win32 child controls: label, class-derived role, rect,
│   │        text, state; deliberately NOT a UIA tree   owner PRD 01.05
│   ├── [v] A3 foreground title, multi-monitor negative coordinates,
│   │        bounded capture arithmetic                 owner PRD 01.06
│   ├── [_] A4 accessibility trees: macOS AX, Linux AT-SPI, Windows UIA COM
│   │        unlock: per-app TCC grant on macOS; AT-SPI D-Bus bus + ATK
│   │        fixture on Linux; each green on real CI before declaring true
│   │        target 0.0.7+ or [no]                      owner PRD 01.06
│   └── [_] A5 image arm in the wire codec: base64 PNG to the model each turn
│            unlock: separate PRD + budget/retention decision
│            target 0.0.7 candidate; [no] in 0.0.6
├── B Actuation
│   ├── [v] B1 pointer: hover, click, drag, scroll + pointer readback
│   ├── [v] B2 keyboard: type, key
│   ├── [v] B3 window: focus, move + rect readback (Linux/Windows)
│   ├── [v] B4 named semantic control: resolve + hover/press (Windows only)
│   ├── [_] B5 actuation receipt: button, downs/ups, double, target, plus
│   │        post-observe live session                 target 0.0.6 W1/W2
│   ├── [_] B6 multi-point drag path + held-key modifiers
│   │        target 0.0.7 (OpenAI `drag.path[]`, `keys`)
│   ├── [_] B7 two-axis scroll, explicit bounded wait
│   │        target 0.0.7 (OpenAI `scroll_x/scroll_y`, `wait`)
│   ├── [_] B8 batched `actions[]` with ordered receipts
│   │        target 0.0.7+; needs its own admission/stale design
│   └── [no] B9 browser-navigation buttons (`back`/`forward`/`wheel` as a
│            button enum): desktop CU has no browser history to navigate
├── C Contract and safety
│   ├── [v] C1 `cu-abi 1.0`, CAPABILITIES, disclose, six typed verdicts
│   ├── [v] C2 grant (`--allow-screen`), required `why`, task-root bounds
│   ├── [v] C3 generation / `as_of` pin / stale refusal / receipts / evidence
│   ├── [v] C4 per-OS capability matrix and named refusals
│   ├── [_] C5 `pending_safety_checks` + acknowledged checks + human hand-off
│   │        target 0.0.7; a policy PRD before any code
│   ├── [_] C6 untrusted-screen-content policy (prompt injection)
│   │        target 0.0.7; distinct from grant/admission
│   └── [no] C7 takeover / owner-lock / watch mode (top-level non-goal)
├── D Loop and state
│   ├── [v] D1 one bounded tool call -> one verdict -> one evidence record
│   ├── [_] D2 screenshot-per-turn loop with a model-visible image (needs A5)
│   ├── [_] D3 session continuity beyond `as_of` (OpenAI uses
│   │        `previous_response_id`; it does NOT restore login/runtime state)
│   └── [_] D4 bounded iterations / time / cost ceilings
├── E Platform and environment
│   ├── [v] E1 three real backends: Win32, CG/CF, X11/XTest via dlopen
│   ├── [v] E2 six cells cross-compiled on one runner, no link-time platform lib
│   ├── [_] E3 environment enum + display width/height in `disclose`
│   │        target 0.0.7; geometry is already in the pixel arm
│   ├── [_] E4 OS-grant matrix in evidence (Screen Recording, Input
│   │        Monitoring, Accessibility)              target 0.0.7
│   └── [no] E5 Wayland-native protocol, runtime backend selection
├── F Verification and delivery
│   ├── [v] F1 CI compile gate + three-OS real-desktop journeys + independent
│   │        reader recomputing matrix, readback, PNG IHDR, generation
│   ├── [v] F2 public company-signed Release, Defender and post-publish court
│   └── [_] F3 app-level effect evidence via a self-hosted echo window
│            target 0.0.7+; unlock: side-effect-free fixture on three OSes
└── G Resource-domain boundary — NOT a capability axis; owner PRD 01.08
    ├── [no] process spawn / inspect / signal / kill / usage / maps / fds:
    │        `exec` owns processes; a window's `pid` is identity metadata
    ├── [no] memory, CPU, load, topology and hardware inventory
    ├── [no] network, disk and device management; displays and input devices
    │        are preconditions of K/V/M, not objects this tool manages
    ├── [no] window-manager product behavior (minicon-winmgr owns it);
    │        here W is target binding: focus/move + inventory + named control
    └── [no] widening `screen` into host control by adding one verb: a new
             tool/grant, its own threat model and its own readback semantics
             are required first (unlock conditions in PRD 01.08)
```

Two directions of the gap, kept separate on purpose:

- **We are ahead**: typed verdicts, actuation/readback evidence, capability
  matrix, named refusals, `as_of` stale protection, three-OS CI journeys and
  signed-release evidence. The OpenAI tool surface specifies none of these.
  Aligning with it must never cost us one of them.
- **They are ahead**: a model-visible image arm, batched actions, explicit
  wait, two-axis scroll, multi-point drag, held keys, and a three-level
  human-confirmation/safety-check policy. These are candidates, not debts:
  each needs its own PRD, gate and owner decision before it enters a version.

## 0.0.7 frontier (chapter stub, not planned)

Opened 2026-11-05 so the tree above has somewhere to put new work. **No plan,
no version bump, no code and no `[v]` claim exists here.** A `[_]` is a
chapter, **not** a lifted non-goal: anything marked `[no]` in an earlier
frontier stays `[no]` until an owner decision lifts it and a version plan
carries the gate. Candidate chapters, all `[_]`, with their unlock conditions:

```text
0.0.7 — TBD (chapters only)
├── [_] 0.7.1 image arm in the wire codec (A5, D2)
│        unlock: budget, retention and model-visible PNG format decision
├── [_] 0.7.2 safety checks and human confirmation (C5, C6)
│        unlock: a policy PRD defining risk classes, who acknowledges,
│        and how a refusal is recorded in evidence
├── [_] 0.7.3 richer actuation (B6, B7, B8)
│        unlock: 0.0.6 interaction closure green on three OSes
├── [_] 0.7.4 accessibility trees (A4)
│        unlock: OS grants and CI fixtures that can be accepted first-hand
├── [_] 0.7.5 environment and grant disclosure (E3, E4)
└── [_] 0.7.6 app-level effect evidence (F3)
         unlock: side-effect-free echo-window fixtures on three OSes
```

`[no]` carried forward from every earlier frontier: plugin host, permission
policy engine, streaming, MCP, a scripting surface over the desktop, a second
model loop, takeover/owner-lock, any new crate or runtime, bun/WASM/ujs
embedding (PRD 31 readiness stays BLOCKED), runtime backend selection, and
host-resource management (G in the tree above; `PRD_01_08` owns the boundary).

## 0.0.6 frontier (planned, not started)

[_] Interaction closure. 0.0.5 made observation and pointer movement real on
three OSes; it never drove `press=true`, `button=middle`, `button=right` or
`double=true` in a CI journey, and a successful click is currently evidenced
only by a pointer coordinate. `plan/plan-0.0.6-interaction-closure.md` is the
plan and `prd/PRD_01_07_interaction_closure.md` is the contract: a typed
**actuation receipt** (`button`, `downs`, `ups`, `double`, `target`) in
`evidence`, three-OS CI journeys for the side-effecting variants, a live
follow-up `observe` with a newer generation, and a public signed v0.0.6 built
by the same chain. The receipt proves a real button event was dispatched, not
that the target application's business action ran. `cu-abi 1.0`, the wire
inputs and the capability matrix (windows 12/12, linux 10/12, macos 8/12) are
unchanged.

[no] AX / AT-SPI / UIA trees and app-level state-change evidence stay out of
0.0.6: a GitHub hosted runner cannot self-grant macOS Accessibility, and the
Xvfb fixture has no AT-SPI bus or ATK bridge, so neither could be accepted
under the "walk the last step yourself" rule. Their unlock conditions, the
PRD 31 readiness gate (still BLOCKED), and the 0.0.5 release invariants that
must not regress are recorded in the plan. No source, version or Release
change accompanies this plan; the tree is still `0.0.5`. In the capability
tree above, 0.0.6 owns B5 and extends C3; the external baseline is
`prd/prd-cu-openai.md`, and the absorbed/deferred deltas are recorded in the
plan's "External benchmark" section.

## 0.0.5 frontier (shipped as a public signed Release)

[v] Computer use is no longer a Windows implementation plus one refusal stub.
`src/screen.rs` compiles exactly one `native` backend per `target_os`
(Windows, macOS, Linux, fallback), all exporting the same function set, and
publishes a `PlatformCapabilities` matrix. `CuHost` admits a verb only when
this compiled cell declares it; `disclose` publishes the matrix, the platform
and per-verb state, and a missing verb is a named `unsupported` refusal rather
than "v1 is Windows only". The store-mode PNG encoder, capture bounds and
readback rules moved above the cfg boundary so all three backends share them.
`prd/PRD_01_06_cross_platform_cu.md` is the contract;
`plan/plan-0.0.5-cross-platform-cu.md` is the design and the OS mechanism map.

[v] macOS and Linux now have real desktop backends, not refusals:
`CoreGraphics`/`CoreFoundation` (macOS) and `X11`/`XTest` (Linux), both
resolved at runtime with `dlopen`/`dlsym`. No link-time platform library was
added, because the six cells are cross-compiled on one macOS runner (zig for
Linux, xwin for Windows) and a `#[link(name="X11")]` would break that build.
Linux declares every verb except the control arm; macOS deliberately leaves
`act:focus` / `act:move` to the Accessibility API and the control arm to an AX
tree. Missing libraries, `$DISPLAY` and OS grants are named refusals, never a
black PNG or a silent no-op.

[v] CI is the test bench. `cu-platforms.yml` builds, tests and release-links
all three OSes (run 41972347018 green) and `scripts/cu-cfg-check.py`
type-checks the macOS, Linux and fallback configurations. `cu-journeys.yml`
run **41975233858** drove a real desktop on all three: Linux under Xvfb with a
titled X client (12 performed, including focus/move rect readback), macOS in
the runner GUI session (10 performed, including a PNG capture and Unicode
typing), Windows with a titled window (14 performed, including `act:control`).
`examples/cu_journey.rs` writes `cu-journey.json`; the independent reader
`scripts/cu-journey-verify.py` recomputes the matrix, pointer readbacks, PNG
IHDR and generation order from those records.

[v] `v0.0.5` is a public company-signed Latest Release
(https://github.com/partnernetsoftware/minicon-harness/releases/tag/v0.0.5),
packed and signed entirely in CI from source `45b13f1`: pack `41975912899`,
Candidate `41976776464`, signing `41976848591`, dry-run `41977023867`, publish
`41977124016`. Defender and the post-publish court are green. An anonymous
download of all 15 assets was re-hashed locally: the APE and both Windows
cells match the receipt `after_sha256`, and the downloaded APE prints
`minicon-harness 0.0.5`.

## 0.0.4 frontier (shipped as a public signed Release)

[v] The `screen` tool is implemented and qualified on Windows in this tree:
multi-display observe, desktop/window/region capture with scale-down, focus,
move, click, hover, drag, type, key, scroll -- every pointer and window action
verified by readback -- behind `--allow-screen`, std-only
(`prd/PRD_01_03_computer_use.md`).

[v] Computer use has a **base**, not just verbs: `src/cu.rs` freezes cu-abi 1.0
-- a discoverable capability manifest, typed inputs/outputs, observation
generations with pinned `as_of`, six verdicts (including `unknown`) and an
evidence receipt per call (`prd/PRD_01_04_cu_base.md`). `op=disclose` reports
the contract even without a grant. This is the host side of the future
`unisacc`/`ujs` dynamic-logic seam; no engine, WASM, bun or plugin host is
embedded or claimed.

[v] Observation is **semantic** as well as pixel-level, and one action is
addressed **by name**: `prd/PRD_01_05_semantic_controls.md`. `op=observe` with
`controls` scans one window's typed child controls (label, class-derived role,
text, rect, visibility, enabled), and `action=control` resolves a control from
the current generation and hovers/clicks its centre with a readback that must
land inside the control's rectangle. This is Win32 `EnumChildWindows` +
`WM_GETTEXT`, not a UIA tree; self-drawn UIs report zero controls and say why.
No new dependency, no new permission layer, cu-abi still 1.0.

Version files (`Cargo.toml`, `Cargo.lock`, `release-policy.json`, the APE
VERSIONINFO) now say **0.0.4**, following
`plan/plan-0.0.4-cu-semantic-observation.md`. The rule from the previous
iteration -- "bump only when a signed six-cell Candidate is cut" -- is
superseded by the owner's 2026-11-04 instruction to move the version: **source
version and published Release are now separate facts**. W6 (pack → candidate →
sign → defender → release) is CI's job, and it is now **done**: public
`v0.0.4` is Latest (`gh release view v0.0.4`: non-draft, non-prerelease,
published 2026-11-04T18:30:16Z, 15 assets). Chain: pack **41875365398**,
Candidate **41881243482**, company signing **41881370195** (attempt 3),
final all-green `release.yml` **41889579027** (verify, execute-linux,
publish, Defender, post-publish). The Defender court scans with
`MpCmdRun -File`, not the rejected `-Path` (`1eb379c`); the post-publish
court and an independent local re-download both confirm that the two
published Windows zips carry the company-signed cells matching the receipt's
`after_sha256`, not the unsigned `before_sha256`. Full evidence is in
`plan/plan-0.0.4-cu-semantic-observation.md`.

## 0.0.1 frontier (shipped)

[v] First public release shipped 2026-10-27: six cells packed into
`minicon-harness.com` (5,619,674 bytes, sha256 8ea3eb68...), each cell executed
on its own native GitHub runner against the packed APE bytes, the release
download re-hashed and executed post-publish, published as
`minicon-harness.com` + `.sha256` + `build-receipt.json` + `README.md` at
releases/tag/v0.0.1. Unsigned; the Authenticode slot is reserved so a later
signed Candidate does not move APE headers.
