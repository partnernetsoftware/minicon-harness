# PRD 01.10 — held-key modifiers

Status: `[v]` shipped as the public company-signed `v0.0.8`; qualified on
three OSes (compile gate **42154222974**, journeys **42154223003**) with the
owner-view Release acceptance recorded in
`plan/plan-0.0.8-held-key-modifiers.md`. Owner of the **contract** for 0.0.8; `plan/plan-0.0.8-held-key-modifiers.md`
owns the work breakdown, gates and release chain. Nothing here claims a
capability that a CI run has not produced. `PRD_01_03` owns the `screen`
tool's bounds, `PRD_01_04` owns the host contract, `PRD_01_07` owns the button
actuation receipt, `PRD_01_08` owns the domain model, `PRD_01_09` owns the
0.0.7 richer actuation. This PRD only widens one declared keyboard verb.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Domain fit

A held modifier is a pure **K (keyboard)** effect inside the KVM domain of
`PRD_01_08`. It adds no resource-management authority, no process, memory,
device or host-control surface, and no harness-side capability. It does not
touch the loop, the wire codec's message shapes or the model's turn budget.
It is deliberately **not** a button event and must never borrow the
`PRD_01_07` receipt.

## Problem

`act:key` can deliver `ctrl+s` as one atomic down/up pair, so a modifier only
exists inside one call. A real desktop session needs a modifier held across
several calls (hold Shift, click two items, release), and OpenAI's `computer`
tool has `key` combinations that assume a keyboard state. 0.0.7 deliberately
deferred this half of B6 because a modifier is not a button event: folding it
into the 0.0.6 receipt would blur what that receipt asserts. 0.0.8 closes it
with a separate, bounded keyboard state and its own evidence.

## Contract

### Additive wire parameters

```text
act:key { action:"key", keys, [hold: true], [release: true] }
disclose { op:"disclose" }  -> evidence["held_keys"]: [string, ...]
```

- `keys` keeps today's spelling, including `ctrl+s` and `win+left`. Omitting
  both flags keeps 0.0.7 behaviour bit-for-bit: one atomic down/up pair and
  the same `foreground=...` readback.
- `hold: true` accepts **only** a whitelisted modifier name (`ctrl`, `shift`,
  `alt`, `win` / `cmd`); a combination or a non-modifier with `hold` is a
  named refusal. At most four modifiers may be held at once.
- `release: true` releases every held modifier. Supplying `hold` and
  `release` together, or `release` with a multi-key `keys` combination (for
  example `ctrl+s`), is a named refusal. An omitted, empty or single-modifier
  `keys` is accepted with `release` and produces no extra keystroke.
- No new capability name, no new `CuVerdict`, no ABI major bump. `cu-abi 1.0`
  is unchanged; `CAPABILITIES` gains two typed parameter rows and a test that
  the wire schema agrees with the manifest.

### Bounds, each a named constant beside the code that enforces it

| Constant | Value | What it bounds |
|---|---|---|
| `SCREEN_TOOL_MAX_HELD_MODIFIERS` | 4 | size of the host-held modifier set; a fifth is refused, never silently dropped |
| `SCREEN_TOOL_HELD_MODIFIERS` | `ctrl, shift, alt, win/cmd` | whitelist; anything else with `hold` is refused |

### Admission (before any native effect)

1. ABI major -> advertised capability -> grant -> platform capability ->
   `as_of` -> typed bounds -> native effect. The order from `PRD_01_04` is
   unchanged; a newer keyboard parameter cannot widen authority.
2. `hold` / `release` are booleans. An unknown modifier, a fifth modifier,
   a conflicting flag pair, or `hold` on a key combination is a typed refusal
   naming the bound.
3. **Before every call**, if the foreground title changed since the last
   keyboard effect, or the grant was lost, the host releases everything held
   and reports the release. No further effect is delivered in that call until
   the release has been published.
4. `CuHost::drop` calls `native::release_all_modifiers()`. A journey or a
   model that forgets to release cannot leave a shared runner with a stuck
   modifier.

### Native surface (four configurations, identical signatures)

```text
native::set_modifier(key: Modifier, down: bool) -> Result<(), String>
native::release_all_modifiers()                 -> Result<(), String>
```

- Windows: symmetric `keybd_event` down/up pairs for the four virtual keys.
- macOS: `CGEventCreateKeyboardEvent` plus `CGEventSetFlags` on **every**
  event posted while a modifier is held, so CG flags and synthetic state
  agree; resolved through the existing `dlopen`/`dlsym` helper.
- Linux: XTest keysyms resolved at runtime; an unmapped modifier is a named
  `unsupported`, never a guessed keycode.
- fallback: identical signatures, named `unsupported` refusals.
- `scripts/cu-cfg-check.py` type-checks all four configurations in the same
  commit as the signature change.

### Evidence

- `op=disclose` adds `evidence["held_keys"]`: a sorted array of the modifiers
  the host currently holds. It is empty on a fresh host and after every
  release.
- `act:key` with a flag adds `evidence["keyboard"] = { held: [...],
  released: [...] }`. `held` is the state *after* the call; `released` names
  what this call let go (including an automatic foreground-loss release).
  Neither field is derived from the request alone: both come from the host's
  own state machine.
- `act:key` never carries `evidence["actuation"]`. The independent reader
  asserts it is null for every keyboard step.
- `readback` keeps its `pressed ... foreground=...` shape, so the 0.0.6
  foreground assertion is unchanged.

## Verdict rules

| Situation | Verdict |
|---|---|
| modifier held, host state and disclose agree | `performed` |
| modifier released, host state and disclose agree | `performed` |
| foreground lost while keys were held | release performed, the call is `unknown`, no new effect delivered |
| unknown modifier / fifth modifier / conflicting flags | `refused`, bound named |
| unmapped keysym, missing library / `$DISPLAY` / OS grant | `unsupported`, named |
| `as_of` pinned to an older generation | `stale` |
| a button receipt appears on a keyboard step | `not_performed` (contract violation) |

## Verification

- `src/cu.rs`: unit tests for the whitelist, the four-modifier bound, flag
  conflicts, foreground-loss auto-release, teardown release, disclose
  round-trip and the "no button receipt on keyboard" rule. All 0.0.6/0.0.7
  receipt, `path_points` and scroll-axis tests stay.
- `scripts/cu-cfg-check.py`: four configurations type-check with the new
  signatures.
- `.github/workflows/cu-platforms.yml`: three OSes, `cargo test --locked`,
  matrix still 12/10/8/0.
- `.github/workflows/cu-journeys.yml`: required `act_key_hold`,
  `disclose_held`, `act_key_release`, `disclose_released` on all three OSes.
  The independent reader recomputes the held set, the release, the absence of
  a button receipt, the existing receipts, `path_points`, both scroll axes and
  the strictly newer live final observation.
- The journey's final `op=disclose` must report an empty held set.
- No reader assertion is weakened to make a journey green.

## Non-goals

- `[no]` explicit bounded `wait`, batched `actions[]`. Each has an unlock
  condition in `plan/plan-0.0.8-held-key-modifiers.md`.
- `[no]` a11y trees, image arm, Wayland-native protocol, runtime backend
  selection, app-level state-change evidence.
- `[no]` new crate, link-time platform library, engine, plugin host,
  bun/WASM/ujs embedding. PRD 31 readiness stays BLOCKED.
- `[no]` an ABI bump, a 13th capability row, or a change to the six false
  matrix cells; their unlock conditions live in `PRD_01_06`.
- `[no]` reusing the button actuation receipt for keyboard state.

## Where this goes next

Only after W1-W4 of plan 0.0.8 are green, including the owner-view Release
acceptance, does the next version get planned. B7b, B8 and the 0.7.x
chapters remain candidates, not debts: each needs its own PRD, its own gate
and an owner decision before it enters a version.
