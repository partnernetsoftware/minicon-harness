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

## VERSION: normalised a11y node schema

The read-only structured accessibility tree from 0.0.11 now publishes a
normalised node schema. Every `a11y_controls[]` row gains `value` (the
`accValue` string), the raw `STATE_SYSTEM_*` `state` bit mask and
`state_names` derived from a fixed 16-bit table. The observation frame names
the schema with `a11y_node_schema: "role-name-state-value-bounds-depth"`.
All earlier fields — `name`, `role`, `role_name`, `rect`, `depth`, `child_id`,
`a11y_source`, `a11y_reason`, `a11y_truncated` — are unchanged.

The raw integer is always published so an independent reader can recompute
`state_names` from it. A guessed set, a non-string value, a wrong schema or a
missing raw mask fails the journey. Unknown state bits stay in the integer
and are never named. A failed native read publishes an empty value or a zero
mask, never a fabricated control.

The motive comes from the W0 frontier-learning loop
(`prd/PRD_01_17_frontier_radar.md`): a cross-platform computer-use agent
needs one normalised node schema — role, name, state, value, bounds and
relations — with per-platform adapters rather than four native accessibility
vocabularies. On Windows, UI Automation supersedes MSAA and WAA measured a
~57% relative gain from the UIA tree; the normalised schema is the
deterministic layer those adapters would translate into. UIA-first routing
itself is deliberately not in this version: a hand-written `IUIAutomation`
COM vtable with an unverified slot is a process crash, not a named refusal.

This is additive evidence on the existing `observe` capability: `cu-abi 1.0`,
the wire action enum, the capability matrix (windows 12/12, linux 10/12,
macos 8/12, other 0/12), the native signature, the four-backend type-check
and every earlier reader assertion are unchanged. No new crate, no new OS
grant, no new library, no elevation, no window-manager behaviour and no
process, CPU, memory or GPU authority: the G boundary in `PRD_01_08` is
unchanged. The whole resource domain is recorded as research in the W0
radar; whether `screen` may ever publish read-only resource telemetry or
control resources stays an explicit, undecided owner decision.

The model-visible image arm, batched actions, AT-SPI / AX trees, incremental
a11y subscriptions, UIA mixed routing, app-level effect evidence and safety
policy remain non-goals or candidates with their unlock conditions. The
bounded observation wait shipped in 0.0.13 is unchanged.

## Verification

CI makes the schema a required assertion rather than a claim:
`.github/workflows/cu-journeys.yml` keeps `observe_a11y` required on all
three OSes, and the independent reader recomputes each row's `state_names`
from the published raw `state` bit mask, checks `value` is a string and
checks the frame's `a11y_node_schema` — on top of every 0.0.6-0.0.13
assertion (receipts, `path_points`, both scroll axes, held keys, `grants[]`,
`environment`, `display_scales[]`, the a11y tree, `target_hwnd`, the wait
evidence and ceiling refusals, generations, live final frame,
declared-but-skipped rule, BOUNDARY). A `BOUNDARY` note excuses nothing else.

`.github/workflows/cu-platforms.yml` builds, tests and release-links all
three OSes; `.github/workflows/cu-journeys.yml` drives a real desktop on each
(Linux under Xvfb, macOS in the runner GUI session, Windows with a titled
window). Every effect is read back, every performed button event carries a
receipt and an independent reader recomputes the verdicts from
`cu-journey.json`. Missing OS grants (Screen Recording, Input Monitoring,
`$DISPLAY`) remain named boundaries.

If something refuses to start, run the file from a terminal and read the
error line; every refusal names the bound it hit.
