#!/usr/bin/env python3
"""Independent reader of a cross-platform CU desktop journey.

The journey writer (`examples/cu_journey.rs`) is deliberately not allowed to
decide its own success. This script recomputes every claim from `records`:
declared capability matrix, performed verdicts, pointer readbacks, PNG bytes
and generation ordering. A green `cargo check` is evidence of shape; this is
evidence of a desktop effect.

A macOS OS-grant refusal (`permission:...`) is a *named boundary*, not a pass
and not a silent failure: it is recorded as such and never upgraded to `[v]`.
"""
import argparse
import json
import re
import struct
import sys
from pathlib import Path

EXPECTED = {
    "windows": {name: True for name in (
        "observe", "controls", "capture", "act:focus", "act:move", "act:click",
        "act:hover", "act:drag", "act:type", "act:key", "act:scroll", "act:control",
    )},
    "linux": {
        "observe": True, "controls": False, "capture": True, "act:focus": True,
        "act:move": True, "act:click": True, "act:hover": True, "act:drag": True,
        "act:type": True, "act:key": True, "act:scroll": True, "act:control": False,
    },
    "macos": {
        "observe": True, "controls": False, "capture": True, "act:focus": False,
        "act:move": False, "act:click": True, "act:hover": True, "act:drag": True,
        "act:type": True, "act:key": True, "act:scroll": True, "act:control": False,
    },
}
POINTER_STEPS = {"act_hover", "act_click", "act_drag", "act_scroll"}
TOLERANCE = 4
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--journey", required=True)
    parser.add_argument("--root", required=True)
    args = parser.parse_args()

    document = json.loads(Path(args.journey).read_text())
    root = Path(args.root)
    failures = []
    notes = []

    def check(condition, message):
        if not condition:
            failures.append(message)

    platform = document.get("platform")
    check(document.get("schema") == 1, "schema must be 1")
    check(platform in EXPECTED, f"unknown platform {platform!r}")
    if platform not in EXPECTED:
        print("\n".join(failures), file=sys.stderr)
        return 1

    declared = {
        row["capability"]: row["implemented"]
        for row in document.get("matrix", [])
    }
    check(len(declared) == 12, f"matrix has {len(declared)} rows, expected 12")
    for name, expected in EXPECTED[platform].items():
        check(
            declared.get(name) is expected,
            f"declared {name}={declared.get(name)!r}, expected {expected!r}",
        )

    records = document.get("records", [])
    check(len(records) >= 10, f"only {len(records)} journey records")
    by_step = {record["step"]: record for record in records}

    # 1. A required step is performed, or (macOS only) a named permission
    #    boundary. Nothing else counts as a desktop journey.
    for record in records:
        step = record["step"]
        verdict = record["verdict"]
        text = record.get("text", "")
        boundary = (
            platform == "macos"
            and "permission:" in text
            and verdict in ("unsupported", "refused")
        )
        if record.get("required") and not boundary:
            check(
                verdict == "performed",
                f"{step}: required step verdict={verdict}: {text.splitlines()[0] if text else ''}",
            )
        if verdict == "unknown" and record.get("required"):
            check(boundary, f"{step}: unconfirmed native outcome is not a journey")
        if verdict == "performed" and step.startswith("act_"):
            readback = record.get("evidence", {}).get("readback")
            check(
                isinstance(readback, str) and readback.strip() != "",
                f"{step}: performed with no readback",
            )
        if boundary:
            notes.append(f"BOUNDARY {step}: named OS-grant refusal (not counted as [v])")

    # 2. The observation is real: at least one window, one display, positive
    #    virtual desktop. A refusal or an empty list is not an observation.
    observed = by_step.get("observe", {})
    evidence = observed.get("evidence", {})
    windows = evidence.get("windows", [])
    displays = evidence.get("displays", [])
    desktop = evidence.get("desktop", {})
    check(observed.get("verdict") == "performed", "plain observe was not performed")
    check(len(windows) >= 1, "observe reported no windows")
    check(len(displays) >= 1, "observe reported no displays")
    check(desktop.get("width", 0) > 0 and desktop.get("height", 0) > 0,
          "observe reported no positive desktop geometry")

    # 3. Pointer effects must be read back at the requested coordinates.
    for step in POINTER_STEPS:
        record = by_step.get(step)
        if not record or record.get("verdict") != "performed":
            continue
        request = record.get("request", {})
        want_x = request.get("to_x", request.get("x"))
        want_y = request.get("to_y", request.get("y"))
        readback = record.get("evidence", {}).get("readback", "")
        match = re.search(r"(-?\d+),\s*(-?\d+)", readback)
        check(match is not None, f"{step}: readback has no pointer position: {readback!r}")
        if match:
            got_x, got_y = int(match.group(1)), int(match.group(2))
            check(abs(got_x - want_x) <= TOLERANCE and abs(got_y - want_y) <= TOLERANCE,
                  f"{step}: readback {got_x},{got_y} != requested {want_x},{want_y}")

    # 4. A performed capture is a real PNG on disk whose IHDR agrees with the
    #    typed evidence. A black/missing image is not a successful capture.
    captured = by_step.get("observe_capture", {})
    if captured.get("verdict") == "performed":
        outcome = captured.get("evidence", {}).get("capture")
        check(isinstance(outcome, dict), "capture observation has no capture evidence")
        if isinstance(outcome, dict):
            image_path = root / outcome["path"]
            check(image_path.is_file(), f"capture file missing: {image_path}")
            if image_path.is_file():
                raw = image_path.read_bytes()
                check(raw.startswith(PNG_SIGNATURE), "capture is not a PNG")
                check(len(raw) >= 24 and raw[12:16] == b"IHDR", "capture has no IHDR")
                if len(raw) >= 24 and raw[12:16] == b"IHDR":
                    width, height = struct.unpack(">II", raw[16:24])
                    check(width == outcome["width"] and height == outcome["height"],
                          f"PNG {width}x{height} != evidence "
                          f"{outcome['width']}x{outcome['height']}")
                check(len(raw) == outcome["bytes"],
                      f"PNG {len(raw)} bytes != evidence {outcome['bytes']}")
                notes.append(
                    f"CAPTURE {image_path}: {outcome['width']}x{outcome['height']} "
                    f"scale={outcome['scale']} bytes={len(raw)}"
                )
    else:
        notes.append("CAPTURE not performed (named boundary or skipped)")

    # 5. Generations of performed observations must strictly advance.
    generations = [
        record["generation"]
        for record in records
        if record["step"].startswith("observe") and record["verdict"] == "performed"
    ]
    check(all(a < b for a, b in zip(generations, generations[1:])),
          f"observation generations did not advance: {generations}")

    # 6. A declared-but-skipped verb is a contract violation; only an
    #    undeclared verb may be skipped.
    for record in records:
        if record["verdict"] == "skipped":
            check(not record.get("declared"),
                  f"{record['step']}: declared capability was skipped")

    performed = [record["step"] for record in records if record["verdict"] == "performed"]
    print(f"cu-journey platform={platform} records={len(records)} performed={len(performed)}")
    for record in records:
        readback = record.get("evidence", {}).get("readback", "")
        print(f"  {record['step']:<18} {record['verdict']:<12} {str(readback)[:90]}")
    for note in notes:
        print(f"  {note}")
    print(f"  performed steps: {', '.join(performed)}")
    check(len(performed) >= 2, "fewer than two performed steps; no journey happened")

    if failures:
        print("\nJOURNEY FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    print("cu-journey verdict: PASS (independent reader)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
