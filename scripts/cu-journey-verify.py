#!/usr/bin/env python3
"""Independent reader of a cross-platform CU desktop journey.

The journey writer (`examples/cu_journey.rs`) is deliberately not allowed to
decide its own success. This script recomputes every claim from `records`:
declared capability matrix, performed verdicts, pointer readbacks, actuation
receipts, PNG bytes, generation ordering, the 0.0.9 environment/grant rows,
the 0.0.10 display-scale rows (six-value set, exact
physical = logical * scale relation, reason rule), the 0.0.11 a11y tree,
the 0.0.12 handle-addressed focus readback and the 0.0.13 bounded wait
evidence. A green `cargo check` is evidence of shape; this is evidence of a
desktop effect.

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
# 0.0.12: handle-addressed focus. A title-target focus and a handle-target
# focus must both publish a desktop-read target_hwnd; the reader recomputes it
# against the focused row of the very next observation. A stale handle is a
# named refusal, and a cell whose matrix declares act:focus unsupported may
# only produce a named capability boundary for the two handle steps.
FOCUS_TARGET_STEPS = (
    "act_focus", "act_focus_by_handle", "act_refocus_after_right_click",
)
FOCUS_HANDLE_STEPS = frozenset({"act_focus_by_handle", "act_focus_stale_handle"})
# 0.0.15: a semantic a11y target. A performed one is recomputed from the
# normalised node schema; a refused one must be a named boundary, never a
# silent guess. No earlier 0.0.6-0.0.14 assertion is weakened.
A11Y_CONTROL_STEP = "act_control_a11y"
A11Y_STATE_VETO = 0x1 | 0x8000 | 0x10000
FOCUS_READBACK_PAIRS = (
    ("act_focus", "observe_focus_readback"),
    ("act_focus_by_handle", "observe_handle_focus_readback"),
)
FOCUS_REQUIRED_STEPS = (
    "observe_focus_readback", "act_focus_by_handle",
    "observe_handle_focus_readback", "act_focus_stale_handle",
)
# 0.0.13: bounded observation wait. A performed wait must publish a
# monotonic elapsed time, cumulative accounting, a call count and a fresh
# generation; an over-bound request must be a named refusal with no wait
# evidence. The reader recomputes all of it from the records.
MAX_WAIT_MS = 2000
MAX_CUMULATIVE_WAIT_MS = 8000
MAX_WAIT_CALLS = 8
WAIT_OVERHEAD_MS = 500
WAIT_PERFORMED_STEPS = ("observe_wait_after_act",)
WAIT_CEILING_STEP = "observe_wait_over_bound"
WAIT_REQUIRED_STEPS = WAIT_PERFORMED_STEPS + (WAIT_CEILING_STEP,)
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
    #    boundary. Nothing else counts as a desktop journey. 0.0.12 adds a
    #    stale-handle refusal and a capability boundary where the published
    #    matrix declares act:focus unsupported; 0.0.13 adds one over-bound
    #    wait refusal. Each exception is named, never a silent skip.
    for record in records:
        step = record["step"]
        verdict = record["verdict"]
        text = record.get("text", "")
        boundary = (
            platform == "macos"
            and "permission:" in text
            and verdict in ("unsupported", "refused")
        )
        focus_capability_boundary = (
            step in FOCUS_HANDLE_STEPS
            and verdict == "unsupported"
            and declared.get("act:focus") is False
            and "act:focus" in text
        )
        stale_focus_refusal = (
            step == "act_focus_stale_handle"
            and verdict in ("stale", "not_performed", "refused")
            and "observe again" in text
        )
        wait_ceiling_refusal = (
            step == WAIT_CEILING_STEP
            and verdict == "refused"
            and "1..=2000" in text
        )
        if record.get("required") and not (
            boundary or focus_capability_boundary or stale_focus_refusal
            or wait_ceiling_refusal
        ):
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
        elif focus_capability_boundary:
            notes.append(f"BOUNDARY {step}: act:focus is not declared on {platform}")
        elif stale_focus_refusal:
            notes.append(f"REFUSAL {step}: stale hwnd was named, not guessed")
        elif wait_ceiling_refusal:
            notes.append(f"REFUSAL {step}: over-bound wait was named, no sleep")

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
    #     the journey. No earlier 0.0.6-0.0.12 assertion is weakened here.
    # 0.0.11: recompute the structured accessibility scan. A refused scan
    # must publish an empty tree plus a named reason; a successful scan must
    # publish real rows whose rectangles, roles, depths and bound are all
    # checked here. No cell may report success with an empty tree.
    a11y_roles = {
        "button", "checkbox", "client", "combobox", "dialog", "document",
        "graphic", "group", "label", "list", "listitem", "menubar",
        "menuitem", "separator", "slider", "tab", "tabitem", "textbox",
        "toolbar", "unknown", "window",
    }
    a11y = by_step.get("observe_a11y")
    check(a11y is not None,
          "observe_a11y: required 0.0.11 step is missing")
    check(a11y is not None and a11y.get("verdict") == "performed",
          "observe_a11y: the scan was not performed; BOUNDARY cannot excuse it")
    if a11y and a11y.get("verdict") == "performed":
        ev = a11y.get("evidence", {})
        rows = ev.get("a11y_controls")
        reason = ev.get("a11y_reason")
        source = ev.get("a11y_source")
        check(isinstance(rows, list),
              f"observe_a11y: a11y_controls={rows!r}, expected a list")
        if reason is not None:
            check(isinstance(reason, str) and reason.strip() != "",
                  "observe_a11y: a refused scan needs a non-empty reason")
            check(isinstance(rows, list) and len(rows) == 0,
                  "observe_a11y: a refused scan published control rows")
        else:
            check(isinstance(source, str) and source.strip() != "",
                  "observe_a11y: a successful scan needs its resolved source")
            check(isinstance(rows, list) and len(rows) >= 1,
                  "observe_a11y: a successful scan published no controls")
        if isinstance(rows, list):
            check(len(rows) <= 400,
                  f"observe_a11y: {len(rows)} rows exceed the 400 bound")
            for i, row in enumerate(rows):
                check(isinstance(row, dict),
                      f"observe_a11y: row {i} is not an object")
                if not isinstance(row, dict):
                    continue
                check(row.get("role_name") in a11y_roles,
                      f"observe_a11y: row {i} role_name={row.get('role_name')!r} "
                      f"is outside the coarse role set")
                depth = row.get("depth")
                check(isinstance(depth, int) and not isinstance(depth, bool)
                      and 0 <= depth <= 12,
                      f"observe_a11y: row {i} depth={depth!r} is outside 0..12")
                check(isinstance(row.get("name"), str),
                      f"observe_a11y: row {i} name is not a string")
                # a11y rows use the same [x, y, width, height] array shape as
                # windows[] and controls[], not the {x,y,width,height} object
                # that rect_values() reads for the display-scale rows.
                rect = row.get("rect")
                check(isinstance(rect, list) and len(rect) == 4
                      and all(isinstance(v, int) and not isinstance(v, bool)
                              for v in rect),
                      f"observe_a11y: row {i} rect is not an int rect: "
                      f"{rect!r}")

    # 0.0.14: recompute the normalised a11y node schema. The reader derives
    # state_names from the published raw MSAA bit mask itself; a host that
    # publishes a guessed set, a missing value or a wrong schema fails here.
    # No earlier 0.0.6-0.0.13 assertion is weakened.
    a11y_state_bits = (
        (0x2, "selected"), (0x4, "focused"), (0x8, "pressed"),
        (0x10, "checked"), (0x20, "mixed"), (0x40, "readonly"),
        (0x100, "default"), (0x200, "expanded"), (0x400, "collapsed"),
        (0x800, "busy"), (0x8000, "invisible"), (0x10000, "offscreen"),
        (0x100000, "focusable"), (0x200000, "selectable"),
        (0x400000, "linked"), (0x40000000, "haspopup"),
    )
    a11y_state_names = {name for _, name in a11y_state_bits}
    if a11y and a11y.get("verdict") == "performed":
        ev14 = a11y.get("evidence", {})
        check(ev14.get("a11y_node_schema") == "role-name-state-value-bounds-depth",
              "observe_a11y: normalised node schema is missing or wrong")
        rows14 = ev14.get("a11y_controls")
        if isinstance(rows14, list):
            for i, row in enumerate(rows14):
                if not isinstance(row, dict):
                    continue
                state = row.get("state")
                check(isinstance(state, int) and not isinstance(state, bool)
                      and state >= 0,
                      f"observe_a11y: row {i} state={state!r} is not a bit mask")
                if not isinstance(state, int) or isinstance(state, bool):
                    continue
                expected = [name for bit, name in a11y_state_bits if state & bit]
                names = row.get("state_names")
                check(names == expected,
                      f"observe_a11y: row {i} state_names={names!r} "
                      f"!= recomputed {expected!r}")
                check(isinstance(names, list)
                      and all(n in a11y_state_names for n in names),
                      f"observe_a11y: row {i} state_names has a non-whitelist value")
                check(isinstance(row.get("value"), str),
                      f"observe_a11y: row {i} value is not a string")

    # 0.0.15: a11y-addressed actuation. The reader recomputes the resolved
    # node from the observe_a11y rows published in the same journey, then
    # checks the act evidence against that row. A named tree/capability
    # refusal is admissible; a guessed target or a guessed state set is not.
    sem = by_step.get(A11Y_CONTROL_STEP)
    check(sem is not None,
          f"{A11Y_CONTROL_STEP}: required 0.0.15 step is missing")
    if sem:
        request = sem.get("request") or {}
        check(request.get("source") == "a11y",
              f"{A11Y_CONTROL_STEP}: request did not ask for source=a11y")
        check(request.get("press") is not True,
              f"{A11Y_CONTROL_STEP}: journey must not press a real button")
        ev = sem.get("evidence", {})
        if sem.get("verdict") == "performed":
            check(ev.get("source") == "a11y",
                  f"{A11Y_CONTROL_STEP}: performed without source=a11y evidence")
            check(ev.get("a11y_node_schema") == "role-name-state-value-bounds-depth",
                  f"{A11Y_CONTROL_STEP}: normalised node schema is missing or wrong")
            node = ev.get("a11y_node")
            check(isinstance(node, dict),
                  f"{A11Y_CONTROL_STEP}: performed with no resolved a11y node")
            if isinstance(node, dict):
                check(node.get("name") == request.get("control"),
                      f"{A11Y_CONTROL_STEP}: resolved name={node.get('name')!r} "
                      f"!= requested {request.get('control')!r}")
                if request.get("role") is not None:
                    check(node.get("role_name") == request.get("role"),
                          f"{A11Y_CONTROL_STEP}: role filter {request.get('role')!r} "
                          f"resolved to {node.get('role_name')!r}")
                check(node.get("role_name") in a11y_roles,
                      f"{A11Y_CONTROL_STEP}: node role_name is outside the role set")
                state = node.get("state")
                check(isinstance(state, int) and not isinstance(state, bool) and state >= 0,
                      f"{A11Y_CONTROL_STEP}: node state={state!r} is not a bit mask")
                if isinstance(state, int) and not isinstance(state, bool):
                    check(state & A11Y_STATE_VETO == 0,
                          f"{A11Y_CONTROL_STEP}: selected an unavailable/invisible/offscreen node")
                    expected = [name for bit, name in a11y_state_bits if state & bit]
                    check(node.get("state_names") == expected,
                          f"{A11Y_CONTROL_STEP}: state_names={node.get('state_names')!r} "
                          f"!= recomputed {expected!r}")
                rect = node.get("rect")
                check(isinstance(rect, list) and len(rect) == 4
                      and all(isinstance(v, int) and not isinstance(v, bool) for v in rect)
                      and rect[2] > 0 and rect[3] > 0,
                      f"{A11Y_CONTROL_STEP}: node rect={rect!r} is not a positive int rect")
                check(ev.get("rect") == rect,
                      f"{A11Y_CONTROL_STEP}: act rect={ev.get('rect')!r} != node rect={rect!r}")
                if isinstance(rect, list) and len(rect) == 4:
                    centre = [rect[0] + rect[2] // 2, rect[1] + rect[3] // 2]
                    check(ev.get("center") == centre,
                          f"{A11Y_CONTROL_STEP}: center={ev.get('center')!r} "
                          f"!= recomputed {centre!r}")
                    match = re.search(r"pointer at (-?\d+),\s*(-?\d+)",
                                      ev.get("readback", ""))
                    check(match is not None,
                          f"{A11Y_CONTROL_STEP}: performed with no pointer readback")
                    if match:
                        px, py = int(match.group(1)), int(match.group(2))
                        check(rect[0] <= px < rect[0] + rect[2]
                              and rect[1] <= py < rect[1] + rect[3],
                              f"{A11Y_CONTROL_STEP}: pointer {px},{py} landed outside "
                              f"the resolved node rect {rect!r}")
                check(ev.get("actuation") is None,
                      f"{A11Y_CONTROL_STEP}: a hover published a button receipt")
        else:
            text = sem.get("text", "")
            named = ("no a11y nodes" in text or "no a11y node matches" in text
                     or "unknown a11y role" in text or "unavailable, invisible" in text
                     or "act:control" in text or platform in text)
            check(named,
                  f"{A11Y_CONTROL_STEP}: refusal has no named reason: {text[:120]!r}")
            check(ev.get("a11y_node") in (None, {}),
                  f"{A11Y_CONTROL_STEP}: a refused act published a resolved node")
            notes.append(f"REFUSAL {A11Y_CONTROL_STEP}: named boundary, not counted as [v]")

    # 0.0.16: a11y effect readback. Every assertion above stays unchanged.
    def check_effect_steps():
        """Local scope: no 0.0.16 name may shadow an earlier reader name."""
        # The reader recomputes state_names, the observed whitelist and the
        # verdict from the two published rows; it never trusts the request.
        effect_whitelist = ("name", "rect", "state", "tree", "value")
        effect_refusals = frozenset({
            "no_frame", "stale", "no_a11y_tree", "no_matching_node",
            "ambiguous_identity", "invalid_expect",
        })
        for step in ("observe_a11y_effect_unchanged", "observe_a11y_effect_mismatch"):
            rec = by_step.get(step)
            check(rec is not None, f"{step}: required 0.0.16 step is missing")
            if not rec:
                continue
            req = rec.get("request") or {}
            effect_req = req.get("effect") or {}
            check(isinstance(effect_req, dict), f"{step}: request has no effect object")
            check(req.get("a11y") == effect_req.get("hwnd"),
                  f"{step}: effect.hwnd must be the scanned window handle")
            declared = effect_req.get("expect")
            check(isinstance(declared, list)
                  and all(field in effect_whitelist for field in declared)
                  and declared == sorted(set(declared)),
                  f"{step}: expect must be a sorted de-duplicated whitelist")
            ev = rec.get("evidence", {})
            row = ev.get("a11y_effect")
            check(isinstance(row, dict), f"{step}: no a11y_effect row published")
            if not isinstance(row, dict):
                continue
            obs = row.get("observed")
            check(isinstance(obs, list)
                  and all(field in effect_whitelist for field in obs)
                  and obs == sorted(set(obs)),
                  f"{step}: observed={obs!r} is not a sorted whitelist")
            verdict = row.get("verdict")
            check(verdict in ("confirmed", "unchanged", "mismatch", "vanished", "refused"),
                  f"{step}: unknown effect verdict {verdict!r}")
            tree = ev.get("a11y_controls")
            if isinstance(tree, list) and not tree:
                check(verdict == "refused",
                      f"{step}: a non-refused effect row was published on an empty a11y tree")
            if verdict == "refused":
                # A refusal publishes no evidence rows at all, so expect/hwnd
                # agreement is checked only on a row that claims a verdict.
                check(row.get("reason") in effect_refusals,
                      f"{step}: unnamed effect refusal {row.get('reason')!r}")
                check(row.get("before") is None and row.get("after") is None,
                      f"{step}: a refusal published before/after rows")
                check(obs == [], f"{step}: a refusal published observed={obs!r}")
                notes.append(f"REFUSAL {step}: {row.get('reason')}, not counted as [v]")
                continue
            check(isinstance(declared, list) and row.get("expect") == sorted(set(declared)),
                  f"{step}: published expect does not match the pre-declared one")
            check(row.get("hwnd") == effect_req.get("hwnd"),
                  f"{step}: published hwnd does not match the request")
            before, after = row.get("before"), row.get("after")
            check(isinstance(before, dict), f"{step}: no before row")
            for side, label in ((before, "before"), (after, "after")):
                if not isinstance(side, dict):
                    continue
                state = side.get("state")
                check(isinstance(state, int) and not isinstance(state, bool) and state >= 0,
                      f"{step}: {label} state={state!r} is not a bit mask")
                names = ([name for bit, name in a11y_state_bits if state & bit]
                         if isinstance(state, int) and not isinstance(state, bool) else None)
                check(side.get("state_names") == names,
                      f"{step}: {label} state_names != recomputed {names!r}")
                rect = side.get("rect")
                check(isinstance(rect, list) and len(rect) == 4
                      and all(isinstance(v, int) and not isinstance(v, bool) for v in rect)
                      and rect[2] > 0 and rect[3] > 0,
                      f"{step}: {label} rect={rect!r} is not a positive int rect")
            if verdict == "vanished":
                check(after is None and obs == [],
                      f"{step}: vanished must publish after=null and observed=[]")
            elif isinstance(before, dict) and isinstance(after, dict):
                recomputed = sorted(
                    field for field in effect_whitelist
                    if field != "tree" and before.get(field) != after.get(field)
                )
                tree_row = row.get("tree") or {}
                if tree_row.get("changed") is True and "tree" not in recomputed:
                    recomputed = sorted(recomputed + ["tree"])
                check(obs == recomputed,
                      f"{step}: observed={obs!r} != recomputed {recomputed!r}")
                published_expect = row.get("expect")
                expected_verdict = (
                    "confirmed" if recomputed and recomputed == published_expect
                    else "unchanged" if not recomputed and not published_expect
                    else "mismatch"
                )
                check(verdict == expected_verdict,
                      f"{step}: verdict={verdict!r} != recomputed {expected_verdict!r}")
                if verdict == "confirmed":
                    check(obs == published_expect and bool(obs),
                          f"{step}: confirmed without a matching non-empty expect")
                check(row.get("before_generation") == rec.get("generation", 0) - 1,
                      f"{step}: before_generation is not the dispatch generation")
                check(row.get("after_generation") == rec.get("generation"),
                      f"{step}: after_generation is not the fresh generation")
    
    check_effect_steps()

    # 0.0.12: recompute handle-addressed focus. A performed title or handle
    # focus must publish a desktop-read target_hwnd; the handle steps must be
    # required on every cell, and the stale handle must be absent from the live
    # window list. A cell whose matrix declares act:focus unsupported may only
    # produce a named capability boundary for the two handle steps. No earlier
    # 0.0.6-0.0.11 assertion is weakened here.
    for step in FOCUS_REQUIRED_STEPS:
        check(step in by_step, f"{step}: required 0.0.12 focus step is missing")
    for focus_step in FOCUS_TARGET_STEPS:
        focus_record = by_step.get(focus_step)
        if not focus_record or focus_record.get("verdict") != "performed":
            continue
        ev = focus_record.get("evidence", {})
        check("target_hwnd" in ev, f"{focus_step}: missing target_hwnd evidence")
        target = ev.get("target_hwnd")
        reason = ev.get("target_hwnd_reason")
        if target is None:
            check(isinstance(reason, str) and reason.strip() != "",
                  f"{focus_step}: null target_hwnd needs a named reason")
        else:
            check(isinstance(target, int) and not isinstance(target, bool)
                  and target > 0,
                  f"{focus_step}: target_hwnd={target!r} is not a live handle")
            check(reason in (None, ""),
                  f"{focus_step}: a real target_hwnd must not carry a reason")
            check(f"target_hwnd={target}" in ev.get("readback", ""),
                  f"{focus_step}: readback does not publish target_hwnd={target}")
    for focus_step, observe_step in FOCUS_READBACK_PAIRS:
        focus_record = by_step.get(focus_step)
        if not focus_record or focus_record.get("verdict") != "performed":
            continue
        target = focus_record.get("evidence", {}).get("target_hwnd")
        if not isinstance(target, int):
            continue
        observation = by_step.get(observe_step)
        check(observation is not None
              and observation.get("verdict") == "performed",
              f"{focus_step}: {observe_step} is not a performed observation")
        if observation and observation.get("verdict") == "performed":
            observed_windows = observation.get("evidence", {}).get("windows", [])
            focused = [w for w in observed_windows
                       if isinstance(w, dict) and w.get("focused") is True]
            check(any(w.get("hwnd") == target for w in focused),
                  f"{focus_step}: target_hwnd={target} is not the focused "
                  f"window in {observe_step}")
    by_handle = by_step.get("act_focus_by_handle")
    if by_handle:
        req = by_handle.get("request", {})
        check(isinstance(req, dict) and "hwnd" in req and "window" not in req,
              "act_focus_by_handle: request must carry exactly one hwnd target")
        handle = req.get("hwnd") if isinstance(req, dict) else None
        check(isinstance(handle, int) and not isinstance(handle, bool)
              and handle > 0,
              f"act_focus_by_handle: hwnd={handle!r} is not a positive integer")
        if declared.get("act:focus") is True:
            check(by_handle.get("verdict") == "performed",
                  "act_focus_by_handle: a declared focus cell must perform the step")
            check(any(isinstance(w, dict) and w.get("hwnd") == handle
                      for w in windows),
                  f"act_focus_by_handle: hwnd={handle!r} is not in the live window list")
    stale = by_step.get("act_focus_stale_handle")
    if stale:
        req = stale.get("request", {})
        check(isinstance(req, dict) and "hwnd" in req and "window" not in req,
              "act_focus_stale_handle: request must carry exactly one hwnd target")
        handle = req.get("hwnd") if isinstance(req, dict) else None
        check(isinstance(handle, int) and not isinstance(handle, bool)
              and handle > 0,
              f"act_focus_stale_handle: hwnd={handle!r} is not a positive integer")
        check(all(not isinstance(w, dict) or w.get("hwnd") != handle
                  for w in windows),
              f"act_focus_stale_handle: hwnd={handle!r} is actually a live window")
        if declared.get("act:focus") is True:
            check(stale.get("verdict") in ("stale", "not_performed", "refused"),
                  "act_focus_stale_handle: a stale handle must be a named refusal")

    # 0.0.13: recompute the bounded observation wait. A performed wait must
    # publish monotonic elapsed time, exact cumulative accounting, a call
    # count and a generation greater than the previous performed observation.
    # The over-bound step must be a named refusal with no wait evidence and
    # no budget change. No earlier 0.0.6-0.0.12 assertion is weakened here.
    for step in WAIT_REQUIRED_STEPS:
        check(step in by_step, f"{step}: required 0.0.13 wait step is missing")
    ordered_observations = [
        record for record in records
        if record["step"].startswith("observe")
        and record.get("verdict") == "performed"
    ]
    wait_rows = []
    # 0.0.17: the invoke-effect observation performs an optional wait that
    # precedes observe_wait_after_act. When present it must be part of the
    # same cumulative budget arithmetic; when skipped nothing changes.
    early = by_step.get("observe_a11y_effect_invoke")
    if early and early.get("verdict") == "performed" and early.get("evidence", {}).get("wait"):
        wait_rows.append((
            "observe_a11y_effect_invoke",
            early.get("request", {}).get("wait_ms"),
            early["evidence"]["wait"].get("elapsed_ms"),
            early["evidence"]["wait"].get("cumulative_ms"),
            early["evidence"]["wait"].get("wait_calls"),
            early,
        ))
    for step in WAIT_PERFORMED_STEPS:
        record = by_step.get(step)
        check(record is not None and record.get("verdict") == "performed",
              f"{step}: required wait observation was not performed")
        if not record or record.get("verdict") != "performed":
            continue
        request = record.get("request", {})
        evidence_wait = record.get("evidence", {}).get("wait")
        check(isinstance(evidence_wait, dict),
              f"{step}: missing wait evidence")
        if not isinstance(evidence_wait, dict):
            continue
        requested = request.get("wait_ms")
        elapsed = evidence_wait.get("elapsed_ms")
        cumulative = evidence_wait.get("cumulative_ms")
        calls = evidence_wait.get("wait_calls")
        check(isinstance(requested, int) and not isinstance(requested, bool)
              and 1 <= requested <= MAX_WAIT_MS,
              f"{step}: wait_ms={requested!r} is outside 1..{MAX_WAIT_MS}")
        check(evidence_wait.get("requested_ms") == requested,
              f"{step}: requested_ms does not match the request")
        check(isinstance(elapsed, int) and not isinstance(elapsed, bool)
              and elapsed > 0,
              f"{step}: elapsed_ms={elapsed!r} is not a positive integer")
        if isinstance(requested, int) and isinstance(elapsed, int):
            check(requested <= elapsed <= requested + WAIT_OVERHEAD_MS,
                  f"{step}: elapsed_ms={elapsed} outside "
                  f"{requested}..{requested + WAIT_OVERHEAD_MS}")
        check(isinstance(cumulative, int) and not isinstance(cumulative, bool)
              and 0 <= cumulative <= MAX_CUMULATIVE_WAIT_MS,
              f"{step}: cumulative_ms={cumulative!r} is outside the D4 budget")
        check(isinstance(calls, int) and not isinstance(calls, bool)
              and 1 <= calls <= MAX_WAIT_CALLS,
              f"{step}: wait_calls={calls!r} is outside the D4 budget")
        check(evidence_wait.get("generation") == record.get("generation"),
              f"{step}: wait generation does not match the result generation")
        wait_rows.append((step, requested, elapsed, cumulative, calls, record))
    for index, row in enumerate(wait_rows, start=1):
        step, requested, elapsed, cumulative, calls, record = row
        expected_cumulative = sum(entry[2] for entry in wait_rows[:index])
        check(cumulative == expected_cumulative,
              f"{step}: cumulative_ms={cumulative} != "
              f"sum of elapsed waits {expected_cumulative}")
        check(calls == index,
              f"{step}: wait_calls={calls}, expected {index}")
        position = next(
            (i for i, observation in enumerate(ordered_observations)
             if observation["step"] == step),
            None,
        )
        check(position is not None and position > 0,
              f"{step}: no preceding performed observation")
        if position is not None and position > 0:
            previous = ordered_observations[position - 1]
            check(record.get("generation", 0) > previous.get("generation", 0),
                  f"{step}: wait generation {record.get('generation')} did not "
                  f"advance past {previous['step']} generation "
                  f"{previous.get('generation')}")
    over_bound = by_step.get(WAIT_CEILING_STEP)
    if over_bound:
        check(over_bound.get("verdict") == "refused",
              f"{WAIT_CEILING_STEP}: an over-bound wait must be refused")
        check(over_bound.get("request", {}).get("wait_ms") == MAX_WAIT_MS + 1,
              f"{WAIT_CEILING_STEP}: request must use wait_ms={MAX_WAIT_MS + 1}")
        check("1..=2000" in over_bound.get("text", ""),
              f"{WAIT_CEILING_STEP}: refusal did not name the 1..=2000 bound")
        check("wait" not in over_bound.get("evidence", {}),
              f"{WAIT_CEILING_STEP}: a refused wait published wait evidence")
    final = by_step.get("disclose_final")
    check(final is not None and final.get("verdict") == "performed",
          "disclose_final: required final disclosure is missing")
    if final and final.get("verdict") == "performed":
        budget = final.get("evidence", {}).get("wait_budget")
        check(isinstance(budget, dict),
              "disclose_final: missing wait_budget evidence")
        if isinstance(budget, dict):
            expected_cumulative = sum(row[2] for row in wait_rows)
            check(budget.get("cumulative_ms") == expected_cumulative,
                  "disclose_final: cumulative_ms does not match the waits")
            check(budget.get("wait_calls") == len(wait_rows),
                  "disclose_final: wait_calls does not match the waits")
            check(budget.get("max_cumulative_ms") == MAX_CUMULATIVE_WAIT_MS,
                  "disclose_final: wrong cumulative ceiling")
            check(budget.get("max_wait_calls") == MAX_WAIT_CALLS,
                  "disclose_final: wrong wait-call ceiling")
            check(budget.get("remaining_ms")
                  == MAX_CUMULATIVE_WAIT_MS - expected_cumulative,
                  "disclose_final: wrong remaining_ms")
            check(budget.get("remaining_calls")
                  == MAX_WAIT_CALLS - len(wait_rows),
                  "disclose_final: wrong remaining_calls")

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
