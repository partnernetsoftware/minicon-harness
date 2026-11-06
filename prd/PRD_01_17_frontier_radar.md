# PRD 01.17 — continuous frontier radar (CU + resource domain)

Owner of **what this project learns, and how a learning becomes a candidate**.
Status: **document only** — it changes no source, version, capability matrix,
`cu-abi`, `[no]` row or the G boundary. Owner direction (2026-11-06):

> Continuously learn the newest computer-use and resource-management
> technology (process, CPU, memory, GPU, keyboard, video, mouse — all of it),
> continuously upsert the PRD, then implement.

Legend: `[v]` delivered with evidence, `[_]` candidate, `[no]` explicit
non-goal. A dated finding is a **research note**, not a capability claim.

## The W0 loop

Every version cycle starts with W0, before W1:

1. **Learn** — one research pass over (a) CU perception, actuation, timing and
   safety (K / V / M / W / S), and (b) the resource domain: process, CPU,
   memory, GPU, device, network and their isolation primitives.
2. **Upsert** — append or revise this file and `PRD.md` in place. Never
   overwrite a dated finding; record the date and the source class (official
   doc, benchmark board, vendor claim, survey). Add the finding to the
   candidate table with its unlock condition.
3. **Decide** — pick the highest candidate whose gate is already open and
   whose change stays inside `cu-abi 1.0`, the 12/10/8/0 matrix and the
   G boundary of `PRD_01_08`. Ordering is a reason, not a version commitment.
4. **Implement** — the usual W1/W2/W3/W4 chain. A capability is claimed only
   from three-OS CI plus the independent journey reader, never from an article.

**Learning is not authority.** A published technique becomes a capability
only after its own PRD, its own gate and — if it crosses G — an explicit owner
decision. This file lifts no `[no]`, adds no verb, no grant and no tool.

## Findings, 2026-11-06

Source classes below matter: a vendor-reported benchmark number and an
official leaderboard entry are not the same kind of evidence.

### CU perception and grounding

- **Interface-first, pixels as fallback is the 2026 consensus.** The order is
  connector / native API / MCP -> accessibility tree -> screenshot coordinates.
  A pixel loop costs roughly 1.5-4 s per action; 92% per-action accuracy over
  10 actions compounds to ~43% whole-task success (production engineering
  survey, not a controlled benchmark). Google stopped Project Mariner on
  2026-05-04 and folded it into Gemini / Chrome auto-browse — a signal that
  *vision-as-default for the web* is retreating, not that pixels disappear for
  native desktop apps, which have no other path.
- **UIA supersedes MSAA on Windows.** Microsoft's own guidance recommends UIA
  selectors for WPF / WinForms / UWP and keeps MSAA for legacy VB6 / classic
  Win32; `UIA3 Raw` exists specifically for custom-rendered and Electron
  controls. Windows Agent Arena reported an open-source perception stack
  rising from 8.6% to 13.1% when the UIA tree was added (~57% relative).
  `UFO2` (Microsoft) fuses UIA with vision, prefers Office COM / native APIs
  over simulated clicks, and reports 27.9% on WAA with GPT-4o.
  **Consequence for this repo:** `v0.0.11` shipped a bounded read-only
  MSAA / IAccessible tree. UIA-first routing with MSAA fallback is the next
  structured-observation candidate; it needs no OS grant and no ABI change.
- **One Windows API is never enough.** PrintWindow returns black frames for
  DirectComposition / UWP surfaces (WGC is the fallback); UIA cache requests
  avoid thousands of cross-process property calls on a Chromium tree; VCL / SAL
  providers can hang on subtree caches and need the MSAA route; DWM window
  rectangles include invisible shadows. `SendInput` is UIPI-limited and UAC's
  secure desktop cannot be clicked by the agent. macOS AX trees are TCC-gated
  (Accessibility / Screen Recording / Input Monitoring); Linux AT-SPI2 needs
  an a11y bus at session start. These are exactly the reasons this repo keeps
  four backend configurations and publishes named reasons instead of guesses.
- **A cross-platform agent needs one normalised node schema** (role, name,
  state, value, bounds, relations) with per-platform adapters, rather than
  teaching the model four native vocabularies. The a11y tree is an
  event-maintained stream, not a static snapshot: polling full trees is
  wasteful and stale; subscribing to changes is nearly free. This is a
  candidate for a later observation version, gated on three-OS fixtures.

### Timing, long-horizon work and evidence

- **Benchmark inflation is real.** OSWorld-Verified (369 Ubuntu tasks, human
  baseline 72.36%) had 16 entries above the human baseline by 2026-10, top
  90.19% on 2026-07-25 (official board, 100-step limit) — it now measures
  competence, not the frontier. OSWorld 2.0 / 2.1 (108 long-horizon
  workflows, ~1.6 h human median, 250+ agent steps, 27+ checkpoints) has a
  best official **binary completion of 44.33%** while vendor-quoted *partial*
  scores reach ~80%. Step budget is a confounder (one model moved 9.1% ->
  23.0% on a limit change alone). Never compare a partial score to a binary
  completion, or a 100-step run to a 500-step run.
- **The gap is a loop-engineering gap, not a grounding gap.** Short-task
  grounding is largely solved; long-horizon failures are cognitive and
  temporal: losing constraints, missing mid-task state, guessing, skipping
  verification. The field's formula converges on `Agent = Model x Harness`.
  This is the same hard evidence that produced this repo's B7b / D4 decision
  (`v0.0.13` bounded observation wait): an observe immediately after an act
  captured a pre-reflow Chromium frame. An explicit, bounded, evidenced wait
  is the smallest honest unit of a self-sustained loop.
- **Independent recomputation is the standard.** Execution-based end-state
  checkers (OSWorld) and parallelised three-OS runs (WAA ~20 min) are what
  make an agent claim reproducible. This repo's `scripts/cu-journey-verify.py`
  is the same idea: the model that produced the journey never grades it.

### Resource domain: process, CPU, memory, GPU, device, network

The owner's original model includes these; `PRD_01_08` records that they are
**not** `screen` authority. Learning them is in scope, exercising them is not.

- **Isolation ladder:** shared-kernel containers (namespace + cgroup) ->
  gVisor user-space kernel (syscall interception) -> Firecracker / Kata
  microVM (one guest kernel per workload) -> full VM. 2026 converged on a
  disposable per-task microVM for untrusted agent code; Docker moved its own
  Sandboxes to microVM isolation. Boot numbers (~125 ms Firecracker, <200 ms
  gVisor, 1-2 s microVM in GKE docs) are measurements in different
  environments, not comparable benchmarks.
- **Kernel control primitives:** cgroups v2 unified hierarchy (`memory.max`
  hard / `memory.high` soft, `cpu.max`, `io`, `cpuset`, device filtering via
  eBPF), PSI pressure telemetry, EEVDF scheduler (6.6+), namespaces,
  Landlock / seccomp-BPF / AppArmor / SELinux. A cgroup is a boundary, not a
  placement algorithm; CPU affinity and NUMA placement are separate.
- **GPU is not a cgroup device file.** MIG gives hardware-backed partitions
  with memory and fault isolation; MPS shares execution capacity without
  equivalent isolation; SR-IOV / IOMMU binds PCIe virtual functions;
  `CUDA_VISIBLE_DEVICES` is a hint, not a security boundary. Leakage paths are
  processes created outside the step cgroup, writable device files and
  container runtimes that reparent processes.
- **Four production boundaries**, all four required: isolation, default-deny
  egress with domain/CIDR allow-lists, credential brokerage (the agent never
  holds a long-lived key), and lifecycle + immutable audit + enforceable
  quotas. NVIDIA OpenShell's declarative policy (Landlock, seccomp, cgroups
  v2, egress proxy, MIG profile) is a representative shape.
- **Acceptance must be from inside the guest**: try to read host paths, device
  files and the metadata endpoint; try each denied network class; saturate
  CPU/memory and confirm the cap fires without hurting a neighbour; test
  cleanup on normal exit, crash, timeout and forced stop. This is the same
  owner-view rule this repo already applies to public Releases.

## Candidate table (upserted, not promised)

| Finding | Repo axis | Candidate | Unlock condition | This cycle |
|---|---|---|---|---|
| UIA supersedes MSAA; UIA-aware agents measurably better | A2 / A4 | UIA-first + MSAA fallback in `observe.a11y`, additive `a11y_source` | three-OS reader rule, no new grant, no ABI change | next implementation candidate |
| WGA / PrintWindow black frames, DWM shadow rects | A5 / E | per-window capture router with named reason | image arm + PNG budget decision (A5/D2) | deferred |
| a11y trees are event streams; normalise node schema | A4 / D | incremental change subscription, one node schema | three-OS AT-SPI / AX grants + fixtures | deferred |
| Wait before re-observation; budget ceilings | B7b / D4 | `observe.wait_ms` + `evidence["wait"]` | none (gate open) | **shipped in 0.0.13 W1/W2** |
| Long-horizon verification and independent readers | F3 / L2 | app-level effect evidence | side-effect-free three-OS fixture | deferred |
| Sandbox isolation ladder, quotas, credential brokerage | G / exec | resource-domain telemetry and control | separate PRD + explicit G decision | **not implemented; G unchanged** |
| Guest-side acceptance of isolation | F / delivery | extend owner-view acceptance to resource cells | only if G is ever lifted | deferred |

## Open decision, deliberately not taken here

`PRD_01_08` says the readback proves **desktop state, never process, memory
or device state**. Three mutually exclusive directions exist; this cycle
chooses (a) and records the other two without implementing them:

- **(a) keep G closed (chosen).** Learn and upsert the whole resource domain,
  implement only inside KVM + W + S. Cost: no process/CPU/memory/GPU verbs.
- **(b) additive read-only telemetry in `disclose`.** Publish host CPU/memory
  pressure, process count or GPU inventory as typed rows, still no control.
  Cost: lifts one `[no]`, needs a three-OS probe with named `unknown` reasons
  and a fresh PRD; a "read-only" row still widens what `screen` may observe.
- **(c) real resource control.** cgroups / MIG / microVM lifecycle actions.
  Cost: a new tool or ABI, new grants, a policy engine and an audit surface —
  this is a different product from a bounded one-file worker.

Any of (b) or (c) requires an explicit owner decision; a research finding is
not one. Nothing in this file authorises a change to the 12/10/8/0 matrix,
`cu-abi 1.0`, the six false cells or any shipped Release asset.
