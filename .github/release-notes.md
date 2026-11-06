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

Version VERSION publishes the **coordinate basis** of the desktop: the ratio
between the logical geometry a screenshot is expressed in and the physical
geometry a pointer event needs. `op=disclose` gains
`evidence["environment"]["display_scales"]`, one row per display with `index`,
`logical_rect`, `physical_rect`, `scale` and an optional `reason`, and every
`observe` display carries the same `scale`. `scale` is one of a bounded typed
set -- `1`, `1.25`, `1.5`, `2`, `3` or `unknown`. A cell that cannot probe
safely publishes `unknown` with a non-empty `reason` naming the OS and the
failed symbol; it never publishes a plausible `1`. For a known scale
`physical_rect` equals `logical_rect * scale` in exact integer arithmetic.
Rows are bounded (16 rows, 160 characters per reason), indexed from zero in
`displays[]` order; no display id, device name, path or user name is published.

Publishing this basis is a **precondition** for a later model-visible image
arm and for cross-DPI / multi-monitor / scaled sessions -- a caller no longer
has to guess a ratio. It is not the fix for a DPI fault: an earlier trial note
claiming a ~1.25x screenshot/desktop mismatch was a controller misreading. CU
self-reports `virtual_desktop`, `desktop` and PNG geometry consistently at
1680x1050, a click taken from CU's own image hit the target on the first try,
and the three-OS CI measures `scale` as `1` / `1` / `unknown` (macOS /
Windows / Linux). The earlier missed clicks came from mixing UIA, PIL and CU
coordinate systems with focus never reaching the input box.

Every probe is **read-only preflight**. Windows resolves `GetDpiForSystem`,
`GetDpiForWindow` and `GetSystemMetricsForDpi`; macOS resolves
`backingScaleFactor` through the existing dlopen/dlsym helper; Linux performs
a read-only `Xft.dpi` / `XRRScreenResources` query and names pure Wayland as
`unknown`; the unlisted-OS fallback reports one named `unknown` row. No probe
changes a display mode, DPI setting or process DPI awareness, requests a
permission, raises a dialog or elevates. Four native backends implement the
same `native::display_scales()` signature, and `scripts/cu-cfg-check.py`
type-checks all four configurations, so a signature drift cannot hide behind
a `cfg`. `cu-abi 1.0`, the wire action enum and the capability matrix
(windows 12/12, linux 10/12, macos 8/12, other 0/12) are unchanged: this is
additive evidence on `disclose`, not a new capability row, and `disclose`
still works even when computer use was never granted. A malformed scale set
publishes no partial disclosure at all -- the call returns `not_performed`.

CI makes the disclosure a required step rather than a claim:
`.github/workflows/cu-journeys.yml` drives `disclose_scales` on all three
OSes, and the independent reader recomputes every row's six-value whitelist,
the exact `physical_rect == logical_rect * scale` relation, the `unknown`
reason rule, zero-based continuous indices and the 16-row / 160-char bounds,
plus the same additive fields on every other performed disclosure and
observation -- all on top of every 0.0.6/0.0.7/0.0.8/0.0.9 assertion
(receipts, `path_points`, both scroll axes, held keys, `grants[]`,
`environment`, generations, live final frame, declared-but-skipped rule,
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
