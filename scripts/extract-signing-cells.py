#!/usr/bin/env python3
"""Extract the two Windows cells from a packed minicon-harness.com APE and
verify them against the build receipt. Used by the signing workflow's sign
job (pwsh cannot run heredocs), writing the cells as <cell>.exe into --out.

Usage: extract-signing-cells.py --ape PATH --receipt PATH --out DIR
"""

from __future__ import annotations

import argparse
import hashlib
import json
import zipfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ape", required=True)
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    receipt = json.loads(Path(args.receipt).read_text(encoding="utf-8"))
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.ape) as archive:
        for cell in ("win-aarch64", "win-x86_64"):
            raw = archive.read(f"cells/{cell}/minicon-harness.exe")
            identity = receipt["payloads"][cell]
            assert hashlib.sha256(raw).hexdigest() == identity["sha256"], cell
            assert len(raw) == identity["bytes"], cell
            (out / f"{cell}.exe").write_bytes(raw)
            print(f"verified {cell}: {len(raw)} bytes")
    print("signing inputs extracted and digest-verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
