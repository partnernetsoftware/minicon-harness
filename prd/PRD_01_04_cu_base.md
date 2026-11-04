# PRD 01.04 — computer-use base: a versioned host capability contract

Owner of the **contract above the desktop mechanisms**. `PRD_01_03` owns what
`screen` can do; this PRD owns how a caller is allowed to ask, what it gets
back, and how the answer is accounted for. Status: `[v]` the base is in the
tree and qualified on Windows; `[_]` binding a dynamic-logic engine to it is
not started.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Why a base and not more verbs

Adding actions to `screen.rs` makes the tool wider. It does not make it a
**base**. A base is the thing a future caller can bind to without the host
having to change for every new policy decision. The contract here is the
"versioned host capabilities and typed policy inputs/outputs" leaf (CONTRACT)
of `minicon/prd/PRD_02_31_01_04_dynamic_logic.md`, applied to computer use
only. It is also CU0 of `minicon/prd/PRD_02_38_computer_use.md`: observe, act
and disclose are separate target-bound choices, and OS permission alone
grants no product authority.

Product direction from the owner (2026-11-04): the harness's later capability
work studies **omp**, but dynamic logic runs on our own **unisa + ujs**
architecture -- not bun, not a Node runtime, not a plugin/MCP host. This PRD
therefore freezes the **host side** of the seam only. It does not select or
embed an engine, and it claims no ABI with any current ujs/WASM runtime.

## The contract

```text
cu-abi 1.0                        host owns: grant, qualification, native effects
├── disclose [v]                  information only; works even when ungranted
│   ├── abi major/minor, caller minor, minor_degraded
│   ├── platform qualified, grant, generation, last frame
│   ├── capabilities[] { name, kind, summary, params[] {name,type,required} }
│   └── receipts[] { generation, capability, verdict, facts_as_of_ms, readback }
├── observe [v]                   creates generation N, facts_as_of_ms
│   └── typed frame { platform, qualified, desktop, displays[], windows[], capture? }
├── act:<name> [v] ×9             one effect; verdict + structured evidence
└── verdict                       performed | not_performed | unknown
                                  | refused | unsupported | stale
```

Two entrances, one implementation:

| Entrance | Who uses it | Shape |
| --- | --- | --- |
| `CuHost::invoke_json(Value)` | the model's `screen` tool call (wire adapter) | `op` + flat arguments |
| `CuHost::invoke_capability(name, Value)` | a future dynamic caller after `disclose` | named capability + typed arguments |

Both converge on `CuHost::invoke(CuRequest)`, which is the only place a
desktop effect can start. `src/cu.rs` holds the ABI, manifest, verdicts,
generations and receipts; `src/screen.rs` holds the FFI and the PNG encoder;
neither can bypass the other.

### Admission order (every step before any native effect)

1. **ABI major** must equal `CU_ABI_MAJOR` (1); a mismatch is refused before
   activation. A newer **minor** is accepted with `minor_degraded=true`; an
   unknown optional field cannot widen authority.
2. **Capability** must appear in `CAPABILITIES`. An unadvertised name is
   `unsupported` and names itself; a caller cannot invent `op` values.
3. **Grant** is the host's: `--allow-screen`, plus platform qualification.
   `disclose` is exempt because it is information, not an effect.
4. **Age**: `as_of` must equal the current generation. A stale generation is
   `stale` and is **never replayed** (CU2: ambiguous effects are not replayed).
5. **Typed bounds**: scale, type length, region dimensions and parameter
   types are checked here and again beside the code that enforces them.

### Capability manifest

`CAPABILITIES` in `src/cu.rs` is the single source of truth. `observe` and
`disclose` plus eight `act:*` names, each with `kind`, `summary` and typed
`params`. `wire.rs`'s `action` enum and `op` enum are tested against this
table, so the advertised JSON cannot drift from the host.

`op=disclose` prints the manifest in the model-facing text and returns the
same content as structured JSON in `CuResult.evidence`. A text-only model and
a typed caller see the same contract.

### Generations, evidence and receipts

- Each successful `observe` increments `generation`; `act` and `disclose` do
  not. A refused observe does not advance it.
- `CuFrame` is typed: desktop rect, displays, windows, capture path/size/
  scale, plus `facts_as_of_ms`. The model-facing text is a rendering of that
  same frame (`ScreenSnapshot::to_text`), never a second source of truth.
- Every call leaves a `CuReceipt` with its verdict and readback. The last
  `CU_MAX_RECEIPTS` (32) are retained and disclosed newest-first.
- `CuVerdict` distinguishes CU2's three outcomes: `performed` (readback
  confirms), `not_performed` (readback contradicts the request), `unknown`
  (the effect is unconfirmed -- e.g. the foreground changed during input).
  Native failures that are neither a named refusal nor a named failure are
  `unknown`, never a success.

## Invariants

1. **One grant, one host.** No per-capability allow-list, no policy engine,
   no second permission layer. A module cannot grant computer-use authority.
2. **Disclosure precedes use.** The manifest is available without a grant.
3. **A refusal is a tool result.** Nothing here aborts the run; every verdict
   is text the caller can read and a typed value it can branch on.
4. **No silent replay, no silent clipping.** Stale calls and out-of-bounds
   regions are refused with the geometry/generation named.
5. **Zero new dependencies.** `serde_json` and `std::sync::Mutex` only. Six
   cells still cross-compile from std plus the two existing crates; on
   non-Windows cells the ABI still works and the native backend still refuses
   by name.
6. **One file per cell.** No runtime to colocate, no library to load at run
   time, no service to keep alive.

## Explicit non-goals

- `[no]` embedding ujs, WASM, bun, Node or any script runtime in this crate.
- `[no]` a plugin/MCP host, a second model loop, or a general permission
  engine.
- `[no]` claiming compatibility with a current `unisacc`/`ujs` ABI. Per PRD
  31's readiness gate, no immutable current revision has been qualified for
  this product; engine revision, callable ABI, budgets, host imports,
  cancellation and package provenance stay **BLOCKED** until inspected at an
  immutable revision and measured with the first experiment.
- `[no]` per-OS native qualification, an accessibility tree, or an image arm
  in the wire codec -- those remain in `PRD_01_03`'s gap table.

## What a future dynamic module may do

Once an engine is qualified against its own gate, the smallest experiment is
one bounded **policy decision** over the typed contract: a module may read
`disclose` output, choose among advertised capability names, supply typed
arguments, and receive a `CuResult` with a verdict. It may **not** raise the
grant, skip admission, invent a capability, change `as_of`, replay a stale
call, read raw credentials, or open a file/network handle of its own. The
host revalidates every call. An incompatible module is refused before
activation; in-flight calls keep the generation they were pinned to.

## Evidence

- `[v]` `cargo test --offline`: 71 passed, 3 ignored (the pixel, contract and
  semantic real-desktop journeys).
- `[v]` `cargo test --offline -- --ignored --nocapture` on this Windows
  machine (2026-11-04): `PRD_01_03`'s pixel journey plus the contract journey
  -- disclose at generation 0, observe at 1, a pinned `act:hover` at `as_of=1`,
  a stale `as_of=0` refused without an effect, a second observe at 2, and a
  disclosure listing the receipts including the stale verdict.
- `[v]` the contract journey publishes `cu-observe.json` and
  `cu-disclose.json` under the task root; an **independent** Python reader
  confirmed `generation 1 → 2`, `abi {major:1, minor:0}`, `grant=granted`,
  11 advertised capabilities, a frame with 17 windows / 1 display, and a
  receipt list containing both `performed` and `stale` verdicts. The typed
  output is not merely asserted by the Rust that produced it.
- `[v]` ABI mismatch, unadvertised capability, mistyped arguments, ungranted
  calls, stale generations, verdict classification and manifest/wire enum
  agreement are unit-tested on every cell.
- `[v]` the production `screen.rs` + `cu.rs` pair was type-checked in its
  non-Windows configuration (the six cells that refuse by name still
  compile against the same `native` surface).

## Where this goes next

1. Keep the contract stable while actions are added: a new action is one row
   in `CAPABILITIES`, one `CuAction` variant, one `Act` translation and a test
   that the wire enum agrees.
2. Qualify the remaining native journeys (drag, middle button, a real
   `press=true` click on a control). The typed control arm and the named
   target were added as `PRD_01_05` with **cu-abi still 1.0**: one manifest
   row, one `CuAction` variant and one wire enum value, exactly the step 1
   pattern. A real UIA accessibility tree remains a stated limitation.
3. Only after an immutable `unisacc`/`ujs` revision is inspected, design the
   module envelope (`abi_major`, `module_version`, `facts_as_of`, bounded
   output) against this contract. That is a separate PRD and a separate gate.
