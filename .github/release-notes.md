minicon-harness is a bounded agent worker in one portable file — no
installer, no bundled runtime. It runs one local file/exec task through a
model and prints the final answer on stdout.

## Downloads

| Platform | Download |
| --- | --- |
| All: Windows, macOS, Linux (x86_64 and ARM64) | `minicon-harness.com` |
| Windows x86_64 | `minicon-harness-VERSION-windows-x86_64.zip` |
| Windows ARM64 | `minicon-harness-VERSION-windows-arm64.zip` |
| Linux x86_64 | `minicon-harness-VERSION-linux-x86_64.tar.gz` |
| Linux ARM64 | `minicon-harness-VERSION-linux-arm64.tar.gz` |
| macOS (Apple Silicon + Intel, universal) | `minicon-harness-VERSION-macos-universal.tar.gz` |

`minicon-harness.com` is one launcher file for all six platforms. Run it
directly; on Windows it runs as an `.exe` regardless of the name. The
per-platform archives carry the same native binary with a plain name if you
prefer not to run the APE launcher.

## Quick start

```bash
chmod +x minicon-harness.com          # macOS/Linux once
export HARNESS_API_KEY=sk-...         # your model API key
./minicon-harness.com --version
./minicon-harness.com --root ./playground \
  --task "write hello.txt containing your name, then show it with cat" \
  --allow-cmd ls --allow-cmd cat
```

`--root` (required) confines the `file` tool; `--allow-cmd` names the only
executables the `exec` tool may run. `--base-url` and `--model` point the
loop at any OpenAI-compatible chat-completions endpoint. One invocation runs
one bounded task: 16 model turns, 64 tool calls, 10 minutes.

Version VERSION ships company-signed bytes: `minicon-harness.com` and the two
Windows cells carry an Authenticode signature (O=PARTNERNET SOFTWARE PTY LTD)
with an RFC 3161 timestamp. `signing-receipt.json` records the before/after
SHA-256 of each signed file, while `build-receipt.json` and the `.sha256`
sidecars bind every other asset to the packed source.

## VERSION: handle-addressed window targeting

`op=act` with `action:"focus"` gains an optional `hwnd`: a positive integer
taken from a prior `observe.windows[]` row. It is mutually exclusive with the
existing `window` title; giving both, neither, a non-integer handle or zero is
a named refusal before any native effect. Every call re-resolves the handle
against a fresh live window list and re-checks the resolved window's current
title. A stale, reused or mismatched handle is a named `stale` refusal, never
a silent fallback to whatever happens to be in the foreground.

Focus evidence and readback now publish `target_hwnd`: the desktop's focused
window handle at dispatch time, read from the live window list and never
echoed from the request. The existing title-target spelling still publishes
it, so old callers gain identity evidence without changing their calls. Focus
posts no button event, so `evidence["actuation"]` stays `null`;
`target_hwnd` is separate typed evidence, not a fabricated button receipt. A
session that exposes no foreground window publishes `null` with a named
`target_hwnd_reason`, never a plausible handle.

The reason is measured. In the L1 self-hosted orchestration trial two
WorkBuddy client windows were both titled `WorkBuddy`; `act:focus` resolved a
window by unique title, so the controller could not name the sub-session
window it wanted and fell back to clicking pixel coordinates. A coordinate
basis cannot fix an ambiguous identity: the same rectangle can belong to
either window. The 0.0.11 `observe.a11y` arm made a handle a *scan* target;
this version makes it an *act* target and reads the focused handle back.

`native::window_by_handle(handle: u64) -> Option<WindowInfo>` is added with
the same signature on Windows, macOS, Linux and the fallback cell. Windows
resolves through the existing `EnumWindows` list and `IsWindow`; macOS and
Linux resolve through their existing window enumerations; the fallback returns
`None`. `scripts/cu-cfg-check.py` type-checks all four configurations in the
same commit. No new crate, no link-time platform library, no OS grant, no
window-manager behaviour and no process or memory authority: the G boundary
in `PRD_01_08` is unchanged.

This is additive evidence on the existing `act:focus` capability: `cu-abi
1.0`, the wire action enum and the capability matrix (windows 12/12, linux
10/12, macos 8/12, other 0/12) are unchanged. `hwnd` is a typed optional
parameter, not a new capability row. A cell whose published matrix declares
`act:focus` unsupported produces a named `unsupported` boundary for the two
handle steps; it never claims a performed focus.

The model-visible image arm, explicit `wait`, batched actions, AT-SPI / AX
a11y trees, app-level effect evidence and safety policy remain non-goals
with their unlock conditions. The bounded Windows MSAA read-only subset
shipped in 0.0.11 is unchanged.

## Verification

CI makes the handle a required step rather than a claim:
`.github/workflows/cu-journeys.yml` drives `observe_focus_readback`,
`act_focus_by_handle`, `observe_handle_focus_readback` and
`act_focus_stale_handle` on all three OSes. The independent reader recomputes
that every performed title- or handle-target focus publishes a positive
`target_hwnd`, that the readback carries the same integer, that it equals the
focused window in the very next observation, that the stale handle is absent
from the live window list, and that a matrix-declared `act:focus=false` cell
only produces a named capability boundary — on top of every 0.0.6-0.0.11
assertion (receipts, `path_points`, both scroll axes, held keys, `grants[]`,
`environment`, `display_scales[]`, the a11y tree, generations, live final
frame, declared-but-skipped rule, BOUNDARY). A `BOUNDARY` note excuses
nothing else.

`.github/workflows/cu-platforms.yml` builds, tests and release-links all
three OSes; `.github/workflows/cu-journeys.yml` drives a real desktop on each
(Linux under Xvfb, macOS in the runner GUI session, Windows with a titled
window). Every effect is read back, every performed button event carries a
receipt and an independent reader recomputes the verdicts from
`cu-journey.json`. Missing OS grants (Screen Recording, Input Monitoring,
`$DISPLAY`) remain named boundaries.

If something refuses to start, run the file from a terminal and read the
error line; every refusal names the bound it hit.
