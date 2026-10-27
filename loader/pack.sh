#!/bin/bash
# Pack minicon-harness.com: stage six cells + compile the cosmocc trampoline,
# then overlay the cells as a ZipOS directory onto the APE.
# Method of record: minicon repo loader/pack.sh (PRD_02_27). This copy keeps
# only what this product needs; the zip overlay uses python3 zipfile because
# theInfo-ZIP `zip` binary is not assumed on every pack host.
# cosmocc → dist/minicon-harness.com ; else host cc → dist/minicon-loader
# (needs MINICON_HARNESS_CELLS).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
FRESH="${HARNESS_PAYLOAD_BUILD:-$ROOT/loader/payload-build}"
COSMO="${COSMOCC_DIR:-$HOME/cosmocc}"
DIST="$HERE/dist"
CELLS="$DIST/cells"

payload() {
  cell="$1"
  rel="$2"
  if [[ -f "$FRESH/$rel" ]]; then
    echo "$FRESH/$rel"
  else
    echo "missing payload $rel (looked in $FRESH)" >&2
    exit 1
  fi
}

stage() {
  cell="$1"
  src="$2"
  leaf="$3"
  mkdir -p "$CELLS/$cell"
  if [[ ! -f "$src" ]]; then
    echo "missing $src" >&2
    exit 1
  fi
  cp "$src" "$CELLS/$cell/$leaf"
}

rm -rf "$DIST"
mkdir -p "$CELLS"

stage osx-aarch64 "$(payload osx-aarch64 osx-aarch64/aarch64-apple-darwin/release/minicon-harness)" minicon-harness
stage osx-x86_64 "$(payload osx-x86_64 osx-x86_64/x86_64-apple-darwin/release/minicon-harness)" minicon-harness
stage lnx-aarch64 "$(payload lnx-aarch64 lnx-aarch64/aarch64-unknown-linux-gnu/release/minicon-harness)" minicon-harness
stage lnx-x86_64 "$(payload lnx-x86_64 lnx-x86_64/x86_64-unknown-linux-gnu/release/minicon-harness)" minicon-harness
stage win-aarch64 "$(payload win-aarch64 win-aarch64/aarch64-pc-windows-msvc/release/minicon-harness.exe)" minicon-harness.exe
stage win-x86_64 "$(payload win-x86_64 win-x86_64/x86_64-pc-windows-msvc/release/minicon-harness.exe)" minicon-harness.exe

if [[ -x "$COSMO/bin/cosmocc" ]]; then
  echo "[pack] cosmocc -> dist/minicon-harness.com"
  mkdir -p "$DIST/.aarch64"
  command -v llvm-rc >/dev/null || { echo "llvm-rc not on PATH" >&2; exit 1; }
  command -v llvm-cvtres >/dev/null || { echo "llvm-cvtres not on PATH" >&2; exit 1; }
  version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n1)
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
  version_commas=${version//./,}
  # The vendored rc already carries this product's identity; assert it matches.
  grep -q "FileVersion\", \"$version\"" "$HERE/ape-version.rc" || {
    echo "ape-version.rc version does not match Cargo.toml $version" >&2
    exit 1
  }
  llvm-rc "/fo$DIST/ape-version.res" -- "$HERE/ape-version.rc"
  llvm-cvtres /machine:x64 /out:"$DIST/ape-version.obj" "$DIST/ape-version.res"
  "$COSMO/bin/x86_64-linux-cosmo-objcopy" -O elf64-x86-64 \
    "$DIST/ape-version.obj" "$DIST/ape-version.raw.o"
  "$COSMO/bin/x86_64-linux-cosmo-objcopy" \
    --rename-section '.rsrc$01=.rodata.pytab.0,alloc,load,readonly,data,contents' \
    --rename-section '.rsrc$02=.rodata.pytab.1,alloc,load,readonly,data,contents' \
    "$DIST/ape-version.raw.o" "$DIST/ape-version.o"
  # VERSIONINFO belongs only to the x86_64 PE face. Cosmocc still requires a
  # same-basename aarch64 concomitant for every explicit x86 input.
  "$COSMO/bin/aarch64-linux-cosmo-as" -o "$DIST/.aarch64/ape-version.o" /dev/null
  "$COSMO/bin/x86_64-linux-cosmo-as" -o "$DIST/authenticode-pad.o" \
    "$HERE/authenticode-pad.S"
  "$COSMO/bin/aarch64-linux-cosmo-as" -o "$DIST/.aarch64/authenticode-pad.o" \
    "$HERE/authenticode-pad.S"
  set +e
  cosmo_log=$("$COSMO/bin/cosmocc" -Os -static -o "$DIST/minicon-harness.com" \
    "$HERE/loader.c" "$DIST/authenticode-pad.o" "$DIST/ape-version.o" 2>&1)
  cosmo_rc=$?
  set -e
  if [[ "$cosmo_rc" -ne 0 ]]; then
    expected="$DIST/minicon-harness.com: PE SizeOfOptionalHeader had incorrect value"
    if [[ "$cosmo_rc" -ne 1 || "$cosmo_log" != "$expected" ]]; then
      printf '%s\n' "$cosmo_log" >&2
      exit "$cosmo_rc"
    fi
    echo "[pack] accepted cosmocc 4.0.2 compact-header validator before directory activation"
  elif [[ -n "$cosmo_log" ]]; then
    printf '%s\n' "$cosmo_log"
  fi
  python3 "$HERE/prepare-authenticode.py" "$DIST/minicon-harness.com" "$DIST/minicon-harness.com.signable"
  mv "$DIST/minicon-harness.com.signable" "$DIST/minicon-harness.com"
  chmod 0755 "$DIST/minicon-harness.com"
  python3 - "$DIST" <<'PY'
# ZipOS overlay: append cells/ to the APE with file-absolute central-directory
# offsets (what `zip -r` + `zip -A` produce). python3 zipfile already writes
# absolute offsets on append; assert the invariant instead of trusting it.
import struct, sys, zipfile
from pathlib import Path

dist = Path(sys.argv[1])
ape = dist / "minicon-harness.com"
before = ape.stat().st_size
with zipfile.ZipFile(ape, "a", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for cell in sorted((dist / "cells").iterdir()):
        for payload in sorted(cell.iterdir()):
            archive.write(payload, f"cells/{cell.name}/{payload.name}")
raw = ape.read_bytes()
eocd = raw.rfind(b"PK\x05\x06")
assert eocd > 0, "no end-of-central-directory after overlay"
cd_offset = struct.unpack_from("<I", raw, eocd + 16)[0]
assert raw[cd_offset : cd_offset + 4] == b"PK\x01\x02", "central directory offset not absolute"
with zipfile.ZipFile(ape) as archive:
    names = archive.namelist()
assert len([n for n in names if n.startswith("cells/")]) == 6, names
print(f"[pack] zip overlay cells/ ok (+{ape.stat().st_size - before} bytes, 6 payloads)")
PY
else
  echo "[pack] no cosmocc; host-cc dispatcher"
  cc -O2 -o "$DIST/minicon-loader" "$HERE/loader.c"
fi

echo "[pack] staged:"
find "$CELLS" -type f -exec ls -lh {} \;
sha256sum "$DIST/minicon-harness.com" > "$DIST/minicon-harness.com.sha256"
echo "[pack] OK $DIST"
