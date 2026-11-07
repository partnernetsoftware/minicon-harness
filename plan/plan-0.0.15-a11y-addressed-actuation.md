# 0.0.15 — a11y-addressed actuation (semantic control targets)

Status: `[v]` **W1-W4 delivered and accepted from the owner's view.**
`v0.0.15` is the Latest public signed Release, published at
2026-11-07T02:38:36Z from `4782d0f`. Chain: compile gate `42329026455`,
three-OS journeys `42329357278`, pack `42329042056`, candidate `42329975979`,
signing `42330027308`, dry-run `42330153097`, publish `42330205680` — all
success. Owner-view acceptance: release page anonymous 200, 15 assets,
six archive sidecars recomputed OK, signed `.com` bytes matched the signing
receipt `after_sha256`, the downloaded APE printed `minicon-harness 0.0.15`,
and the downloaded `.com` and Windows `.exe` were locally Authenticode
`Valid` with RFC 3161 timestamps.

This file does not overwrite `plan-0.0.15-uia-probe-and-routing.md`:
that document stays the verbatim W0 diagnostic record of the UIA probe and of
the downgrade decision. This plan is the **controller's route switch** for the
same version cycle, taken under T8 D2.1 (version topic) and D2.3 (lifting one
`[no]` with a dated reason and explicit bounds). The UIA-first W2 remains
`[no]`; only the MSAA semantic-addressing route is opened.

Owner direction (`/goal`, 2026-11-07): keep enhancing computer use, use
minicon-harness's own CU, never Python, and **fill in whatever is missing**.
Owner direction (2026-11-06 evening): do not start code or tests until the
plan is complete — 三思后行. This file is that completion.

## Why the route switched (X-grade evidence, this machine, 2026-11-07)

`examples/cu_orchestrate.rs` drove the real desktop through the repo's own
`CuHost::invoke_json` wire. No Python, no ctypes, no screenshot, no UIA:

| Request | Verdict | Result |
|---|---|---|
| `{"op":"observe"}` | `performed` | 16 windows, desktop `0,0 1680x1050`, focused `WorkBuddy` hwnd `33689578` |
| `{"op":"observe","a11y":33689578}` | `performed` | **43 MSAA nodes, 8 `button`, 11 named, 42 non-zero rects**, `a11y_source=handle:33689578 window:"WorkBuddy"` |
| `{"op":"observe","controls":33689578}` | `refused` | no Win32 child windows on the self-drawn Electron window |

Named, addressable buttons on the L3 target (all with real `[x,y,w,h]`):
`最小化`, `最大化`, `关闭`, `对话内搜索`, `分享`, `打开右侧面板`,
`新建任务`, `窗口置顶`; plus `document` / `label` rows. States carry
`focusable` / `focused` / `readonly` from the 0.0.14 normalised schema. The
shipped 0.0.11 MSAA tree therefore **already sees** exactly the semantic
controls that UIA could not expose; what is missing is the ability to *act* on
them. `act:control` only resolves `frame.snapshot.controls` (Win32), which is
empty for this window, so every semantic target is currently unaddressable.
That is the gap this version fills.

W0 pass 7 (`PRD_01_17`, grades O/P/S) records the 2026 consensus behind the
shape: addressable accessibility handles beat pixel coordinates; press by
identity (`accDoDefaultAction` / `AXUIElementPerformAction` / `atspi.Action.DoAction`)
rather than by a guessed `(x,y)`; verify after every step. 0.0.15 takes the
**bounded first half** of that: resolve by identity from the already-shipped
normalised node schema, then reuse the existing hover/press actuation with a
readback that must land inside the node's rectangle. Native action dispatch is
recorded as a study candidate, not smuggled into this version.

## Exact wire contract (additive; `cu-abi 1.0` unchanged)

`act:control` gains two optional parameters. Every existing call keeps its
0.0.6-0.0.14 behaviour byte-for-byte: `source` defaults to `controls`.

| Param | Type | Default | Rule |
|---|---|---|---|
| `source` | string enum `"controls"` / `"a11y"` | `"controls"` | any other value is refused before any pointer exists |
| `role` | string | none | only with `source:"a11y"`; filters candidates by the 0.0.14 `role_name` vocabulary; an unknown role is refused, not ignored |
| `control` | string (required) | — | exact `name` wins; otherwise a unique case-insensitive substring over `name` then `value`; 0 or 2+ matches are refused with the candidates named |
| `press` | bool | `false` | unchanged: hover by default, click only when explicitly true |

`source:"a11y"` resolution is a **pure function** over the current generation's
`a11y_controls[]`:

1. Require a frame, then `frame.generation == state.generation` (existing stale
   rule, unchanged).
2. Require a non-empty `a11y_controls[]` **from a scan performed in that very
   frame**; an empty or unscanned tree is a named `no_a11y_tree` refusal, never
   a silent fall back to Win32 controls or to coordinates.
3. Drop nodes whose state has `invisible` (0x8000), `offscreen` (0x10000) or
   `unavailable` (0x1); a node that is not operable is refused with its state
   names, not clicked through.
4. Apply the optional `role` filter, then exact-name, then unique substring.
   Ambiguity refuses with up to six candidate names and `role_name`s.
5. Require a positive `width`/`height` rectangle and a centre inside the
   virtual desktop (existing rules, unchanged).
6. Perform `hover` (or `click` when `press:true`) through the same backend as
   `act:control` today; the pointer readback must land **inside that node's
   rectangle**, else `not_performed` / `unknown` — never a success echoed from
   the request. `verify_actuation` keeps the button receipt unchanged.

### Evidence (additive rows only)

The `act:control` evidence object gains:

```text
source                "controls" | "a11y"
a11y_node             null, or { name, value, role, role_name, state,
                      state_names, rect[4], depth, child_id }
a11y_node_schema      "role-name-state-value-bounds-depth" (a11y only)
```

`control`, `role`, `rect`, `center`, `press`, `readback`, `verdict`,
`generation`, `actuation`, `scroll`, `target_hwnd` keep their 0.0.6-0.0.14
meaning. For an a11y target `role` is the node's `role_name` and `rect`/`center`
come from the node, so the independent reader can recompute containment from
published values alone. `state_names` is recomputed from the raw `state` bit
mask by the reader (the 0.0.14 rule, reused on act evidence).

## Downgrade rule (mandatory, written before W1)

Stop and publish the diagnosis instead of shipping if any of these occurs:
the a11y tree is empty on the real target; a node resolves but its rectangle
is zero/negative; the pointer readback cannot be parsed or lands outside the
node; an `invisible`/`offscreen` node could still be selected; an ambiguous
name resolves to one arbitrary node; or `source` is accepted for an action
other than `control`. No reader assertion may be weakened to make a journey
green. This is the same probe-first shape used by 0.0.11 and 0.0.15-W1.

## Reader contract (independent, `scripts/cu-journey-verify.py`)

- Every 0.0.6-0.0.14 assertion stays, unchanged and in the same order.
- New `act_control_a11y` step: recompute containment from the published
  `a11y_node.rect` and the pointer readback; recompute `state_names` from
  `state`; reject a guessed `source`, a negative/zero rectangle, an
  `invisible`/`offscreen` selected node, and a `press:true` that lacks a button
  actuation receipt.
- `disclose` must publish the two new params on `act:control` with their types;
  the capability count stays 11 and the wire action enum stays the same nine
  verbs — `source` is a parameter, not a new capability or verb.

## Three-OS behaviour

| Cell | Behaviour |
|---|---|
| Windows | MSAA tree is real; `source:"a11y"` resolves, hovers/presses and readbacks inside the node rect |
| macOS / Linux / fallback | `a11y_controls: []` with the existing named reason; `source:"a11y"` is a named `no_a11y_tree` refusal, never a guess |
| matrix | 12/10/8/0 unchanged; no new grant, crate, link-time platform library or native signature |
| four-cfg type gate | `cu-cfg-check.py` types all four configurations in the same commit |

## Non-goals

- `[no]` UIA-first routing, UIA write/invoke patterns, event subscription
  (the W1 downgrade decision is not reopened).
- `[no]` `accDoDefaultAction` / `AXUIElementPerformAction` / `atspi.DoAction`
  native dispatch in this version — study candidate for a later cycle.
- `[no]` an image arm, batched `actions[]`, app-level effect evidence, a model
  loop, an OS grant request, a DPI change, or any new capability/tool/verb.
- `[no]` widening `screen` into host resources; `PRD_01_08` G stays closed,
  direction (a) decided in `PRD_01_24`.
- `[no]` re-publishing or overwriting any shipped v0.0.4-v0.0.14 asset.

## Plan-completeness checklist (`PRD_01_23`) — 6/6

| # | Requirement | Where | State |
|---|---|---|---|
| 1 | exact contract: params, defaults, resolution order, state veto | "Exact wire contract" | `[v]` plan |
| 2 | downgrade branch: empty tree, zero rect, readback, ambiguity | "Downgrade rule" | `[v]` plan |
| 3 | reader contract: additive rows, retained assertions, recomputation | "Reader contract" | `[v]` plan |
| 4 | three-OS behaviour and four-cfg gate | "Three-OS behaviour" | `[v]` plan |
| 5 | non-goals and G boundary restated | "Non-goals" | `[v]` plan |
| 6 | W1 eligible only after 1-5, with X-grade target evidence | this table + "Why the route switched" | `[v]` plan |

**All six items were written down before W1 started.** A plan fact is not a
shipped capability; W1/W2 then produce the evidence, and only three-OS CI plus
owner-view acceptance can turn the row into `[v]`.

## Work breakdown and gates

```text
0.0.15 — a11y-addressed actuation
├── W1 Rust: pure a11y resolver + wire params + host dispatch + unit tests
│   invariant: no new crate, native signature, grant, verb or capability row
├── W2 journey + independent reader + four-cfg type gate + three-OS CI
│   invariant: no 0.0.6-0.0.14 assertion weakened
├── W3 four version sources + README/PRD truth
└── W4 public signed Release + owner-view acceptance + next-version plan
```
