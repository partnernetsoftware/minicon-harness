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

Version VERSION widens two declared desktop verbs without declaring a new
capability. `action=drag` accepts a bounded `path[]` of 2..16 points: each
segment is interpolated with 12 stepped moves, the whole path is admitted
against the virtual desktop before any native effect, and the pointer readback
must land at the **last** point. The actuation receipt gains `path_points`,
counted by the backend at the real dispatch sites. Omitting `path` keeps the
0.0.6 single-leg drag bit-for-bit, and every button event still carries the
receipt (`button`, `downs`, `ups`, `double`, `target`) introduced in 0.0.6.

`action=scroll` accepts `delta_x` and `delta_y` within +-1440 and publishes
them as structured `evidence["scroll"]`; `delta` stays accepted as the
vertical alias so 0.0.6 callers keep working. Both axes zero is a named
refusal, never a silent no-op. Win32 uses `MOUSEEVENTF_HWHEEL`, macOS uses the
non-variadic `CGEventCreateScrollWheelEvent2` with two wheels, and X11/XTest
uses buttons 6/7 for the horizontal axis; native button numbers and sign
conventions never leak into evidence. A wheel event posts no button receipt
and the tool claims no application scroll effect.

`cu-abi 1.0`, the wire inputs and the capability matrix (windows 12/12,
linux 10/12, macos 8/12) are unchanged. CI drives `act_drag_path`,
`act_scroll_x` and `act_scroll_y` as required steps on all three OSes, and the
independent reader recomputes the path's last point, `path_points` and both
wheel axes on top of every 0.0.6 assertion (receipts, generations, live final
frame, declared-but-skipped rule, BOUNDARY). `scripts/cu-cfg-check.py`
type-checks four backend configurations -- Windows, macOS, Linux and the
unlisted-OS fallback -- so a signature change cannot leave a backend behind.

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
