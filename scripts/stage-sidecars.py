#!/usr/bin/env python3
"""Stages the three 0.0.20 artifacts from locally built binaries.

The 0.0.19 gap was that `minicon-harness-mcp` was built and tested but never
archived, so the companion thesis had no downloadable artifact. This script is
the local counterpart of the workflow's archive step: it stages the core, the
MCP adapter and the gate into one directory with per-artifact sidecars, so the
same three names exist everywhere before signing runs.

It does not sign, and it does not publish. Signing needs the release policy and
the signing cells; this only proves the artifact set is complete and hashes.

Usage: stage-sidecars.py [--release]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

#: The three products. The first is the worker core; the other two are sidecars
#: (`PRD.md` "Product definition"), and all three ship.
PRODUCTS = [
    ("minicon-harness", "the worker core: file, exec, screen"),
    ("minicon-harness-mcp", "the MCP adapter: the same three tools over stdio"),
    ("minicon-gate", "the LLM gate: the pool and the egress decision"),
]


def version() -> str:
    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("version = "):
            return line.split('"')[1]
    raise SystemExit("stage-sidecars: Cargo.toml has no version")


def build(release: bool) -> Path:
    profile = "release" if release else "debug"
    command = ["cargo", "build", "--locked", "--bins"]
    if release:
        command.insert(2, "--release")
    subprocess.run(command, cwd=ROOT, check=True)
    return ROOT / "target" / profile


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true", help="build and stage the release profile")
    parser.add_argument("--out", default="dist", help="output directory (default: dist)")
    args = parser.parse_args()

    profile = build(args.release)
    suffix = ".exe" if sys.platform.startswith("win") else ""
    platform = f"{sys.platform}-{subprocess.check_output(['rustc', '-vV']).decode().split('host: ')[1].split()[0]}"
    destination = ROOT / args.out / f"minicon-harness-{version()}-{platform}"
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)

    recorded = {}
    for name, purpose in PRODUCTS:
        source = profile / f"{name}{suffix}"
        if not source.exists():
            print(f"stage-sidecars: {source} is missing", file=sys.stderr)
            return 2
        target = destination / source.name
        shutil.copy2(source, target)
        digest = hashlib.sha256(target.read_bytes()).hexdigest()
        recorded[source.name] = {"sha256": digest, "bytes": target.stat().st_size, "purpose": purpose}
        # A sidecar per artifact, the shape the core already uses.
        (destination / f"{source.name}.sha256").write_text(f"{digest}  {source.name}\n", encoding="utf-8")

    # The release legal files travel with the product.
    for legal in ("LICENSE", "README.md"):
        source = ROOT / legal
        if source.exists():
            shutil.copy2(source, destination / legal)

    receipt = {
        "schema": 1,
        "version": version(),
        "platform": platform,
        "profile": "release" if args.release else "debug",
        "artifacts": recorded,
        "signed": False,
    }
    (destination / "stage-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")

    # Verify what was staged: every artifact present, every hash matching.
    for leaf, identity in recorded.items():
        raw = (destination / leaf).read_bytes()
        if hashlib.sha256(raw).hexdigest() != identity["sha256"]:
            print(f"stage-sidecars: {leaf} hash mismatch after copy", file=sys.stderr)
            return 1
        sidecar = (destination / f"{leaf}.sha256").read_text(encoding="utf-8").split()[0]
        if sidecar != identity["sha256"]:
            print(f"stage-sidecars: {leaf}.sha256 disagrees with the receipt", file=sys.stderr)
            return 1

    print(f"stage-sidecars: {len(recorded)} artifacts staged in {destination}")
    for leaf, identity in recorded.items():
        print(f"  {leaf}  {identity['bytes']} bytes  {identity['sha256'][:12]}  {identity['purpose']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
