# Product review and roadmap -- 2026-11-09

Status: **review + proposal, not a version plan.** Changes no source, version,
capability row, `[no]` row or published asset. The PRD body is deliberately not
rewritten here; section 7 is the proposed restructure and section 8 is the
evidence for it. The owner keeps the veto.

Scope read: `PRD.md` (1173 lines), `README.md`, `prd/PRD_01_08`, `_12`, `_17`,
`_23`, `_24`, `plan/v0.0.19.md`, `plan/v0.0.20.md`, `Cargo.toml`.

---

## 1. Executive summary

1. **The product has three surfaces, but only one is shipped as a product.**
   The worker core (0.0.19) is released; the MCP adapter is built and tested
   but **not in any Release archive**; the gate is a direction record. The
   companion thesis is therefore still a claim, not an installable thing.
2. **The CU branch is ahead of every named competitor on evidence and behind
   on distribution.** Typed verdicts, readbacks, a 12/10/8/0 matrix, three-OS
   journeys and signed releases are genuinely rare. None of it reaches a user
   who cannot mount the adapter.
3. **The biggest CU gate (A5 / image arm) is not on the critical path for the
   companion product.** MCP carries image content natively: the adapter can
   return a PNG as an MCP image block and let the *host* model see it. That
   reaches L2-equivalent behaviour without lifting the core `[no]`. This
   resolves the apparent deadlock between "L2 is not reached" and "the product
   needs a loop".
4. **The roadmap is version-centric, not outcome-centric.** 0.0.6-0.0.18 each
   shipped one mechanism. The result is deep, but there is no stated user,
   workflow, install path or success metric anywhere in the PRD.
5. **The doc system is now the main scaling risk.** The PRD repeats the same
   status, roadmap and `[no]` list in 6-10 places, and several copies
   contradict each other (section 8). Every future cycle pays this tax.

**One paragraph for the owner:** stop adding CU mechanisms for one release,
close the H-line packaging gap, make the adapter mountable in under ten
minutes with recipes for three real clients, then ship the gate as the thin
egress layer that makes the family usable behind real accounts and networks.
The evidence culture is the moat; distribution is the bottleneck.

## 2. What the product actually is today

| Surface | Artifact | State | Can a stranger use it? |
|---|---|---|---|
| Worker core | `minicon-harness.com`, 6 native cells | published 0.0.19, core accepted | yes, but only as a scripted one-shot worker |
| MCP adapter ("hands") | `minicon-harness-mcp` binary | builds, 145 tests, E1-E7 green | **no** -- not a Release asset, no client recipe |
| Gate ("mouth") | `minicon-gate` | direction record only | no |
| Orchestration (L3) | none | target definition | no |

Three products, one repo, one PRD. The PRD's `## Product outcome` section
(lines 297-367) still describes only the worker core and is a pre-0.0.4
snapshot; the companion positioning the owner actually chose appears only in
the roadmap block and in plan files.

## 3. What is strong (do not lose it)

- **Evidence discipline.** `[v]` requires three-OS CI plus an independent
  reader plus owner-view acceptance. The v1 0.0.16 contract was falsified by
  measurement and rewritten. This is the differentiator; most agent tooling
  ships on assertions.
- **Bounded surface.** One file, three tools, named bounds, refusal as
  information. Easy to reason about, easy to audit, no policy engine.
- **CU depth on Windows.** MSAA tree, identity-addressed actuation, effect
  readback, native default action, text write. This is real capability, not a
  wrapper over PowerShell.
- **Honest failure vocabulary.** `unknown`, `stale`, `not_performed`,
  `unsupported` with reasons. Competitors return success or an exception.
- **Release engineering.** Signed cells, receipts, sidecars, Defender court,
  post-publish re-hash. Expensive, but it is why a `[v]` means something.

## 4. What is weak (product, not craft)

### 4.1 No user, no workflow, no success metric
The PRD says what the software does and how it is proven. It never says who
uses it, in what workflow, or what "working" looks like outside CI. "A script
hands a task to a model" is a capability statement, not a product statement.

### 4.2 Three product surfaces, no family story
`minicon-harness`, `minicon-harness-mcp`, `minicon-gate` (and `agenterm`, which
is referenced as owning the management surface but is not in this repo) have no
defined relationship. A reader cannot tell what the product family is called,
what the entry point is, or what is deliberately not built here.

### 4.3 The H-line is 90% done and 0% deliverable
The adapter is written, equivalence-tested and smoke-run. It is not in the
Release. This single gap invalidates the entire companion positioning and is
cheaper to close than any new capability. **This is the highest-leverage item
in the repo.**

### 4.4 A5 is treated as a blocker when the companion path does not need it
`PRD_01_12` makes L2 depend on A5 / D2 (a model-visible image arm). True for
the in-repo worker. False for the MCP companion: MCP has an image content type
and the mounting harness already has vision. The roadmap should split the L2
definition into **L2-worker** (needs A5) and **L2-companion** (needs the
adapter to return an image block + the host to see it). Today the PRD conflates
them, which makes the product look further away than it is.

### 4.5 Version-centric sequencing
Each version = one mechanism. Nothing sequences by user outcome. The result:
0.0.13 (wait) and 0.0.14 (schema) are prerequisites for a loop that was never
shipped, while the adapter that would let someone else run that loop sits
unpackaged.

### 4.6 Release engineering is becoming the bottleneck
6.25 MiB ceiling, two version strings in the APE resource, CRLF/LF traps,
six courts plus journeys plus signing plus Defender. Adding a second binary and
later a third multiplies this. The adapter needs a **lighter** packaging path
(per-OS plain binaries, own sidecar, no extra `.com` court) or it will not ship
at the cadence the product needs.

### 4.7 The gate is a crowded market if it is generic
LiteLLM, one-api/new-api, OpenRouter and every cloud gateway already route
OpenAI-compatible calls. The gate's only defensible reasons are: (a) per-lane
egress proxy scoped by account/model, (b) agent/workflow bindings as data,
(c) loopback-only with the same evidence discipline, (d) it is the companion
layer for *this* harness family. If it grows budgets, rate limits, key
management or a UI it becomes a worse LiteLLM.

### 4.8 Doc mass is outgrowing the code
PRD 73.6 KB / 1173 lines; `plan/` ~330 KB. The same status table exists in
PRD, `PRD_01_23`, `PRD_01_17`, `PRD_01_12` and README. Several copies are
stale. Onboarding cost is now higher than the codebase.

## 5. The strategic reframe

**The product is not "a harness". It is "the trustworthy local capability and
egress layer for other harnesses".**

```text
                    ┌──────────────────────────────────────┐
   other harnesses  │  Claude Code · OpenCode · Codex ·    │
   (borrow the loop)│  Cursor · Hermes · DSH                │
                    └───────────────┬──────────────────────┘
                                    │ MCP (stdio)
                    ┌───────────────▼──────────────────────┐
                    │  minicon-harness-mcp  ("hands")      │
                    │  screen / file / exec, unchanged      │
                    └───────────────┬──────────────────────┘
                                    │ bounded core
                    ┌───────────────▼──────────────────────┐
                    │  worker core (one file, 3 tools)     │
                    │  reference host + the CU engine      │
                    └──────────────────────────────────────┘

   model calls from any of the above
                    ┌──────────────────────────────────────┐
                    │  minicon-gate  ("mouth")             │
                    │  pool(account, model, args, proxy)   │
                    │  agent/workflow bindings as data      │
                    └──────────────────────────────────────┘
```

Consequences:

- **Wedge = Windows local CU for harnesses that lack it.** That is the one
  claim no listed competitor can make today (DSH CU is macOS-only experimental,
  Hermes depends on `cua-driver`, Claude Code has no local CU).
- **Worker core = reference implementation**, not the only product. Its
  one-file story stays; it no longer has to carry the whole product.
- **Gate = operational layer**, not a gateway product. It exists so a harness
  behind two accounts and a local proxy works, with evidence.
- **L3 orchestration = the demo that proves the wedge**, not a roadmap
  commitment. It is already defined as an acceptance journey in `PRD_01_12`.

## 6. Proposed roadmap (outcome-first, versions provisional)

The rule: **one theme per release, one exit criterion per theme, and no new CU
mechanism until the previous theme has a real user.**

### NOW -- 0.0.20: "Ship the hands"

Goal: a stranger can mount the adapter and drive one observe + one act.

- Adapter artifact in the signed Release (`minicon-harness-mcp`, per-OS plain
  binary; own sidecar; no extra `.com` court).
- Two-tier qualification actually run: scripted raw JSON-RPC on three OSes +
  one real client on Windows.
- `docs/clients/` recipes: Claude Code, OpenCode, Codex CLI (one page each,
  copy-paste config).
- README "first 10 minutes": download -> config -> observe -> act.
- H-line contract PRD (`prd/PRD_01_26_mcp_adapter.md`), because a shipped
  surface needs a contract document, not a plan file.
- MCP image content: decide and, if cheap, ship `observe` returning the PNG as
  an MCP `image` block. This is the companion's A5 and it lifts nothing.

Exit: adapter downloadable, three clients documented, one real mount recorded,
one image-block demo. **No new CU verb.**

### NEXT -- 0.0.21: "Make it stay mounted"

Goal: repeated daily use, not a demo.

- Adapter lifecycle: bounded startup, clean stderr logging, request timeout,
  cancellation, one resident host, restart behaviour.
- Error/refusal mapping audit against real clients (what they do with
  `isError`, large results, image blocks).
- Platform honesty: one non-Windows cell qualified for `screen` (macOS AX or
  Linux AT-SPI) or an explicit "Windows-first" product statement -- pick one,
  do not leave it implied.
- Usage feedback loop: issue template, a `doctor` subcommand that prints the
  same `disclose` row a user can paste.
- Lightweight plans for non-capability work (packaging/docs), so the 6/6
  ceremony is reserved for capability changes.

Exit: one external user runs it twice in a week; `doctor` output is enough to
diagnose a failed mount.

### LATER -- 0.0.22-0.0.23: "Ship the mouth"

Goal: two providers x two accounts x two proxies in one config, with
deterministic fallback and a reader that recomputes it.

- Gate slice 1-3 from `plan/v0.0.20.md` (pool, accounts, agents, per-lane
  proxy, fallback, inspect, evidence).
- Harness integration: `minicon-harness --base-url gate --model gate/<agent>`;
  same for OpenCode.
- Workflows remain declarations; `/resolve` only.
- No budgets, rate limits, key management or UI. Ever, in this repo.

Exit: one config routes a 429 from lane A to lane B through a different proxy,
and `scripts/gate-verify.py` recomputes the selection from events.

### BET -- 0.1.x: "Close the loop"

Only if NOW/NEXT/LATER pull. Candidates, not commitments:

- `openai-codex` subscription lane (external token refresher).
- Harness lanes in the gate (CLI / MCP / ACP) -- the original M-line "mouth".
- L2-worker: A5 / D2 image arm, D4 ceilings, F3 second half, C5/C6.
- L3 demo: the controller drives the WorkBuddy GUI and aggregates sub-tasks.

Explicitly **not** on the roadmap: UIA routing (closed by measurement),
resource-domain authority (G closed), plugin host, policy engine, management
UI, Wayland-native, incremental a11y events (no symmetric three-OS surface).

### Sequencing rationale

Hands before mouth is already the recorded ordering, and this review keeps it,
but for a sharper reason: **the adapter is the only surface that can acquire a
user without the gate.** The gate has no user until a harness is already
mounted and has an account/proxy problem. Shipping the gate first would produce
a second unvalidated product.

## 7. Proposed PRD restructure

Target: `PRD.md` from 1173 lines to ~250-300, one fact per fact.

```text
PRD.md
├── 1. Product definition        what / who / why / family (new, ~40 lines)
├── 2. Status at a glance        one table: published / accepted / claimed
├── 3. Roadmap                   NOW / NEXT / LATER / BET (links ROADMAP.md)
├── 4. Governing invariants      the current 6, unchanged
├── 5. Domain model + G boundary one paragraph + link PRD_01_08
├── 6. Capability index T1-T9    one line per tree, link subtree
├── 7. Non-goals                 THE canonical list (single copy)
├── 8. Evidence + decision rules grades, [v] definition, T8, links
└── 9. Document map              prd/ plan/ docs/ HISTORY.md
```

Move map:

| Current PRD block | Destination |
|---|---|
| `0.0.1` .. `0.0.19` frontier prose (lines ~541-1173) | `HISTORY.md`, verbatim |
| Version roadmap + line-freeze block (46-99) | `ROADMAP.md` (rewritten) |
| Product outcome tree (297-367) | rewrite in `PRD.md` §1; old snapshot to `HISTORY.md` |
| CU capability tree (406-502) | keep as §6 index; detail stays in subtrees |
| T1-T9 trees (130-260) | keep as §6, one line per node |
| W0 ledger summary (271-295) | keep, or move to `PRD_01_17` (already there) |
| Repeated `[no]` blocks (8 copies) | §7 only; per-version line becomes "no `[no]` changed" |
| Duplicate status sentences | §2 only |

New files:

- `ROADMAP.md` -- section 6 of this review, maintained as the single roadmap.
- `HISTORY.md` -- all release history, verbatim, newest first or oldest first.
- `prd/PRD_01_26_mcp_adapter.md` -- H-line contract (currently only a plan).
- `prd/PRD_01_27_llm_gate.md` -- M-line contract, when the gate becomes a version.
- `docs/clients/{claude-code,opencode,codex-cli}.md`.
- `docs/status.md` -- optional machine-readable status table.

Non-goal of the restructure: deleting evidence. Every CI run id and dated
decision stays, just not in the entry document.

## 8. Concrete doc defects (evidence for the restructure)

| # | Where | Defect |
|---|---|---|
| 1 | `PRD.md` 31-36 | "0.0.19 published" and "v0.0.18 remains the latest accepted Release" and "v0.0.18 is the latest public signed Release" in one paragraph |
| 2 | `PRD.md` 208-224 (T7) | version tree stops at 0.0.18; no 0.0.19 row |
| 3 | `PRD.md` 355-366 | the `[no]` paragraph is line-broken into nonsense ("...a second model loop / authority.") |
| 4 | `PRD.md` 118-127 | `plan/v0.0.19.md` listed twice (inside and outside the tree) |
| 5 | `README.md` 152, 199-201 | "v0.0.15 is the latest" and "v0.0.18 is the latest" both present |
| 6 | `README.md` 246-264 (Layout) | no `src/mcp.rs`, no adapter binary, no gate |
| 7 | `Cargo.toml` 5 | description says "exactly two tools, file and exec" |
| 8 | `prd/PRD_01_17` 492-494 | H-line row still says "open: W0/W1"; 0.0.19 shipped |
| 9 | `prd/PRD_01_23` T7 | no 0.0.19 row; C9 still "eligible" without the packaging gap |
| 10 | `prd/PRD_01_12` 128-135 | "all four version sources read 0.0.15" in present tense inside a current section |
| 11 | repo-wide | no H-line contract PRD; the adapter's contract lives in a plan file |
| 12 | repo-wide | family names undefined (`minicon-harness` vs `-mcp` vs `minicon-gate` vs `agenterm`) |

## 9. Three decisions I would take now

1. **0.0.20 = H-line closeout, not the gate.** The adapter is the only surface
   that can produce a user this quarter. The gate direction record stays open
   and unversioned until a mounted harness has a real account/proxy problem.
   (This supersedes the provisional 0.0.20 numbering in `plan/v0.0.20.md`; the
   content stands, the version slot moves.)
2. **Split L2 into L2-worker and L2-companion.** A5 is required only for the
   former. The companion reaches the same closure through MCP image content.
   `PRD_01_12` is upserted, no `[no]` changes.
3. **One fact, one file.** Roadmap -> `ROADMAP.md`; history -> `HISTORY.md`;
   status -> one table; non-goals -> one section. The PRD becomes readable in
   ten minutes.

## Correction 2026-11-09 (owner direction)

Decision 1 in section 9 is superseded. The repo's recorded roadmap assigns
`>= v0.0.20` to the M-line gateway sidecar, and the owner direction of
2026-11-09 confirms it. **0.0.20 is the M-line gate skeleton**, not the H-line
closeout: the already-built adapter folds into 0.0.20 W0 as a release-
engineering precondition (it is the gate's first consumer), not as the version
theme. Everything else in this review stands; see `plan/v0.0.20.md` for the
corrected 0.0.20 plan.

## 10. What this review did not do

- No PRD rewrite, no file moves, no version bump, no code.
- No new capability claim; nothing here is `[v]`.
- No change to the H-line 6/6 table, the M-line direction, the G boundary or
  any published asset.

If the owner approves the restructure, the next step is mechanical: create
`HISTORY.md` by moving the frontier sections verbatim, then rewrite `PRD.md`
to section 7's outline with links, then fix the twelve defects in section 8.
