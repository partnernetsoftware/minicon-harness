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

## VERSION: bounded observation wait

`op=observe` gains an optional `wait_ms`: an integer 1..=2000. The host
sleeps once for that interval before taking a fresh snapshot — the snapshot
is never the frame that was already on screen. The result publishes additive
`evidence["wait"]` with `requested_ms`, `elapsed_ms` measured from a
monotonic clock, `cumulative_ms`, `wait_calls`, the new `generation`, and a
named reason only when a ceiling is hit. An ordinary `observe` without
`wait_ms` is unchanged.

Three ceilings are enforced per `CuHost` before any sleep: 2000 ms per call,
8000 ms cumulative and 8 wait calls. Zero, negative, non-integer, over-bound
and over-budget requests are named refusals. `disclose` publishes the
remaining budget as additive evidence, so a caller can see what is left
without guessing. This is one bounded sleep per call — not a polling loop,
not a condition wait, not a model turn and not a new capability row.

The reason is measured. In the L1 self-hosted orchestration trial an
`observe` issued immediately after an `act` captured the pre-reflow Chromium
frame, and `type` plus its send click had to be split into separate runs
before the new state was visible. A coordinate basis (0.0.10), a structured
Windows a11y tree (0.0.11) and an unambiguous window handle (0.0.12) do not
fix stale timing. The 2026 frontier evidence points the same way: short-task
grounding is largely solved while hour-long workflows still fail on losing
constraints and skipping verification, so an explicit, bounded, evidenced
wait is the smallest honest unit of a self-sustained loop.

This is additive evidence on the existing `observe` and `disclose`
capabilities: `cu-abi 1.0`, the wire action enum, the capability matrix
(windows 12/12, linux 10/12, macos 8/12, other 0/12), the four-backend
type-check and every earlier reader assertion are unchanged. No new crate,
no native surface change, no OS grant, no window-manager behaviour and no
process, CPU, memory or GPU authority: the G boundary in `PRD_01_08` is
unchanged.

Alongside this release the project adds a recurring W0 frontier-learning
loop (`prd/PRD_01_17_frontier_radar.md`): learn current computer-use and
resource-domain technology, upsert the PRD in place, then implement only the
candidate whose gate is open. A published technique is a research note, not
authority; no `[no]` is lifted by a research pass.

The model-visible image arm, batched actions, AT-SPI / AX a11y trees,
app-level effect evidence, safety policy and UIA mixed routing remain
non-goals or candidates with their unlock conditions. The bounded Windows
MSAA read-only subset shipped in 0.0.11 is unchanged.

## Verification

CI makes the wait a required step rather than a claim:
`.github/workflows/cu-journeys.yml` drives `observe_wait_after_act` and an
over-bound refusal on all three OSes. The independent reader recomputes
`elapsed_ms >= requested_ms`, `elapsed_ms <= requested_ms + 500`, the
cumulative sums, the call count, the generation increase and every ceiling
refusal, and rejects a missing or mismatched `wait` field — on top of every
0.0.6-0.0.12 assertion (receipts, `path_points`, both scroll axes, held
keys, `grants[]`, `environment`, `display_scales[]`, the a11y tree,
`target_hwnd`, generations, live final frame, declared-but-skipped rule,
BOUNDARY). A `BOUNDARY` note excuses nothing else.

`.github/workflows/cu-platforms.yml` builds, tests and release-links all
three OSes; `.github/workflows/cu-journeys.yml` drives a real desktop on each
(Linux under Xvfb, macOS in the runner GUI session, Windows with a titled
window). Every effect is read back, every performed button event carries a
receipt and an independent reader recomputes the verdicts from
`cu-journey.json`. Missing OS grants (Screen Recording, Input Monitoring,
`$DISPLAY`) remain named boundaries.

If something refuses to start, run the file from a terminal and read the
error line; every refusal names the bound it hit.
