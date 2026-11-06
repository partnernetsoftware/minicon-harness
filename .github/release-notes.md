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
loop at any OpenAI-compatible chat-completions endpoint (default:
DeepSeek's). One invocation runs one bounded task: 16 model turns, 64 tool
calls, 10 minutes.

Version VERSION ships company-signed bytes: `minicon-harness.com` and the two
Windows cells carry an Authenticode signature (O=PARTNERNET SOFTWARE PTY LTD)
with an RFC 3161 timestamp. `signing-receipt.json` records the before/after
SHA-256 of each signed file, while `build-receipt.json` and the `.sha256`
sidecars bind every other asset to the packed source.

Version VERSION answers "why did that click not land" **before** a click is
attempted, without ever asking for a permission. `op=disclose` gains two
additive pieces of evidence on the existing capability:

- `environment` — `platform`, `session` (a coarse kind: `gui` / `remote` /
  `headless` / `unknown`), `display_count`, `virtual_desktop` (x, y, width,
  height) and `foreground_present`. No window titles, user names, paths or
  environment values are published; the session is reported as a kind, never
  as the value of `$DISPLAY` or `XDG_SESSION_TYPE`.
- `grants` — one typed row per named probe, each `name` / `status` /
  optional `reason`. `status` comes from a four-value whitelist: `granted`,
  `denied`, `unknown`, `not_applicable`. `unknown` ("could not be probed
  safely") and `not_applicable` always carry a non-empty reason; `unknown` is
  never reported as `granted`, and `denied` is never auto-escalated. Rows are
  sorted by `name` and unique, and the probe set is bounded (8 rows, 160
  characters per reason) so a disclosure cannot become a dump.

Every probe is **preflight only**. macOS resolves
`CGPreflightScreenCaptureAccess`, `CGPreflightPostEventAccess` and
`AXIsProcessTrusted` -- the `...WithOptions` prompt variants are deliberately
never called; Linux resolves `XOpenDisplay` and `libXtst` at runtime and
reports whether an input-posting symbol is reachable; Windows reports the
foreground window and input-desktop state; the unlisted-OS fallback reports
every probe `not_applicable` with a named reason. No permission is requested,
no TCC dialog is raised, no privilege is elevated, and no input event is
posted: the whole path is a read. A malformed probe set publishes no partial
disclosure at all -- the call returns `not_performed` instead.

Four native backends implement the same two functions
(`native::environment()` and `native::grants()`), and
`scripts/cu-cfg-check.py` type-checks all four configurations, so a signature
drift cannot hide behind a `cfg`. `cu-abi 1.0`, the wire action enum and the
capability matrix (windows 12/12, linux 10/12, macos 8/12, other 0/12) are
unchanged: this is additive evidence on `disclose`, not a new capability row,
and `disclose` still works even when computer use was never granted.

CI makes the disclosure a required step rather than a claim:
`.github/workflows/cu-journeys.yml` drives `disclose_grants` on all three
OSes, and the independent reader recomputes `environment` (platform matching
the journey's own, a non-empty session, a positive `display_count`, a boolean
`foreground_present`, positive virtual-desktop width and height), the
`grants[]` whitelist, the reason rule, the sort order and uniqueness, and
checks that no row contradicts an effect the same journey performed -- all on
top of every 0.0.6/0.0.7/0.0.8 assertion (receipts, `path_points`, both scroll
axes, held keys, generations, live final frame, declared-but-skipped rule,
BOUNDARY). A `BOUNDARY` note excuses nothing: a missing or inconsistent
disclosure fails the journey.

The three native backend families from 0.0.5 still use Win32 `user32`/`gdi32`,
macOS `CoreGraphics`/`CoreFoundation` and Linux `X11`/`XTest`, resolved at
runtime with `dlopen`/`dlsym` so the six cells still cross-compile on one
runner without a link-time platform library. Missing OS grants (Screen
Recording, Input Monitoring, `$DISPLAY`) remain named boundaries.

Verification is CI, not a laptop: `.github/workflows/cu-platforms.yml` builds,
tests and release-links all three OSes, and `.github/workflows/cu-journeys.yml`
drives a real desktop on each (Linux under Xvfb, macOS in the runner GUI
session, Windows with a titled window). Every effect is read back, every
performed button event carries a receipt, every performed wheel event
publishes its two axes, and an independent reader recomputes
the verdicts, pointer positions, receipts, PNG bytes and generation order from
`cu-journey.json`. `act:control` and the typed Win32 child-control arm stay
Windows-only; macOS AX and Linux AT-SPI trees are explicit non-goals with
per-cell unlock conditions.

If something refuses to start, run the file from a terminal and read the
error line; every refusal names the bound it hit.
