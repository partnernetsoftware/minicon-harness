#!/usr/bin/env python3
"""Local mirror of the CI verify job: receipt + APE + 5 archive members."""
import hashlib
import json
import tarfile
import zipfile
from pathlib import Path

rec = json.loads(Path("bundle/build-receipt.json").read_text())
raw = Path("bundle/minicon-harness.com").read_bytes()
assert rec["minicon_harness_com_sha256"] == hashlib.sha256(raw).hexdigest()
assert rec["source_sha"] == "b2be6ff8d4f34eb62ef31356a774738e3c9c680a"
assert rec["version"] == "0.0.2"
assert rec["source_dirty"] is False and rec["within_ceiling"] is True
assert rec["signed"] is False
sidecar = Path("bundle/minicon-harness.com.sha256").read_text().split()[0]
assert sidecar == rec["minicon_harness_com_sha256"]

expected = {
    "minicon-harness-0.0.2-windows-x86_64.zip": ("win-x86_64", "minicon-harness.exe"),
    "minicon-harness-0.0.2-windows-arm64.zip": ("win-aarch64", "minicon-harness.exe"),
    "minicon-harness-0.0.2-linux-x86_64.tar.gz": ("lnx-x86_64", "minicon-harness"),
    "minicon-harness-0.0.2-linux-arm64.tar.gz": ("lnx-aarch64", "minicon-harness"),
    "minicon-harness-0.0.2-macos-universal.tar.gz": None,
}
for name, cell in expected.items():
    p = Path("native") / name
    assert hashlib.sha256(p.read_bytes()).hexdigest() == Path(
        f"native/{name}.sha256"
    ).read_text().split()[0], name
    suffix = ".zip" if name.endswith(".zip") else ".tar.gz"
    leaf = "minicon-harness.exe" if suffix == ".zip" else "minicon-harness"
    member = f"{name[: -len(suffix)]}/{leaf}"
    raw = (
        zipfile.ZipFile(p).read(member)
        if suffix == ".zip"
        else tarfile.open(p).extractfile(member).read()
    )
    if cell:
        cell_id, _leaf = cell
        ident = rec["payloads"][cell_id]
        assert (
            hashlib.sha256(raw).hexdigest() == ident["sha256"]
            and len(raw) == ident["bytes"]
        ), name
    print(f"OK {name}: member {len(raw)}B")
print("ALL VERIFIED — same checks the CI verify job runs")
