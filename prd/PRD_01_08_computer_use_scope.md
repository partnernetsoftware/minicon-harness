# PRD 01.08 — computer-use scope: KVM, window targets and the resource boundary

Owner of **what "computer use" means in this repository**. `PRD_01_03` owns what
the `screen` tool can do; `PRD_01_04` owns the contract above it; this PRD owns
the domain model and the boundary around it. Status: **document only** -- it
changes no source, version, capability matrix or Release. Every capability
claim below points back to a PRD that already owns it.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Why this document exists

`PRD.md`'s CU capability tree (A Observation, B Actuation, C Contract and
safety, D Loop and state, E Platform and environment, F Verification and
delivery) is an index of **engineering concerns**, not a definition of the
domain. It answers "what is built, who owns it, which version" but not "which
parts of a machine does this product operate".

The owner's model (2026-11-05) is that computer use is **KVM (keyboard, video,
mouse) plus resource management** -- window, process, memory and hardware
management. This PRD makes that model explicit, states exactly how much of it
minicon-harness carries, and writes down where the rest belongs. It prevents
two symmetric mistakes: reading the A-F tree as a promise to manage the host,
and reading "we do not manage the host" as a gap in the desktop work.

## Definition

> In this repository, computer use is the bounded operation of **one local
> interactive session**: read its video output, send keyboard and mouse input,
> and bind those effects to a verified window/control target. It is not the
> management of the host's processes, memory, devices or other resources.

Three planes, in the order an effect travels through them:

```text
device plane      K / V / M       the physical or virtual console I/O
target plane      W               the window or control an effect is bound to
session plane     S               generation, focus, grants, desktop state
```

`CuHost` (`src/cu.rs`) is the only entrance to any of them. One invocation,
one grant (`--allow-screen`), one effect per `act` call, one readback that
proves **desktop state** -- never process state, memory state or device state.
That last sentence is the whole boundary.

## Domain axes

### K — keyboard `[v]`

| | |
|---|---|
| Delivered | `act:type` (4096 chars), `act:key` (`ctrl+s`, `win+left`); foreground title before/after is the evidence |
| Native | `SendInput` (Windows), `CGEvent` (macOS), `XTest` (Linux), all via the compiled cell |
| Bounds | `SCREEN_TOOL_MAX_TYPE_CHARS` 4096; a changed foreground is reported, not treated as failure |
| Gaps | held-key modifiers / `keys[]` (B6), explicit `wait` (B7); no keyboard-layout or IME guarantee is claimed |
| Owner | `PRD_01_03` (verbs), `PRD_01_06` (per-OS matrix), `PRD_01_07` (receipt discipline) |

### V — video `[v]`

| | |
|---|---|
| Delivered | virtual desktop rect, `displays[]` incl. negative coordinates, ordered `windows[]` (handle, pid, rect, state, class, title), one PNG under the task root (desktop / window / region, scale 1..8), typed Win32 child `controls[]` (label, class-derived role, text, rect, visible, enabled) |
| Bounds | 64 windows, 16 MiB PNG, scale 8, 12 retained captures, 8192 px per region side, 128 controls, 256 chars of control text, 200 ms per `WM_GETTEXT` |
| Honesty | a capture is a **local artifact**; the model reads its path, not its pixels. A self-drawn UI reports zero controls and says why |
| Gaps | accessibility trees (A4), image arm in the wire codec (A5 / D2), environment enum in `disclose` (E3) |
| Owner | `PRD_01_03` (pixel arm), `PRD_01_05` (control arm), `PRD_01_06` (three backends) |

### M — mouse / pointer `[v]`

| | |
|---|---|
| Delivered | `act:hover`, `act:click` (left/middle/right, `double`), `act:drag` (12 steps per segment, optional 2..16-point `path[]`), `act:scroll` (two axes); pointer readback with 4 px tolerance |
| Receipt | since 0.0.6 every button event carries a typed actuation receipt (`button`, `downs`, `ups`, `double`, `target`, since 0.0.7 `path_points`) counted by the backend; a missing receipt is `unknown`, a mismatched one is `not_performed` (`PRD_01_07`). A scroll posts no button event; it publishes `evidence["scroll"]` with both axes (`PRD_01_09`) |
| Gaps | held-key modifiers (B6b), explicit bounded `wait` (B7b), batched `actions[]` (B8); each deferred with its own unlock condition |
| Owner | `PRD_01_03`, `PRD_01_07`, `PRD_01_09` |

### W — window and control target management `[v]` (deliberately narrow)

This is **target binding**, not a window manager.

| | |
|---|---|
| Delivered | window inventory with deterministic order; exact title / unique-substring resolution; `act:focus`; `act:move` with rect readback (4 px); `act:control` resolving a named control from the pinned generation |
| Revalidation | a handle is never trusted across calls: every operation re-resolves the title and re-checks the live window/control |
| Not delivered | `[no]` minimize / maximize / close / restore, z-order policy, workspace switching, tiling, window rules, application launch/quit management |
| External owner | `minicon/prd/PRD_02_38_computer_use.md` states that **minicon-winmgr owns window management product behavior**; a shared placement mechanism does not make this tool a CU coordinator |
| Note | the `pid` carried on each window is **identity metadata** for target resolution, not a process-management capability (see below) |

### S — session state `[v]` (and its limits)

| | |
|---|---|
| Delivered | `observe` generations with pinned `as_of`, stale refusal, foreground evidence, six typed verdicts, 32 retained receipts, per-OS 12/10/8/0 capability matrix in `disclose` |
| Preconditions, not managed objects | `$DISPLAY`, Screen Recording, Input Monitoring, Accessibility, UIPI; a missing grant is a named `unsupported` refusal, never an auto-escalation |
| Gaps | OS-grant matrix in evidence (E4); session continuity beyond `as_of` (D3) |
| Non-goals | `[no]` takeover / owner-lock / watch mode (C7), `[no]` a service identity or resident session owner |

## Resource-management boundary

The owner's wider model is real, but it is **not this product's scope**. The
sibling `agenterm-cu` does have `process`, `system` and `network` verb families
(`agenterm/docs/agenterm-cu-verbs.md`: `ps`, `process-maps`, `process-kill`,
`resource-status`, `host-memory-status`, `processor-topology-status`,
`network-interfaces`, `job-*`). `PRD_01_03`'s migration assessment examined
that surface and concluded **contracts migrate, implementations do not**: it is
a resident service with durable leases, a grant store and a second artifact.
Importing it would end the one-file, one-invocation premise.

| Resource domain | Who owns it here | What `screen` actually exposes | Status |
|---|---|---|---|
| Process lifecycle (spawn / inspect / signal / kill / usage / maps / fds) | `exec` tool: one allow-listed basename argv, no shell, 30 s kill, 1 MiB capture | a `pid` per window, as identity metadata only | `[no]` as CU |
| Memory, CPU, load, page faults, topology | nothing; an allow-listed program run via `exec` may report them | nothing | `[no]` |
| Hardware / devices / peripherals / hotplug | nothing | displays and the pointer/keyboard are **preconditions** of V/K/M, not objects to enumerate or configure | `[no]` |
| Storage / filesystem | `file` tool, root-confined read/write/edit/undo | captures are evidence artifacts under `<root>/.harness/capture/`, newest 12 retained; that is evidence retention, not storage management | `[no]` |
| Network | `wire.rs`: one bounded HTTPS request to the model endpoint, no redirects, 4 MiB body | nothing | `[no]` |
| OS grants / permissions | the host, as admission prerequisites | a named refusal with the platform and missing grant | `[no]` automatic grant or escalation |
| Window-manager product behavior | the `minicon-winmgr` product | focus/move + inventory as target binding | `[no]` as CU |
| Runtime / engine / durable job owners | PRD 31 readiness stays BLOCKED | the `cu-abi 1.0` seam only (`PRD_01_04`) | `[no]` |

Consequences of this table:

1. `exec` can start and kill a process; `screen` cannot. Saying "CU manages
   processes" would conflate two tools with two different grants, two
   different bounds and two different readback semantics.
2. A desktop readback (pointer, rect, foreground) proves nothing about a
   process's memory, a device's state or an application's business action.
   `PRD_01_07` already says this for clicks; it is true for the whole domain.
3. The A-F tree's C/D/E/F leaves are evidence about the KVM/W work, not
   additional resource domains. Counting them as coverage would inflate the
   product's scope on paper.

## How to read the A-F tree

| Tree branch | Role | Domain axis |
|---|---|---|
| A Observation | capability axis | V (A1-A3, A5), W/control inventory (A2), future trees (A4) |
| B Actuation | capability axis | M (B1, B5-B8), K (B2, B6), W (B3, B4) |
| C Contract and safety | **cross-cutting** | admission, grant, verdicts, receipts; no device effect |
| D Loop and state | **cross-cutting** | one call -> one verdict, budgets, session continuity |
| E Platform and environment | **cross-cutting** | which compiled cell can do K/V/M/W, and under which grants |
| F Verification and delivery | **cross-cutting** | CI, signed Release, independent readers |
| G Resource-domain boundary (`PRD.md`) | **boundary** | explicitly `[no]`; see the table above |

So: **A and B are what the desktop can do; C-F are how a call is admitted,
looped, compiled and evidenced; G is what computer use is not.** The tree stays
the single index of feature status. This PRD stays the single definition of
the domain. Neither replaces the other, and leaf IDs (A5, B5, C3, E4 ...)
are unchanged so the 0.0.6 plan and the 0.0.7 chapters keep their references.

## The computer-use document branch

```text
prd/
├── PRD_01_03_computer_use.md        [v] the screen tool, verbs, bounds,
│                                        migration assessment, gap table
├── PRD_01_04_cu_base.md             [v] cu-abi 1.0 host contract
├── PRD_01_05_semantic_controls.md   [v] typed Win32 controls, named target
├── PRD_01_06_cross_platform_cu.md   [v] three real backends, capability matrix
├── PRD_01_07_interaction_closure.md [v] 0.0.6 actuation receipt contract
├── PRD_01_08_computer_use_scope.md  domain model + resource boundary
├── PRD_01_09_richer_actuation.md    [v] 0.0.7 path drag + two-axis scroll
└── prd-cu-openai.md                 external reference, NOT this repo's contract
```

Sibling references, not owned here: `minicon/prd/PRD_02_38_computer_use.md`
(the CU0..CU4 outcome tree whose invariants were migrated),
`agenterm/prd/PRD_02_28_agenterm_cu.md` + `docs/agenterm-cu-verbs.md` (the
richer CU surface that was deliberately not migrated), and
`minicon-winmgr` (window-management product). Version plans live in `plan/`;
version status lives in `PRD.md`.

## If computer use is ever widened into host resources

`[no]` until an owner decision lifts it. The unlock conditions are structural,
not a matter of adding one verb:

1. A new PRD with its own threat model: process/memory/device state has a
   different authority surface from a pointer position, and `PRD_02_38`'s
   CU admission bounds only this tool's own operations.
2. A fourth tool (or a widening of `exec`) with its own grant, its own bounds
   and its own named constants -- `screen`'s `--allow-screen` must not become
   a host-control grant by accident.
3. A readback/evidence semantics that proves the claimed resource state, plus
   an independent reader and a three-OS CI journey, before any `[v]`.
4. A version gate carrying the change. Resident owners, durable job leases and
   a colocated runtime library remain out: they contradict one file, one
   invocation, no service identity.

## Invariants

1. Computer use operates one local interactive session, one effect per call;
   it never manages host resources.
2. A `pid` in an observation is identity metadata, not authority over a
   process.
3. Displays, input devices and OS grants are preconditions of K/V/M; a
   missing one is a named refusal, never an auto-escalation.
4. Cross-cutting layers (contract, loop, platform, verification) are not
   domain coverage and must not be reported as such.
5. Readback proves desktop state. An application-level or host-level effect
   needs its own fixture, its own evidence and its own PRD.
