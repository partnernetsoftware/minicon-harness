# PRD 01.11 — grant and environment disclosure

Status: `[v]` **delivered** in 0.0.9 with three-OS CI evidence — compile gate
`42164845780` and real-desktop journeys `42164845836` on
`f28eae53b1490cc6761f9b6e265c4358e1691278`, where `disclose_grants` is a
mandatory journey step and the independent reader recomputes `environment`,
`grants[]`, the four-value whitelist, the reason rule, the sort order and
uniqueness. Owner of the **contract** for 0.0.9; `plan/plan-0.0.9-grant-disclosure.md`
owns the work breakdown, gates and release chain. Nothing here claims a
capability that a CI run has not produced. `PRD_01_03` owns the `screen`
tool's bounds, `PRD_01_04` owns the host contract, `PRD_01_08` owns the
domain model and the resource boundary, `PRD_01_10` owns the keyboard state
machine. This PRD only widens the existing `disclose` evidence.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Domain fit

A grant / environment probe is pure **S (session state)** inside the KVM+W+S
model of `PRD_01_08`. `PRD_01_08` already lists `$DISPLAY`, Screen Recording,
Input Monitoring, Accessibility and UIPI as **preconditions, not managed
objects**. This PRD publishes their status; it does not manage, request or
escalate them. It adds no resource-management authority, no process, memory,
device or host-control surface, and no harness-side capability. It is not a
desktop effect, never posts an input event, and must never borrow the
`PRD_01_07` receipt.

## Problem

`disclose` today reports `qualified` (one boolean from the compiled cell),
`grant` (`--allow-screen`, a host flag) and a static `platform_capabilities`
matrix. A caller cannot tell *why* a cell is unqualified: a missing X server,
a denied Screen Recording grant, a TCC-restricted Input Monitoring grant, a
pure Wayland session and a headless runner all look the same. The only way to
learn is to attempt an effect and read the refusal. That is honest but
wasteful, and on macOS a model may conclude it should ask for a permission
that this tool must never request. 0.0.9 publishes the preconditions
themselves as structured, typed, read-only evidence.

## Contract

### Additive evidence on `disclose`

```text
disclose { op:"disclose" }
  -> evidence["environment"]: {
       platform, session, display_count, virtual_desktop,
       foreground_present
     }
  -> evidence["grants"]: [ { name, status, reason? }, ... ]
```

- `status` is exactly one of `granted`, `denied`, `unknown`,
  `not_applicable`. A `reason` is required (non-empty) for `unknown` and
  `not_applicable`, optional for `granted` / `denied`.
- `session` is a coarse kind, not an environment value: `gui`, `x11`,
  `wayland-x11-compat`, `headless` or `unavailable`. No user name, display
  number, path or window title is published.
- `virtual_desktop` is the same `{x, y, width, height}` rect already used by
  `observe`; reading it is a pure geometry query. `foreground_present` is a
  boolean, not a title.
- No new capability name, no new `CuVerdict`, no ABI major bump. `cu-abi 1.0`
  is unchanged; `CAPABILITIES` is unchanged and `disclose` gains no typed
  parameter rows (it takes none today).
- `disclose` stays available when computer use is **not** granted: a denied
  host may still read its own precondition status.

### Bounds, each a named constant beside the code that enforces it

| Constant | Value | What it bounds |
|---|---|---|
| `SCREEN_TOOL_MAX_GRANT_PROBES` | 8 | rows one disclosure may publish; more are refused, never silently clipped |
| `SCREEN_TOOL_MAX_GRANT_REASON` | 160 | characters of one reason; longer reasons are truncated with a marker |

### Admission and honesty

1. `disclose` is pure information. No ABI, grant or platform gate may turn a
   probe into an effect; the existing `CuKind::Disclose` exemption stands.
2. **No probe may request a permission.** Only preflight and read-only
   symbols are resolved. A missing symbol, a failed read-only connection or
   an OS that cannot answer without a prompt produces `unknown` with a named
   reason. `denied` is reported, never escalated.
3. A probe that cannot be evaluated on the compiled cell is
   `not_applicable` with the cell named, never omitted so a reader cannot
   infer a shape.
4. A probe result must not contradict a performed effect in the same
   session: if `observe` succeeded, a `x11_display` / `screen_capture`
   style probe may not say `denied`. The independent reader checks this.
5. `CuHost::drop` and every other effect are untouched. Nothing here holds
   state across calls.

### Native surface (four configurations, identical signatures)

```text
native::environment() -> EnvironmentProbe
native::grants()      -> Vec<GrantProbe>
```

- Windows: read-only process integrity / foreground-window presence;
  `screen_capture` (GDI needs no TCC), `input_posting` (UIPI), and
  `window_enumeration`.
- macOS: `CGPreflightScreenRecordingAccess`, `CGPreflightPostEventAccess`,
  `AXIsProcessTrusted` (never `AXIsProcessTrustedWithOptions`), resolved
  through the existing `dlopen`/`dlsym` helper. A missing symbol is
  `unknown`, not a prompt.
- Linux: `DISPLAY` / `XDG_SESSION_TYPE` kind plus a read-only `XOpenDisplay`
  and `dlsym` check for XTest. A pure Wayland session is named.
- fallback: every probe `not_applicable`, one named reason per row.
- `scripts/cu-cfg-check.py` type-checks all four configurations in the same
  commit as the signature change.

### Evidence

- `environment` and `grants` are published by `op=disclose` only. `observe`,
  `act` and every refusal keep their current evidence shape.
- `grants` rows are sorted by `name` and have unique names, so an independent
  reader can recompute the array from the request-free disclosure.
- `evidence["actuation"]`, `evidence["keyboard"]` and
  `evidence["scroll"]` are absent on a plain disclosure, exactly as today.
- `held_keys` from 0.0.8 stays and is unaffected.

## Verdict rules

| Situation | Verdict |
|---|---|
| disclosure published with well-formed environment and grants | `performed` |
| a probe could not be answered safely | `performed` with that row `unknown` + reason |
| a probe name is duplicated or a status is outside the whitelist | `not_performed` (contract violation, caught in unit tests) |
| a grant row contradicts an effect performed in the same session | `not_performed` (independent reader) |
| `--allow-screen` was never granted | `performed`; disclosure is information, not an effect |

## Verification

- `src/cu.rs`: unit tests for the status whitelist, the reason rule, the
  probe bound, sorted/unique names, the ungranted disclosure path and the
  absence of any new capability row. All 0.0.6/0.0.7/0.0.8 tests stay.
- `scripts/cu-cfg-check.py`: four configurations type-check with the new
  signatures.
- `.github/workflows/cu-platforms.yml`: three OSes, `cargo test --locked`,
  matrix still 12/10/8/0.
- `.github/workflows/cu-journeys.yml`: required `disclose_grants` on all
  three OSes. The independent reader recomputes the probe whitelist, the
  uniqueness, the reason rule, the environment shape and the no-contradiction
  rule, plus every existing assertion.
- No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` explicit bounded `wait`, batched `actions[]`. Each has an unlock
  condition in `plan/plan-0.0.9-grant-disclosure.md`.
- `[no]` a11y trees, image arm, Wayland-native protocol, runtime backend
  selection, app-level state-change evidence, held-key effect evidence.
- `[no]` requesting, escalating or persisting an OS grant; no TCC dialog,
  no `AXIsProcessTrustedWithOptions`, no UAC prompt.
- `[no]` new crate, link-time platform library, engine, plugin host,
  bun/WASM/ujs embedding. PRD 31 readiness stays BLOCKED.
- `[no]` an ABI bump, a 13th capability row, or a change to the six false
  matrix cells; their unlock conditions live in `PRD_01_06`.
- `[no]` publishing window titles, user names, paths or environment values
  beyond the coarse session kind.

## Where this goes next

Only after W1-W4 of plan 0.0.9 are green, including the owner-view Release
acceptance, does the next version get planned. B7b, B8 and the 0.7.x
chapters remain candidates, not debts: each needs its own PRD, its own gate
and a dated controller decision (T8) before it enters a version.
