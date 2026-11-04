# PRD 01.05 — semantic computer use: typed controls and named targets

Owner of the **control arm** of observation and the first **semantic action**.
`PRD_01_03` owns the pixel arm and the native mechanisms; `PRD_01_04` owns the
versioned host contract above both. Status: `[v]` implemented and qualified on
Windows (2026-11-04); `[_]` a UIA accessibility tree and per-OS qualification
remain open.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## The decision

`PRD_01_03`'s gap table offered two routes to semantic observation: hand-write
a UIA COM subset (hundreds of lines of unsafe vtable/VARIANT code) or take
`minicon-platform` as an optional feature. Both break a stated invariant
(zero new dependency, one file per cell). The owner's direction was to design
the CU capability ourselves, so 0.0.4 takes a **third route**: Win32 already
exposes child windows through plain `user32` FFI.

`EnumChildWindows` + `GetClassNameW` + `GetWindowRect` + `IsWindowVisible` /
`IsWindowEnabled` + `WM_GETTEXT` via `SendMessageTimeoutW` yields a typed,
bounded, deterministic inventory of controls. `SMTO_ABORTIFHUNG` and a 200 ms
timeout mean a hung foreign window costs one bounded wait, never a stalled
invocation. No COM, no crate, no extra file, no second permission layer.

**It is not UIA.** Roles are a fixed table mapping *Win32 class names* to
coarse names (`Button`→`button`, `Edit`→`textbox`, `SysListView32`→`list`).
There are no value patterns, no automation peers, no text ranges. A
Chromium/Electron-style window that draws its own widgets usually exposes zero
child windows; the tool reports zero **and says why** in the observation
instead of returning an empty success. The frame names the basis explicitly:
`controls_role_basis: "win32-class"`.

## The contract surface

```text
op=observe [controls]                op=act action=control
├── controls=true  → foreground      ├── control: exact label, or a unique
│   window                          │   substring; ambiguity is refused
├── controls="title" → one window   ├── press=false (default): hover only
└── typed frame                     ├── press=true: one click at centre
    ├── controls_source             └── evidence: pointer readback must land
    ├── controls_truncated          │   INSIDE the control rect, else
    └── controls[] { label, role,   │   not_performed / unknown
                     class, text,       (frame generation is pinned by as_of)
                     rect, visible,
                     enabled }
```

`controls` is a **separate, opt-in read**. A plain `op=observe` enumerates no
child windows at all, so the pixel path costs exactly what it did in 0.0.4's
first iteration. The scan resolves its window through the same `find` rule an
action uses; no hwnd is carried across calls.

### Bounds

| Constant | Value | What it bounds |
| --- | --- | --- |
| `SCREEN_TOOL_MAX_CONTROLS` | 128 | child windows in one scan; a hit is **reported** as `truncated=true` |
| `SCREEN_TOOL_MAX_CONTROL_TEXT` | 256 | characters per control text; control characters become spaces |
| `SCREEN_TOOL_CONTROL_TIMEOUT_MS` | 200 | one cross-process `WM_GETTEXT` |

### `act:control` admission

Every step runs before a device context, a pointer move or a click:

1. A frame exists (an `observe` happened) and belongs to the current
   generation; otherwise `refused`/`stale`, never a replay.
2. The frame actually contains controls. Empty is a fact about the window,
   not a target to guess.
3. `resolve_control`: exact label wins; otherwise a case-insensitive
   substring must match **exactly one** control. Ambiguity lists candidates.
4. The control is visible and enabled, and has a non-zero rectangle.
5. Its centre lies inside the virtual desktop (so the OS cannot clamp the
   pointer to a different point).
6. Only then `Act::Hover` (default) or `Act::Click` (`press=true`).
7. The pointer readback is parsed from `GetCursorPos` and must lie **inside
   the control's rectangle**. Outside → `not_performed`; unparseable →
   `unknown`. A control that moved between observe and act is not a success.

`cu-abi` stays **1.0**: the calling shape did not change. One new
`CAPABILITIES` row, one `CuAction` variant and one wire enum value are kept
in agreement by a test; `disclose` advertises the new capability to any
caller that asks. A synthetic-frame unit test covers every named-target
refusal, and the real-desktop journey covers the performed path.

## Invariants carried over

1. One grant, one host: no per-control allow-list, no new authority.
2. `ScreenSnapshot` remains the single typed source of truth; the model-facing
   text is a rendering of it. `CuFrame::to_json` adds the same controls array
   for typed callers.
3. No silent clipping, no guessing: truncation, zero controls, ambiguity and
   stale generations are all named refusals or named facts.
4. Readback is evidence, not the request echoed back.
5. Zero new dependencies, one file per cell. Non-Windows cells compile the
   same typed contract and refuse by name.
6. No embedded engine. Per PRD 31's readiness gate, `unisacc`/`ujs` ABI,
   budgets, imports and provenance remain **BLOCKED**; the owner's direction
   (study omp for capability shape, run dynamic logic on unisa+ujs, not bun)
   is recorded but no runtime is claimed.

## Open gaps, stated rather than papered over

| Gap | Consequence today | Note |
| --- | --- | --- |
| `[-]` no UIA tree | roles are class-derived; no values, patterns or text ranges | this PRD's route is deliberately smaller, not a substitute |
| `[-]` self-drawn UIs | Chromium/Electron windows usually report 0 controls | named in the observation text; `act:control` then refuses by name |
| `[-]` Windows only | other cells compile and refuse by name | PRD 38 journey per OS, unchanged |
| `[_]` pointer-path qualification | `press=true` (a real click on a control) is not exercised by the ignored journey | the journey is hover-only on purpose; click shares the proven pointer path |
| `[no]` engine binding | -- | PRD 01.04's gate still applies |

## Evidence

- `[v]` `cargo test --offline`: 71 passed, 3 ignored (the two earlier
  real-desktop journeys and this one).
- `[v]` `cargo test --offline -- --ignored` on this Windows machine
  (2026-11-04): all three journeys pass. The semantic journey scans the real
  foreground window (`WorkBuddy`, 1 control), then a deterministic in-process
  Win32 fixture (a `STATIC` parent with a `BUTTON` child titled `OK`), resolves
  it **by title**, enumerates `button:OK` at `rect=[228,251,90,26]`, hovers at
  `center=[273,264]` and reads back `pointer at 273,264 ... inside control
  rect`. A pinned stale generation is refused with `verdict=stale`.
- `[v]` the journey publishes `cu-controls.json`; an **independent** Python
  reader verified unique labels, the class-derived role, centre-inside-rect,
  `press=false`, `verdict=performed`, the readback text and the stale verdict.
  The result is not merely asserted by the Rust that produced it.
- `[v]` the production `screen.rs` + `cu.rs` pair type-checks in its
  non-Windows configuration (six cells still refuse by name).
- `[v]` bounds, role table, label uniqueness, name resolution (exact / unique
  substring / ambiguous / missing), pointer-readback parsing and the full
  named-target refusal matrix are unit-tested on every cell.

## Where this goes next

1. Keep `cu-abi` stable: a new semantic capability is one manifest row, one
   `CuAction` variant, one wire enum value and one agreement test.
2. Qualify `press=true` on a real desktop once there is a fixture whose click
   has no side effect.
3. Only after an immutable `unisacc`/`ujs` revision is inspected, design the
   module envelope against `PRD_01_04`. That remains a separate PRD and gate.
