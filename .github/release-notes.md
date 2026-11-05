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

Version VERSION closes the interaction loop. 0.0.5 made observation and
pointer movement real on three OSes, but a successful click still proved only
that the pointer ended up in the right place. Every button event now carries a
typed actuation receipt (`button`, `downs`, `ups`, `double`, `target`) counted
by the native backend at dispatch time and published in
`evidence["actuation"]`. `CuHost` compares it with the request: a missing
receipt is `unknown`, a mismatched one is `not_performed`, and neither is ever
upgraded to a success. The receipt proves a real button event was dispatched;
it does not claim the target application's business action ran.

CI now drives the variants that were in the wire schema but never exercised:
`middle`, `right`, `double`, middle-button drag, and a real `press=true` click
on a named Win32 control (Windows only). `cu-abi 1.0`, the wire inputs and the
capability matrix (windows 12/12, linux 10/12, macos 8/12) are unchanged. The
final observation must have a strictly newer generation and still report a
live window and display, so a cached frame cannot pass as a session that
survived the side-effecting clicks.

The three native backends from 0.0.5 are unchanged: Win32 `user32`/`gdi32`,
macOS `CoreGraphics`/`CoreFoundation` and Linux `X11`/`XTest`, resolved at
runtime with `dlopen`/`dlsym` so the six cells still cross-compile on one
runner without a link-time platform library. Missing OS grants (Screen
Recording, Input Monitoring, `$DISPLAY`) remain named boundaries.

Verification is CI, not a laptop: `.github/workflows/cu-platforms.yml` builds,
tests and release-links all three OSes, and `.github/workflows/cu-journeys.yml`
drives a real desktop on each (Linux under Xvfb, macOS in the runner GUI
session, Windows with a titled window). Every effect is read back, every
performed button event carries a receipt, and an independent reader recomputes
the verdicts, pointer positions, receipts, PNG bytes and generation order from
`cu-journey.json`. `act:control` and the typed Win32 child-control arm stay
Windows-only; macOS AX and Linux AT-SPI trees are explicit non-goals with
per-cell unlock conditions.

If something refuses to start, run the file from a terminal and read the
error line; every refusal names the bound it hit.
