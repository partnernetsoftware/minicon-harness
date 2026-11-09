# minicon-harness product requirements

minicon-harness is a family of small, bounded, evidence-first agent
infrastructure components: one worker core and two companion surfaces.

**The product is the trustworthy local capability and egress layer for other
harnesses.** It does not try to be the harness, the orchestrator or the
management console. A harness keeps its own model loop; minicon-harness
supplies the hands (computer use, files, commands) and the mouth (account,
model and proxy routing), with the same bounds, refusals and evidence the core
has shipped since 0.0.1.

## Product definition

### 1. Worker core -- `minicon-harness`

One portable file, exactly three tools (`file`, `exec`, `screen`), an
OpenAI-compatible model loop, no installer, no bundled runtime and no
permission-policy engine. A script hands one narrowly scoped local task to a
model and gets the final answer on stdout in one bounded invocation. Runs on
Windows / macOS / Linux, x86_64 and ARM64, as six native cells packed into a
`.com`. This is the reference host and the computer-use engine; the tools' own
bounds are the whole policy. Contract: `prd/PRD_01_03_computer_use.md`;
delivery: `prd/PRD_01_01_com_delivery.md`.

### 2. MCP adapter -- `minicon-harness-mcp` ("hands")

Exposes the same three tools over stdio MCP so external harnesses (Claude
Code, OpenCode, Codex, Cursor, Hermes, DSH) can mount minicon-harness as their
local computer-use / execution backend. Translation only: MCP tool call -> one
bounded core invocation -> MCP response; every bound, grant, receipt and
refusal passes through unchanged. The adapter is a separate artifact beside the
core, never a plugin host inside it. Status: **built and equivalence-tested;
not yet a Release asset** -- closing that is the next product item.

### 3. LLM gate -- `minicon-gate` ("mouth")

A loopback-only sidecar that pools the tuple (provider-and-account, model,
modelArgs, http-proxy), binds agents to ordered lane candidates, and accepts
OpenAI-compatible calls. The tuple is stored as three axes -- **endpoint x
credential x egress** -- so a lane is their product and a dead proxy or a dead
account cools one axis, not every lane through it. Workflows are declarations
only: the gate validates, resolves and routes, and never executes a step.
Protocol converter and router only -- no tool execution, no policy engine, no
plugin runtime, no management UI. Contract:
[`prd/PRD_01_27_llm_gate.md`](prd/PRD_01_27_llm_gate.md) (`[_]`). Status:
**published and accepted as a signed Release** (`v0.0.20`, `plan/v0.0.20.md`); the H-line adapter now ships beside it.

### What is deliberately not built here

The family entry and management surface belongs to `agenterm`. This repository
does not build a GUI, a key-management console, budgets, rate limits, a
workflow engine or a second model loop. The canonical list is "Non-goals"
below.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal, `[study]` research note with no implementation
claim, `[decision]` a dated controller decision already taken
(`prd/PRD_01_24_decision_authority_subtree.md`). The owner keeps a veto, not
an approval queue; no document here asks the owner to choose.

## Status at a glance

Three different facts must not be conflated: **published** (a Release asset
exists), **accepted** (owner-view acceptance is recorded) and **claimed**
(`[v]`, backed by three-OS CI plus the independent reader). Release history,
CI run ids and dated evidence are archived in `prd_hist.md`; this table is the
only current status.

| Version | Topic | Published | Accepted | Claimed |
|---|---|---|---|---|
| 0.0.1 | first public release, six cells | yes | yes | `[v]` |
| 0.0.2 | edit op, `why`, journal, undo/diff, HARNESS.md | yes | yes | `[v]` |
| 0.0.3 | signed-release chain | yes | yes | `[v]` |
| 0.0.4 | `screen` on Windows, cu-abi 1.0, semantic controls | yes | yes | `[v]` |
| 0.0.5 | three real backends, 12/10/8/0 matrix | yes | yes | `[v]` |
| 0.0.6 | actuation receipt + interaction closure | yes | yes | `[v]` |
| 0.0.7 | drag path, two-axis scroll | yes | yes | `[v]` |
| 0.0.8 | held-key modifiers | yes | yes | `[v]` |
| 0.0.9 | grant + environment disclosure | yes | yes | `[v]` |
| 0.0.10 | display scale / coordinate basis | yes | yes | `[v]` |
| 0.0.11 | MSAA structured control disclosure | yes | yes | `[v]` |
| 0.0.12 | handle-addressed focus | yes | yes | `[v]` |
| 0.0.13 | bounded observation wait | yes | yes | `[v]` |
| 0.0.14 | normalised a11y node schema | yes | yes | `[v]` |
| 0.0.15 | a11y-addressed actuation | yes | yes | `[v]` |
| 0.0.16 | a11y effect readback (expect vs observed) | yes | yes | `[v]` |
| 0.0.17 | native default action + tree dimension | yes | yes | `[v]` |
| 0.0.18 | a11y text write (`set_value`) | yes | yes | `[v]` |
| 0.0.19 | MCP adapter (H-line) | yes | core yes | `[v]` -- closed by 0.0.20's artifacts |
| 0.0.20 | LLM gate skeleton (M-line) | yes | yes | `[v]` -- gate + both sidecars in every archive |

Current phase: **0.0.20 published as a signed Release and accepted.** The
M-line gate skeleton shipped with the H-line packaging gap closed in the same
release: every archive now carries `minicon-harness`, `minicon-harness-mcp` and
`minicon-gate` on linux, macos and windows.

Evidence, all from run records rather than from a local machine:

| Run | What it proves |
|---|---|
| `42575589000` Gate loopback tier | linux 27 s, macos 48 s, windows 1 m 57 s -- the spike, the loopback and tunnel tiers, the SSE tier, the definition planes, the independent reader against the real binary, the full suite |
| `42577015780` minicon-harness.com | six courts green; the native archives contain both sidecars |
| `42578207057` Trusted Signing Court | Authenticode `Valid` on the APE and both Windows cells, Microsoft RFC 3161 timestamp, `release_eligible: true` |
| `42578498598` Release | verify, published-bytes execution, publish, Defender verdict and post-publish all green |

The contract is `prd/PRD_01_27_llm_gate.md` and the plan is
`plan/v0.0.20.md`; both carry W2-W4 records including every deviation. The
three contract questions flagged in the records -- `credentials[].endpoint`
versus `scope`, "the tunnel is established by the gate" versus `ureq` writing
the CONNECT, and the payoff example's `--base-url` -- were resolved in favour
of running code and are stated in the W4 record rather than re-litigated here.

## Roadmap (outcomes, not commitments)

Sequencing rule: one theme per release, one exit criterion per theme, and no
new CU mechanism until the previous theme has a real user. Version numbers are
provisional; every version still needs its own PRD, its own gate and a plan
that passes the 6/6 checklist before any code. Full analysis and rationale:
`plan/product-review-2026-11-09.md`.

### NOW -- 0.0.20: ship the mouth skeleton (M-line)

The recorded roadmap assigns 0.0.20 to the M-line gateway sidecar; the owner
direction of 2026-11-09 confirms the pool tuple. The gate is the smallest
surface that solves the measured proxy problem.

- `minicon-gate`: loopback-only; config + revision; the three-axis pool of
  `PRD_01_27` (endpoints, credentials, egress) with deterministic lane
  expansion; gate-established egress tunnels; per-axis cooldown that reorders;
  agent bindings; workflow declarations + `/resolve`;
  `/v1/chat/completions` (non-stream + SSE pass-through); `/v1/models`;
  health; read-only inspect; typed evidence rows + JSONL mirror.
- Contract: `prd/PRD_01_27_llm_gate.md`; plan: `plan/v0.0.20.md`.
- H-line closeout is a W0 precondition: the already-built
  `minicon-harness-mcp` becomes a signed second Release artifact, because it
  is the gate's first consumer.

Exit: two providers x two credentials x two egresses in one config; a 429
cools the credential while `direct` stays eligible; the reader recomputes the
expanded order and every skip from the revision alone; the adapter is
downloadable and mounted from one real client.

### NEXT -- 0.0.21: make the gate adoptable

- SSE / `data: [DONE]` compatibility audited against OpenCode, Claude Code and
  one raw client.
- Client recipes (`docs/clients/`), `doctor` output and error-text audit.
- `openai-responses` and `anthropic-messages` lanes if pulled.
- Adapter lifecycle hardening (timeouts, cancellation, restart).

Exit: one external client uses the gate for a week; one non-OpenAI protocol
lane verified.

### LATER -- 0.0.22+: harness lanes and subscription lanes

- Harness lanes (CLI / MCP / ACP) -- the original M-line "mouth".
- `openai-codex` subscription lane (external token refresher).
- One non-Windows CU cell qualified, or an explicit Windows-first statement.
- Weighted selection, tag queries, adapter `doctor`.

Exit: a harness backend is routed through the gate with the same evidence rows.

### BET -- 0.1.x: close the loop

Only if NOW/NEXT/LATER pull. Candidates, not commitments:

- L2-worker: A5 / D2 image arm, D4 ceilings, F3 second half, C5/C6.
- L3 demo: the controller drives the WorkBuddy GUI and aggregates sub-tasks.
- MCP image-content arm for the adapter (the companion's A5) if it has not
  shipped earlier.

Explicitly not on the roadmap: UIA routing (closed by measurement),
resource-domain authority (G closed), plugin host, policy engine, management
UI, Wayland-native, incremental a11y events.

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

The owner's model is **KVM plus resource management**
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

## Capability index (T1-T9)

One line per dimension; each subtree document owns the detail and the
dated records behind it.

### T1 鈥?perception (full tree: `PRD_01_19`)

```text
T1 perception
鈹溾攢鈹€ P1 pixel arm            [v] bounded PNG + scale disclosure; [no] image arm
鈹溾攢鈹€ P2 structured controls  [v] Win32 controls, MSAA tree, normalised schema
鈹?                          [v] a11y node is an act target (0.0.15)
鈹?                          [no] UIA routing (pass 5 downgrade);
鈹?                          [_] AT-SPI/AX, incremental events
鈹溾攢鈹€ P3 hybrid routing       [study] API -> tree -> pixels; [no] code-first here
鈹溾攢鈹€ P4 target identity      [v] hwnd focus, target_hwnd, stale refusal
鈹溾攢鈹€ P5 timing / freshness   [v] bounded wait + ceilings; [no] polling loop
鈹斺攢鈹€ P6 grounding            [study] resolution budget, bidirectional scaling
```

### T2 鈥?actuation (full tree: `PRD_01_20`)

```text
T2 actuation
鈹溾攢鈹€ K keyboard    [v] type / key / held modifiers; [no] layout or IME claim
鈹溾攢鈹€ M mouse       [v] click receipt / drag path / two-axis scroll
鈹溾攢鈹€ W window      [v] title + hwnd focus, live re-validation
鈹溾攢鈹€ A grammar     [v] one effect, why, verdicts, receipts;
鈹?                [v] `source`/`role` admission (0.0.15);
鈹?                [_] batched actions
鈹溾攢鈹€ C code-first  [study] emit-script-then-run; [no] in this repo
鈹斺攢鈹€ S safety      [v] UIPI/TCC named refusals; [_] injection defence, approval
```

### T3 鈥?resource domain (full tree: `PRD_01_21`, all `[study]` / `[no]`)

```text
T3 resource domain                     NOT screen authority (G)
鈹溾攢鈹€ R1 process / pid / namespaces / pids.max
鈹溾攢鈹€ R2 CPU       EEVDF, cpu.max/weight, throttling, cpuset, PSI
鈹溾攢鈹€ R3 memory    max/high/low/min, oom.group, THP/HugeTLB, PSI
鈹溾攢鈹€ R4 GPU       MIG, SR-IOV+IOMMU, vGPU, MPS, CUDA_VISIBLE_DEVICES
鈹溾攢鈹€ R5 I/O       io.max/io.weight, io_uring, tmpfs, snapshots
鈹溾攢鈹€ R6 net/device eBPF, default-deny egress, metadata block, LSM
鈹溾攢鈹€ R7 isolation container -> gVisor -> microVM -> full KVM
鈹溾攢鈹€ R8 lifecycle  credential broker, three timeouts, audit, quotas, stop
鈹溾攢鈹€ R9 cost       PSI telemetry, 7.3x price spread, memory-dominant bills
鈹斺攢鈹€ R10 acceptance guest-side read/cap/cleanup probes
    鈹斺攢鈹€ [decision] (a) keep G closed 鈥?controller decision 2026-11-06, T8
```

### T4 鈥?platform (full tree: `PRD_01_22`)

```text
T4 platform                             12 / 10 / 8 / 0 matrix
鈹溾攢鈹€ PL1 Windows  [v] EnumWindows, MSAA, SendInput; [study] UIA, UIPI, WGC
鈹溾攢鈹€ PL2 macOS    [v] 8/12, TCC boundaries, scale; [study] AX, bundle id
鈹溾攢鈹€ PL3 Linux    [v] 10/12, XTest, empty a11y tree; [study] AT-SPI2, portals
鈹溾攢鈹€ PL4 fallback [v] 0/12 with named reasons
鈹斺攢鈹€ PL5 contract [v] four cfgs, identical native signatures, type gate
```

### T5 鈥?capability maturity (full tree: `PRD_01_23`)

```text
T5 A-F capability index -> T1/T2/T4/T6 subtrees
鈹斺攢鈹€ L0 read-only [v] -> L1 evidenced interaction [v]
    -> L2 self-sustained loop [_] gates: A5/D2, B7b/D4 [v], A4, F3
    -> L3 self-hosted orchestration [_] gates: L2 + F3 + C5/C6
```

### T6 鈥?evidence (full tree: `PRD_01_22`)

```text
T6 evidence
鈹溾攢鈹€ E1 CI gates          [v] platforms + journeys + four-cfg type gate
鈹溾攢鈹€ E2 independent reader[v] producer never grades; no assertion weakened
鈹溾攢鈹€ E3 typed rows        [v] receipts, grants, scales, a11y schema, wait
鈹溾攢鈹€ E4 release chain     [v] receipts, sidecars, Authenticode, owner view
鈹溾攢鈹€ E5 external methods  [study] OSWorld 2.1 release contract, accept+reject
鈹斺攢鈹€ E6 evidence grades   [study] O / B / P / V / S / X; only X can be [v]
```

### T7 鈥?version state (full tree: `PRD_01_23`)

```text
T7 versions
鈹溾攢鈹€ V0.0.1 - V0.0.14   [v] published, accepted signed Releases
鈹溾攢鈹€ V0.0.15            [v] W1 UIA diagnostic probe; [v] a11y-addressed
鈹?                     actuation shipped as a public signed Release
鈹溾攢鈹€ V0.0.16            [v] a11y effect readback, expect-vs-observed;
鈹?                     public signed Release, accepted from the owner's view
鈹溾攢鈹€ V0.0.17            [v] native default action + tree dimension;
鈹?                     public signed Release, accepted from the owner's view
鈹溾攢鈹€ V0.0.18            [v] a11y text write (set_value); public signed
鈹?                     Release, accepted from the owner's view
鈹溾攢鈹€ V0.0.19            [v] core published + accepted; H-line MCP adapter
鈹?                     built and E1-E7 tested, but NOT a Release asset, so
鈹?                     the H-line is `[_]` (named packaging gap)
鈹溾攢鈹€ V0.0.20            [v] M-line gate skeleton; published, three-OS tier green
鈹溾攢鈹€ release evidence   0.0.1-0.0.19 CI ids + dated records -> `prd_hist.md`
鈹溾攢鈹€ line freeze        [decision] satisfied; harness/omp line eligible
鈹溾攢鈹€ candidate pool      [study] ordered by gate, never a version commitment
鈹斺攢鈹€ standing [no]      [no] unchanged; canonical list in "Non-goals"
```

### T9 鈥?semantic addressing (full tree: `PRD_01_25`)

```text
T9 semantic addressing          identity -> bounded effect -> readback
鈹溾攢鈹€ S1 identity   [v] Win32 controls, MSAA a11y nodes, live hwnd;
鈹?                [no] a pixel coordinate as an identity
鈹溾攢鈹€ S2 resolver   [v] exact name -> unique substring -> optional role filter;
鈹?                [v] ambiguity / unknown role / vetoed state refused
鈹溾攢鈹€ S3 admission  [v] `source` enum, generation pin, empty tree named refusal;
鈹?                [v] press=false default, no click from a bare resolution
鈹溾攢鈹€ S4 effect     [v] same hover/click backend, readback inside the node rect;
鈹?                [v] native default-action + tree dimension shipped in 0.0.17
鈹溾攢鈹€ S5 evidence   [v] `a11y_node` schema, reader recomputes state_names,
鈹?                centre and containment; no earlier assertion weakened
鈹溾攢鈹€ S6 three OS   [v] Windows real button hover; other cells named refusal;
鈹?                [v] 12/10/8/0 matrix and four-cfg gate unchanged
鈹斺攢鈹€ S7 maturity   [v] L1 addressable interaction; [_] L2 / L3 gates unchanged
```

### T8 鈥?decision authority (full tree: `PRD_01_24`)

```text
T8 decision authority (controller-autonomous under /goal)
鈹溾攢鈹€ D1 owner directions, dated and recorded verbatim                [v]
鈹溾攢鈹€ D2 controller decisions, no prior owner approval              [decision]
鈹?  鈹溾攢鈹€ version topic / ordering / gate / W1-W4 timing
鈹?  鈹溾攢鈹€ additive capability inside cu-abi 1.0 + 12/10/8/0 matrix
鈹?  鈹溾攢鈹€ lifting one [no] with dated reason and bounds
鈹?  鈹溾攢鈹€ G boundary direction: (a) keep closed, 2026-11-06
鈹?  鈹斺攢鈹€ release chain, CI gates, owner-view acceptance
鈹溾攢鈹€ D3 owner veto after the fact, never a pre-approval gate         [v]
鈹溾攢鈹€ D4 escalation discipline                                       [no]
鈹?  鈹斺攢鈹€ [no] pasting options + waiting on a reversible default
鈹斺攢鈹€ D5 decision record: date / choice / reason / evidence / trigger [v]
```

## CU capability tree (benchmarked against `prd/prd-cu-openai.md`)

This is the **single index of computer-use features**: where
a capability lives, who owns its contract, and which version may implement it.
Legend as above: `[v]` delivered with evidence, `[-]` partial/open, `[_]`
planned, `[no]` explicit non-goal. A `[_]` means a chapter exists, not that
code exists. The external baseline is `prd/prd-cu-openai.md` (OpenAI Responses
API `computer` tool, Codex, ChatGPT agent/Work); raw evidence is kept outside
the repo in a temporary corpus. No OpenAI field name is imported by this tree.

```text
minicon-harness computer use
鈹溾攢鈹€ A Observation
鈹?  鈹溾攢鈹€ [v] A1 pixel arm: virtual screen, displays, window inventory, PNG
鈹?  鈹?       captured under the task root; desktop/window/region, scale
鈹?  鈹?       owner PRD 01.03 / 01.06
鈹?  鈹溾攢鈹€ [v] A2 typed Win32 child controls: label, class-derived role, rect,
鈹?  鈹?       text, state; deliberately NOT a UIA tree   owner PRD 01.05
鈹?  鈹溾攢鈹€ [v] A3 foreground title, multi-monitor negative coordinates,
鈹?  鈹?       bounded capture arithmetic                 owner PRD 01.06
鈹?  鈹溾攢鈹€ [v] A4 Windows MSAA read-only tree in `observe.a11y` (0.0.11)
鈹?  鈹?       bounded 400 rows / depth 12; no new grant, crate or elevation
鈹?  鈹?       owner PRD 01.15; AT-SPI (Linux) / AX (macOS) still [_]
鈹?  鈹?       unlock: per-app TCC grant on macOS; AT-SPI D-Bus bus + ATK
鈹?  鈹?       fixture on Linux; each green on real CI before declaring true
鈹?  鈹斺攢鈹€ [_] A5 image arm in the wire codec: base64 PNG to the model each turn
鈹?           unlock: separate PRD + budget/retention decision
鈹溾攢鈹€ B Actuation
鈹?  鈹溾攢鈹€ [v] B1 pointer: hover, click, drag, scroll + pointer readback
鈹?  鈹溾攢鈹€ [v] B2 keyboard: type, key
鈹?  鈹溾攢鈹€ [v] B3 window: focus, move + rect readback (Linux/Windows)
鈹?  鈹溾攢鈹€ [v] B4 named semantic control: resolve + hover/press (Windows only)
鈹?  鈹溾攢鈹€ [v] B5 actuation receipt: button, downs/ups, double, target, plus
鈹?  鈹?       post-observe live session                 shipped in 0.0.6
鈹?  鈹溾攢鈹€ [v] B6a multi-point drag path                  qualified in 0.0.7
鈹?  鈹?       16 bounded points, 12 steps per segment, `path_points` receipt
鈹?  鈹溾攢鈹€ [v] B6b held-key modifiers                    qualified in 0.0.8
鈹?  鈹?       host-owned held set, four-backend set/release, teardown release
鈹?  鈹溾攢鈹€ [v] B7a two-axis scroll                      qualified in 0.0.7
鈹?  鈹?       `delta_x`/`delta_y`, `delta` stays the vertical alias
鈹?  鈹溾攢鈹€ [v] B7b explicit bounded wait                 shipped in 0.0.13
鈹?  鈹?       one sleep per observe call, monotonic elapsed evidence
鈹?  鈹溾攢鈹€ [_] B8 batched `actions[]` with ordered receipts
鈹?  鈹?       unlock: its own admission/stale design
鈹?  鈹斺攢鈹€ [no] B9 browser-navigation buttons (`back`/`forward`/`wheel` as a
鈹?           button enum): desktop CU has no browser history to navigate
鈹溾攢鈹€ C Contract and safety
鈹?  鈹溾攢鈹€ [v] C1 `cu-abi 1.0`, CAPABILITIES, disclose, six typed verdicts
鈹?  鈹溾攢鈹€ [v] C2 grant (`--allow-screen`), required `why`, task-root bounds
鈹?  鈹溾攢鈹€ [v] C3 generation / `as_of` pin / stale refusal / receipts / evidence
鈹?  鈹溾攢鈹€ [v] C4 per-OS capability matrix and named refusals
鈹?  鈹溾攢鈹€ [_] C5 `pending_safety_checks` + acknowledged checks + human hand-off
鈹?  鈹?       unlock: a policy PRD before any code
鈹?  鈹溾攢鈹€ [_] C6 untrusted-screen-content policy (prompt injection)
鈹?  鈹?       unlock: distinct from grant/admission
鈹?  鈹斺攢鈹€ [no] C7 takeover / owner-lock / watch mode (top-level non-goal)
鈹溾攢鈹€ D Loop and state
鈹?  鈹溾攢鈹€ [v] D1 one bounded tool call -> one verdict -> one evidence record
鈹?  鈹溾攢鈹€ [_] D2 screenshot-per-turn loop with a model-visible image (needs A5)
鈹?  鈹溾攢鈹€ [_] D3 session continuity beyond `as_of` (OpenAI uses
鈹?  鈹?       `previous_response_id`; it does NOT restore login/runtime state)
鈹?  鈹斺攢鈹€ [_] D4 bounded iterations / time / cost ceilings
鈹?           partial: per-host wait ceilings shipped in 0.0.13;
鈹?           iteration and cost ceilings stay gated on L2
鈹溾攢鈹€ E Platform and environment
鈹?  鈹溾攢鈹€ [v] E1 three real backends: Win32, CG/CF, X11/XTest via dlopen
鈹?  鈹溾攢鈹€ [v] E2 six cells cross-compiled on one runner, no link-time platform lib
鈹?  鈹溾攢鈹€ [v] E3 environment enum + display width/height in `disclose`
鈹?  鈹?       delivered in 0.0.9; geometry is already in the pixel arm
鈹?  鈹溾攢鈹€ [v] E4 OS-grant matrix in evidence (Screen Recording, Input
鈹?  鈹?       Monitoring, Accessibility) 鈥?delivered in 0.0.9
鈹?  鈹溾攢鈹€ [v] E5a display scale / coordinate basis in `disclose`
鈹?  鈹?       delivered in 0.0.10; additive evidence, not a capability row
鈹?  鈹溾攢鈹€ [v] E5c structured `a11y_controls[]` in `observe`
鈹?  鈹?       delivered in 0.0.11; Windows MSAA, other cells unknown+reason
鈹?  鈹溾攢鈹€ [v] E5b `act:focus` hwnd + `target_hwnd`   delivered in 0.0.12
鈹?  鈹?       three-OS CI, handle identity is an act target, not a scan arm
鈹?  鈹斺攢鈹€ [no] E5 Wayland-native protocol, runtime backend selection
鈹溾攢鈹€ F Verification and delivery
鈹?  鈹溾攢鈹€ [v] F1 CI compile gate + three-OS real-desktop journeys + independent
鈹?  鈹?       reader recomputing matrix, readback, PNG IHDR, generation
鈹?  鈹溾攢鈹€ [v] F2 public company-signed Release, Defender and post-publish court
鈹?  鈹斺攢鈹€ [_] F3 app-level effect evidence via a self-hosted echo window
鈹?           unlock: side-effect-free fixture on three OSes
鈹斺攢鈹€ G Resource-domain boundary 鈥?NOT a capability axis; owner PRD 01.08
    鈹溾攢鈹€ [no] process spawn / inspect / signal / kill / usage / maps / fds:
    鈹?       `exec` owns processes; a window's `pid` is identity metadata
    鈹溾攢鈹€ [no] memory, CPU, load, topology and hardware inventory
    鈹溾攢鈹€ [no] network, disk and device management; displays and input devices
    鈹?       are preconditions of K/V/M, not objects this tool manages
    鈹溾攢鈹€ [no] window-manager product behavior (minicon-winmgr owns it);
    鈹?       here W is target binding: focus/move + inventory + named control
    鈹斺攢鈹€ [no] widening `screen` into host control by adding one verb: a new
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

`prd/PRD_01_12_self_hosted_orchestration.md` defines four levels 鈥?L0
read-only observation, L1 evidenced interaction, L2 self-sustained loop, L3
self-hosted orchestration 鈥?and records the owner's end state: this session
acts as controller, using `screen` to create, dispatch, read and retire
sub-task sessions in the WorkBuddy client's own GUI, then aggregate their
results. L2 is the lowest level at which that loop closes on its own; L3 is the
full form. It is an **acceptance journey for computer use**, not a new verb,
tool, harness capability or version plan, and it changes no `[v]`, `[_]` or
`[no]` mark in the tree above.

Candidate ordering follows what the L2/L3 loop needs (A5/D2 -> B7b + D4 ->
A4 -> F3 -> C5/C6, each with its unlock condition in the trees above).
Ordering is a reason to consider an item first, never a version commitment;
the dated ordering history is archived in `prd_hist.md`, and the live
candidate pool lives in `PRD_01_17` / `PRD_01_23`.

## Non-goals (canonical)

This is the single canonical list. Every other document inherits it; a
per-version section only states whether it changed, never repeats the list.

### Worker core

- `[no]` plugin host, permission-policy engine, streaming, MCP, or a scripting
  surface **inside the worker core**. The MCP adapter and the gate are
  sidecars beside it: they widen no tool, add no plugin host to the core, and
  inherit every core `[no]` unless a dated decision says otherwise.
- `[no]` a second model loop, takeover / owner-lock / watch mode.
- `[no]` any new crate or runtime, bun / WASM / ujs embedding (PRD 31
  readiness stays BLOCKED), runtime backend selection, Wayland-native protocol.
- `[no]` host-resource management: process, memory, CPU, GPU, device, network
  and storage authority (G boundary, `PRD_01_08`). `exec` owns processes,
  `file` owns storage, `screen` stays K/V/M/W/S plus target binding.

### MCP adapter

- `[no]` a network listener (stdio only), a plugin host, a policy engine, a
  streaming transform, its own model loop, or any widening of file / exec /
  screen semantics. Every core refusal passes through byte-identical.

### LLM gate

- `[no]` tool execution, a policy engine (cooldown and concurrency are
  transport state: they reorder or refuse one attempt, they do not decide
  whether a task may run), a plugin runtime, a management UI, workflow
  execution / a scheduler, a second model loop, request or response caching,
  credentials at rest, a provider metadata table, or a non-loopback listener
  by default. Contract owner: `prd/PRD_01_27_llm_gate.md`.

## Continuous learning (W0)

Each cycle keeps learning current computer-use and resource-domain technology,
records dated notes with an evidence grade in `PRD_01_17`, folds findings into
the T1-T9 trees, and **completes the plan before starting code or tests** --
涓夋€濆悗琛?(think three times before acting) and 鍒嗚€屾不涔?(divide and conquer).
A research finding is not authority: no `[v]`, `cu-abi`, matrix row, `[no]` or
G boundary changes because an article exists. Only three-OS CI plus the
independent reader plus owner-view acceptance produce a `[v]`.

## Evidence and decision rules

Evidence grades: **O** official, **B** benchmark board, **P** paper,
**V** vendor claim, **S** survey / secondary, **X** local measurement. Only
this repo's CI and owner-view acceptance (**X**) produce `[v]`. A study note
never becomes a capability by itself.

Decision authority (T8, `PRD_01_24`): under the standing delegation the
controller owns learn -> structure -> plan -> order -> gate -> implement ->
release -> accept, records every decision as date / choice / reason / evidence
/ reopen trigger, and reports it; the owner keeps a post-hoc veto, never an
approval queue. `[decision]` means a dated controller decision already taken.

## Document map

```text
PRD.md                                     top: product, status, roadmap, trees
prd_hist.md                                archived history, release evidence, dated records
plan/product-review-2026-11-09.md          product review, rationale, PRD split
prd/
鈹溾攢鈹€ PRD_01_08_computer_use_scope.md        K/V/M/W/S domain model + G boundary
鈹溾攢鈹€ PRD_01_12_self_hosted_orchestration.md L0-L3 end state
鈹溾攢鈹€ PRD_01_17_frontier_radar.md            W0 learning ledger + candidate table
鈹溾攢鈹€ PRD_01_19_cu_perception_subtree.md     T1 observation / perception
鈹溾攢鈹€ PRD_01_20_cu_actuation_subtree.md      T2 actuation / input
鈹溾攢鈹€ PRD_01_21_resource_domain_subtree.md   T3 resource domain (all study / no)
鈹溾攢鈹€ PRD_01_22_platform_evidence_subtree.md T4 platform + T6 evidence
鈹溾攢鈹€ PRD_01_23_capability_maturity_subtree.md T5 maturity + T7 version state
鈹溾攢鈹€ PRD_01_24_decision_authority_subtree.md T8 who decides what (controller)
鈹溾攢鈹€ PRD_01_25_semantic_addressing_subtree.md T9 identity -> bounded effect
鈹溾攢鈹€ PRD_01_27_llm_gate.md                   M-line gate contract (0.0.20, [v])
鈹溾攢鈹€ PRD_01_01 .. PRD_01_16, PRD_01_18    per-version contracts (unchanged)
鈹斺攢鈹€ prd-cu-openai.md                      external reference, NOT a contract
plan/
鈹溾攢鈹€ plan-0.0.1 .. plan-0.0.18             per-version plans (archived records)
鈹溾攢鈹€ v0.0.19.md                            H-line plan + D10 answers + W0 pass 11
鈹溾攢鈹€ v0.0.20.md                            0.0.20 plan -- LLM gate skeleton (6/6, D13)
鈹斺攢鈹€ v0.0.20-handoff.md                    implementation task order for the dev agent
```

Every dated record moved out of this file is kept verbatim in `prd_hist.md`:
the phase and roadmap history, the W0 learning ledger, the pre-0.0.4 product
snapshot and the release frontiers 0.0.1-0.0.15 plus the 0.0.16-0.0.18
records. The 0.0.19 record lives in `plan/v0.0.19.md`.

History and evidence are archived in [`prd_hist.md`](prd_hist.md); new
dated records go there, never back into this file.
