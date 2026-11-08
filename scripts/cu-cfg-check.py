#!/usr/bin/env python3
"""Whole-file rustc type-check of every computer-use backend configuration.

Six cells are cross-compiled on one macOS runner (zig for Linux, xwin for
Windows). A foreign SDK is not available on every runner, so this gate
cfg-rewrites `src/screen.rs` into a temporary crate and type-checks it with
the host rustc, once per backend. It proves the backend contract cannot drift
between macOS, Linux, an unlisted OS and the Windows implementation, without
pretending a compile check qualifies a desktop journey.

The Windows FFI is hand-written and needs no foreign SDK to type-check, so
`windows` is a fourth cfg-rewrite here. `drag`/`scroll` and the 0.0.8
`set_modifier`/`release_all_modifiers` change signature on all four
configurations in the same commit; this gate is what catches a backend that
was left behind.

Run after `cargo test --locked` (the serde_json rlib must exist).
"""
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SRC = REPO / "src" / "screen.rs"
CU = REPO / "src" / "cu.rs"
# The macOS/Linux backends are separate files included with `#[path]`. Copy
# them next to the rewritten screen.rs so every cfg configuration type-checks
# the real backend source, not a stub.
EXTRA = sorted((REPO / "src").glob("screen_*.rs"))
DEPS = REPO / "target" / "debug" / "deps"

# cfg(any()) disables, cfg(all()) enables. Exactly one `native` module must
# survive each rewrite; the macro is enabled only for non-Windows configs.
CONFIGS = {
    "windows": {
        '#[cfg(windows)]': '#[cfg(all())]',
        '#[cfg(not(windows))]': '#[cfg(any())]',
        '#[cfg(target_os = "macos")]': '#[cfg(any())]',
        '#[cfg(target_os = "linux")]': '#[cfg(any())]',
        '#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]': '#[cfg(any())]',
    },
    "macos": {
        '#[cfg(windows)]': '#[cfg(any())]',
        '#[cfg(not(windows))]': '#[cfg(all())]',
        '#[cfg(target_os = "macos")]': '#[cfg(all())]',
        '#[cfg(target_os = "linux")]': '#[cfg(any())]',
        '#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]': '#[cfg(any())]',
    },
    "linux": {
        '#[cfg(windows)]': '#[cfg(any())]',
        '#[cfg(not(windows))]': '#[cfg(all())]',
        '#[cfg(target_os = "macos")]': '#[cfg(any())]',
        '#[cfg(target_os = "linux")]': '#[cfg(all())]',
        '#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]': '#[cfg(any())]',
    },
    "other": {
        '#[cfg(windows)]': '#[cfg(any())]',
        '#[cfg(not(windows))]': '#[cfg(all())]',
        '#[cfg(target_os = "macos")]': '#[cfg(any())]',
        '#[cfg(target_os = "linux")]': '#[cfg(any())]',
        '#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]': '#[cfg(all())]',
    },
}


def main() -> int:
    screen = SRC.read_text(encoding="utf-8")
    cu = CU.read_text(encoding="utf-8")
    # A backend that forgot the keyboard surface must fail here, not only in
    # a cfg configuration where the code is compiled but never called.
    sources = {"screen.rs": screen}
    for extra in EXTRA:
        sources[extra.name] = extra.read_text(encoding="utf-8")
    # 0.0.9 adds `environment()` / `grants()`, 0.0.10 adds
    # `display_scales()` and 0.0.12 adds `window_by_handle()` to all four
    # backends in the same commit; a backend that forgot any of them must fail
    # here, not only in a cfg configuration where the code is compiled but
    # never called.
    required = (
        "pub fn set_modifier",
        "pub fn release_all_modifiers",
        "pub fn environment(",
        "pub fn grants(",
        "pub fn display_scales(",
        "pub fn window_by_handle(",
    )
    for signature in required:
        for name in ("screen.rs", "screen_linux.rs", "screen_macos.rs"):
            if signature not in sources[name]:
                print(f"{name}: missing {signature}", file=sys.stderr)
                return 2
    rlibs = sorted(DEPS.glob("libserde_json-*.rlib"))
    if not rlibs:
        print("no serde_json rlib; run cargo test --locked first", file=sys.stderr)
        return 2
    root = Path(tempfile.mkdtemp(prefix="mh-cfgcheck-"))
    try:
        for name, rewrites in CONFIGS.items():
            text = screen
            for old, new in rewrites.items():
                if old not in text:
                    print(f"{name}: missing cfg marker {old}", file=sys.stderr)
                    return 2
                text = text.replace(old, new)
            d = root / name
            d.mkdir()
            (d / "screen.rs").write_text(text, encoding="utf-8")
            (d / "cu.rs").write_text(cu, encoding="utf-8")
            for extra in EXTRA:
                (d / extra.name).write_text(extra.read_text(encoding="utf-8"), encoding="utf-8")
            (d / "lib.rs").write_text("pub mod screen;\npub mod cu;\n")
            proc = subprocess.run(
                [
                    "rustc", "--crate-type", "lib", "--edition", "2021",
                    "--emit", "metadata", "-o", str(d / "out.rmeta"),
                    "--cap-lints", "allow",
                    "--extern", f"serde_json={rlibs[0]}",
                    "-L", f"dependency={DEPS}",
                    str(d / "lib.rs"),
                ],
                capture_output=True,
                text=True,
            )
            if proc.returncode:
                print(f"{name}: backend type-check FAILED\n{proc.stderr[:4000]}", file=sys.stderr)
                return 1
            print(f"{name}: backend type-check ok")
        print("four computer-use backend configurations type-check")
        return 0
    finally:
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
