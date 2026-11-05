# 0.0.5 — cross-platform computer use: one contract, three backends

Status: `[v]` W1 design + contract + compile gate delivered (2026-11-05).
`[_]` W2/W3 (native mechanisms per OS, real-desktop journeys) are specified
here, not claimed. Owner direction this plan implements: **the CU design must
be cross-platform and computer use must cover linux, macOS and Windows** --
not a Windows implementation plus a refusal for the other five cells.

## Why the current shape cannot be called cross-platform

Audit of `main` at `61dffe1` (all facts, not impressions):

| # | Fact | Consequence |
|---|------|-------------|
| 1 | `src/screen.rs` has `#[cfg(windows)] mod native` and one `#[cfg(not(windows))] mod native` whose 18 functions all `Err(unqualified())` | macOS/Linux cells are not backends, they are one uniform refusal |
| 2 | `cu.rs:577-588` refuses `observe`/`act` with the literal text `"v1 is Windows only"` | the contract itself is platform-specific |
| 3 | `disclose` reports a single boolean `qualified` | a model cannot discover *which* verbs exist on this cell |
| 4 | `prd/PRD_01_03` and `PRD_01_05` both carry `[-] Windows only` rows | the PRDs freeze the gap rather than describe a design |
| 5 | the store-mode PNG encoder lives **inside** the Windows `native` module | a second backend cannot reuse the pixel arm |
| 6 | six cells are cross-compiled on **one macOS runner**: Linux via `cargo zigbuild` (zig sysroot), Windows via `cargo xwin` (clang-cl), Darwin via `cargo build` | a `#[link(name="X11")]` or framework link is a *link-time* dependency that two of the three toolchains cannot satisfy |
| 7 | `platform_qualified()` is a global bool consumed by `CuHost` before any capability check | no per-verb admission, so a partially implemented OS cannot be honest |

Point 6 is the decision that shapes everything. Adding `x11rb`, `objc2`,
`windows-sys` or `minicon-platform` -- the obvious route -- breaks the
zero-new-dependency invariant for **all six cells at once**, and a Linux
cell linking `libX11.so` cannot be produced by the zig cross toolchain.

## Architectural decision (ours)

**One host contract, three native backends, runtime dynamic loading.**

```text
CuHost (abi 1.0, unchanged calling shape)
  | admission: abi -> grant -> platform capability -> as_of -> typed bounds
  v
screen.rs platform-neutral layer
  |  types, bounds, PNG encoder, downsample, readback rules, labels
  v
Backend (one module per target_os, identical function set)
  windows : user32/gdi32      (static import libs, already working)
  macos   : CoreGraphics / CoreFoundation / ApplicationServices
            via dlopen + dlsym
  linux   : X11 / XTest      via dlopen + dlsym
  other   : named refusal    (never an empty success)
```

Three rules hold for every backend:

1. **No link-time platform library.** Darwin and Linux backends resolve every
   foreign symbol with `dlopen`/`dlsym` at first use. `cargo check --target`
   and the zig/xwin linkers therefore see only libc. This preserves "six cells
   built on one runner" without smuggling a host `.so` into a cross build.
2. **A capability is a declared fact, not a guess.** Each backend publishes a
   `PlatformCapabilities` matrix. `CuHost` admits a verb only if that matrix
   says the verb is implemented; anything else returns a named `unsupported`
   refusal naming the missing capability and the platform. No backend may
   report success for a verb it does not implement.
3. **Readback is platform-neutral.** Pointer readback within
   `SCREEN_TOOL_POINTER_TOLERANCE_PX`, window-rect readback within
   `SCREEN_TOOL_MOVE_TOLERANCE_PX`, and foreground-title evidence are required
   from every backend that claims those verbs. An API call returning is never
   evidence.

### Why not the alternatives

| Route | Why rejected |
|-------|--------------|
| `x11rb` / `objc2` / `windows-sys` crates | new dependency in all six cells; breaks the audited std-only surface |
| `minicon-platform` feature | multi-OS platform crate, changes three invariants at once; PRD 01.03 already marks it `[no]` |
| `#[link(name="X11")]` / `#[link(kind="framework")]` | zig and xwin cross-linkers have no such sysroot; breaks point 6 |
| one refusal stub for all non-Windows | that is the current shape, not a design |
| Wayland protocol client | no stable wire protocol without a crate; see non-goals |

## Backend contract

Every backend exports exactly this set. Signatures are frozen; a new verb is a
contract change and must appear in `CAPABILITIES` and the wire enum in the same
commit.

```text
const PLATFORM: &str                     // "windows" | "macos" | "linux" | <os>
const CAPABILITIES: PlatformCapabilities
fn windows()                 -> Result<Vec<WindowInfo>, String>
fn virtual_screen()          -> Result<(i32,i32,i32,i32), String>
fn displays()                -> Result<Vec<DisplayInfo>, String>
fn foreground_title()        -> Option<String>
fn controls(handle: u64)     -> Result<(Vec<ControlInfo>, bool), String>
fn handle_is_live(u64, &str) -> Result<(), String>
fn find(&str)                -> Result<WindowInfo, String>
fn focus(u64)                -> Result<(), String>
fn move_window(u64,i32,i32,i32,i32) -> Result<(), String>
fn place_pointer(i32,i32)    -> Result<(i32,i32), String>
fn drag(i32,i32,i32,i32,ClickButton) -> Result<(i32,i32), String>
fn click(i32,i32,ClickButton,bool)   -> Result<(i32,i32), String>
fn scroll(i32,i32,i32)       -> Result<(i32,i32), String>
fn type_text(&str)           -> Result<(), String>
fn press_key(&str)           -> Result<(), String>
fn capture_desktop(u32)      -> Result<Captured, String>
fn capture_window(u64,u32)   -> Result<Captured, String>
fn capture_region(i32,i32,i32,i32,u32) -> Result<Captured, String>
fn now_millis()              -> u128
```

`qualification` is derived, never hard-coded:

```text
platform_qualified() == CAPABILITIES.any_effect_implemented()
platform_capabilities() -> PlatformCapabilities   // visible in disclose
```

### Capability matrix

`[v]` implemented and read back on a real desktop, `[?]` implemented but only
compile-verified, `[-]` named refusal. W1 delivers the matrix itself and the
three refusing backends; W2/W3 turn macOS/Linux rows into `[?]` and then `[v]`.
**W1 makes no claim that macOS or Linux can yet drive a desktop.**

| Verb | windows | macos | linux | other |
|------|---------|-------|-------|-------|
| observe windows / displays / virtual screen | `[v]` | `[-]` | `[-]` | `[-]` |
| foreground title | `[v]` | `[-]` | `[-]` | `[-]` |
| pointer place + readback | `[v]` | `[-]` | `[-]` | `[-]` |
| click / drag / scroll / middle | `[v]` | `[-]` | `[-]` | `[-]` |
| type / key | `[v]` | `[-]` | `[-]` | `[-]` |
| focus / move window | `[v]` | `[-]` | `[-]` | `[-]` |
| capture desktop / region / window | `[v]` | `[-]` | `[-]` | `[-]` |
| typed child controls | `[v]` Win32 | `[-]` AX gate | `[-]` no a11y tree | `[-]` |

A `[?]` is **not** a claim of correctness. It means a real implementation
exists and compiles on that cell; it becomes `[v]` only after W3's per-OS
journey produces an observation and a readback. `disclose` reports this matrix
as typed evidence on every cell, so a caller never has to infer capability
from the OS name.

## OS mechanism map

| Concern | windows | macos | linux |
|---------|---------|-------|-------|
| window inventory | `EnumWindows` | `CGWindowListCopyWindowInfo` (`kCGWindowListOptionOnScreenOnly`, exclude desktop elements) | `_NET_CLIENT_LIST` / `XQueryTree` via `XGetWindowProperty` |
| display geometry | `GetSystemMetrics(SM_*VIRTUALSCREEN)` + `EnumDisplayMonitors` | `CGGetActiveDisplayList` + `CGDisplayBounds` | `XDisplayWidth/Height` + `XRRSizes` when XRandR is present |
| window title / pid | `GetWindowTextW` / `GetWindowThreadProcessId` | `kCGWindowOwnerName`, `kCGWindowName`, `kCGWindowOwnerPID` | `_NET_WM_NAME`, `WM_NAME`, `_NET_WM_PID` |
| foreground | `GetForegroundWindow` | `kCGWindowLayer==0` + `NSWorkspace.frontmostApplication` (or active-window ordering) | `_NET_ACTIVE_WINDOW` |
| pointer read | `GetCursorPos` | `CGEventGetLocation(CGEventCreate(null))` | `XQueryPointer` |
| pointer write | `SetCursorPos` | `CGWarpMouseCursorPosition` | `XWarpPointer` / `XTestFakeMotionEvent` |
| input | `SendInput` | `CGEventPost` (mouse/keyboard events) | `XTestFakeButtonEvent` / `FakeKeyEvent` (`libXtst`) |
| pixels | `BitBlt` + `GetDIBits` (BGRA) | `CGDisplayCreateImage` -> `CGDataProviderCopyData` (RGBA/BGRA) | `XGetImage` -> `XImage.data` (BGRA) |
| controls | `EnumChildWindows` + `WM_GETTEXT` | needs Accessibility; AX tree is a separate design | no stable a11y tree without AT-SPI |
| shared encoding | store-mode PNG (`png::encode_rgb`) | same | same |

Native libraries are opened by soname, not by build-time linkage:

```text
macos: /System/Library/Frameworks/CoreGraphics.framework/CoreGraphics
       /System/Library/Frameworks/CoreFoundation.framework/CoreFoundation
       /System/Library/Frameworks/ApplicationServices.framework/ApplicationServices
linux: libX11.so.6, libXtst.so.6
```

Failure to open a library is a **named refusal** (`unsupported: <library>
unavailable`), never a panic and never an empty observation.

## Permissions and honest boundaries

- **macOS**: Screen Recording is required for `CGDisplayCreateImage` and window
  names; Accessibility/Input Monitoring for posting events. A missing grant
  yields a named refusal that says which permission and which verb, not a black
  PNG or a silent no-op. The harness never prompts for permission.
- **Linux**: X11 needs a reachable `$DISPLAY` (Xorg or XWayland). A pure
  Wayland session with no XWayland is refused by name with
  `XDG_SESSION_TYPE=wayland` and the missing display; this design does **not**
  implement a Wayland protocol client. X11 has no OS permission gate, so the
  grant remains `--allow-screen`.
- **Windows**: unchanged; no UAC/UIAccess elevation, `SendInput` subject to UIPI.
- **Controls**: `act:control` stays a Windows capability. macOS AX and Linux
  AT-SPI are deliberately out of scope; `disclose` says so. Reporting zero
  controls on a self-drawn UI is a named result, never a successful empty read.

## Compile and verification gates

1. `cargo test --offline` on the host (Windows path unchanged, 71+ tests).
2. Three whole-file `rustc` type-checks, one per backend configuration, by
   cfg-rewriting `screen.rs`/`cu.rs` into a temporary file. Catches signature
   drift without needing a foreign SDK locally.
3. **New CI gate** `.github/workflows/cu-platforms.yml`: a three-OS matrix
   (`ubuntu-24.04`, `macos-15`, `windows-2025`) runs the full test suite and a
   **release link** of the host cell, so each backend is compiled *and linked*
   on its real OS. `scripts/cu-cfg-check.py` then cfg-rewrites `screen.rs` and
   type-checks all three backend configurations with host `rustc`. A cross
   `cargo check --target` was rejected: `ring`'s build script needs a cross C
   compiler that the zig/xwin toolchains do not provide for every target.
   The host links plus the cfg type-checks cover the same shapes without that
   dependency.
4. W3 per-OS real-desktop journey: Linux under `Xvfb` on `ubuntu-24.04`, macOS
   on the GUI session of `macos-15`, Windows on `windows-2025`. Each publishes
   a JSON observation plus an independent reader, exactly like the 0.0.4
   `cu-controls.json` evidence. Only a green journey turns `[?]` into `[v]`.

## Outcome DAG

```text
Outcome: one CU contract with three real backends and honest per-verb admission
├── [v] W1 — cross-platform design + contract
│   invariant: one backend function set, a capability matrix per OS, no
│   link-time platform library, PNG encoder shared, no "Windows only" text
│   in host/contract/PRD, three-OS build + three-config type-check green
│   evidence: this plan, PRD_01_06, `cu-platforms.yml` run, host test suite
├── [_] W2 — native mechanisms for macOS and Linux
│   invariant: dlopen/dlsym only, named refusal on missing library or
│   permission, every claimed verb has a readback, no new dependency
│   dependency: W1
├── [_] W3 — per-OS real-desktop journeys
│   invariant: one observation + one readback per OS, published JSON read by
│   an independent script, `[?]` -> `[v]` only here
│   dependency: W2, CI GUI sessions
└── [_] W4 — version bump and signed release of the cross-platform cells
    invariant: same W6 chain as 0.0.4, no unsigned Windows cells
    dependency: W3
```

## Non-goals

- `[no]` Wayland-native protocol, AT-SPI, macOS AX tree, UIA COM.
- `[no]` any new crate, runtime, plugin host, bun/WASM/ujs embedding
  (PRD 31's readiness gate stays BLOCKED).
- `[no]` runtime backend selection or a second file per cell. One cell, one
  backend, chosen at compile time.
- `[no]` claiming `[v]` for a verb that only compiled. A `cargo check` green is
  evidence of shape, not of a working desktop.
