# 0.0.5 — cross-platform computer use: three real backends, CI-proven, publicly signed

Status: `[v]` W1 (design + contract + compile gate, run **41939696616**).
`[_]` W2/W3/W4 in progress. Owner direction this plan implements:

1. **the CU design must be cross-platform and computer use must cover linux,
   macOS and Windows** -- not a Windows implementation plus a refusal for the
   other five cells;
2. **use GitHub CI to do the testing** -- a desktop effect is only `[v]` when a
   CI runner produced an observation and a readback, because a local Windows
   laptop cannot honestly claim a macOS or Linux desktop;
3. **write the 0.0.5 plan, then develop it through to a public signed Release**
   -- source version and public Release are separate facts, as in 0.0.4.

## Decision: what "satisfies lnx, osx, win" means here

| Layer | 0.0.4 | 0.0.5 target |
|-------|-------|--------------|
| contract | one host ABI, one Windows backend | one host ABI, **three** backends with an identical function set |
| linux | uniform refusal | X11/XTest backend via `dlopen`, real effects under Xvfb |
| macOS | uniform refusal | CoreGraphics/CoreFoundation backend via `dlopen`, real effects where the OS grant allows |
| Windows | working | unchanged, plus a CI journey instead of laptop-only evidence |
| evidence | local `cargo test` | **CI journeys**: JSON observation + independent reader on all three OSes |
| delivery | v0.0.4 public signed | v0.0.5 public signed, six cells, signed Windows cells in both zips |

Three rules, unchanged from W1 and now enforced by code:

1. **No link-time platform library.** Darwin/Linux resolve every foreign symbol
   with `dlopen`/`dlsym` at first use. The six-cell pack still cross-compiles on
   one macOS runner (zig for Linux, xwin for Windows); `#[link(name="X11")]`
   or a framework link would break it.
2. **A capability is a declared fact.** Each backend publishes a compile-time
   `PlatformCapabilities` matrix. `CuHost` admits only advertised verbs; a
   missing library or OS permission produces a **named refusal**, never an
   empty success, never a black PNG. Runtime permission failure is not the same
   as a missing implementation: the matrix says what the code implements, the
   verdict says what this session allowed.
3. **Readback is platform-neutral.** Pointer position, window rect and
   foreground evidence are read back from the desktop after every effect. An
   API call returning is never evidence.

## Capability matrix (declared by each compiled cell)

`[v]` CI journey produced an observation + readback, `[-]` named refusal.

| Verb | windows | macos | linux | other |
|------|---------|-------|-------|-------|
| observe windows / displays / virtual screen | `[v]` | `[v]` | `[v]` | `[-]` |
| foreground title | `[v]` | `[v]` | `[v]` | `[-]` |
| pointer place + readback | `[v]` | `[v]` | `[v]` | `[-]` |
| click / drag / scroll / middle | `[v]` | `[v]` | `[v]` | `[-]` |
| type / key | `[v]` | `[v]` | `[v]` | `[-]` |
| focus / move window | `[v]` | `[-]` AX/AppKit gate | `[v]` | `[-]` |
| capture desktop / region / window | `[v]` | `[v]` | `[v]` | `[-]` |
| typed child controls / act:control | `[v]` Win32 | `[-]` | `[-]` | `[-]` |

macOS `act:focus`/`act:move` are deliberately **not** implemented: raising and
moving another process's window needs the Accessibility API and an AX grant,
not CoreGraphics. They stay named refusals. `act:control` and the controls
observe arm stay Windows-only (AX tree and AT-SPI are non-goals).

## OS mechanism map (W2)

| Concern | windows | macos | linux |
|---------|---------|-------|-------|
| load | import libs | `dlopen` CoreGraphics / CoreFoundation | `dlopen` libX11.so.6 / libXtst.so.6 |
| window inventory | `EnumWindows` | `CGWindowListCopyWindowInfo` | `_NET_CLIENT_LIST` then `XQueryTree` |
| title / pid / class | Win32 | `kCGWindowName` / `OwnerPID` / `OwnerName` | `_NET_WM_NAME`, `WM_NAME`, `_NET_WM_PID`, `WM_CLASS` |
| geometry | `GetWindowRect` | `kCGWindowBounds` via `CGRectMakeWithDictionaryRepresentation` | `XGetGeometry` + `XTranslateCoordinates` |
| displays | `EnumDisplayMonitors` | `CGGetActiveDisplayList` + `CGDisplayBounds` | `XDisplayWidth/Height` (one X screen) |
| pointer read | `GetCursorPos` | `CGEventGetLocation(CGEventCreate(null))` | `XQueryPointer` |
| pointer write | `SetCursorPos` | `CGWarpMouseCursorPosition` | `XWarpPointer` |
| input | `SendInput` | `CGEventPost` (mouse/key/scroll) | `XTestFake*` |
| pixels | `GetDIBits` (BGRA) | `CGDisplayCreateImage` -> `CFDataGetBytePtr` | `XGetImage` -> `XGetPixel` |
| shared encoding | store-mode PNG `png::encode_rgb` | same | same |

X11 single-screen reporting is a **fact of the backend**, not a lie about
multi-monitor: when RandR/Xinerama geometry is unavailable the cell reports
exactly one display with the real `XDisplayWidth/Height`. Wayland-native
protocol, AT-SPI, macOS AX tree and UIA COM remain `[no]`.

## W3: CI is the test bench

New workflow `.github/workflows/cu-journeys.yml`, one job per OS:

| Runner | Session | Journey |
|--------|---------|---------|
| `ubuntu-24.04` | `Xvfb :99 -screen 0 1280x800x24` + `xmessage` as a titled client | full: observe -> capture -> hover -> click -> drag -> scroll -> type -> key -> focus -> move |
| `macos-15` | runner GUI session | same verbs minus focus/move; a missing Screen Recording / Accessibility grant is recorded as a named refusal, not a fake pass |
| `windows-2025` | runner GUI session | full journey, including `act:control` where a Win32 child window exists |

Each job drives the **real binary** through the `screen` tool (grant
`--allow-screen`), writes `cu-journey.json` with one record per verb
(`verdict`, `readback`, `platform`), and an **independent reader**
`scripts/cu-journey-verify.py` asserts:

- every verb the cell declares implemented produced `performed` with a
  non-empty readback;
- observation reported `windows>=1`, `displays>=1`, `desktop` with positive
  dimensions;
- a capture file exists, is a PNG, and its width/height agree with the report;
- pointer readback matches the requested coordinates within
  `SCREEN_TOOL_POINTER_TOLERANCE_PX`;
- no verb reported `performed` with an empty readback, and no unsupported verb
  was attempted.

A named refusal caused by a missing macOS OS grant is **not** silently
upgraded to `[v]`; the journey records it and the plan/PRD say so. If macOS
cannot post events on the runner, that row stays a named boundary in the
delivery record while the Linux and Windows rows are `[v]`.

CI is also the compile gate: `cu-platforms.yml` keeps the three-OS full test
suite, release link and three-config cfg type-check. `scripts/cu-cfg-check.py`
is extended to copy the per-OS backend files into its temporary crate, so a
signature drift between `screen_linux.rs`, `screen_macos.rs` and the Windows
`native` module fails the gate.

## W4: version bump and public signed Release

1. Four version sources in one commit: `Cargo.toml`, `Cargo.lock`,
   `release-policy.json`, `loader/ape-version.rc` -> `0.0.5`.
2. `.github/release-notes.md` describes the three backends and the CI journeys;
   no `unsigned` text, no `VERSION` placeholder after substitution.
3. Re-run the **same** W6 chain proven for 0.0.4, at one `head_sha`:
   `minicon-harness-com.yml` pack -> `candidate.yml` seal ->
   `company-signing.yml` sign -> `release.yml` publish/Defender/post-publish.
4. Invariants that 0.0.4 established and 0.0.5 must not regress:
   - `environment: release` on publish (the 0.0.3 403 root cause);
   - signed Windows cells rewritten into both Windows zips, sidecars rewritten;
   - post-publish court re-hashes **downloaded** assets against
     `signing-receipt.json` `after_sha256`, never `before_sha256`;
   - Defender uses `-File` (not the invalid `-Path`) and resolves `TAG` from
     `verify` through `needs`.
5. Owner-view acceptance, not a command exit code: an unauthenticated `curl`
   to the Release page and one asset returns 200; a fresh download of all
   assets is re-hashed locally; `minicon-harness.com --version` prints
   `0.0.5`; `gh release list` shows `v0.0.5` as Latest.

## Outcome DAG

```text
Outcome: one CU contract, three real backends, CI-proven journeys, public signed v0.0.5
├── [v] W1 — cross-platform design + contract + compile gate
│   evidence: run 41939696616, PRD_01_06, cu-platforms.yml
├── [_] W2 — native mechanisms for macOS and Linux
│   invariant: dlopen/dlsym only, named refusal on missing library or grant,
│   every claimed verb has a readback, zero new dependencies, six-cell pack green
│   evidence: screen_linux.rs, screen_macos.rs, three-config cfg check,
│   three-OS build + link
├── [_] W3 — per-OS real-desktop journeys in CI
│   invariant: JSON observation + independent reader, all three OS jobs green
│   evidence: cu-journeys.yml run, cu-journey.json artifacts
├── [_] W4 — 0.0.5 version bump and signed public Release
│   invariant: pack -> candidate -> signing -> defender -> release, no unsigned
│   Windows cells, post-publish court green, owner-view download verified
│   evidence: run IDs + anonymous HTTP 200 + local re-hash
└── [_] W5 — delivery record: plan/PRD/README/evidence updated to the truth
```

## Non-goals

- `[no]` Wayland-native protocol, AT-SPI, macOS AX tree, UIA COM.
- `[no]` any new crate, runtime, plugin host, bun/WASM/ujs embedding
  (PRD 31 readiness gate stays BLOCKED).
- `[no]` runtime backend selection. One cell, one backend, chosen at compile time.
- `[no]` claiming `[v]` for a verb that only compiled or only returned from an
  API call. CI evidence is a readback, not a log line.
- `[no]` re-publishing/overwriting v0.0.4 assets. 0.0.5 is a new tag.

## Delivery record

Filled in as each W closes, with run IDs and the owner-view acceptance result.
