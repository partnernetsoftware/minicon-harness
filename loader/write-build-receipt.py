#!/usr/bin/env python3
"""Emit dist/build-receipt.json after pack.sh: tool versions + payload digests.

Adapted from the minicon loader method for the harness product: no zig /
zigbuild / xwin identities (cells build with plain cargo cross targets in
CI lanes), no Authenticode signing claim at 0.0.1, ceiling 5 MiB.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
DIST = HERE / "dist"
CELLS = DIST / "cells"
COM = DIST / "minicon-harness.com"
LOADER_SOURCE = HERE / "loader.c"
AUTHENTICODE_PAD_SOURCE = HERE / "authenticode-pad.S"
AUTHENTICODE_PREPARE_SOURCE = HERE / "prepare-authenticode.py"
VERSION_RESOURCE_SOURCE = HERE / "ape-version.rc"

# Owner-stamped 2026-10-27 (prd/PRD_01_01_com_delivery.md). Never auto-raise.
CANDIDATE_CEILING_BYTES = 6_291_456

EXPECTED_CELLS = {
    "osx-aarch64": "minicon-harness",
    "osx-x86_64": "minicon-harness",
    "lnx-aarch64": "minicon-harness",
    "lnx-x86_64": "minicon-harness",
    "win-aarch64": "minicon-harness.exe",
    "win-x86_64": "minicon-harness.exe",
}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def cmd(*args: str) -> str:
    try:
        return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT).strip().splitlines()[0]
    except (OSError, subprocess.CalledProcessError):
        return ""


def cargo_version() -> str:
    try:
        return subprocess.check_output(["cargo", "--version"], text=True).strip()
    except OSError:
        return ""


def rustc_version() -> str:
    try:
        return subprocess.check_output(["rustc", "--version"], text=True).strip()
    except OSError:
        return ""


def git(*args: str) -> str:
    return subprocess.check_output(["git", "-C", str(HERE.parent), *args], text=True).strip()


def source_identity() -> dict[str, object]:
    """HEAD plus a digest of the packed source tree (diff + untracked)."""
    head = git("rev-parse", "HEAD")
    porcelain = git("status", "--porcelain", "-uall")
    diff = git("diff", "HEAD")
    h = hashlib.sha256()
    h.update(porcelain.encode())
    h.update(diff.encode())
    return {
        "source_sha": head,
        "source_dirty": bool(porcelain),
        "source_tree_digest": h.hexdigest(),
    }


def payload_digests() -> dict[str, dict[str, object]]:
    payloads: dict[str, dict[str, object]] = {}
    for cell, leaf in EXPECTED_CELLS.items():
        path = CELLS / cell / leaf
        if not path.is_file():
            raise SystemExit(f"missing cell payload {path}")
        raw = path.read_bytes()
        payloads[cell] = {
            "sha256": hashlib.sha256(raw).hexdigest(),
            "bytes": len(raw),
        }
    return payloads


def main() -> None:
    if not COM.is_file():
        raise SystemExit(f"missing {COM}; run pack.sh first")
    com_bytes = COM.read_bytes()
    com_sha = hashlib.sha256(com_bytes).hexdigest()
    version = subprocess.check_output(
        ["sed", "-n", r"s/^version = \"\([^\"]*\)\"/\1/p", str(HERE.parent / "Cargo.toml")],
        text=True,
    ).strip()
    receipt: dict[str, object] = {
        "schema": 1,
        "product": "minicon-harness",
        "version": version,
        "minicon_harness_com_sha256": com_sha,
        "minicon_harness_com_bytes": len(com_bytes),
        "candidate_ceiling_bytes": CANDIDATE_CEILING_BYTES,
        "within_ceiling": len(com_bytes) <= CANDIDATE_CEILING_BYTES,
        "loader_source_sha256": sha256(LOADER_SOURCE),
        "authenticode_pad_source_sha256": sha256(AUTHENTICODE_PAD_SOURCE),
        "authenticode_prepare_source_sha256": sha256(AUTHENTICODE_PREPARE_SOURCE),
        "version_resource_source_sha256": sha256(VERSION_RESOURCE_SOURCE),
        "signed": False,
        "tools": {
            "rustc": rustc_version(),
            "cargo": cargo_version(),
            "cosmocc": cmd(str(Path.home() / "cosmocc" / "bin" / "cosmocc"), "--version"),
        },
        "payloads": payload_digests(),
        **source_identity(),
    }
    empty = [k for k, v in receipt["tools"].items() if not v]  # type: ignore[union-attr]
    if empty:
        raise SystemExit(f"empty tool identity fields: {empty}")
    if not receipt["within_ceiling"]:
        raise SystemExit(
            f"minicon-harness.com is {len(com_bytes)} bytes, over the {CANDIDATE_CEILING_BYTES}-byte "
            "Candidate ceiling; raising it is an explicit owner decision (PRD 01.01)"
        )
    (DIST / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(
        json.dumps(
            {
                "minicon_harness_com_sha256": com_sha,
                "bytes": len(com_bytes),
                "source_sha": receipt["source_sha"],
                "source_dirty": receipt["source_dirty"],
                "source_tree_digest": receipt["source_tree_digest"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
