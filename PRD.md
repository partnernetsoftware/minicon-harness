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
`[no]` explicit non-goal, `[study]` research note with no implementation
claim, `[decision]` a dated controller decision already taken
(`prd/PRD_01_24_decision_authority_subtree.md`). The owner keeps a veto, not
an approval queue; no document here asks the owner to choose.

## Document tree, W0 learning ledger and multi-dimensional views

Owner direction (2026-11-06 evening): keep learning the newest computer-use
and resource-management knowledge, **keep the learning process notes in the
PRD**, expand the PRD into **trees over several dimensions**, split large
branches into `*_subtree.md` documents and index them here, and **do not
start code or tests until the plan is complete** — 三思后行 (think three
times before acting) and 分而治之 (divide and conquer). This replaces the
earlier "implement next" pace for the current phase.

**Current phase: STUDY + STRUCTURE.** No W1/W2 code, no test, no version bump
and no Release chain is started for 0.0.15. Last shipped version is
`v0.0.14`; all four version sources read `0.0.14`. Every tree node below is
either `[v]` with three-OS evidence, `[study]`, `[_]`, `[no]` or `[decision]`;
a research finding never becomes a capability by itself.

### Document index (subtree documents)

```text
PRD.md                                     top index, trees, ledger, versions
prd/
├── PRD_01_08_computer_use_scope.md        K/V/M/W/S domain model + G boundary
├── PRD_01_12_self_hosted_orchestration.md  L0-L3 end state
├── PRD_01_17_frontier_radar.md            W0 learning ledger + candidate table
├── PRD_01_19_cu_perception_subtree.md     T1 observation / perception
├── PRD_01_20_cu_actuation_subtree.md      T2 actuation / input
├── PRD_01_21_resource_domain_subtree.md   T3 resource domain (largest branch)
├── PRD_01_22_platform_evidence_subtree.md T4 platform + T6 evidence
├── PRD_01_23_capability_maturity_subtree.md T5 maturity + T7 version state
├── PRD_01_24_decision_authority_subtree.md T8 who decides what (controller)
├── PRD_01_01 .. PRD_01_16, PRD_01_18      per-version contracts (unchanged)
└── prd-cu-openai.md                        external reference, NOT a contract
plan/
└── plan-0.0.15-uia-probe-and-routing.md   0.0.15 planning, NOT started
```

### T1 — perception (full tree: `PRD_01_19`)

```text
T1 perception
├── P1 pixel arm            [v] bounded PNG + scale disclosure; [no] image arm
├── P2 structured controls  [v] Win32 controls, MSAA tree, normalised schema
│                           [_] UIA routing (probe gate), AT-SPI/AX, events
├── P3 hybrid routing       [study] API -> tree -> pixels; [no] code-first here
├── P4 target identity      [v] hwnd focus, target_hwnd, stale refusal
├── P5 timing / freshness   [v] bounded wait + ceilings; [no] polling loop
└── P6 grounding            [study] resolution budget, bidirectional scaling
```

### T2 — actuation (full tree: `PRD_01_20`)

```text
T2 actuation
├── K keyboard    [v] type / key / held modifiers; [no] layout or IME claim
├── M mouse       [v] click receipt / drag path / two-axis scroll
├── W window      [v] title + hwnd focus, live re-validation
├── A grammar     [v] one effect, why, verdicts, receipts; [_] batched actions
├── C code-first  [study] emit-script-then-run; [no] in this repo
└── S safety      [v] UIPI/TCC named refusals; [_] injection defence, approval
```

### T3 — resource domain (full tree: `PRD_01_21`, all `[study]` / `[no]`)

```text
T3 resource domain                     NOT screen authority (G)
├── R1 process / pid / namespaces / pids.max
├── R2 CPU       EEVDF, cpu.max/weight, throttling, cpuset, PSI
├── R3 memory    max/high/low/min, oom.group, THP/HugeTLB, PSI
├── R4 GPU       MIG, SR-IOV+IOMMU, vGPU, MPS, CUDA_VISIBLE_DEVICES
├── R5 I/O       io.max/io.weight, io_uring, tmpfs, snapshots
├── R6 net/device eBPF, default-deny egress, metadata block, LSM
├── R7 isolation container -> gVisor -> microVM -> full KVM
├── R8 lifecycle  credential broker, three timeouts, audit, quotas, stop
├── R9 cost       PSI telemetry, 7.3x price spread, memory-dominant bills
└── R10 acceptance guest-side read/cap/cleanup probes
    └── [decision] (a) keep G closed — controller decision 2026-11-06, T8
```

### T4 — platform (full tree: `PRD_01_22`)

```text
T4 platform                             12 / 10 / 8 / 0 matrix
├── PL1 Windows  [v] EnumWindows, MSAA, SendInput; [study] UIA, UIPI, WGC
├── PL2 macOS    [v] 8/12, TCC boundaries, scale; [study] AX, bundle id
├── PL3 Linux    [v] 10/12, XTest, empty a11y tree; [study] AT-SPI2, portals
├── PL4 fallback [v] 0/12 with named reasons
└── PL5 contract [v] four cfgs, identical native signatures, type gate
```

### T5 — capability maturity (full tree: `PRD_01_23`)

```text
T5 A-F capability index -> T1/T2/T4/T6 subtrees
└── L0 read-only [v] -> L1 evidenced interaction [v]
    -> L2 self-sustained loop [_] gates: A5/D2, B7b/D4 [v], A4, F3
    -> L3 self-hosted orchestration [_] gates: L2 + F3 + C5/C6
```

### T6 — evidence (full tree: `PRD_01_22`)

```text
T6 evidence
├── E1 CI gates          [v] platforms + journeys + four-cfg type gate
├── E2 independent reader[v] producer never grades; no assertion weakened
├── E3 typed rows        [v] receipts, grants, scales, a11y schema, wait
├── E4 release chain     [v] receipts, sidecars, Authenticode, owner view
├── E5 external methods  [study] OSWorld 2.1 release contract, accept+reject
└── E6 evidence grades   [study] O / B / P / V / S / X; only X can be [v]
```

### T7 — version state (full tree: `PRD_01_23`)

```text
T7 versions
├── V0.0.1 - V0.0.14   [v] published, accepted signed Releases
├── V0.0.15            [_] plan complete 6/6, W1 eligible, not started
├── candidate pool      [study] ordered by gate, never a version commitment
└── standing [no]      [no] unchanged by any study pass
```

### T8 — decision authority (full tree: `PRD_01_24`)

```text
T8 decision authority (controller-autonomous under /goal)
├── D1 owner directions, dated and recorded verbatim                [v]
├── D2 controller decisions, no prior owner approval              [decision]
│   ├── version topic / ordering / gate / W1-W4 timing
│   ├── additive capability inside cu-abi 1.0 + 12/10/8/0 matrix
│   ├── lifting one [no] with dated reason and bounds
│   ├── G boundary direction: (a) keep closed, 2026-11-06
│   └── release chain, CI gates, owner-view acceptance
├── D3 owner veto after the fact, never a pre-approval gate         [v]
├── D4 escalation discipline                                       [no]
│   └── [no] pasting options + waiting on a reversible default
└── D5 decision record: date / choice / reason / evidence / trigger [v]
```

Owner direction (2026-11-06 22:15): "为什么要我做决定，你能自己安排吗？" —
an explicit correction of an escalation error. The controller had pushed a
reversible, high-confidence G-boundary choice back to the owner. Under the
standing `/goal` delegation the controller owns learn -> structure -> plan
-> order -> gate -> implement -> release -> accept, and reports the decision
with its reason. `[decision]` now means **a dated controller decision already
taken**, not "pending owner". Historical "owner decision" wording is kept
where the owner actually spoke (e.g. the 0.0.11 a11y lift).

### W0 learning ledger (summary; full notes in `PRD_01_17` and each subtree)

| Date | Pass | Query | Sources | Recorded caveat |
|---|---|---|---|---|
| 2026-11-06 | 1 | CU route + industry shape, resource isolation ladder | O/B/P/V/S | OSWorld metric + step-budget mismatch; vendor share numbers are V |
| 2026-11-06 | 1 | UIA vs MSAA, WAA, UFO2, three-OS grants | O/P/S | WAA gain is one stack; "no grant on Windows" is integrity conditional |
| 2026-11-06 | 2 | `IUIAutomation` vtable order and interop traps | O/S | RECT != UiaRect; swallowed HRESULT hides failure |
| 2026-11-06 | 3 | perception, grounding, code-first, safety | S/P/O/V | per-step latency is not a controlled A/B; NOVA 57% is OSWorld-specific |
| 2026-11-06 | 3 | cgroups v2, EEVDF, GPU virtualisation, sandbox cost | O/S/V | MPS shares failure domain; 7.3x spread has two KVM endpoints |
| 2026-11-06 | 3 | grants, input tiers, OSWorld 2.1 release contract | O/P/S | 2.1 manifest disclaims a full 108-task evaluation |
| 2026-11-06 | 4 | local Windows SDK `UIAutomationClient.h`/`.idl`: CLSID, IIDs, vtable slots, `RECT`, `TreeScope` | O (installed SDK 10.0.22000.0) | `RECT` is left/top/right/bottom, **not** `[x,y,w,h]`; header is authoritative for its SDK, not for the runtime; probe must still print raw HRESULTs |
| 2026-11-06 | 4 | host-side vs guest-side resource telemetry trust boundary | O/P/S | guest self-report is debugging data, not a security boundary; host-side signals sit on the far side of isolation |
| 2026-11-06 | 4 | decision authority under `/goal` | owner direction | a reversible default must not be escalated; owner keeps a veto, not an approval queue |

Evidence grades: **O** official, **B** benchmark board, **P** paper,
**V** vendor claim, **S** survey / secondary, **X** local measurement. Only
this repo's CI and owner-view acceptance (**X**) produce `[v]`.

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
│   │   required), granted per invocation by --allow-screen, three native
│   │   backends (Win32 / CG-CF / X11-XTest) with a declared 12/10/8/0
│   │   per-OS capability matrix
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
├── 0.0.3 frontier (signed releases) — closed by 0.0.4 and 0.0.5
│   ├── [v] S1 signing qualification: three owner secrets, one
│   │   qualification_only run, receipt read back (plan-0.0.3)
│   ├── [v] S2 release-policy.json signing.mode = required
│   ├── [v] S3 release.yml consumes the signed chain: mode=required
│   │   publishes only receipt-verified signed bytes; mode=off keeps the
│   │   unsigned path
│   ├── [v] S4 Defender court scans the exact published bytes with
│   │   `MpCmdRun -File`; a dirty verdict or an empty TAG blocks the release
│   ├── [v] S5 release automation repaired: the 403 root cause was a deleted
│   │   `environment: release` gate, not an org policy; v0.0.4 and v0.0.5
│   │   were published by CI alone
│   └── [_] S6 docs court: README/README.zh cover 0.0.2 flags + signing
        (the four 0.0.1 delivery rows below are recorded in the 0.0.1
        frontier and `plan/plan-0.0.1-com-release.md`)
└── [no] plugin host, permission policy, streaming, MCP, a scripting surface
    over the desktop, a second model loop
    (sessions enter 0.0.2 as plain journals; the takeover/owner-lock
    protocol stays out; `screen` exposes eight bounded operations, not an
    automation language)
    authority. The model API cannot execute tools; the tools cannot reach the
    model API. Scope extensions need a dated controller decision (T8).
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
│   ├── [v] A4 Windows MSAA read-only tree in `observe.a11y` (0.0.11)
│   │        bounded 400 rows / depth 12; no new grant, crate or elevation
│   │        owner PRD 01.15; AT-SPI (Linux) / AX (macOS) still [_]
│   │        unlock: per-app TCC grant on macOS; AT-SPI D-Bus bus + ATK
│   │        fixture on Linux; each green on real CI before declaring true
│   └── [_] A5 image arm in the wire codec: base64 PNG to the model each turn
│            unlock: separate PRD + budget/retention decision
│            target 0.0.7 candidate; [no] in 0.0.6
├── B Actuation
│   ├── [v] B1 pointer: hover, click, drag, scroll + pointer readback
│   ├── [v] B2 keyboard: type, key
│   ├── [v] B3 window: focus, move + rect readback (Linux/Windows)
│   ├── [v] B4 named semantic control: resolve + hover/press (Windows only)
│   ├── [v] B5 actuation receipt: button, downs/ups, double, target, plus
│   │        post-observe live session                 shipped in 0.0.6
│   ├── [v] B6a multi-point drag path                  qualified in 0.0.7
│   │        16 bounded points, 12 steps per segment, `path_points` receipt
│   ├── [v] B6b held-key modifiers                    qualified in 0.0.8
│   │        host-owned held set, four-backend set/release, teardown release
│   ├── [v] B7a two-axis scroll                      qualified in 0.0.7
│   │        `delta_x`/`delta_y`, `delta` stays the vertical alias
│   ├── [v] B7b explicit bounded wait                 shipped in 0.0.13
│   │        one sleep per observe call, monotonic elapsed evidence
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
│            partial: per-host wait ceilings shipped in 0.0.13;
│            iteration and cost ceilings stay gated on L2
├── E Platform and environment
│   ├── [v] E1 three real backends: Win32, CG/CF, X11/XTest via dlopen
│   ├── [v] E2 six cells cross-compiled on one runner, no link-time platform lib
│   ├── [v] E3 environment enum + display width/height in `disclose`
│   │        delivered in 0.0.9; geometry is already in the pixel arm
│   ├── [v] E4 OS-grant matrix in evidence (Screen Recording, Input
│   │        Monitoring, Accessibility) — delivered in 0.0.9
│   ├── [v] E5a display scale / coordinate basis in `disclose`
│   │        delivered in 0.0.10; additive evidence, not a capability row
│   ├── [v] E5c structured `a11y_controls[]` in `observe`
│   │        delivered in 0.0.11; Windows MSAA, other cells unknown+reason
│   ├── [v] E5b `act:focus` hwnd + `target_hwnd`   delivered in 0.0.12
│   │        three-OS CI, handle identity is an act target, not a scan arm
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
  wait and a three-level human-confirmation/safety-check policy. Two-axis
  scroll, multi-point drag and held-key modifiers were delivered in 0.0.7 /
  0.0.8 as additive verbs with their own evidence. The remaining candidates
  are not debts: each needs its own PRD, gate and dated controller decision
  (T8).

## Capability levels and the self-hosted orchestration target

`prd/PRD_01_12_self_hosted_orchestration.md` defines four levels — L0
read-only observation, L1 evidenced interaction, L2 self-sustained loop, L3
self-hosted orchestration — and records the owner's end state: this session
acts as controller, using `screen` to create, dispatch, read and retire
sub-task sessions in the WorkBuddy client's own GUI, then aggregate their
results. L2 is the lowest level at which that loop closes on its own; L3 is the
full form. It is an **acceptance journey for computer use**, not a new verb,
tool, harness capability or version plan, and it changes no `[v]`, `[_]` or
`[no]` mark in the tree above.

The candidate pool is now *ordered* by what that loop needs: display scale /
coordinate-basis disclosure (delivered in 0.0.10) -> A5 / D2 (model-visible
image arm) -> B7b + D4 (bounded wait and ceilings) -> A4
(reliable target location; the client is Electron / Chromium, so A2's Win32
child controls do not reach it) -> F3 (app-level effect evidence) -> C5 / C6
(safety and hand-off). Ordering is **not** a version commitment: every item
still needs its own PRD, its own gate, a dated controller decision and a
version plan that carries the gate. The next version is planned only after the previous one
is published and accepted. Plan 0.0.9 and its frontier are unchanged; E3 / E4
read-only grant disclosure is the precondition that explains why a click
cannot land before any of L2 exists.

## 0.0.10 frontier (shipped as a public signed Release)

W1/W2 landed in `040d50d` with the compile gate `42177115600` and the
three-OS journeys `42177115725` both green; the controller's independent
reader recomputed all three `cu-journey.json` artifacts as PASS. W3 closed
the truth of these docs (including the corrected coordinate-basis rationale
and the B7b hard evidence), and W4 landed in `e0e2671` with the full release
chain `42183119567` -> `42184544940` -> `42184644285` -> `42184866240` ->
`42184961997`, all success. `v0.0.10` is public Latest, non-draft, 15 assets;
the owner-view acceptance recomputed 6 sidecars and 3 signed cells and ran
the downloaded APE, which prints `minicon-harness 0.0.10`. The plan is
`plan/plan-0.0.10-display-scale-disclosure.md`, the contract is
`prd/PRD_01_13_display_scale_disclosure.md`.

Why this comes before the image arm: publishing the coordinate basis is a
**precondition** for A5 (the model-visible image arm) and for cross-DPI /
multi-monitor / scaled sessions, so no caller has to guess a ratio. An
earlier L1 trial note claiming a ~1.25x screenshot/desktop mismatch was a
controller misreading: CU self-reports `virtual_desktop`, `desktop` and PNG
geometry consistently at 1680x1050, a click taken from CU's own image hit the
target on the first try, and three-OS CI measures `scale` as `1` / `1` /
`unknown` (macOS / Windows / Linux). The earlier misses came from mixing UIA,
PIL and CU coordinate systems with focus never reaching the input box; they
were not a DPI scale.

```text
0.0.10 — display scale / coordinate-basis disclosure (shipped as a
          public signed Release)
├── [v] E5a `environment.display_scales[]` + `observe.displays[].scale`
│        delivered in 0.0.10 with three-OS CI evidence
│        read-only DPI/backing-scale probe, four backends, unknown + reason
│        no new capability row; matrix 12/10/8/0; cu-abi 1.0
├── [_] A5 / D2 model-visible image arm     unlock: PNG budget/retention
├── [_] B7b bounded wait + D4 ceilings      unlock: loop/budget decision
├── [_] A4 accessibility trees              unlock: OS grants + fixtures
├── [_] F3 app-level effect evidence        unlock: echo-window fixtures
├── [_] C5 / C6 safety + hand-off           unlock: a policy PRD first
├── [_] B8 batched actions[]                unlock: admission/stale design
```

## 0.0.11 frontier (public signed Release accepted)

**0.0.11 = structured control disclosure.** The L1 self-hosted trial drove
WorkBuddy through CU and found that `observe.controls[]` (Win32
`EnumChildWindows`) reports only one `Chrome_RenderWidgetHostHWND` with
`role=unknown` for a Chromium window: no input box, no send button. The
controller fell back to screenshots and a vision read of pixel coordinates,
which is slow, token-heavy and not evidence produced by the CU contract. On
2026-11-06 the owner required the missing structured read to be built into
this version rather than guessed around, and cancelled the sub-session
dispatch experiment: 0.0.11 is planned and implemented by the controller.

This is an **owner decision lifting one earlier `[no]`** ("a11y trees"),
recorded here instead of silently changed. The lift is bounded to a Windows
MSAA/IAccessible read-only tree with no new crate, no new OS grant and no
elevation; Linux / macOS / fallback publish an empty tree with a named
`a11y_reason`. Every other `[no]` and the G resource boundary in `PRD_01_08`
are unchanged.

Contract owner: `prd/PRD_01_15_structured_control_disclosure.md`; work
breakdown and gates: `plan/plan-0.0.11-structured-control-disclosure.md`.
W1/W2 landed in `f0433d6`: 109 unit tests, four-configuration type gate,
three-OS `observe_a11y` step and independent reader recompute.

```text
0.0.11 — structured control disclosure (delivered, public signed Release)
├── [v] E5c `observe.a11y` + `a11y_controls[]` / `a11y_reason`
│        Windows MSAA read-only tree; handle-addressed scan; additive row
│        no new capability row; matrix 12/10/8/0; cu-abi 1.0
├── [_] E5b `act:focus` hwnd + `target_hwnd`   moved to 0.0.12, PRD_01_14
│        same measured motive: two windows both titled "WorkBuddy"
├── [_] A4 AT-SPI (Linux) / AX (macOS) trees  unlock: OS grants + fixtures
├── [_] A5 / D2 model-visible image arm       unlock: PNG budget/retention
├── [_] B7b bounded wait + D4 ceilings        unlock: loop/budget decision
│        hard L1 evidence: observe stale frames, act sequencing
├── [_] F3 app-level effect evidence          unlock: echo-window fixtures
├── [_] C5 / C6 safety + hand-off             unlock: a policy PRD first
└── [_] B8 batched actions[]                  unlock: admission/stale design
```

Ordering is not a version commitment. The controller implements, polls CI,
independently recomputes the journey artifacts, decides the release gate and
accepts from the owner's view.

W3/W4 landed in `cf4fe27`; `v0.0.11` was published as a public signed
Release at 2026-11-06T10:19Z and accepted from the owner's view (15 assets,
six recomputed sidecars, a valid signed receipt chain and an anonymous APE
run printing `minicon-harness 0.0.11`).

## 0.0.12 frontier (public signed Release accepted)

**0.0.12 = handle-addressed window targeting.** `act:focus` gains an optional
`hwnd`, mutually exclusive with `window`; every call re-validates the live
handle; focus evidence and readback publish `target_hwnd` from the desktop,
while `actuation` stays `null`. The measured motive is unchanged: two
WorkBuddy windows share the title `WorkBuddy`, so a title is not an identity.
W1/W2 landed in `e843715`: 116 tests (113 passed, 3 ignored), the
four-configuration type gate, three-OS CI `42205711217` / `42205711264`
success and an independent reader recompute PASS on Linux 31 / macOS 27 /
Windows 34 performed steps. W3/W4 landed in `e1887f8`; the public signed
Release `v0.0.12` was published at 2026-11-06T11:22:38Z from
journey `42208042397`, platforms `42207927764`, pack `42208244759`,
candidate `42209382249`, signing `42209452910`, dry-run `42209665726` and
publish `42209730386` — all success. Owner-view acceptance: public page and
assets anonymous 200, 15 assets, six recomputed sidecars, build/signing
receipts matched `e1887f8` / `0.0.12`, three signed assets matched
`after_sha256` and were locally `Valid` with RFC 3161 timestamps, and the
downloaded APE printed `minicon-harness 0.0.12`. `cu-abi 1.0`, the 12/10/8/0
matrix, every earlier reader assertion and every `[no]` stay unchanged.

`[no]` carried forward unchanged: plugin host, permission policy engine,
streaming, MCP, a scripting surface over the desktop, a second model loop,
takeover/owner-lock, any new crate or runtime, bun/WASM/ujs embedding
(PRD 31 readiness stays BLOCKED), runtime backend selection, Wayland-native
protocol and host-resource management (G; `PRD_01_08` owns the boundary).

## 0.0.13 frontier (public signed Release accepted)

**0.0.13 = bounded observation wait.** `op=observe` gains an optional
`wait_ms` (1..=2000): one bounded sleep, then a fresh snapshot and additive
`evidence["wait"]` carrying requested, monotonic elapsed, cumulative and
call-count values plus the new generation. Per-host ceilings (2000 ms per
call, 8000 ms cumulative, 8 calls) are named refusals before any sleep;
`disclose` publishes the remaining budget. W1/W2 landed in `40276ae` (CI
`42213269581` / `42213269486` success); W3/W4 landed in `59e6fd2`. The public
signed Release `v0.0.13` was published at 2026-11-06T12:17:48Z from
platforms `42214274457`, journey `42214326088`, pack `42214546620`,
candidate `42216049183`, signing `42216112607`, dry-run `42216372517` and
publish `42216487467` — all success. Owner-view acceptance: public page and
assets anonymous 200, 15 assets, six recomputed sidecars, build/signing
receipts matched `59e6fd2` / `0.0.13`, three signed assets matched
`after_sha256` and were locally `Valid` with RFC 3161 timestamps, and the
downloaded APE printed `minicon-harness 0.0.13`. An independent reader
recompute of the released journeys PASSed on Linux 32 / macOS 28 / Windows
35 performed steps, including `observe_wait_after_act` and the over-bound
refusal. `cu-abi 1.0`, the 12/10/8/0 matrix, every earlier reader assertion
and the G boundary are unchanged.

The measured reason is the L1 hard evidence recorded for B7b: an `observe`
issued immediately after an `act` captured a pre-reflow Chromium frame, and
`type` plus its send click had to be split into separate runs. A coordinate
basis, an a11y tree and a window handle do not fix stale timing. Under the
standing `/goal` delegation, the controller lifts the older `explicit wait`
non-goal for this bounded read-only observe parameter only. A5 / D2 image
arm, B8 batched actions, AT-SPI / AX a11y trees, F3 app-level effect
evidence, C5 / C6 safety and window-manager behaviour remain `[no]` with
their unlock conditions. Contract owner:
`prd/PRD_01_16_bounded_wait.md`; work breakdown:
`plan/plan-0.0.13-bounded-wait.md`.

## Continuous frontier learning (W0, owner direction 2026-11-06)

The owner extended `/goal` three times on 2026-11-06: first to keep learning
current computer-use **and** resource-management technology (process, CPU,
memory, GPU, keyboard, video, mouse — all of it) and upserting the PRD; then
to keep the **learning process notes** in the PRD, expand it into
**multi-dimensional trees**, split large branches into indexed subtree
documents, and **not start code or tests until the plan is complete**; and at
22:15 to stop escalating reversible choices — the controller decides and
reports the reason (T8, `PRD_01_24`). W0 is owned by
`prd/PRD_01_17_frontier_radar.md`; the multi-dimensional index is at the top
of this file.

1. **Learn** — K / V / M / W / S perception, actuation, timing, verification
   and safety, plus process / CPU / memory / GPU / device / network / storage
   / credentials / lifecycle / cost.
2. **Upsert** — append or revise the radar, the subtree documents and this
   file's ledger in place; never overwrite a dated finding; record the query,
   the source class, the caveat and where the finding went.
3. **Structure** — fold each finding into the correct T1-T8 node and keep the
   document index accurate.
4. **Complete the plan** — pass the plan-completeness checklist in
   `PRD_01_23`; **only then** implement through W1/W2/W3/W4. A capability is
   claimed from three-OS CI plus the independent journey reader, never from
   an article. Pass 4 completed the 0.0.15 plan (6/6) on 2026-11-06.

**Learning is not authority.** `PRD_01_08` still assigns process, CPU,
memory, GPU and device management outside `screen`; the radar records that
whole domain as research. The G direction is **decided by the controller on
2026-11-06**: keep G closed (a), with four explicit reopen triggers for
read-only telemetry and `[no]` for real resource control
(`PRD_01_24`). No `[no]` is lifted by a research pass. **Current phase:
STUDY + STRUCTURE**, not implementation; the 0.0.15 plan is complete (6/6)
and W1 is eligible but not started.

First W0 pass (2026-11-06) produced three reusable conclusions: interface-
first perception with UIA ahead of MSAA on Windows (WAA measured ~57%
relative gain from the UIA tree; `v0.0.11` shipped only the MSAA subset);
OSWorld-Verified vs 2.0 showing short-task grounding is solved while
hour-long workflows fail on timing and verification (the same B7b / D4 gap
this version closes); and the resource-domain ladder from cgroups v2 /
Landlock / seccomp / MIG to per-task microVMs with four production
boundaries. The open-gate implementation candidate was the normalised a11y
node schema (0.0.14 W1/W2); UIA-first routing needs its own COM-vtable probe
and resource-domain work stays gated behind G.

## 0.0.14 frontier (public signed Release accepted)

**0.0.14 = normalised a11y node schema**, the first implementation candidate
selected by W0 (`PRD_01_18`,
`plan/plan-0.0.14-normalised-a11y-nodes.md`). Each `a11y_controls[]` row
gains `value`, the raw `STATE_SYSTEM_*` `state` bit mask and `state_names`
derived from a fixed table; the frame publishes `a11y_node_schema`; the
independent reader recomputes `state_names` from the published integer and
rejects a guessed set. W1/W2 landed in `31e257d` (122 tests: 119 passed,
3 ignored; three-OS CI `42217541734` / `42217725863` success); W3/W4 landed
in `efe96a2`. The public signed Release `v0.0.14` was published at
2026-11-06T12:43:47Z from platforms `42218173775`, journey `42218235696`,
pack `42218406807`, candidate `42219432928`, signing `42219503249`, dry-run
`42219748983` and publish `42219817323` — all success. Owner-view acceptance:
public page and assets anonymous 200, 15 assets, six recomputed sidecars,
build/signing receipts matched `efe96a2` / `0.0.14`, three signed assets
matched `after_sha256` and were locally `Valid` with RFC 3161 timestamps,
and the downloaded APE printed `minicon-harness 0.0.14`. The released
journeys were independently recomputed PASS on all three OSes. `cu-abi 1.0`,
the 12/10/8/0 matrix, the native signature, every earlier reader assertion
and the G boundary are unchanged.

UIA-first mixed routing stays a W0 candidate with a probe gate: the installed
Windows SDK headers now prove the vtable slots, but a real-desktop Rust probe
must still print raw HRESULTs before any production routing. Resource-domain
telemetry and control remain behind the G boundary, whose direction (a) is a
dated controller decision in `PRD_01_24`.

## 0.0.15 next plan (plan complete 6/6, W1 eligible, not started)

**No code, test, version bump or Release chain is started for 0.0.15.** Under
the owner's 2026-11-06 evening direction the phase is study and structure;
W0 pass 4 completed the plan-completeness checklist in `PRD_01_23` (6/6) on
2026-11-06 and the controller starts W1 in its own cadence (`PRD_01_24`).
The next planned version is **UIA probe-first, then mixed structured routing**
(`plan/plan-0.0.15-uia-probe-and-routing.md`).

Pass 4 replaced the second-hand vtable claim with the **installed Windows SDK
10.0.22000.0 headers**: `CLSID_CUIAutomation`, three IIDs and the exact
vtable slots (`ElementFromHandle` 6, `CreateTrueCondition` 21, `FindAll` 6,
`get_CurrentName` 23, `get_CurrentBoundingRectangle` 43). It also found a new
trap — `RECT` is left/top/right/bottom while this repo publishes
`[x,y,width,height]`, so the adapter must convert, and the independent reader
rejects a negative width/height. Every COM call prints raw HRESULTs; a wrong
slot is still a process crash. W1 is a Rust diagnostic probe; W2 ships
UIA-first + MSAA fallback **only if** the probe passes on a real Windows
desktop. A failed probe stops the version with a diagnosis and a downgrade to
the next W0 candidate: no guessed slots, no half-working route and no
weakened reader. `cu-abi 1.0`, the 12/10/8/0 matrix and the G boundary stay
unchanged; G direction (a) is a decided controller decision, not a choice
offered back to the owner.

## 0.0.9 frontier (shipped as a public signed Release)

Planned after `v0.0.8` was published, per the owner's "plan the next version
after each public release" loop. **W1-W4 are closed: W1/W2 proven by
`42164845780` (compile gate) and `42164845836` (three-OS journeys); W3/W4
landed in `99ea157`, whose public signed Release `v0.0.9` came from pack
`42166110442` -> candidate `42167224386` -> signing `42167283497` -> dry-run
`42167468391` -> publish `42167566196`. The controller re-hashed all 15
downloaded assets and ran the downloaded APE (`minicon-harness 0.0.9`).**
Direction unchanged: concentrate
on computer use; harness-side capability work stays deferred. The plan is
`plan/plan-0.0.9-grant-disclosure.md`, the contract is
`prd/PRD_01_11_grant_disclosure.md`. A `[_]` is a chapter, **not** a
lifted non-goal: anything marked `[no]` in an earlier frontier stays `[no]`
until a dated controller decision lifts it and a version plan carries the gate.

```text
0.0.9 — grant and environment disclosure (implemented)
├── [v] E3/E4 read-only environment + grants[] on `op=disclose`
│        four typed statuses; `unknown` always carries a reason
│        preflight / read-only APIs only, no permission prompt
│        no new capability row; matrix 12/10/8/0; cu-abi 1.0
├── [_] B7b explicit bounded wait          deferred: loop/budget decision (D4)
├── [_] B8 batched actions[]               deferred: admission/stale design
├── [_] 0.7.1 image arm (A5, D2)            unlock: PNG budget/retention
├── [_] 0.7.2 safety checks (C5, C6)        unlock: a policy PRD first
├── [_] 0.7.4 accessibility trees (A4)      unlock: OS grants + fixtures
├── [_] 0.7.6 app-level effect evidence     unlock: echo-window fixtures
└── [_] held-key effect evidence (F3)       unlock: three-OS echo fixture
```

`[no]` carried forward from every earlier frontier: plugin host, permission
policy engine, streaming, MCP, a scripting surface over the desktop, a second
model loop, takeover/owner-lock, any new crate or runtime, bun/WASM/ujs
embedding (PRD 31 readiness stays BLOCKED), runtime backend selection, and
host-resource management (G in the tree above; `PRD_01_08` owns the boundary).

## 0.0.8 frontier (shipped as a public signed Release)

Planned after `v0.0.7` was published, per the owner's "plan the next version
after each public release" loop, then implemented. Direction unchanged:
concentrate on computer use; harness-side capability work stays deferred. The
plan is `plan/plan-0.0.8-held-key-modifiers.md`, the contract is
`prd/PRD_01_10_held_key_modifiers.md`. W1/W2 are green on three OSes:
compile gate **42154222974**, journeys **42154223003**. The three
`cu-journey.json` artifacts were downloaded and re-read locally (Linux 25 /
macOS 22 / Windows 28 performed steps) with the held set recomputed from
each record. W3/W4 are complete; the Release chain and the owner-view
acceptance are recorded below. A `[_]` is a chapter, **not** a lifted
non-goal: anything marked `[no]` in an earlier frontier stays `[no]` until
a dated controller decision lifts it and a version plan carries the gate.

```text
0.0.8 — held-key modifiers (implemented, shipped)
├── [v] B6b held-key modifiers on `act:key`
│        `hold` / `release`, four whitelisted modifiers, host-owned state
│        auto-release on foreground change / lost grant / host teardown
│        `evidence["keyboard"]` + `evidence["held_keys"]`; no button receipt
│        matrix stays 12/10/8/0; cu-abi 1.0; additive wire params only
├── [_] B7b explicit bounded wait          deferred: loop/budget decision (D4)
├── [_] B8 batched actions[]               deferred: admission/stale design
├── [_] 0.7.1 image arm (A5, D2)            unlock: PNG budget/retention
├── [_] 0.7.2 safety checks (C5, C6)        unlock: a policy PRD first
├── [_] 0.7.4 accessibility trees (A4)      unlock: OS grants + fixtures
├── [_] 0.7.5 environment/grant disclosure  unlock: disclose design
└── [_] 0.7.6 app-level effect evidence     unlock: echo-window fixtures
```

`[no]` carried forward from every earlier frontier: plugin host, permission
policy engine, streaming, MCP, a scripting surface over the desktop, a second
model loop, takeover/owner-lock, any new crate or runtime, bun/WASM/ujs
embedding (PRD 31 readiness stays BLOCKED), runtime backend selection, and
host-resource management (G in the tree above; `PRD_01_08` owns the boundary).

[v] `v0.0.8` is a public company-signed Latest Release
(https://github.com/partnernetsoftware/minicon-harness/releases/tag/v0.0.8)
from source `38be0e22dedc06c295f7a36bf66b69f76861d55d`: pack **42154879691**, candidate
**42155897541**, signing **42155962651**, dry-run **42156176166**,
publish **42156239210**. Owner-view acceptance: the Release page and an
asset answer anonymous HTTP 200; all 15 assets were re-downloaded and six
sidecars recomputed locally; the APE and both signed Windows cells match the
receipt's `after_sha256` (and differ from `before_sha256`); the downloaded
APE prints `minicon-harness 0.0.8`.

## 0.0.7 frontier (shipped as a public signed Release)

Planned 2026-11-05 **after `v0.0.6` was published**, per the owner's
"plan the next version after each public release" loop; implemented the
same day on `main`. Direction: keep concentrating on computer use;
harness-side capability work stays deferred. The plan is
`plan/plan-0.0.7-richer-actuation.md`, the contract is
`prd/PRD_01_09_richer_actuation.md`. A `[_]` is a chapter, **not** a lifted
non-goal: anything marked `[no]` in an earlier frontier stays `[no]` until an
dated controller decision lifts it and a version plan carries the gate.

[v] `act:drag` accepts a bounded `path[]` (2..16 points, every segment
interpolated with 12 stepped moves, whole path admitted against the virtual
desktop before any native effect). The pointer readback must land at the
**last** point and the actuation receipt gains `path_points`, counted by the
native backend. Omitting `path` keeps 0.0.6's single-leg drag unchanged.
`act:scroll` accepts `delta_x`/`delta_y` within +-1440, publishes them as
`evidence["scroll"]`, and keeps `delta` as the vertical alias; `(0,0)` and a
`delta`/`delta_y` conflict are named refusals. Both are additive wire
parameters: `cu-abi 1.0` and the 12-row matrix are unchanged.

[v] Compile gate **42067350856** and journeys **42067350269** are green on
windows/macos/linux. The independent reader was re-run locally on all three
downloaded `cu-journey.json` artifacts (Linux 20, macOS 17, Windows 23
performed steps): `act_drag_path` reports `path_points=3` and lands at the
last path point on each OS; `act_scroll_x` publishes `(120,0)` and
`act_scroll_y` publishes `(0,-120)`. `scripts/cu-cfg-check.py` type-checks
**four** backend configurations (Windows, macOS, Linux, unlisted-OS
fallback) with the new `drag`/`scroll` signatures.

[v] `v0.0.7` is a public company-signed Latest Release
(https://github.com/partnernetsoftware/minicon-harness/releases/tag/v0.0.7)
from source `ef743ec`: pack **42068228345**, candidate
**42070465260**, signing **42070561984**, dry-run **42070968482**,
publish **42071076064**. Owner-view acceptance: the Release page and an
asset answer anonymous HTTP 200; all 15 assets were re-downloaded and six
sidecars recomputed locally; the APE and both signed Windows cells match the
receipt's `after_sha256` (and differ from `before_sha256`); the downloaded
APE prints `minicon-harness 0.0.7`.

```text
0.0.7 — richer actuation (implemented)
├── [v] 0.7.3 path drag + two-axis scroll (B6a, B7a)
│        unlock MET: 0.0.6 interaction closure green on three OSes
│        matrix stays 12/10/8/0; cu-abi 1.0; additive wire params only
├── [v] held-key modifiers (B6 half)                delivered in 0.0.8
│        unlock MET: four-backend keyboard state machine + teardown release
├── [_] explicit bounded wait (B7 half)             deferred to 0.0.8+
│        unlock: a loop/budget decision (D4), not a desktop verb
├── [_] batched actions[] (B8)                      deferred to 0.0.8+
│        unlock: its own admission/stale design
├── [_] 0.7.1 image arm (A5, D2)                    unlock: PNG budget/retention
├── [_] 0.7.2 safety checks (C5, C6)                unlock: a policy PRD first
├── [_] 0.7.4 accessibility trees (A4)               unlock: OS grants + fixtures
├── [_] 0.7.5 environment/grant disclosure (E3, E4) unlock: disclose design
└── [_] 0.7.6 app-level effect evidence (F3)        unlock: echo-window fixtures
```

`[no]` carried forward from every earlier frontier: plugin host, permission
policy engine, streaming, MCP, a scripting surface over the desktop, a second
model loop, takeover/owner-lock, any new crate or runtime, bun/WASM/ujs
embedding (PRD 31 readiness stays BLOCKED), runtime backend selection, and
host-resource management (G in the tree above; `PRD_01_08` owns the boundary).

## 0.0.6 frontier (shipped as a public signed Release)

[v] Interaction closure shipped. 0.0.5 made observation and pointer movement
real on three OSes but never drove `press=true`, `button=middle`,
`button=right` or `double=true` in a CI journey; a successful click was
evidenced only by a pointer coordinate. 0.0.6 adds a typed **actuation
receipt** (`button`, `downs`, `ups`, `double`, `target`) counted by the native
backend at dispatch time and published in `evidence["actuation"]`. `CuHost`
compares it with the request: a missing receipt is `unknown`, a mismatched one
is `not_performed`, and neither is ever upgraded to a success. `cu-abi 1.0`,
the wire inputs and the 12-row capability matrix (windows 12/12, linux 10/12,
macos 8/12) are unchanged. The receipt proves a real button event was
dispatched, not that the target application's business action ran.

[v] CI drove the previously undriven variants on all three OSes: middle,
right, double, middle-button drag and (Windows) a real `press=true` click on a
named control. The independent reader recomputes receipts, pointer readbacks,
PNG IHDR and generation order; `observe_final` must be strictly newer and
still report a live window and display. Compile gate **42056894173** and
journeys **42056894326** are green on windows/macos/linux (Linux 17, macOS 14,
Windows 20 performed steps).

[v] `v0.0.6` is a public company-signed Latest Release
(https://github.com/partnernetsoftware/minicon-harness/releases/tag/v0.0.6)
from source `bf840c9`: pack **42057696157**, candidate **42059780606**,
signing **42059902962**, dry-run **42060245443**, publish **42060339772**.
Owner-view acceptance: the Release page and an asset answer anonymous HTTP
200; all 15 assets were re-downloaded and six sidecars recomputed locally;
the APE and both signed Windows cells match the receipt's `after_sha256`
(and differ from `before_sha256`); the downloaded APE prints
`minicon-harness 0.0.6`.

[no] AX / AT-SPI / UIA trees and app-level state-change evidence stay out:
a hosted runner cannot self-grant macOS Accessibility, and the Xvfb fixture
has no AT-SPI bus or ATK bridge. Their unlock conditions and the PRD 31
readiness gate (still BLOCKED) are recorded in the plan and `PRD_01_06`.
0.0.6 owns B5 and extends C3 in the tree above; the external baseline is
`prd/prd-cu-openai.md`, whose absorbed/deferred deltas are in the plan's
"External benchmark" section.

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
