# PRD 01.17 — continuous frontier radar and W0 learning ledger

Owner of **what this project learns, how the learning is recorded, and how a
finding becomes a candidate**. Status: **document only** — it changes no
source, version, capability matrix, `cu-abi`, `[no]` row or the G boundary.

Owner directions, in order:

- **2026-11-06 (day):** continuously learn the newest computer-use and
  resource-management technology (process, CPU, memory, GPU, keyboard, video,
  mouse — all of it), continuously upsert the PRD, then implement.
- **2026-11-06 (evening):** keep learning, and **the learning process itself
  must leave notes in the PRD**; the PRD must expand into **trees over several
  dimensions**; large branches are split into `*_subtree.md` documents and
  indexed in `PRD.md`; **do not start code or tests until the plan is
  complete** — 三思后行 (think three times before acting) and 分而治之
  (divide and conquer). This supersedes the "implement next" pace for the
  current phase.

- **2026-11-06 (22:15):** 为什么要我做决定，你能自己安排吗？ — the owner
  rejects having reversible choices escalated back. The controller decides
  and reports the reason; the owner keeps a veto, not an approval queue
  (`PRD_01_24`, T8).

Legend: `[v]` delivered with evidence, `[_]` candidate, `[no]` explicit
non-goal, `[study]` research note with no implementation claim, `[decision]`
a dated controller decision already taken. A dated finding is a **research
note**, not a capability claim.

## The W0 loop (revised for the study phase)

1. **Learn** — one research pass over (a) CU perception, actuation, timing,
   verification and safety (K / V / M / W / S), and (b) the resource domain:
   process, CPU, memory, GPU, device, network, storage, credentials,
   lifecycle and cost.
2. **Upsert** — append or revise this file, the relevant subtree document and
   the `PRD.md` ledger **in place**. Never overwrite a dated finding; record
   the date, the query, the source class, the conflict or caveat and where
   the finding went.
3. **Structure** — fold each finding into the correct node of T1-T8, split
   any oversized branch into its own subtree document, and keep `PRD.md`'s
   document index accurate.
4. **Complete the plan** — for the next version, write the contract, the
   downgrade branch, the reader rules, the three-OS behaviour and the
   non-goals, then pass the plan-completeness checklist in `PRD_01_23`.
5. **Only then implement** — the usual W1/W2/W3/W4 chain. A capability is
   claimed only from three-OS CI plus the independent journey reader, never
   from an article.

**Learning is not authority.** A published technique becomes a capability
only after its own PRD, its own gate and a dated controller decision
(`PRD_01_24`). This file lifts no `[no]`, adds no verb, no grant and no tool.

## Current phase (updated 2026-11-07)

```text
STUDY -> STRUCTURE -> PLAN COMPLETION -> ONLY THEN CODE + TESTS
```

The STUDY + STRUCTURE phase is **closed**. W0 pass 7 produced X-grade
evidence that the L3 target is addressable on the MSAA arm (43 nodes, 8
buttons, 11 named, while the Win32 scan is refused); a fresh 6/6 plan
(`plan/plan-0.0.15-a11y-addressed-actuation.md`) was completed **before**
any code started, and W1-W4 then ran. The 2026-11-06 UIA probe record is
preserved verbatim above as history; its downgrade still holds for the UIA
route. All four version sources now read `0.0.15`.

## Evidence grades

| Grade | Meaning | Rule for this repo |
|---|---|---|
| **O** | official documentation / API reference | may inform a design decision |
| **B** | benchmark leaderboard / official board | quote with metric and step budget |
| **P** | paper / preprint | describe the method, not a product score |
| **V** | vendor claim / marketing material | never treat as an independent fact |
| **S** | survey / secondary engineering write-up | cross-check before quoting a number |
| **X** | local measurement / this repo's CI | the only grade that can support `[v]` |

Vendor share numbers, cross-environment cold-start times, partial-credit vs
binary scores and results from different step budgets are **not comparable**.

## Subtree map

| Tree | Dimension | Owner document |
|---|---|---|
| T1 | observation / perception | `prd/PRD_01_19_cu_perception_subtree.md` |
| T2 | actuation / input | `prd/PRD_01_20_cu_actuation_subtree.md` |
| T3 | resource domain (largest) | `prd/PRD_01_21_resource_domain_subtree.md` |
| T4 / T6 | platform + evidence | `prd/PRD_01_22_platform_evidence_subtree.md` |
| T5 / T7 | capability maturity + versions | `prd/PRD_01_23_capability_maturity_subtree.md` |
| T8 | decision authority | `prd/PRD_01_24_decision_authority_subtree.md` |
| domain model / G | K/V/M/W/S boundary | `prd/PRD_01_08_computer_use_scope.md` |
| L0-L3 end state | self-hosted orchestration | `prd/PRD_01_12_self_hosted_orchestration.md` |

## Learning log (process notes, dated)

| Date | Pass | Query intent | Source classes | Conflict / caveat recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | CU technology route + industry shape; resource isolation ladder | O / B / P / V / S | OSWorld Verified >90% vs 2.0 binary ~20-44%; partial vs binary and 100 vs 500 steps are not comparable; domestic market shares are vendor self-reports | `PRD_01_17`, T1/T3/T7 |
| 2026-11-06 | 1 | UIA vs MSAA, WAA, UFO2; three-OS a11y grants | O / P / S | WAA 8.6%->13.1% is one perception stack; "no grant on Windows" is integrity-level conditional | T1/T4 |
| 2026-11-06 | 2 | `IUIAutomation` COM vtable order and interop traps | O / S (Go binding, MS Q&A) | `RECT` (4x int) != `UiaRect` (4x double); a swallowed HRESULT hides a failed read; physical pixels vs the 0.0.10 scale basis | T1 P2.4, `plan-0.0.15` |
| 2026-11-06 | 3 | perception routes, grounding, code-first, safety | S / P / O / V | 150 ms vs 2.2-5 s per step are different setups, not a controlled A/B; code-first claims are vendor launches; NOVA's 57% is OSWorld-specific | T1/T2/T6 |
| 2026-11-06 | 3 | cgroups v2 / EEVDF / PSI / GPU virtualisation / sandbox cost | O / S / V (multi-source) | MPS shares a failure domain; `CUDA_VISIBLE_DEVICES` is not a boundary; 7.3x price spread at equal specs has two full-KVM endpoints; memory is 72-80% of active-CPU bills | T3 |
| 2026-11-06 | 3 | platform grants, input tiers, OSWorld 2.1 release contract | O / P / S | 2.1 manifest pins components but disclaims a full 108-task evaluation; a tag or smoke test is not full verification | T4/T6 |
| 2026-11-06 | 4 | local `UIAutomationClient.h` / `.idl` (SDK 10.0.22000.0): CLSID/IID, exact vtable slots, `RECT`, `TreeScope` | O (installed SDK) | `RECT` is left/top/right/bottom, not `[x,y,w,h]`; the header proves slots for this SDK, not for every runtime; raw HRESULTs still needed | T1 P2.4, `plan-0.0.15` |
| 2026-11-06 | 4 | resource telemetry trust: host-side vs guest-side observability | O / P / S | guest self-report is debugging data, never a security boundary; host-side signals are on the far side of isolation | T3 R9/R10 |
| 2026-11-06 | 4 | decision authority under `/goal` | owner direction | a reversible default was wrongly escalated; owner veto replaces the approval queue | `PRD_01_24`, T8 |
| 2026-11-06 | 5 | W1 UIA probe verdict on a real Windows desktop | X (this machine) | every COM slot returned S_OK and the desktop root read `0,0,1680,1050`, yet the target WorkBuddy provider gave 8 Pane nodes with 0 non-Pane controls; cold-start RECTs were all zero but four re-runs returned `840,0,1680,1014`. A correct vtable implies neither a useful provider nor a stable first read | `plan-0.0.15`, T1 P2.4, T7 |
| 2026-11-06 | 6 | Electron `AXMode` activation, AT-SPI2/Wayland tiers, macOS TCC, AT-SPI pidfd leak | O (Electron/Chromium docs) / S / V | a11y tree emptiness is an app-launch/AT-detection property, not a CU verb; TCC grants bind to the responsible process and code signature; Wayland is semantic-only | T1, T3 R10, T4, `plan-0.0.15` reopen trigger |

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

## Findings, 2026-11-06 (second pass, UIA gate)

A second W0 pass targeted the one candidate whose gate was closed for a
technical reason rather than a policy one: hand-written UIA COM.

- **The `IUIAutomation` vtable order was first published by a secondary
  source** (Go `w32uiautomation` binding): after the three `IUnknown` slots
  come `CompareElements`, `CompareRuntimeIds`, `GetRootElement`,
  `ElementFromHandle` (slot 6), `ElementFromPoint`, `GetFocusedElement`, the
  four `*BuildCache` variants, `CreateTreeWalker`, the three view walkers,
  the three view conditions, `CreateCacheRequest`, `CreateTrueCondition`
  (slot 21), `CreateFalseCondition`, `CreatePropertyCondition`, the
  and/or/not condition combinators, the event handlers, and finally
  `ElementFromIAccessible` / `*BuildCache`. **Pass 4 below verified these
  slots against the installed Windows SDK headers** and is now the primary
  source; the binding is retained only as the history of the finding.
- **Three real failure modes are documented, not guessed** (Microsoft Q&A
  and Windows-MCP / UIA wrappers): `get_CurrentBoundingRectangle` returns a
  `RECT` of four 32-bit integers (left/top/right/bottom), **not** the
  four-double `UiaRect`; swallowing a failed HRESULT into a default
  zero-sized rectangle makes a read failure indistinguishable from a real
  empty box; UIA bounds are physical pixels, so they must be reconciled with
  the DPI basis this repo already discloses in `v0.0.10`. Virtualised lists
  expose only realised rows, and providers expose less when a window is
  minimised, locked or in another integrity/session context.
- **The practical route is a probe first** (same shape as the 0.0.11 MSAA
  probe): a small Rust example that creates `CLSID_CUIAutomation`, calls
  `ElementFromHandle` + `CreateTrueCondition` + `FindAll`, reads one name and
  one `RECT`, prints the raw HRESULTs and exits — before any production
  routing. A wrong slot is still a crash, not a named refusal.

**Gate status (pass 4):** *vtable order verified against the locally
installed Windows SDK headers*, real-desktop probe still required. No UIA
code ships in 0.0.14 or 0.0.15 until the probe prints raw HRESULTs.

## Findings, 2026-11-06 (fourth pass, local SDK verification)

The second pass cited a Go binding for `IUIAutomationVtbl`. Pass 4 replaces
that secondary source with the **installed Windows SDK 10.0.22000.0**
`UIAutomationClient.h` / `UIAutomationClient.idl` (grade O), read locally:

- `CLSID_CUIAutomation = ff48dba4-60ef-4201-aa87-54103eef594e`;
  `CLSID_CUIAutomation8 = e22ad333-b25f-460c-83d0-0581107395c9` (a different
  proxy behaviour, **not** the class the probe should instantiate).
- `IID_IUIAutomation = 30cbe57d-d9d0-452a-ab13-7ac5ac4825ee`;
  `IID_IUIAutomationElement = d22108aa-8ac5-49a5-837b-37bbb3d7591e`;
  `IID_IUIAutomationElementArray = 14314595-b4bc-4055-95f2-58f2e42c9855`.
- Slot numbering is confirmed from the C vtable (`IUIAutomationVtbl`):
  `QueryInterface` 0, `AddRef` 1, `Release` 2, `CompareElements` 3,
  `CompareRuntimeIds` 4, `GetRootElement` 5, **`ElementFromHandle` 6**,
  `ElementFromPoint` 7, `GetFocusedElement` 8, four `*BuildCache` 9-12,
  `CreateTreeWalker` 13, three view walkers 14-16, three view conditions
  17-19, `CreateCacheRequest` 20, **`CreateTrueCondition` 21**,
  `CreateFalseCondition` 22, `CreatePropertyCondition` 23. The Go binding's
  slots 6 / 21 are correct.
- `IUIAutomationElement`: `SetFocus` 3, `GetRuntimeId` 4, `FindFirst` 5,
  **`FindAll` 6**; `get_CurrentProcessId` 20, `get_CurrentControlType` 21,
  `get_CurrentLocalizedControlType` 22, **`get_CurrentName` 23**, and after
  the long propget run **`get_CurrentBoundingRectangle` 43**.
  `IUIAutomationElementArray`: `get_Length` 3, `GetElement` 4.
- **New trap discovered:** `get_CurrentBoundingRectangle` writes a Win32
  `RECT` (four `LONG`: left / top / right / bottom), while this repo's
  `a11y_controls[].rect` is `[x, y, width, height]`. The adapter must compute
  `x=left, y=top, width=right-left, height=bottom-top`; passing the raw four
  ints through would silently corrupt every rectangle. `TreeScope_Subtree`
  is `0x1|0x2|0x4 = 7`; `UIA_HWND` is `void *`.
- **Grade boundary:** a header is authoritative for the SDK it ships with.
  It does not prove the runtime's provider behaviour, so the probe still has
  to print raw HRESULTs before any production routing. This is why the gate
  stays *probe required*, not *vtable solved*.

The fourth pass also recorded the resource-telemetry trust split (host-side
signals are evidence; guest self-reports are debugging data) in T3 and the
decision-authority correction in `PRD_01_24` / T8.

## Findings, 2026-11-06 (fifth pass, W1 probe verdict)

`examples/cu_probe_uia.rs` ran on the real Windows desktop (grade **X**, the
only grade that can support `[v]`). It is a diagnostic, not a shipped
capability: no `src/` file, version source, capability row, ABI or matrix cell
changed.

- **The COM surface is correct.** `CoCreateInstance`, `ElementFromHandle`
  (slot 6), `CreateTrueCondition` (slot 21), `FindAll` (element slot 6),
  `get_Length` (array slot 3), `GetElement` (array slot 4),
  `get_CurrentControlType` (21), `get_CurrentName` (23) and
  `get_CurrentBoundingRectangle` (43) all returned `S_OK`. The desktop root
  read `RECT=0,0,1680,1050` -> `[0,0,1680,1050]`, proving both the slot and
  the `left/top/right/bottom` -> `[x,y,w,h]` conversion. Brave (399 nodes),
  a MiniCon window (639), Sourcetree (438), GVIM (22) and `cmd.exe` (8) all
  produced named, bounded rows.
- **The target provider is the failure, not the vtable.** Both WorkBuddy
  windows returned exactly 8 nodes, all `control_type=50033` (Pane), only one
  named, and **zero non-Pane controls** — no input box or button to address.
  The shipped 0.0.11 MSAA tree reads real controls on that same client. The
  first reads also returned all-zero `RECT`s with `S_OK`; four consecutive
  re-runs of the same handle returned `840,0,1680,1014` stably. This is the
  Chromium/provider behaviour recorded above (unrealised / virtualised rows,
  window or integrity context), now measured on the one window L3 needs — and
  a cold-start scan that flips from empty to populated without a generation
  change cannot satisfy the reader's "no guessed tree" rule.
- **Consequence.** A UIA-first route would *regress* `observe.a11y` for the
  target while adding a large FFI surface. The mandatory downgrade rule in
  `plan/plan-0.0.15-uia-probe-and-routing.md` fired on the zero-rectangle
  reads, and the absence of addressable controls independently rules out the
  routing. W2 stops, no UIA capability is claimed, no version bump or Release
  chain runs. The probe remains in the tree as reproducible evidence.
- **Method note (correcting the first reading).** The all-zero rectangles
  were the provider's cold-start state, not a permanent refusal: the same
  handle returned `840,0,1680,1014` on four consecutive re-runs. The durable
  finding is the **zero non-Pane controls**, not the zero rectangles. A single
  successful `CoCreateInstance` or a single empty tree is not evidence.

## Findings, 2026-11-06 (sixth pass, why the Electron tree was empty)

W0 pass 6 asked why a correct UIA surface produced only Pane nodes on the
L3 target. Sources: official Electron / Chromium accessibility docs (O),
platform engineering write-ups (S) and vendor docs (V). **None of this is
X-grade evidence in this repo**; it explains the pass 5 measurement, it does
not claim a capability.

- **Chromium builds its full web a11y tree lazily.** It raises `AXMode` to
  full only when it detects an assistive technology, or when the app is
  started with `--force-renderer-accessibility`. Before that, an observer
  sees "a shell with almost no children" — exactly the 8 Pane nodes measured.
  Electron documents `app.setAccessibilitySupportEnabled(enabled)`
  (Windows/macOS only, after `ready`) and the macOS `AXManualAccessibility`
  attribute that third-party assistive software can set on the app's
  `AXUIElement`. Chromium's own `ax_dump_tree` inspector requires Narrator /
  NVDA / JAWS on Windows, VoiceOver on macOS, Orca on Linux, or the runtime
  flag.
- **Consequence for this repo's G boundary.** Making the target expose its
  tree means changing the **target application's** launch flags or app-level
  setting, or driving an OS accessibility grant. All three are outside
  `screen`: this repo is a bounded one-file worker with no OS grant
  requests and no authority over how another app was launched. So the W1
  finding is not a bug to fix in the CU reader — it is an environmental
  precondition. It is recorded as a study note, not as a new verb.
- **Linux is more fragmented, not less.** AT-SPI2 needs a session D-Bus
  (`dbus-launch` in headless CI), `toolkit-accessibility=true` and
  `QT_ACCESSIBILITY=1` for Qt apps; Wayland is described as semantic-only
  with interactive portal use disabled, and synthetic input needs
  `gnome-ponytail-daemon`. `Newton` (AccessKit-based, Wayland-native) is
  experimental in 2026 — track it, do not depend on it. Custom-drawn
  widgets remain opaque to AT-SPI.
- **macOS TCC is two grants, not one, and both fail silently.**
  Accessibility covers AX tree reads *and* synthetic events
  (`kAXErrorAPIDisabled = -25211`); Screen Recording covers pixels *and*
  other apps' window titles. The grant binds to the **responsible process**
  (usually a hosting GUI app, not the CLI binary) and to its code signature;
  a rebuilt binary invalidates it, a stale row can read "on" while denying,
  and a running helper can cache a stale Screen Recording answer — the
  documented fix is a short-lived verified probe re-read on focus. This is
  why `PRD_01_08` publishes named `unknown` grant rows instead of guessing.
- **Resource-domain cross-link (T3 / R10).** A 2026 `at-spi2-core` pidfd leak
  can fill the fd table (`LimitNOFILE=1024`), wedge the portal into an
  `accept`-EMFILE spin, and make *every* AT-SPI tree walk block for the full
  15 s D-Bus timeout. A stale a11y tree can therefore be a **host resource
  exhaustion symptom**, not just a UI problem. This is a study note about
  why G stays closed; it grants `screen` no process or fd authority.

**What this changes in the plan.** It does not reopen W2: the downgrade rule
already fired and the missing tree is an app-launch precondition outside
`screen`. It does sharpen the reopen trigger to include a target that has its
full accessibility mode enabled, and it adds Linux/macOS gate conditions to
the AT-SPI / AX candidate. No implementation starts from this pass.

## Findings, 2026-11-07 (seventh pass, semantic addressing and the real target)

Pass 7 asked what the 2026 field considers a *reliable* target, and then
measured the answer on this machine through the repo's own CU wire. Grades:
O/P/S for the field notes, **X** for the local measurement. The full tree is
`PRD_01_25_semantic_addressing_subtree.md` (new dimension T9).

- **Addressable handles beat coordinates.** Surveys, OSWorld action/observation
  analyses and grounding playbooks separate semantic grounding ("click
  Submit") from spatial grounding (the pixel of Submit on this rendering at
  this DPI); the latter dominates failures. The recommended shape is hybrid:
  prefer an a11y handle where one exists, cross-check against the screen, and
  verify after every action. **Caveat:** the published per-step reliability
  arithmetic is model-specific, and a PyAutoGUI "step" can contain a Python
  control structure, so cross-paper success rates are not a controlled A/B.
  This repo claims no success-rate gain — only one more addressable target.
- **A11y trees are not complete semantic descriptions.** OSWorld's own
  observation notes (a11y tree / screenshot / screenshot+a11y_tree /
  set-of-marks) say trees may be long, incomplete, redundant or wrong, while
  screenshots preserve layout but not boundaries. With A5 `[no]`, this repo
  implements the **cross-check** half via generation pinning and a pointer
  readback inside the node rect; it explicitly does not do visual grounding.
- **Native default action exists on all three OSes** — MSAA
  `accDoDefaultAction`, macOS `AXUIElementPerformAction(kAXPressAction)`,
  Linux `org.a11y.atspi.Action.DoAction` — and can press without a pointer,
  focus or foreground. But an invoke has **no pointer readback**, which is this
  repo's whole notion of evidence. It therefore stays `[study]` (T9 S7): a new
  evidence kind is needed before it can become a capability.
- **MSAA vs UIA is a provider/launch-mode question, not old vs new.** Flutter
  and Chromium expose `IAccessible`(+`IAccessibleEx`) and
  `IRawElementProviderFragment` from the same node, and UIA patterns are
  optional. A correct `CoCreateInstance` can coexist with an unusable UIA tree
  while MSAA on the same hwnd yields real buttons. This narrows the pass-5
  finding to the **UIA provider**, which is what the W1 probe actually read.
- **X-grade measurement on the L3 target (2026-11-07).**
  `examples/cu_orchestrate.rs` drove `CuHost::invoke_json` — no Python, no
  ctypes, no screenshot, no UIA: `observe` -> 16 windows, focused `WorkBuddy`
  hwnd `33689578`; `{"op":"observe","a11y":33689578}` -> **43 nodes, 8
  `button`, 11 named, 42 non-zero rects**; `{"op":"observe","controls":
  33689578}` -> **refused** (no Win32 child window). Named bounded buttons:
  `最小化`, `最大化`, `关闭`, `对话内搜索`, `分享`, `打开右侧面板`,
  `新建任务`, `窗口置顶`. So the client was *observable* but not
  *addressable* — the gap between L1 and L3. This **does not overturn** the
  W1 downgrade (UIA-first would still regress `observe.a11y`); it opens a
  different, already-evidenced route.
- **Linux/macOS gates and the resource cross-link stand.** AT-SPI2 needs a
  session bus and toolkit flags; Wayland is semantic-only with portal input;
  macOS binds two TCC grants to the responsible process and signature. An
  `at-spi2-core` pidfd leak can wedge the portal into an `accept`-EMFILE spin
  and make every tree walk hit the 15 s D-Bus timeout — a stale a11y tree can
  be a host resource symptom. This is a reason G stays closed, not a grant of
  process/fd authority.

**What this changes in the plan.** `plan/plan-0.0.15-a11y-addressed-
actuation.md` was written and passed its 6/6 checklist **before** any code
started; it switches the 0.0.15 route to a11y-addressed actuation. The UIA
W2 stays `[no]`. A study note is still not a decision: the switch is dated in
`PRD_01_24`, and only three-OS CI plus owner-view acceptance produce `[v]`.

## Candidate table (upserted, not promised)

| Finding | Repo axis | Candidate | Unlock condition | This cycle |
|---|---|---|---|---|
| UIA supersedes MSAA; UIA-aware agents measurably better | A2 / A4 | UIA-first + MSAA fallback in `observe.a11y`, additive `a11y_source` | **closed by W0 pass 5**: the probe verified every COM slot and five other apps' trees, but the target WorkBuddy provider gives 8 Pane nodes / 0 non-Pane controls and a cold-start zero-rect window. UIA-first would regress the L3 target; the downgrade rule fired | `[no]` W2 stopped |
| WGA / PrintWindow black frames, DWM shadow rects | A5 / E | per-window capture router with named reason | image arm + PNG budget decision (A5/D2) | deferred |
| Cross-platform agents need one normalised node schema | A4 | `a11y_controls[]` gains `state` / `state_names` / `value` plus `a11y_node_schema`; the reader recomputes the bit mask | none (gate open) | **shipped in 0.0.14 W1/W2** |
| a11y trees are event streams | A4 / D | incremental change subscription | three-OS AT-SPI / AX grants + fixtures; the schema above is its prerequisite; pass 6 adds the Linux session-bus / Wayland and macOS dual-TCC conditions | deferred |
| Wait before re-observation; budget ceilings | B7b / D4 | `observe.wait_ms` + `evidence["wait"]` | none (gate open) | **shipped in 0.0.13 W1/W2** |
| Long-horizon verification and independent readers | F3 / L2 | app-level effect evidence | side-effect-free three-OS fixture | deferred |
| Sandbox isolation ladder, quotas, credential brokerage | G / exec | resource-domain telemetry and control | separate PRD + 4 reopen triggers (`PRD_01_24`) | **not implemented; G decided closed (a)** |
| Host-side vs guest-side telemetry trust | G / T3 | typed pressure rows, only from the far side of isolation | three-OS X-grade probes + threat model + reader | deferred (study note, R9/R10) |
| Guest-side acceptance of isolation | F / delivery | extend owner-view acceptance to resource cells | only if G is ever lifted | deferred |
| Addressable a11y handles beat pixel grounding | A2 / A4 / T9 | `act:control` resolves the normalised MSAA node by identity (`source`/`role`) | **opened by W0 pass 7 X-grade measurement**; plan 6/6 complete | **shipped in 0.0.15** |
| Native default action (`accDoDefaultAction` / AXPress / `atspi.DoAction`) | T9 S7 | press by identity without a pointer | three-OS probes + a new effect-evidence kind + a policy PRD | `[study]` — an invoke has no pointer readback |
| "Verify after every step" is the dominant long-horizon failure | F3 / L3 / T9 | typed before/after readback of the same a11y node across one act | none beyond a 6/6 plan; uses shipped `accValue`/`state` + 0.0.13 wait | **planned for 0.0.16, not started** |

## G-boundary decision — taken by the controller (2026-11-06 22:15)

`PRD_01_08` says the readback proves **desktop state, never process, memory
or device state**. Three mutually exclusive directions exist; the controller
**decides (a)** and records the other two with their unlock conditions. This
is an escalation corrected by the owner's 22:15 direction, recorded with its
reason in `PRD_01_24`, not a choice offered back:

- **(a) keep G closed (decided).** Learn and upsert the whole resource domain,
  implement only inside KVM + W + S. Cost: no process/CPU/memory/GPU verbs.
- **(b) additive read-only telemetry in `disclose`.** Publish host CPU/memory
  pressure, process count or GPU inventory as typed rows, still no control.
  Cost: lifts one `[no]`, needs a three-OS probe with named `unknown` reasons
  and a fresh PRD; a "read-only" row still widens what `screen` may observe.
  **Deferred**, with four explicit reopen triggers in `PRD_01_24`.
- **(c) real resource control.** cgroups / MIG / microVM lifecycle actions.
  Cost: a new tool or ABI, new grants, a policy engine and an audit surface —
  this is a different product from a bounded one-file worker. **`[no]`**
  unless the product definition itself changes.

A research finding is not a decision. Nothing in this file authorises a
change to the 12/10/8/0 matrix, `cu-abi 1.0`, the six false cells or any
shipped Release asset.
