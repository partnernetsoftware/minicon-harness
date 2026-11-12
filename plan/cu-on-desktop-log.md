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

## Not yet exercised

`act:control` invoke, `set_value`, `click`, `drag`, `scroll`, screenshots and
the effect readback. They are next, in that order, against a window that can
be restored if something misbehaves.
