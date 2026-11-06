#!/usr/bin/env python3
"""Independent reader of a cross-platform CU desktop journey.

The journey writer (`examples/cu_journey.rs`) is deliberately not allowed to
decide its own success. This script recomputes every claim from `records`:
declared capability matrix, performed verdicts, pointer readbacks, actuation
receipts, PNG bytes, generation ordering, the 0.0.9 environment/grant rows
and the 0.0.10 display-scale rows (six-value set, exact
physical = logical * scale relation, reason rule). A green `cargo check` is
evidence of shape; this is evidence of a desktop effect.

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
POINTER_STEPS = {
    "act_hover", "act_click", "act_click_middle", "act_click_right",
    "act_click_double", "act_drag", "act_drag_middle", "act_scroll",
    "act_drag_path", "act_scroll_x", "act_scroll_y",
}
# Steps that dispatch a real button event. A performed one must carry a
# receipt produced by the backend and matching the request, not an echo.
BUTTON_STEPS = {
    "act_click", "act_click_middle", "act_click_right", "act_click_double",
    "act_drag", "act_drag_middle", "act_drag_path", "act_control_press",
}
REQUIRED_INTERACTIONS = (
    "act_click_middle", "act_click_right", "act_click_double", "act_drag_middle",
    "act_drag_path", "act_scroll_x", "act_scroll_y",
)
SCROLL_STEPS = ("act_scroll", "act_scroll_x", "act_scroll_y")
# 0.0.8: a held modifier must be disclosed, released and never mistaken for
# a button event. The independent reader recomputes the whole sequence.
KEYBOARD_STEPS = (
    "act_key_hold", "disclose_held", "act_key_release", "disclose_released",
    "disclose_final",
)
TOLERANCE = 4
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"

# 0.0.10: the bounded typed scale set and the exact integer ratio
# (physical / logical) of each known member. `unknown` has no ratio, so no
# relation can be recomputed for that row; it must instead carry a reason.
SCALE_RATIOS = {
    "1": (1, 1), "1.25": (5, 4), "1.5": (3, 2), "2": (2, 1), "3": (3, 1),
}
SCALE_SET = frozenset(SCALE_RATIOS) | {"unknown"}
MAX_DISPLAY_SCALES = 16
MAX_SCALE_REASON = 160


def rect_values(rect):
    """Return (x, y, width, height) as ints, or None if any component is not
    an int. A bool is not a coordinate, and a rect that cannot be read as four
    ints cannot have its physical = logical * scale relation recomputed."""
    if not isinstance(rect, dict):
        return None
    values = []
    for key in ("x", "y", "width", "height"):
        value = rect.get(key)
        if not isinstance(value, int) or isinstance(value, bool):
            return None
        values.append(value)
    return tuple(values)


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
        # A path drag lands at its LAST point, not at a to_x/to_y the
        # request may not even carry. Recompute it from the request itself.
        path = request.get("path")
        if isinstance(path, list) and len(path) >= 2:
            want_x, want_y = path[-1]
        readback = record.get("evidence", {}).get("readback", "")
        match = re.search(r"(-?\d+),\s*(-?\d+)", readback)
        check(match is not None, f"{step}: readback has no pointer position: {readback!r}")
        if match:
            got_x, got_y = int(match.group(1)), int(match.group(2))
            check(abs(got_x - want_x) <= TOLERANCE and abs(got_y - want_y) <= TOLERANCE,
                  f"{step}: readback {got_x},{got_y} != requested {want_x},{want_y}")

    # 3b. Interaction closure: every performed button event carries a typed
    #     actuation receipt that agrees with the request. A missing receipt is
    #     an unconfirmed effect, not a successful click.
    for record in records:
        step = record["step"]
        if step not in BUTTON_STEPS or record["verdict"] != "performed":
            continue
        request = record.get("request") or {}
        want_button = request.get("button", "left")
        want_double = bool(request.get("double", False))
        want_downs = 2 if want_double else 1
        path = request.get("path")
        want_points = len(path) if isinstance(path, list) else (1 if step.startswith("act_drag") else 0)
        receipt = record.get("evidence", {}).get("actuation")
        check(isinstance(receipt, dict),
              f"{step}: performed button event has no actuation receipt")
        if isinstance(receipt, dict):
            check(receipt.get("button") == want_button,
                  f"{step}: receipt button={receipt.get('button')!r}, requested {want_button!r}")
            check(receipt.get("downs") == want_downs,
                  f"{step}: receipt downs={receipt.get('downs')!r}, expected {want_downs}")
            check(receipt.get("ups") == receipt.get("downs") == want_downs,
                  f"{step}: receipt ups={receipt.get('ups')!r} != downs")
            check(receipt.get("double") is want_double,
                  f"{step}: receipt double={receipt.get('double')!r}, expected {want_double}")
            target = receipt.get("target")
            check(isinstance(target, str) and target.strip() != "",
                  f"{step}: receipt has no non-empty target")
            check(receipt.get("path_points") == want_points,
                  f"{step}: receipt path_points={receipt.get('path_points')!r}, "
                  f"expected {want_points}")

    # 3c. Two-axis scroll: the published structured axes must equal the
    #     requested pair. `delta` stays the vertical alias, so an old-style
    #     request expects (0, delta). No application scroll effect is claimed.
    for step in SCROLL_STEPS:
        record = by_step.get(step)
        if not record or record.get("verdict") != "performed":
            continue
        request = record.get("request") or {}
        want_x = request.get("delta_x", 0)
        want_y = request.get("delta_y", request.get("delta", 0))
        axes = record.get("evidence", {}).get("scroll")
        check(isinstance(axes, dict),
              f"{step}: performed wheel event has no structured scroll evidence")
        if isinstance(axes, dict):
            check(axes.get("delta_x") == want_x,
                  f"{step}: scroll delta_x={axes.get('delta_x')!r}, expected {want_x!r}")
            check(axes.get("delta_y") == want_y,
                  f"{step}: scroll delta_y={axes.get('delta_y')!r}, expected {want_y!r}")

    for step in REQUIRED_INTERACTIONS:
        check(step in by_step, f"{step}: required 0.0.6 interaction step is missing")
    if platform == "windows":
        check("act_control_press" in by_step,
              "windows: required act_control_press step is missing")

    # 3d. Held-key modifiers: the host's own state machine is recomputed from
    #     the requests, the keyboard evidence and the disclosures. A named OS
    #     grant boundary must never excuse a stuck modifier.
    for step in KEYBOARD_STEPS:
        check(step in by_step, f"{step}: required 0.0.8 held-key step is missing")
        record = by_step.get(step)
        if not record:
            continue
        check(record["verdict"] == "performed",
              f"{step}: held-key step verdict={record['verdict']} is not a journey")
        evidence = record.get("evidence", {})
        check(evidence.get("actuation") is None,
              f"{step}: keyboard step carries a button actuation receipt")
    hold = by_step.get("act_key_hold", {})
    if hold.get("verdict") == "performed":
        request = hold.get("request") or {}
        wanted = request.get("keys")
        keyboard = hold.get("evidence", {}).get("keyboard")
        check(request.get("hold") is True, "act_key_hold: request did not ask to hold")
        check(isinstance(keyboard, dict), "act_key_hold: no keyboard evidence")
        if isinstance(keyboard, dict):
            check(keyboard.get("held") == [wanted],
                  f"act_key_hold: held={keyboard.get('held')!r}, expected [{wanted!r}]")
            check(keyboard.get("released") == [],
                  f"act_key_hold: released={keyboard.get('released')!r}, expected []")
        check(hold.get("evidence", {}).get("held_keys") == [wanted],
              f"act_key_hold: held_keys disagrees with the request {wanted!r}")
    disclosed_held = by_step.get("disclose_held", {})
    if disclosed_held.get("verdict") == "performed":
        check(disclosed_held.get("evidence", {}).get("held_keys")
              == [(hold.get("request") or {}).get("keys")],
              "disclose_held: disclosure does not report the requested held modifier")
    release = by_step.get("act_key_release", {})
    if release.get("verdict") == "performed":
        request = release.get("request") or {}
        keyboard = release.get("evidence", {}).get("keyboard")
        check(request.get("release") is True, "act_key_release: request did not ask to release")
        check(isinstance(keyboard, dict), "act_key_release: no keyboard evidence")
        if isinstance(keyboard, dict):
            released = keyboard.get("released")
            check(isinstance(released, list) and len(released) > 0,
                  f"act_key_release: released={released!r}, expected a non-empty list")
            check(keyboard.get("held") == [],
                  f"act_key_release: held={keyboard.get('held')!r}, expected []")
        check(release.get("evidence", {}).get("held_keys") == [],
              "act_key_release: held_keys is not empty after the release")
    for step in ("disclose_released", "disclose_final", "disclose"):
        record = by_step.get(step)
        if record and record.get("verdict") == "performed":
            check(record.get("evidence", {}).get("held_keys") == [],
                  f"{step}: held_keys is not empty; the session may have a stuck modifier")

    # 3e. 0.0.9 read-only disclosure. `disclose` is pure information: it
    #     requests no OS grant and posts no input, so a named permission
    #     boundary must never excuse a missing or partial disclosure.
    grant_statuses = {"granted", "denied", "unknown", "not_applicable"}
    session_kinds = {"gui", "x11", "wayland-x11-compat", "headless", "unavailable"}
    disclosure = by_step.get("disclose_grants")
    check(disclosure is not None, "disclose_grants: required 0.0.9 step is missing")
    check(disclosure is not None and disclosure.get("verdict") == "performed",
          "disclose_grants: disclosure was not performed; BOUNDARY cannot excuse it")
    if disclosure and disclosure.get("verdict") == "performed":
        env = disclosure.get("evidence", {}).get("environment")
        check(isinstance(env, dict), "disclose_grants: no environment evidence")
        if isinstance(env, dict):
            check(env.get("platform") == platform,
                  f"disclose_grants: environment platform={env.get('platform')!r}, "
                  f"expected {platform!r}")
            check(env.get("session") in session_kinds,
                  f"disclose_grants: session={env.get('session')!r} is not a coarse kind")
            count = env.get("display_count")
            check(isinstance(count, int) and not isinstance(count, bool) and count > 0,
                  f"disclose_grants: display_count={count!r}, expected a positive int")
            rect = env.get("virtual_desktop")
            check(isinstance(rect, dict) and rect.get("width", 0) > 0
                  and rect.get("height", 0) > 0,
                  f"disclose_grants: virtual_desktop has no positive area: {rect!r}")
            check(isinstance(env.get("foreground_present"), bool),
                  "disclose_grants: foreground_present is not a boolean")
            check(not any(key in env for key in ("title", "user", "path", "display")),
                  "disclose_grants: environment publishes identifying strings")
        grants = disclosure.get("evidence", {}).get("grants")
        check(isinstance(grants, list) and len(grants) >= 2,
              f"disclose_grants: grants={grants!r}, expected at least two named probes")
        if isinstance(grants, list) and grants:
            names = []
            for i, row in enumerate(grants):
                check(isinstance(row, dict),
                      f"disclose_grants: grant row {i} is not an object")
                if not isinstance(row, dict):
                    continue
                name = row.get("name")
                status = row.get("status")
                names.append(name)
                check(isinstance(name, str) and name.strip() != "",
                      f"disclose_grants: grant row {i} has no name")
                check(status in grant_statuses,
                      f"disclose_grants: {name!r} status={status!r} outside the whitelist")
                reason = row.get("reason")
                if status in ("unknown", "not_applicable"):
                    check(isinstance(reason, str) and reason.strip() != "",
                          f"disclose_grants: {name!r} {status} has no reason")
                if isinstance(reason, str):
                    check(len(reason) <= 160,
                          f"disclose_grants: {name!r} reason exceeds the 160-char bound")
            check(all(isinstance(n, str) for n in names),
                  f"disclose_grants: probe names are not all strings: {names!r}")
            if all(isinstance(n, str) for n in names):
                check(len(names) == len(set(names)),
                      f"disclose_grants: duplicate probe names {names!r}")
                check(names == sorted(names),
                      f"disclose_grants: probe names are not sorted: {names!r}")
            # A grant row must not contradict an effect performed in the same
            # session: a successful observe proves the display is reachable.
            if observed.get("verdict") == "performed" and len(windows) >= 1:
                for row in grants:
                    if isinstance(row, dict) and row.get("name") in (
                            "screen_capture", "x11_display", "window_enumeration"):
                        check(row.get("status") != "denied",
                              f"disclose_grants: {row.get('name')!r}=denied contradicts "
                              f"a performed observe in the same session")
        # Every other performed disclosure publishes the same additive fields.
        for step in ("disclose", "disclose_held", "disclose_released", "disclose_final"):
            rec = by_step.get(step)
            if rec and rec.get("verdict") == "performed":
                ev = rec.get("evidence", {})
                check(isinstance(ev.get("environment"), dict),
                      f"{step}: performed disclosure has no environment evidence")
                check(isinstance(ev.get("grants"), list),
                      f"{step}: performed disclosure has no grants evidence")
        notes.append(
            f"DISCLOSURE session={env.get('session') if isinstance(env, dict) else '?'} "
            f"probes={len(grants) if isinstance(grants, list) else 0}"
        )

    # 3f. 0.0.10 read-only display-scale disclosure. Like 3e, `disclose` is
    #     pure information, so a named permission boundary must never excuse
    #     a missing or partial scale disclosure. The reader recomputes every
    #     row from the published evidence: the six-value typed set, the exact
    #     physical_rect == logical_rect * scale relation, the unknown/reason
    #     rule, the 16-row bound, zero-based continuous indices and the
    #     160-char reason bound. Any contradiction is not_performed and fails
    #     the journey. No earlier 0.0.6-0.0.9 assertion is weakened here.
    scale_disclosure = by_step.get("disclose_scales")
    check(scale_disclosure is not None,
          "disclose_scales: required 0.0.10 step is missing")
    check(scale_disclosure is not None
          and scale_disclosure.get("verdict") == "performed",
          "disclose_scales: disclosure was not performed; BOUNDARY cannot excuse it")
    if scale_disclosure and scale_disclosure.get("verdict") == "performed":
        scale_env = scale_disclosure.get("evidence", {}).get("environment")
        check(isinstance(scale_env, dict),
              "disclose_scales: no environment evidence")
        scales = scale_env.get("display_scales") if isinstance(scale_env, dict) else None
        check(isinstance(scales, list) and len(scales) >= 1,
              f"disclose_scales: display_scales={scales!r}, expected a non-empty list")
        if isinstance(scales, list):
            check(len(scales) <= MAX_DISPLAY_SCALES,
                  f"disclose_scales: {len(scales)} rows exceed the bound of "
                  f"{MAX_DISPLAY_SCALES}")
            for i, row in enumerate(scales):
                check(isinstance(row, dict),
                      f"disclose_scales: row {i} is not an object")
                if not isinstance(row, dict):
                    continue
                index = row.get("index")
                check(isinstance(index, int) and not isinstance(index, bool)
                      and index == i,
                      f"disclose_scales: row {i} index={index!r}, expected {i} "
                      f"(zero-based, continuous)")
                tag = row.get("scale")
                check(tag in SCALE_SET,
                      f"disclose_scales: row {i} scale={tag!r} is outside the six-value set")
                logical = rect_values(row.get("logical_rect"))
                physical = rect_values(row.get("physical_rect"))
                check(logical is not None,
                      f"disclose_scales: row {i} logical_rect is not an "
                      f"int rect: {row.get('logical_rect')!r}")
                check(physical is not None,
                      f"disclose_scales: row {i} physical_rect is not an "
                      f"int rect: {row.get('physical_rect')!r}")
                reason = row.get("reason")
                if tag == "unknown":
                    check(isinstance(reason, str) and reason.strip() != "",
                          f"disclose_scales: row {i} is unknown without a non-empty reason")
                elif tag in SCALE_RATIOS and logical is not None and physical is not None:
                    num, den = SCALE_RATIOS[tag]
                    # Exact integer recompute: physical == logical * num / den,
                    # with every component dividing exactly (negative origins
                    # included). No floating point, no rounding, no guess at 1.
                    recomputed = all(
                        (value * num) % den == 0 and (value * num) // den == got
                        for value, got in zip(logical, physical)
                    )
                    check(recomputed,
                          f"disclose_scales: row {i} physical_rect={physical} != "
                          f"logical_rect={logical} * {tag}")
                if isinstance(reason, str):
                    check(len(reason) <= MAX_SCALE_REASON,
                          f"disclose_scales: row {i} reason exceeds the "
                          f"{MAX_SCALE_REASON}-char bound")
            notes.append(
                f"DISCLOSURE_SCALES rows={len(scales)} "
                f"scales={[r.get('scale') for r in scales if isinstance(r, dict)]}"
            )
        # Every performed disclosure and observation publishes the same
        # additive fields: environment.display_scales and displays[i].scale.
        for step in ("disclose", "disclose_held", "disclose_released",
                     "disclose_final", "disclose_grants"):
            rec = by_step.get(step)
            if rec and rec.get("verdict") == "performed":
                check(isinstance(
                    rec.get("evidence", {}).get("environment", {}).get("display_scales"),
                    list),
                    f"{step}: performed disclosure has no display_scales evidence")
        for step in ("observe", "observe_final", "observe_capture"):
            rec = by_step.get(step)
            if rec and rec.get("verdict") == "performed":
                for display in rec.get("evidence", {}).get("displays", []):
                    if isinstance(display, dict):
                        check(display.get("scale") in SCALE_SET,
                              f"{step}: display scale={display.get('scale')!r} "
                              f"is outside the six-value set")

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

    # 5b. The final observation must be a strictly newer, still-live frame,
    #     not cached state after the side-effecting clicks.
    final = by_step.get("observe_final", {})
    final_evidence = final.get("evidence", {})
    check(final.get("verdict") == "performed", "observe_final was not performed")
    check(final.get("generation", 0) > observed.get("generation", 0),
          "observe_final generation did not advance past the first observation")
    check(len(final_evidence.get("windows", [])) >= 1,
          "observe_final reported no windows; session is not live")
    check(len(final_evidence.get("displays", [])) >= 1,
          "observe_final reported no displays; session is not live")

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
