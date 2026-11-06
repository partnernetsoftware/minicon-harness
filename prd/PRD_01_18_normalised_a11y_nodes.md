# PRD 01.18 — normalised a11y node schema (0.0.14)

Status: `[v]` W1/W2 delivered in `31e257d`; W3/W4 in `efe96a2`, after
`v0.0.13` was published as a public signed Release and accepted from the
owner's view. Three-OS CI `42217541734` / `42217725863` and `42218173775` /
`42218235696` are success; `v0.0.14` is a published, accepted signed
Release. Owner of the **contract** for 0.0.14; `plan/plan-0.0.14-normalised-a11y-nodes.md` owns the
work breakdown, gates and release chain. Nothing here claims a capability a
CI run has not produced.

Legend: `[v]` delivered with evidence, `[_]` planned, `[no]` non-goal.

## Domain fit

Inside KVM + W + S (`PRD_01_08`) this is **A4 observation**: a read-only,
bounded, typed description of controls in one live window. It posts no input,
manages no process, reads no memory, touches no device and changes no grant.
The G resource-domain boundary is unchanged. Its motive comes from W0
(`PRD_01_17`): a cross-platform agent needs one normalised node schema rather
than four native accessibility vocabularies, and UIA / AX / AT-SPI adapters
need that schema before they can be routed.

## Contract

```text
observe { op:"observe", a11y: true | hwnd }
  -> evidence["a11y_controls"][i] gains
       "value":       string (accValue BSTR, empty when unreadable)
       "state":       integer (raw STATE_SYSTEM_* bit mask)
       "state_names": array of strings from a fixed 16-bit table
  -> evidence["a11y_node_schema"] = "role-name-state-value-bounds-depth"
```

- All 0.0.11 fields stay: `name`, `role`, `role_name`, `rect` (four-integer
  virtual-desktop rectangle), `depth`, `child_id`, plus `a11y_source`,
  `a11y_reason`, `a11y_truncated` and `a11y_role_basis`.
- `state_names` is derived from `state` by a **pure fixed table**, in bit
  order: selected, focused, pressed, checked, mixed, readonly, default,
  expanded, collapsed, busy, invisible, offscreen, focusable, selectable,
  linked, haspopup. Unknown bits are kept in the raw integer and never named.
- The raw integer is always published so the independent journey reader can
  recompute `state_names` itself. A host that publishes a guessed set fails
  the journey.
- `value` is bounded to the same 256 UTF-16 code units as `name`; a failed
  `get_accValue` is an empty string, not a fabricated value.

### Bounds

No new numeric bound: the existing 400-row / depth-12 a11y limits and the
existing `SCREEN_TOOL_MAX_*` constants are unchanged. No new capability row,
no new `CuVerdict`, no ABI bump and no change to the 12/10/8/0 matrix.

### Native surface

`native::a11y_controls(handle) -> Result<(Vec<A11yControl>, bool), String>`
has the **same signature on all four configurations**. Windows reads two more
slots of the already-bound `IAccessible` vtable (`accValue` 11,
`get_accState` 14); macOS, Linux and the fallback keep publishing an empty
tree plus a named reason. No new library, no new OS API, no elevation, no
write to the target process and no UIA COM object is created.

### Admission and honesty

1. `observe.a11y` stays a read; `--allow-screen` admission and every earlier
   observe arm are unchanged.
2. `a11y_node_schema` is a fixed literal, not a version string the host may
   invent. A frame without it is not a 0.0.14 observation.
3. `state_names` must equal the reader's own derivation from `state`. A
   contradiction is a journey failure, not a warning.
4. A cell without the platform a11y API publishes zero rows plus a named
   reason; a refused scan never publishes rows.

## Verification

- `src/cu.rs`: unit tests for the pure bit table, the node JSON fields and
  the unchanged capability / ABI counts. All 0.0.6-0.0.13 tests stay.
- `scripts/cu-journey-verify.py`: recomputes `state_names` from `state` for
  every row, checks `value` is a string and checks `a11y_node_schema`.
  No earlier assertion is weakened.
- Three-OS CI journeys keep `observe_a11y` required; the compile gate types
  all four backend configurations.

## Non-goals

- `[no]` `IUIAutomation` / UIA3 Raw routing in this version: a hand-written
  COM vtable with an unverified slot is a process crash, not a named refusal.
  It stays a W0 candidate until a real Windows probe verifies every slot.
- `[no]` incremental a11y change subscription, a normalised relation field,
  an image arm, batched actions, app-level effect evidence or safety policy.
- `[no]` any process, CPU, memory, GPU or device authority. `PRD_01_17`
  records that whole domain as research and keeps the G decision open.
