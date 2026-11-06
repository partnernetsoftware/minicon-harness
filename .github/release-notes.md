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

## VERSION: structured control disclosure

`op=observe` gains an optional `a11y` arm. `a11y: true` scans the foreground
window; `a11y: <hwnd>` scans exactly that live window, which is the only
unambiguous identity when two windows share a title. Each scan publishes
`evidence["a11y_controls"]` — a bounded, read-only tree of `name`, MSAA
`role`, coarse `role_name`, `rect` in virtual-desktop coordinates (the same
basis as `windows[]` and the PNG arm), `depth` and `child_id` — together with
`a11y_source`, `a11y_truncated` and `a11y_role_basis`.

The reason is measured. The existing typed `controls[]` arm is Win32
`EnumChildWindows` + `WM_GETTEXT`; against a Chromium/Electron window it
reports one `Chrome_RenderWidgetHostHWND` with `role=unknown`, so a controller
cannot read the input box or the send button and falls back to screenshots
plus a vision read of pixel coordinates. That is slow, expensive and is not
evidence produced by the CU contract. This version adds the structured read
that was missing.

On Windows the tree comes from hand-written, read-only MSAA/IAccessible COM
bindings (`oleacc`/`ole32`): no new crate, no new OS grant, no elevation, no
dialog, no write to the target process. The Win64 `VARIANT` is 24 bytes; the
implementation and the four-configuration type gate both enforce that. The
scan is bounded at 400 rows and depth 12, and hitting either bound publishes
`a11y_truncated` rather than a silent cut. Roles come from a fixed coarse
table; an unrecognised role is `unknown`, never guessed.

Linux, macOS and the unlisted-OS fallback do **not** implement AT-SPI or AX
trees in this version. They publish an empty `a11y_controls[]` plus a named
`a11y_reason`; they never present a Win32-style inventory as an
accessibility tree. A stale `hwnd` is re-validated against a fresh live
window list on every call and becomes a named reason, never a silent best
effort.

This is additive evidence on the existing `observe` capability: `cu-abi 1.0`,
the wire action enum and the capability matrix (windows 12/12, linux 10/12,
macos 8/12, other 0/12) are unchanged, as are `controls[]` and every earlier
reader assertion. `a11y` is a typed optional parameter, not a new capability
row. The `act:focus` by `hwnd` / `target_hwnd` contract remains deferred to
0.0.12; the `a11y` handle is a scan target, not an act target.

An owner decision on 2026-11-06 lifted the older "a11y trees" non-goal for
this bounded Windows read-only subset only. The model-visible image arm,
explicit `wait`, batched actions, app-level effect evidence and safety policy
remain non-goals with their unlock conditions.

## Verification

CI makes the tree a required step rather than a claim:
`.github/workflows/cu-journeys.yml` drives `observe_a11y` on all three OSes,
addressed by a live `hwnd`, and the independent reader recomputes every row's
role whitelist, depth bound, name, integer rectangle and the 400-row bound,
plus the empty-tree-plus-reason rule for cells without an a11y API — on top
of every 0.0.6-0.0.10 assertion (receipts, `path_points`, both scroll axes,
held keys, `grants[]`, `environment`, `display_scales[]`, generations, live
final frame, declared-but-skipped rule, BOUNDARY). A `BOUNDARY` note excuses
nothing. `scripts/cu-cfg-check.py` type-checks all four backend
configurations so a signature drift cannot hide behind a `cfg`.

`.github/workflows/cu-platforms.yml` builds, tests and release-links all
three OSes; `.github/workflows/cu-journeys.yml` drives a real desktop on each
(Linux under Xvfb, macOS in the runner GUI session, Windows with a titled
window). Every effect is read back, every performed button event carries a
receipt and an independent reader recomputes the verdicts from
`cu-journey.json`. Missing OS grants (Screen Recording, Input Monitoring,
`$DISPLAY`) remain named boundaries.

If something refuses to start, run the file from a terminal and read the
error line; every refusal names the bound it hit.
