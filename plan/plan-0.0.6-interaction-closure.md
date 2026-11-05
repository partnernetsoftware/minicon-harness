# 0.0.6 — interaction closure: CI proves a real click, not just a moved pointer

Status: `[_]` plan + contract delivered (2026-11-05). **No source, version or
Release change is part of this step.** W1-W4 below are the development plan;
each carries its own gate and none is claimed until a CI run exists. Owner
direction this plan implements: **"开工做0.0.6的计划"** -- write the next
version's plan, with the same discipline as 0.0.4/0.0.5: a plan is a set of
falsifiable gates, not a wish list.

Baseline this plan starts from: `v0.0.5` is a public company-signed Latest
Release from source `45b13f1`; three real backends; three-OS compile gate
`41972347018`; three-OS desktop journeys `41975233858`; publish chain
`41977124016` (5/5 jobs). `cu-abi 1.0` is frozen and stays at 1.0.

## Decision: what 0.0.6 is, and what it deliberately is not

0.0.5 made **observation and pointer movement** real on three OSes. It stopped
one step short of an interaction: every journey click used `button=left`, no
journey ever sent `press=true`, and the only proof that a button was pressed is
that the pointer ended up at the right coordinates. `src/cu.rs:923-927`
already implements `Act::Click` for `press=true`, `src/wire.rs:343-351` already
advertises `middle` and `double` -- but no CI run has ever driven them. Three
PRDs list exactly this as the next qualification
(`PRD_01_03:177`, `PRD_01_04:164-165`, `PRD_01_05:111,139-140`).

**0.0.6 closes that gap.** The theme is interaction closure: a bounded
side-effecting action must leave three independently recomputable facts, and
GitHub CI must produce them on all three OSes.

Candidates considered and rejected for this version, with the reason recorded
so a later version does not re-litigate them from scratch:

| Candidate | Decision | Reason / unlock condition |
|-----------|----------|---------------------------|
| macOS AX controls + `act:control`, AX focus/move | `[no]` this version | `AXIsProcessTrusted()` is a system-level TCC grant that a GitHub hosted runner cannot self-grant. 0.0.5 proved Screen Recording + Input Monitoring are available, **not** Accessibility. Under the owner's acceptance rule ("自己走完最后一步"), shipping code that can only be named-refused on CI is not a delivery. Unlock: a self-hosted macOS runner with an Accessibility grant, or an owner-operated real-desktop journey with published evidence. |
| Linux AT-SPI control tree | `[no]` this version | AT-SPI needs an `at-spi2-core` D-Bus bus plus an ATK-enabled application. The Xvfb fixture is a bare Xlib `xmessage` with no ATK bridge, so no a11y tree exists to read. Adding GTK + dbus + a bus daemon changes the "zero new runtime dependency" boundary. Unlock: a dedicated fixture job with a GTK echo window and a stable bus, proven green before any capability is declared. |
| real UIA tree on Windows | `[no]` this version | 0.0.4 deliberately chose the smaller Win32 child-window route; a COM/UIA tree is a separate design with its own PRD. The existing `controls` arm stays honest about being class-name derived. |
| `ujs`/`unisa` dynamic logic (PRD 31) | `[no]` BLOCKED | `PRD_01_04:118-122` requires an immutable revision inspected and measured across six items (engine revision, callable ABI, budgets, host imports, cancellation, provenance). No such revision is qualified in this repo. Readiness stays BLOCKED; no engine, runtime, WASM or bun is embedded. |
| image arm / Wayland-native / runtime backend switching | `[no]` carried | Unchanged 0.0.5 non-goals. |

Consequence: the capability matrix **does not grow** in 0.0.6. Windows stays
12/12, Linux 10/12, macOS 8/12, other 0/12. What grows is the strength of the
evidence for the verbs already declared `true`. This is an intentional,
defensible version boundary: a version that proves a real click happened is
worth more than a version that declares three new verbs and refuses them all.

| Layer | 0.0.5 | 0.0.6 target |
|-------|-------|--------------|
| pointer verbs | performed + pointer readback | performed + pointer readback + **actuation receipt** |
| `press=true` | implemented, never exercised | **required CI step on Windows**, `act:control` receipt checked |
| `middle` / `right` / `double` | in the wire schema, never driven | **required CI steps on all three OSes** |
| control arm | Windows-only, hover-only journey | Windows-only, hover **and** a harmless real click |
| follow-up | `observe_final` recorded | `observe_final` must prove a live session + strictly newer generation |
| docs | 0.0.5 truth recorded | the stale 0.0.3/gap-table statements corrected |
| delivery | v0.0.5 public signed | v0.0.6 public signed, no regression in the four release invariants |

## External benchmark: what 0.0.6 absorbs and defers

Added after the owner's request to benchmark against Codex / ChatGPT / OpenAI.
The structured reading is `prd/prd-cu-openai.md` (external reference, **not a
contract**); raw SDK types, official-page excerpts and an evidence-tier index
sit in a temporary corpus outside the repo. Source quality is graded there:
first-party SDK types (generated from the OpenAPI spec) and the Microsoft Learn
integration guide are tier A, first-party pages blocked by 403/Cloudflare are
tier B, third-party synthesis is tier C.

The comparison **does not change the WBS below**. It makes the boundary
explicit so 0.0.7 does not have to re-derive it.

| OpenAI CU surface (tier A/B) | minicon-harness today | 0.0.6 decision |
|---|---|---|
| `double_click` (separate action) | `double` in wire, never driven in CI | **absorb**: required `act_click_double` step |
| `click.button` = left/right/wheel/back/forward | `left`/`right`/`middle` in wire; only left driven | **absorb**: middle/right required; wheel/back/forward stay `[no]` (browser-only) |
| `act:control` has no equivalent; no typed targets | named control with `press=false` hover only | **absorb on Windows**: `press=true` real click with rect readback + receipt |
| `status: "completed"` on a `computer_call` | six typed verdicts + readback + evidence | **keep ours**; never downgrade to a generation status |
| `actions[]` batched, executed in order | one invoke -> one verdict -> one receipt | **defer to 0.0.7 (B8)**; batching needs its own admission/stale design |
| `drag.path[]`, `keys[]` held modifiers | single from→to drag, no held keys | **defer to 0.0.7 (B6)** |
| `scroll_x` / `scroll_y`, `wait` | one `delta` axis, no wait verb | **defer to 0.0.7 (B7)** |
| base64 PNG returned to the model every turn | PNG written under the task root; text + evidence only | **defer to 0.0.7 (A5/D2)**; needs a separate PRD |
| `pending_safety_checks` + 3-level human confirmation | grant (`--allow-screen`), `why`, refused, task-root bounds | **defer to 0.0.7 (C5/C6)**; policy PRD first |
| macOS AX tree (Codex desktop app, tier C) | named refusal, `controls=false` | **stay `[no]`** for 0.0.6; the same per-app TCC grant problem is why CI cannot qualify it |

Two consequences worth writing down:

1. **No regression in exchange for alignment.** Typed verdicts, actuation
   readback, the 12-row capability matrix, `as_of` stale protection, receipts,
   three-OS CI journeys and signed-release evidence are specified nowhere in
   the OpenAI tool surface. They are our advantage, not compatibility debt.
2. **0.0.7 has a home.** `PRD.md` → "CU capability tree" and "0.0.7 frontier
   (chapter stub, not planned)" now hold A4–A7 / B6–B8 / C5–C6 / D2–D4 /
   E3–E4 / F3 with unlock conditions. Nothing there is a promise.

## Contract: actuation receipt (spec owner `prd/PRD_01_07_interaction_closure.md`)

A click is not evidenced by a coordinate alone. Every `act` that dispatches a
button event must return a structured, typed receipt in addition to the
existing `readback` text:

```rust
pub struct ActuationReceipt {
    pub button: &'static str,  // "left" | "middle" | "right"
    pub downs: u32,            // 1, or 2 for double
    pub ups: u32,              // must equal downs
    pub double: bool,
    pub target: String,        // the resolved window id/title the event was sent to
}
```

Rules:

1. **Additive, ABI-preserving.** `cu-abi 1.0` stays 1.0. The model-visible
   `readback` string keeps its current shape (`pointer at X,Y`,
   `scrolled N at X,Y`); the receipt is a new `evidence["actuation"]` object.
   No wire input parameter changes.
2. **One function set, four configurations.** `native::click`, `drag`,
   `scroll` and the Windows `act:control` path return the receipt alongside
   the pointer tuple. The macOS, Linux and fallback backends keep identical
   signatures; `scripts/cu-cfg-check.py` type-checks all four cfgs.
3. **The receipt is a claim about dispatch, not about application semantics.**
   It proves a real button event was posted at the resolved rect with the
   requested button and count. It does **not** claim the target application's
   business action ran. Saying otherwise would repeat the exact mistake this
   repo's contract exists to prevent. App-level state change stays a non-goal
   until a self-hosted *echo window* fixture can expose it with no side
   effects.
4. **Missing evidence is a failure.** A `performed` verdict with no receipt,
   or a receipt whose `button`/`downs`/`ups` disagree with the request, is
   `unknown` / `not_performed` -- never silently upgraded.
5. **Named boundaries still win.** A missing OS grant or library stays
   `cu: unsupported:` -> `Unsupported`. No journey step is relaxed to make a
   matrix green.

## Work breakdown and gates

```text
0.0.6 — interaction closure
├── [_] W1 — actuation receipt in the contract
│   files: src/screen.rs, src/screen_macos.rs, src/screen_linux.rs,
│          src/cu.rs, scripts/cu-cfg-check.py
│   invariant: cu-abi stays 1.0; readback text unchanged; all four cfgs
│   type-check; cargo test --locked green on all three OS CI jobs
│   evidence: cu-platforms.yml run + matrix test unchanged at 12/10/8/0
├── [_] W2 — real interaction journeys in CI
│   files: examples/cu_journey.rs, scripts/cu-journey-verify.py,
│          .github/workflows/cu-journeys.yml
│   invariant: new required steps act_click_middle, act_click_right,
│   act_click_double, act_drag_middle and (Windows only)
│   act_control_press; every performed step has pointer readback,
│   a matching actuation receipt and a strictly newer follow-up generation;
│   fixtures stay side-effect-free (clicks land in empty client area,
│   never on a button/menu)
│   evidence: cu-journeys.yml run, three cu-journey.json artifacts,
│   independent reader output
├── [_] W3 — the truth of the docs
│   files: PRD.md, README.md, prd/PRD_01_03_computer_use.md,
│          prd/PRD_01_04_cu_base.md, prd/PRD_01_05_semantic_controls.md,
│          prd/PRD_01_06_cross_platform_cu.md
│   invariant: PRD.md 0.0.3 S1-S6 reconciled with the signed public releases
│   and release-policy.json mode=required; the "no desktop effect on
│   macOS/Linux" gap row removed; README Layout lists the three backends;
│   the six false matrix cells carry their unlock conditions, not a vague
│   "separate design"
│   evidence: grep for stale claims returns nothing; git diff --check clean
└── [_] W4 — 0.0.6 version bump and public signed Release
    files: Cargo.toml, Cargo.lock, release-policy.json,
           loader/ape-version.rc, .github/release-notes.md
    invariant: four version sources in ONE commit; dry-run before publish;
    the 0.0.5 four release invariants must not regress
    evidence: run IDs + owner-view anonymous download and re-hash
```

### W2 step design

| New step | Request | Required on | Why it is harmless |
|----------|---------|-------------|--------------------|
| `act_click_middle` | `action=click, button=middle` on empty client area | all 3 | middle-click in an empty document/text area does not activate a control |
| `act_click_right` | `action=click, button=right` on empty client area | all 3 | a context menu may open; the follow-up `observe` must still be live, and the next step's focus call closes it. If a runner cannot guarantee this, the step is **demoted to named refusal**, not faked |
| `act_click_double` | `action=click, double=true` on empty client area | all 3 | double-click selects a word; no file, no navigation, no destructive action |
| `act_drag_middle` | `action=drag, button=middle` | linux + windows | macOS middle drag is only admitted if the CG event path proves a readback; otherwise named refusal |
| `act_control_press` | `action=control, press=true` on a text box | windows only | clicking a text box focuses it; the control rect + pointer readback + `downs=1,ups=1` receipt are checked. Linux/macOS keep `act_control=false` and skip by name |

The independent reader (`scripts/cu-journey-verify.py`) must be extended, not
weakened:

- `EXPECTED` stays exactly `windows=12`, `linux=10`, `macos=8` true verbs.
- A new assertion: for every `act_*` step whose verdict is `performed`,
  `evidence["actuation"]` exists, `ups == downs`, `button` equals the request,
  and `double` implies `downs == 2`.
- A new assertion: `observe_final.generation > observe.generation` and the
  final observe still reports at least one window and one display.
- The existing `BOUNDARY` rule (macOS + `permission:` + named refusal) stays;
  it must not be usable to excuse a missing receipt on a `performed` step.

### W4 release invariants (must not regress)

1. `publish` keeps `environment: release` with `github.token` -- the 0.0.3
   403 root cause was a deleted gate, not an org policy.
2. Signed Windows cells are rewritten into both Windows zips and their
   `.sha256` sidecars are recomputed; the APE alone being signed is not enough.
3. `post-publish` re-hashes the **downloaded** assets against
   `signing-receipt.json` `after_sha256`.
4. Defender uses `MpCmdRun -File` (not `-Path`) and `TAG` resolves through
   `needs: [verify, ...]`.

Chain, all `workflow_dispatch` on `main`, one shared `head_sha`:

```
minicon-harness-com.yml pack -> candidate.yml (source_sha, pack_run_id)
-> company-signing.yml (qualification_only=false) -> release.yml dry_run=true
-> release.yml dry_run=false (publish + Defender + post-publish)
```

Owner-view acceptance is the last step, not the exit code: anonymous HTTP 200
on the Release page and one asset, a fresh download of all 15 assets re-hashed
locally against the receipt and sidecars, and the downloaded APE printing
`minicon-harness 0.0.6`.

## Risks and downgrade rules

- **Right-click opens a context menu** and can change the foreground window
  before the next step. Mitigation: order it last among clicks, re-focus the
  fixture afterwards, and let `observe_final` prove the session is live. If a
  runner is unstable, demote that one variant to a named refusal with the OS
  and reason recorded -- never delete the assertion.
- **macOS middle button / `CGEventCreateMouseEvent`** uses `kCGMouseButtonCenter`;
  a fixed Rust fn pointer must not call a C variadic (the 0.0.5 arm64 lesson).
  Any new CG symbol goes through the existing `dlopen`/`dlsym` helper.
- **Linux XTest Button 2/3** and **Win32 `MOUSEEVENTF_MIDDLEDOWN`** differ in
  constants; the receipt's `button` string is the cross-platform normalisation,
  so the reader never depends on native constants.
- **`act_control_press` on Windows** must target a text box, not a button: the
  fixture is `notepad.exe`, whose only clickable child is the edit control in
  0.0.5's journey (`textbox:cu`). If a runner exposes no child window, the
  step is skipped as an undeclared verb only if the matrix says so -- but the
  Windows matrix declares `act:control=true`, so instead the journey must
  report `not_performed` and fail the gate. No fake pass.
- **Scope creep.** No new crate, no new link-time platform library, no new
  tool, no ABI bump, no engine embedding. If a W1/W2 change needs one of
  those, stop and split it into a separate PRD rather than widening 0.0.6.

## Non-goals

- `[no]` AX / AT-SPI / UIA COM control trees; the six false matrix cells stay
  false and gain explicit unlock conditions in W3.
- `[no]` proving application-level state change after a click; a self-hosted
  echo-window fixture is a separate design and a separate gate.
- `[no]` Wayland-native protocol, image arm in the wire codec, runtime backend
  selection.
- `[no]` any new crate, runtime, plugin host, bun/WASM/ujs embedding; PRD 31
  readiness stays BLOCKED.
- `[no]` re-publishing or overwriting v0.0.4 / v0.0.5 assets. 0.0.6 is a new
  tag.
- `[no]` claiming `[v]` for a verb that only compiled, or relaxing a reader
  assertion to make a journey green.

## Delivery record

Filled in as each W closes, with the CI run IDs, the three journey artifacts,
the release chain runs and the owner-view anonymous acceptance result. Until
then every W above stays `[_]` and the source version stays `0.0.5`.
