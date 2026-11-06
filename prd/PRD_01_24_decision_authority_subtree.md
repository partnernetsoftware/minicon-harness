# PRD 01.24 — decision authority subtree (T8)

Status: **study + structure only**; changes no source, version, capability
row, `cu-abi`, `[no]` row, G boundary or published asset. Owner of **who
decides what** in this repo. Learning ledger: `PRD_01_17`.

Owner direction (2026-11-06, 22:15):

> 为什么要我做决定，你能自己安排吗？
> ("Why am I making the decision? Can you arrange it yourself?")

This is an explicit correction, not a question to be answered with options.
The controller had ended the previous cycle by pasting the three G-boundary
choices back to the owner. That was an **escalation error**: a reversible,
high-confidence decision with an obvious safe default was pushed upward. Under
the standing `/goal` delegation the controller owns the whole loop — learn,
structure, plan, order versions, open gates, implement, release, accept — and
reports the decision with its reason. Nothing in this document asks the owner
to choose.

## T8 — decision authority tree

```text
T8 Decision authority (controller-autonomous under /goal)
├── D1 owner directions, recorded verbatim with date                [v]
│   └── /goal, 2026-11-06 day/evening, 2026-11-06 22:15 correction
├── D2 controller decisions — no prior owner approval              [decided]
│   ├── D2.1 version topic, candidate ordering, gate, W1-W4 timing
│   ├── D2.2 additive capability inside cu-abi 1.0 + 12/10/8/0 matrix
│   ├── D2.3 lifting one [no] with dated reason and explicit bounds
│   ├── D2.4 the G boundary direction (this document: (a))
│   └── D2.5 release chain, CI gates and owner-view acceptance
├── D3 owner veto, after the fact — never a pre-approval gate      [v]
│   └── one sentence from the owner overrides any D2 decision
├── D4 escalation discipline                                      [no]
│   ├── [no] pasting options + waiting for a reversible decision
│   │        with a safe default; that was the 22:15 correction
│   └── unlock: only an irreversible choice with no defensible default
│        (deleting published assets, changing the product definition),
│        where the controller must still record the reason and stop
└── D5 decision record format                                     [v]
    └── date / choice / reason / evidence grade / reopen trigger
```

`[decision]` in every other document means **a dated decision already taken
by the controller**, not "waiting for the owner". Historical records that say
"owner decision" are kept verbatim where the owner actually spoke (for
example the 0.0.11 a11y lift); from 2026-11-06 22:15 the same class of
decision is D2 and is taken by the controller.

## Decision record 2026-11-06 22:15 — the G boundary

**Choice: (a) keep G closed.** `screen` stays K/V/M/W/S plus the window
target; process, CPU, memory, GPU, device and network state remain outside
it. The whole resource domain continues to be learned and structured in
`PRD_01_21` as `[study]`, with no authority attached.

**Reason.** `PRD_01_08` makes G a structural boundary, not a missing verb.
Direction (b) (typed read-only telemetry in `disclose`) still widens what
`screen` may observe and needs a threat model, three-OS probes and an
independent reader; direction (c) needs a fourth tool or an ABI change, new
grants, a policy engine and an audit surface — a different product from a
bounded one-file worker. With the current phase being STUDY + STRUCTURE,
opening either is not defensible. Keeping G closed is reversible and has a
zero-risk default, so it is decided, not offered as a choice.

**Reopen triggers for (b), all four required, judged by the controller:**

1. X-grade evidence from three-OS read-only probes, with named `unknown`
   reasons on every cell that cannot report a value.
2. A dedicated PRD with its own threat model, evidence semantics and the
   exact additive rows (no new verb, no new grant, no `cu-abi` bump).
3. An independent reader rule plus a three-OS CI journey that fails closed.
4. A plan-completeness pass in `PRD_01_23`.

**Direction (c) is `[no]`** unless the product definition itself changes;
that is D4 territory, not a version decision.

**Evidence grade:** O (`PRD_01_08` boundary), X (this repo's shipped
12/10/8/0 matrix and four-backend gate). No new capability is claimed.

## Decision record 2026-11-06 — stop W2 after the W1 probe

**Choice:** stop 0.0.15 W2 (UIA-first + MSAA fallback), claim no UIA
capability, run no version bump and no Release chain. Keep
`examples/cu_probe_uia.rs` in the tree as reproducible X-grade evidence.
`v0.0.14` remains the last shipped Release.

**Reason.** The W1 probe on the real desktop returned `S_OK` from every COM
slot and a usable `RECT` from the desktop root, while Brave / Sourcetree / GVIM
/ cmd produced hundreds of named, bounded, non-Pane rows. But both WorkBuddy
windows (the L3 self-hosted-orchestration target) exposed only 8 nodes, all
Pane, only one named and **zero non-Pane controls** — the shipped 0.0.11 MSAA
tree reads real controls on that same client. Cold-start reads also returned
all-zero rectangles with `S_OK` before the same handle settled to
`840,0,1680,1014` on four consecutive re-runs. Shipping UIA-first would
therefore make `observe.a11y` *worse* on the exact window the end state needs,
and the plan's mandatory downgrade rule (written before W1, not invented
afterwards) requires stopping on the zero-rectangle reads. No owner choice was
requested.

**Reopen trigger:** a probe that reads named, non-Pane, stable bounded nodes
from the WorkBuddy client itself across repeated reads, plus its own PRD and
independent reader rule. A successful `CoCreateInstance` on some other
window, or a single non-empty tree, is not sufficient.

**Evidence grade:** X (local measurement, `plan-0.0.15` W1 delivery record).

## Decision record 2026-11-06 22:15 — working cadence

The controller arranges the rest itself, in this order, without returning for
a choice:

1. W0 pass 4: verify the UIA COM surface against the **locally installed
   Windows SDK headers** (grade O/X), replacing the second-hand vtable claim;
   learn the resource-telemetry trust boundary as a study note. **Done.**
2. Upsert T1/T3/T5/T7/T8, the W0 ledger and the 0.0.15 plan in place. **Done.**
3. Complete the 0.0.15 plan-completeness checklist (6/6) and mark W1
   eligible. **Done.**
4. Start W1 (Rust UIA diagnostic probe) after step 3, then follow its
   downgrade rule. **Done — the rule fired, W2 is stopped (decision above).**
5. Continue W0 with the next open-gate candidate; report decisions, reasons
   and results, not a menu.

## Learning notes (dated)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 4 | local `UIAutomationClient.h` / `.idl` vtable, CLSID/IID, `RECT`, `TreeScope` | O (installed SDK 10.0.22000.0) | a header is authoritative for the SDK it ships with, not for every runtime; a probe must still print raw HRESULTs | T1 P2.4, `plan-0.0.15` |
| 2026-11-06 | 4 | host-side vs guest-side resource telemetry trust | O / P / S | guest self-reports are debugging data, never a security boundary; host-side signals sit on the far side of the isolation boundary | T3 R9/R10 |
| 2026-11-06 | 4 | decision authority under `/goal` | owner direction | a reversible default must not be escalated; the owner keeps a veto, not an approval queue | this file, T8 |
| 2026-11-06 | 5 | W1 UIA probe verdict on the real desktop | X (this machine) | every slot S_OK + usable desktop root RECT, yet the target provider gave 8 Pane nodes / 0 non-Pane controls; cold-start zero RECTs became stable on re-run. The downgrade rule stopped W2 without an owner choice | `plan-0.0.15`, `PRD_01_17` pass 5, T7 |

## Non-goals

- `[no]` re-opening the G boundary in this phase.
- `[no]` adding a resource-domain verb, tool, grant or capability row.
- `[no]` treating a study note as a decision, or a decision as evidence.
- `[no]` rewriting published version history or shipped Release assets.
