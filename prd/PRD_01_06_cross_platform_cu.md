# PRD 01.06 — cross-platform computer use contract

Status: `[v]` W1-W4 (contract, three real backends, three-OS compile gate,
real-desktop journeys, and `v0.0.5` shipped as a public company-signed Latest
Release).
Owner direction: `computer-use 要满足 lnx, osx, win`. This PRD specifies the
contract; `plan/plan-0.0.5-cross-platform-cu.md` carries the design, the OS
mechanism map and the rejected alternatives.

## Problem

0.0.4 shipped `cu-abi 1.0`, semantic controls and a signed six-cell Release,
but `screen.rs` had exactly two backends: a full Windows implementation and a
single `#[cfg(not(windows))]` module whose 18 functions all returned the same
refusal. `cu.rs:577` hard-coded `"v1 is Windows only"` into the admission path.
The contract itself was platform-specific, so five of the six cells could only
say "no", never "here is what I can do".

## Invariants

1. **No new dependency, no link-time platform library.** `Cargo.toml` is
   unchanged. Windows keeps its static `user32`/`gdi32` imports; macOS and
   Linux backends must resolve foreign symbols at runtime (`dlopen`/`dlsym`),
   because the six cells are cross-compiled on one macOS runner with zig for
   Linux and xwin for Windows. A `#[link(name="X11")]` would break that build.
2. **One backend per `target_os`.** `#[cfg(windows)]`,
   `#[cfg(target_os="macos")]`, `#[cfg(target_os="linux")]` and one fallback.
   Every backend exports the identical function set; a new verb changes the
   contract, `cu::CAPABILITIES` and the wire enum in the same commit.
3. **Capability is declared, not inferred.** `PlatformCapabilities` has one
   flag per verb. `CuHost` admits a call only if the compiled cell declares that
   verb. `disclose` publishes the whole matrix as typed evidence.
4. **A missing verb is a named refusal.** `unsupported` names the verb, the
   platform and the missing capability. No empty observation, no black PNG, no
   "API call returned" as evidence.
5. **The pixel arm is shared.** The store-mode PNG encoder, capture bounds,
   `downsample` and all readback rules live above the `cfg` boundary. No
   backend reimplements them.
6. **cu-abi stays 1.0.** The calling shape does not change; the matrix is
   additive evidence in `disclose` and in refusal evidence.

## Contract

```text
screen::PlatformCapabilities   // 12 flags, rows(), supports(name), any_effect_implemented()
screen::platform_capabilities() -> PlatformCapabilities
screen::platform_qualified()   == capabilities().any_effect_implemented()
screen::platform_name()        // "windows" | "macos" | "linux" | <os>
cu::capability_matrix(&caps)   -> Value  // [{capability, implemented}, ...]
```

Admission order in `CuHost::invoke` (unchanged elsewhere):

```text
ABI major -> advertised capability -> grant -> platform capability
          -> as_of generation -> typed parameter bounds -> native effect
```

`observe` has two optional arms with separate flags: a request with `capture`
needs `capture`, a request with `controls` needs `controls`. Each refusal
carries `verdict`, `platform`, `observe_arm` and the full matrix in evidence.

`disclose` is pure information and needs no grant. Its evidence now contains
`platform` and `platform_capabilities`; its text lists one
`platform_verb=<name> implemented=<bool>` line per row.

## Platform boundaries

| OS | Backend | Mechanism (W2) | Permission / boundary |
|----|---------|----------------|------------------------|
| windows | `user32`/`gdi32` | already implemented | UIPI applies; no elevation |
| macos | `screen_macos.rs` | CoreGraphics / CoreFoundation via `dlopen` | Screen Recording for pixels and window names; Input Monitoring for events; missing grant is a named refusal. `act:focus`/`act:move` need AX and are not implemented |
| linux | `screen_linux.rs` | X11 / XTest via `dlopen` | needs `$DISPLAY` (Xorg or XWayland); pure Wayland is refused by name |
| other | one refusing module | none | not in the six-cell matrix |

`act:control` stays Windows-only: macOS AX trees and Linux AT-SPI are a
separate design, and a self-drawn UI reports zero controls as a named result.

## Qualification states

| State | Meaning | How it becomes `[v]` |
|-------|---------|----------------------|
| `[-]` | named refusal, no implementation | W2 adds the real mechanism |
| `[?]` | implementation exists, compiles and links on that OS | W3 per-OS real-desktop journey + independent reader |
| `[v]` | observation and readback verified on a real desktop | -- |

W3 state: Windows `[v]` 12/12 verbs, Linux `[v]` 10/12, macOS `[v]` 8/12,
`other` `[-]` for all. The remaining rows are declared `false` in the
compiled matrix and are named refusals. All of this is reported by `disclose`,
not hidden behind a boolean.

## Verification

- `cargo test --offline` on the host: 72 passed, 3 ignored.
- `scripts/cu-cfg-check.py`: whole-file `rustc` type-check of `screen.rs` +
  `cu.rs` in the macOS, Linux and fallback configurations.
- `.github/workflows/cu-platforms.yml`: three-OS matrix running the full test
  suite, a release link, and the cfg gate. The matrix test prints the compiled
  cell's platform and matrix into the CI log. Run **41939696616** is green on
  windows/macos/linux; see the W1 delivery record in the plan for the printed
  matrices and the one intermediate failure that was fixed.
- A green compile gate is evidence of **shape only**. No macOS or Linux
  desktop capability is claimed until W3.

## Non-goals

- `[no]` Wayland-native protocol client, AT-SPI, macOS AX tree, UIA COM.
- `[no]` new crate, engine, WASM, bun, ujs/unisa embedding (PRD 31 readiness
  stays BLOCKED).
- `[no]` runtime backend switching: one cell, one compile-time backend.
- `[no]` claiming a `[v]` verb from a compile check.
