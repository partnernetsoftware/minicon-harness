# PRD 01.12 — capability levels and the self-hosted orchestration target

Status: `[_]` a target definition, **not an implementation, not a version plan,
not a `[v]` claim**. It defines what "computer use has reached a certain level"
means and records the owner's end state. It changes no code, no version source,
no capability row and no `[v]` / `[_]` / `[no]` mark in `PRD.md`. It was written
after `v0.0.8` was published and accepted, and before any W of plan 0.0.9 ran.

Ownership: this document owns the **levels** and the **acceptance scenario**.
`PRD_01_03` owns the `screen` bounds, `PRD_01_04` the host contract,
`PRD_01_06` the capability matrix and the six false cells, `PRD_01_07` the
actuation receipt, `PRD_01_08` the domain model and the resource boundary,
`PRD_01_09` / `01_10` / `01_11` their own additive verbs and evidence. Version
plans live in `plan/`; version status lives in `PRD.md`.

Legend: `[v]` delivered with evidence, `[_]` defined or planned, `[no]` explicit
non-goal.

## The end state

The owner's direction: once computer use reaches a sufficient level, use it to
**interact with the WorkBuddy client itself** — create, dispatch, read and
retire sub-task sessions through the client's GUI — so that this session
(`minicon-harness`) becomes the controller: it issues instructions, checks
gates, collects evidence and aggregates results, and does not do the sub-tasks
itself.

This is an **acceptance journey for computer use**, not an orchestration
product. Deliberately, the path is the GUI: a client-side session API or CLI
would be a more direct way to manage sessions, but it would prove nothing about
`screen`. Driving the tool's own host is the hardest closed-loop evidence the
CU surface can produce, because every observe, act, receipt and readback must
be real on a desktop that the tool did not build.

## Domain fit

Inside the KVM + W + S model of `PRD_01_08`, this is entirely K (typing),
V (window inventory and pixels), M (pointer), W (target binding to a window)
and S (session state). A session in the client is a window and text on a
desktop; it is **not** process, memory, CPU, device, network or hardware
management authority. The `G Resource-domain boundary` is unchanged: a
window's `pid` stays identity metadata, `exec` still owns processes, and
`screen` gains no host-control verb. Nothing here requests or escalates an OS
grant; the preflight-only rule of `PRD_01_11` applies unchanged.

## Capability levels

```text
L0 read-only observation                                  [v] shipped
   virtual desktop, displays[], ordered windows[], PNG under the task root,
   disclose with a static capability matrix
   can locate the WorkBuddy window; the model cannot see the image and no
   interaction loop closes
   evidence: A1, A3, C1, C4, E1, E2, F1

L1 evidenced interaction                            [v] shipped 0.0.6-0.0.8
   click / drag path / two-axis scroll / type / key / held-key modifiers,
   each with an actuation receipt, pointer or foreground readback, and
   generation / as_of pinning
   can click an input box, send a message and switch sessions, but a controller
   supplies every coordinate and every verdict; screenshots still do not
   return to the model through the wire
   evidence: B1-B7a, C3, F1, F2

L2 self-sustained loop                                     [_] not shipped
   observe -> decide -> act -> wait -> re-observe, iterated without a human
   supplying each coordinate, until a sub-task reaches a terminal state
   required: A5 / D2 model-visible image arm (base64 PNG in the wire codec);
             B7b explicit bounded wait + D4 iteration / time / cost ceilings;
             A4 or an equivalent reliable target location
   why A4: the WorkBuddy client is Electron / Chromium, so the typed Win32
           child-control enumeration of A2 does not reach its rendered UI;
           a bounded Windows MSAA tree shipped in v0.0.11, but L2 still
           requires a full A4 / equivalent reliable target location on all
           three OSes or an image-based target
   why B7b (hard evidence, not a model-capability gap): in the L1 trial an
           `observe` issued immediately after an `act` captured the
           pre-reflow Chromium frame; `type` and its send click had to be
           split into separate runs before the new state was visible
   every turn still carries generation / as_of, a receipt and a readback
   unlock: each item keeps its own PRD, gate and owner decision

L3 self-hosted orchestration                    [_] target, not shipped
   L2 plus F3 app-level effect evidence and C5 / C6 safety + human hand-off
   the controller session manages sub-task sessions in the client GUI and
   aggregates their results; an actuation receipt alone is not enough here,
   because it proves a button was posted, not that a sub-task started or
   produced output
   adds no verb, no tool, no capability row and no harness-side capability
   unlock: three-OS side-effect-free application fixture, a policy PRD,
           bounded iterations and an explicit hand-off
```

**The threshold the owner asked about is L2.** L2 is the lowest level at which
the loop can close on its own; L3 is the complete form in which the result is
trustworthy enough to aggregate. Anything less than L2 remains a
human-supervised sequence of single acts, which is what L1 already is.

## Hard boundaries

1. No new capability row, no `cu-abi` bump, no change to the 12 / 10 / 8 / 0
   matrix or the six false cells. The levels describe evidence and closure,
   never a new permission surface.
2. `[no]` carried forward unchanged: plugin host, permission policy engine,
   streaming, MCP, a scripting surface over the desktop, a second model loop,
   takeover / owner-lock / watch mode (C7), any new crate or runtime,
   bun / WASM / ujs embedding (PRD 31 readiness stays BLOCKED), runtime
   backend selection, Wayland-native protocol, and host-resource management.
   A level definition lifts none of them.
3. **No unbounded self-reference.** The controller must not drive CU into its
   own input area to spawn sessions without end. Every dispatch needs D4
   ceilings and an explicit C5 / C6 hand-off; a session that cannot prove its
   terminal state is `unknown`, never `performed`.
4. No permission prompt, no TCC dialog, no `AXIsProcessTrustedWithOptions`,
   no UAC. A probe that cannot answer safely is `unknown` with a reason.
5. Honesty over completeness: no `[v]` for a level that only compiled, and no
   reader assertion weakened to make a journey green. F1 / F2 and the
   independent reader stay the authority on whether a level was reached.

## Effect on version planning

- `v0.0.9` is a published, accepted signed Release. Its E3 / E4 read-only
  disclosure is a precondition of the end state: without a typed grant status,
  a controller cannot tell why a click did not land. `v0.0.10` delivered the
  coordinate basis (E5a), `v0.0.11` delivered bounded Windows MSAA structured
  control disclosure (E5c), and `v0.0.12` delivered handle-addressed window
  targeting (E5b); all are published, accepted signed Releases. `v0.0.13`
  delivers bounded observation wait (`PRD_01_16`); all four version sources
  read `0.0.13`. Later versions are **not planned**: the W0 frontier radar
  (`PRD_01_17`) upserts candidates without promising a version.
- The candidate pool stays *ordered* by what the L2/L3 loop needs: A5 / D2 ->
  B7b + D4 -> A4 -> F3 -> C5 / C6, with B8 and the other 0.7.x chapters
  keeping their recorded unlock conditions. B7b leads with the hard evidence
  above, not with a model-capability claim. On 2026-11-06 the controller,
  under the standing `/goal` delegation, selected B7b + D4 for 0.0.13 ahead
  of A5: the measured failure is act/observation sequencing, not a lack of
  pixels. This lifts the older `explicit wait` non-goal only for the bounded
  read-only `observe.wait_ms` parameter in `PRD_01_16`; A5 / D2, B8, AT-SPI /
  AX trees, F3 and C5 / C6 remain deferred. Ordering is a reason to consider
  an item first, **not a version commitment**. L2 is still not reached.
- Each candidate still needs its own PRD, its own gate, an owner decision and a
  version plan that carries the gate before it enters a version. The next
  version is planned only after the previous one is published as a public
  signed Release and accepted from the owner's view.
- The owner's standing direction holds: concentrate on computer use; harness
  capabilities stay deferred.

## Non-goals

- `[no]` a client-side session API, CLI or plugin as the orchestration path.
  Those may be more efficient, but they are outside the CU acceptance journey.
- `[no]` a second model loop, an image budget decision, a policy engine, a
  full three-OS a11y implementation or an application fixture in this
  document. The bounded Windows MSAA subset shipped in `v0.0.11` under
  `PRD_01_15`; AT-SPI / AX trees remain deferred.
- `[no]` claiming any level is reached before three-OS CI journeys and the
  independent reader say so.
- `[no]` re-planning, re-publishing or overwriting any shipped Release.

## Where this goes next

Nothing is scheduled by this document. When a future version plan proposes one
of the L2/L3 prerequisites, it must cite this PRD, state which level it moves,
and carry its own gate; otherwise the level table above stays exactly as
written.
