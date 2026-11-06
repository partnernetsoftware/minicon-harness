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

Version VERSION widens one declared keyboard verb without declaring a new
capability. `action=key` accepts `hold: true` with exactly one whitelisted
modifier (`ctrl`, `shift`, `alt`, `win`/`cmd`) and `release: true` to let the
whole set go. `CuHost` owns the held set (at most four), `op=disclose`
publishes `evidence["held_keys"]`, and each flagged call publishes
`evidence["keyboard"]` with the held and released names **after** the native
effect -- never an echo of the request. A changed foreground, a lost grant or
host teardown releases everything before any further effect and reports the
release; an unconfirmed keyboard effect is `unknown`, never `performed`.
A keyboard step never carries a button actuation receipt.

Four native backends change together: Windows posts symmetric `SendInput`
key-down/up pairs, macOS applies `CGEventSetFlags` on **every** event posted
while a modifier is held (mouse, wheel and keyboard alike), Linux resolves
X11 keysyms at runtime and reports an unmapped modifier as a named
`unsupported`, and the unlisted-OS fallback refuses by name.
`scripts/cu-cfg-check.py` type-checks all four configurations, so a signature
drift cannot hide behind a `cfg`.

`cu-abi 1.0`, the wire action enum and the capability matrix (windows 12/12,
linux 10/12, macos 8/12) are unchanged: `hold` / `release` are additive
parameters, not new capabilities. CI drives `act_key_hold`, `disclose_held`,
`act_key_release`, `disclose_released` and a final empty-set `disclose` as
required steps on all three OSes. The independent reader recomputes the held
set, the release, the final empty set and the absence of a button receipt on
every keyboard step, on top of every 0.0.6/0.0.7 assertion (receipts,
`path_points`, both scroll axes, generations, live final frame,
declared-but-skipped rule, BOUNDARY). A named OS-grant boundary never excuses
a stuck modifier.

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
