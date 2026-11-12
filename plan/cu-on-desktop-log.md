# On-desktop use log — computer use, 2026-11-12

Written while actually driving the release `minicon-harness-mcp` over MCP stdio
against the live desktop, per the owner's direction ("use it while you improve
it"). Every entry is a measured fact from this session, not a review opinion.

Environment: Windows, 1680x1050 single display, 19 windows, `cu-abi 1.0`,
`minicon-harness-mcp 0.0.21` (release build), driver `mcp-driver.mjs` in the
session workspace.

## What works

- `observe` (window inventory) — 19/19 windows in **16 ms**, and minimized
  windows publish a usable rect rather than the `-32000` tombstone
  (`hwnd=128583710 rect=-8,-8 1696x1030 minimized`).
- `observe a11y` on the foreground WorkBuddy window — 382 nodes with names,
  roles, states, rects and values, `verdict=performed`.
- The MCP adapter answers `initialize` with `protocolVersion 2025-11-25` and
  the three-tool list; one bounded call per request.

## Findings, ranked by how much they cost the caller

### F1 — a misspelled or unknown parameter is silently ignored

`screen {op:"observe", a11y:true, hwnd:19798326}` returns a **successful**
observation of the *foreground* window. The caller asked for a specific window
and got a different one, with `verdict=performed` and no warning. The only way
to notice is to read the `a11y: source=handle:...` line and compare it to the
request.

Root cause: there is no unknown-parameter check anywhere in the tool argument
parsing. `hwnd` is not a parameter of `observe` (the window is named through
`a11y: <hwnd>`), so it is accepted and dropped.

Why it matters: a silent wrong-window observation is exactly the class of bug
this project's evidence discipline exists to prevent. It is also the first
thing a real client author will hit, because `hwnd` is the name every other
verb uses.

### F2 — the window identity in the reply is easy to miss

The observation body starts with the window inventory and ends with
`a11y: source=handle:1442988 window:"..."`. A caller that reads the node list
sees "honest" data for the wrong window. The identity is present, but it is
buried after 19 inventory lines and before 382 node lines.

### F3 — `observe a11y` of a background window took 12-17 s

Three consecutive a11y observations of background/minimized windows took
16.8 s, 16.5 s and 13.7 s, against 16 ms for the inventory. The plan's own
record says the UIA kick plus bounded retry wakes a Chromium renderer; that
retry appears to run its full budget even when the tree is available. Not yet
diagnosed — recorded as measured, not explained.

### F4 — `observe a11y` of a specific non-Chromium hwnd did not answer in 60 s

`{"op":"observe","a11y":40307282}` (an explorer `CabinetWClass` window that the
inventory lists as minimized, `pid=149648`) produced **no reply at all** in 60
seconds; the driver timed out and killed the process. The same release answered
`{"op":"observe","a11y":true}` in 13.9 s with 400 nodes (truncated at the
400-row bound).

So the failure is specific to naming a background window by handle, not to
a11y scanning in general. Not yet diagnosed: the retry budget, the per-node
walk, or a blocking COM call on the minimized window are the candidates. This
is the one finding that makes a client *hang* rather than *lie*, and it is
ranked first for that reason.

## Fixed during this session

**F1 fixed** (commit `d3b7de0`): `parse_request` now refuses an argument the
requested `op` does not read, by name, with the argument that probably was
meant (`hwnd` → `a11y: <hwnd>`; `title` → `window: "<title>"`) and the list the
op does accept. The check is **per op**, because `hwnd` is legal for `act` and
must stay so. Verified on the live desktop: the same call that used to return
the foreground window now answers

```text
screen: refused: `hwnd` is not an observe argument; name the window with
`a11y: <hwnd>` for an accessibility scan, or `window: "<title>"` for a capture
```

and the valid forms still work (`a11y: true` returned 400 nodes).

## ZCode session, 2026-11-12 (the owner's target window)

Window: `hwnd=19798326`, `pid=302640`, `Chrome_WidgetWin_1`, title `ZCode`,
**minimized**, saved placement `503,147 1216x808`.

### What worked

`observe {a11y: 19798326}` published **123 nodes** in 4.1 s with names, roles,
states and rects, and disclosed the minimized remap honestly:

```text
a11y: window is minimized: raw rects are tombstone- or client-basis and have
been translated onto the saved normal placement (live origin=-32000,-32000
placement origin=503,147 screen basis; the placement was captured, never
changed, and a Chromium window may keep staler bounds than the placement)
```

That disclosure is exactly right: the window was still minimized, and the tree
came back with usable coordinates.

### F5 — `as_of` and `source: "a11y"` need host state that one MCP call cannot see

`act:control {control:"切换侧边栏", source:"a11y", as_of:1}` answered

```text
cu: stale: this call is pinned to generation 1, the host is at 0
```

and without `as_of`:

```text
screen: refused: there is no observation yet; call op=observe with `controls`
before using act:control
```

Both are correct given the design, and the design is the problem. The screen
host lives inside the adapter process. An MCP client sends `observe` and the
next `act` as **two separate `tools/call` requests**; the adapter keeps one
resident host for its whole stdio session (the 0.0.21 plan proves this with
`the_screen_host_is_resident_across_stdio_calls`), so this *does* work when both
calls share a session — measured: the same two calls in one session reached
generation 1 and the invoke ran.

What does not work is the case a client actually hits: a session that observes,
is restarted or reconnects, and then acts. `as_of` pins a generation the new
host never had, and the refusal tells the caller to observe again — which is
correct, but a caller who passed `as_of` from a previous session gets `stale`
rather than "your generation is from another host".

### F6 — CORRECTED: the invoke did work; my first reading was wrong

**Retraction.** The first version of this entry said the invoke reported
`performed` and changed nothing, and called it the most dangerous finding in
the log. That was a **measurement error on my side**, and the correction is
recorded rather than the wrong claim quietly deleted.

What actually happened: I observed in one MCP process, acted in a second one,
and observed again in a third. Each process builds a fresh screen host, so the
third observation could not see anything the second had done — and the second
had, in fact, worked. The receipt even said `focus changed` and
`invoke via uia-invoke-pattern`; I read a "unchanged" tree from the wrong process
and concluded "fake success".

What the same-session run shows:

```text
observe generation = 1 nodes = 123
act => readback: invoke; control="切换侧边栏" role=button rect=516,164 28x28
      invoke via uia-invoke-pattern; effect observed: the window tree changed (66 nodes)
      cu-abi=1.0 generation=1 verdict=performed
```

and a following observation: `nodes=66`, with `自动化` and `插件市场` gone from
the tree and only `切换侧边栏` left in the header. **The sidebar collapsed.** A
minimized Chromium window was operated without restoring it.

The lesson is worth more than the bug would have been: **cross-process
observation of a stateful UI is not evidence.** The session-scoped host is not a
convenience, it is what makes the evidence meaningful, and my own tooling
violated that by spawning a process per call.

### What the retraction changed in the code

The invoke now carries an **effect readback** anyway, because the wrong claim
was only possible while "S_OK" and "the interface changed" were the same word:

```text
; effect observed: the window tree changed (N nodes)
; effect NOT observed: the window is unchanged after the invoke (N nodes, the
  target label is still present) -- an S_OK from the provider is not a DOM effect
; effect unverified: the window tree could not be re-read
```

A `performed` invoke whose target label is still present **and** whose node
count is unchanged is downgraded to `not_performed`. The check reads control
names only, through `screen::a11y_control_names`, and is skipped when the host
has no observed node count to compare against. It cost one re-scan per invoke;
on ZCode that is ~4 s, which is the price of not calling S_OK a DOM effect.

### ZCode results, measured both ways

| act | before | readback | after |
|---|---|---|---|
| `切换侧边栏` (sidebar open) | 123 nodes | `effect observed: the window tree changed (66 nodes)` | 66 nodes, `自动化`/`插件市场` gone |
| `切换侧边栏` (sidebar closed) | 66 nodes | `effect observed: the window tree changed (123 nodes)` | 123 nodes, sidebar back |
| `后退` | 123 nodes | `effect observed: the window tree changed (66 nodes)` | navigation really moved |
| `前进` (disabled, `state=1`) | 66 nodes | refused before effect | unchanged |

Every one of these ran against a **minimized** ZCode window: the window row was
`rect=503,147 ... minimized` throughout, and no call restored it. Two of the
four `focus changed` (Chromium focusing its render widget on a default action);
the disclosure is what made that visible.

## Not yet exercised
`act:control` invoke, `set_value`, `click`, `drag`, `scroll`, screenshots and
the effect readback. They are next, in that order, against a window that can
be restored if something misbehaves.
