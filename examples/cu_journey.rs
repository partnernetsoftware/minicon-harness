//! Cross-platform computer-use journey driver, run by GitHub Actions.
//!
//! This drives the **real compiled backend** through `CuHost::invoke_json` with
//! `--allow-screen` semantics (`ScreenTool::new(root, true)`), writes one JSON
//! record per verb, and never decides its own success: `scripts/cu-journey-
//! verify.py` is an independent reader of that file. A green `cargo check` is
//! not a desktop journey; this is.

use minicon_harness::cu::CuHost;
use minicon_harness::screen::{platform_capabilities, platform_name, ScreenTool};
use serde_json::{json, Value};
use std::path::PathBuf;

struct Journey {
    host: CuHost,
    #[allow(dead_code)]
    root: PathBuf,
    records: Vec<Value>,
}

impl Journey {
    fn run(&mut self, step: &str, capability: &str, declared: bool, required: bool, request: Value) -> Value {
        eprintln!("cu-journey step={step} capability={capability} declared={declared}");
        let result = self.host.invoke_json(&request);
        let record = json!({
            "step": step,
            "capability": capability,
            "declared": declared,
            "required": required,
            "verdict": result.verdict.tag(),
            "generation": result.generation,
            "text": result.text,
            "evidence": result.evidence,
            "request": request,
        });
        eprintln!(
            "cu-journey step={step} verdict={}",
            result.verdict.tag()
        );
        self.records.push(record.clone());
        record
    }

    fn skip(&mut self, step: &str, capability: &str, reason: &str) {
        self.records.push(json!({
            "step": step,
            "capability": capability,
            "declared": false,
            "required": false,
            "verdict": "skipped",
            "generation": 0,
            "text": reason,
            "evidence": json!({ "reason": reason }),
            "request": Value::Null,
        }));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = PathBuf::from("cu-journey-root");
    let mut out = PathBuf::from("cu-journey.json");
    let mut wanted_window: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--root" => root = PathBuf::from(value),
            "--out" => out = PathBuf::from(value),
            "--window" => wanted_window = Some(value),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    std::fs::create_dir_all(&root)?;
    let caps = platform_capabilities();
    let mut journey = Journey {
        host: CuHost::new(ScreenTool::new(&root, true)),
        root: root.clone(),
        records: Vec::new(),
    };

    let disclosed = journey.run(
        "disclose",
        "disclose",
        true,
        true,
        json!({ "op": "disclose" }),
    );
    let matrix = disclosed["evidence"]["platform_capabilities"].clone();

    let observed = journey.run(
        "observe",
        "observe",
        caps.observe,
        true,
        json!({ "op": "observe" }),
    );
    let evidence = &observed["evidence"];
    let dx = evidence["desktop"]["x"].as_i64().unwrap_or(0);
    let dy = evidence["desktop"]["y"].as_i64().unwrap_or(0);
    let dw = evidence["desktop"]["width"].as_i64().unwrap_or(0);
    let dh = evidence["desktop"]["height"].as_i64().unwrap_or(0);
    let x = (dx + (dw / 4).min(200)) as i32;
    let y = (dy + (dh / 4).min(150)) as i32;
    let to_x = x + 80.min((dw as i32).saturating_sub(x + dx as i32).max(10));
    let to_y = y + 60.min((dh as i32).saturating_sub(y + dy as i32).max(10));
    let title = wanted_window.or_else(|| {
        evidence["windows"]
            .as_array()
            .and_then(|windows| windows.first())
            .and_then(|window| window["title"].as_str())
            .map(str::to_owned)
    });
    // 0.0.11: a handle is the only unambiguous window identity when two
    // windows share a title. The a11y scan is addressed by that handle, never
    // by guessing which foreground window the runner happens to have.
    let target_hwnd = evidence["windows"]
        .as_array()
        .and_then(|windows| windows.first())
        .and_then(|window| window["hwnd"].as_u64());

    if let Some(hwnd) = target_hwnd {
        journey.run(
            "observe_a11y",
            "observe",
            caps.observe,
            true,
            json!({ "op": "observe", "a11y": hwnd }),
        );
    } else {
        journey.skip("observe_a11y", "observe", "no live window handle to scan");
    }

    if caps.capture {
        journey.run(
            "observe_capture",
            "observe",
            true,
            true,
            json!({ "op": "observe", "capture": true, "scale": 2 }),
        );
    } else {
        journey.skip("observe_capture", "observe", "capture arm is not implemented on this cell");
    }

    let pointer_steps: Vec<(&str, &str, bool, Value)> = vec![
        ("act_hover", "act:hover", caps.act_hover, json!({ "op": "act", "why": "CU journey: hover readback", "action": "hover", "x": x, "y": y })),
        ("act_click", "act:click", caps.act_click, json!({ "op": "act", "why": "CU journey: click readback", "action": "click", "x": x, "y": y, "button": "left" })),
        ("act_drag", "act:drag", caps.act_drag, json!({ "op": "act", "why": "CU journey: drag readback", "action": "drag", "x": x, "y": y, "to_x": to_x, "to_y": to_y, "button": "left" })),
        ("act_scroll", "act:scroll", caps.act_scroll, json!({ "op": "act", "why": "CU journey: scroll readback", "action": "scroll", "x": x, "y": y, "delta": 120 })),
    ];
    for (step, capability, declared, request) in pointer_steps {
        if declared {
            journey.run(step, capability, true, true, request);
        } else {
            journey.skip(step, capability, "verb is not implemented on this cell");
        }
    }

    // 0.0.6 interaction closure: middle / double / middle-drag were in the
    // wire schema but never driven by CI. Every one must leave an actuation
    // receipt; the independent reader checks it against the request.
    let button_steps: Vec<(&str, &str, bool, Value)> = vec![
        ("act_click_middle", "act:click", caps.act_click, json!({ "op": "act", "why": "CU journey: middle button receipt", "action": "click", "x": x, "y": y, "button": "middle" })),
        ("act_click_double", "act:click", caps.act_click, json!({ "op": "act", "why": "CU journey: double click receipt", "action": "click", "x": x, "y": y, "button": "left", "double": true })),
        ("act_drag_middle", "act:drag", caps.act_drag, json!({ "op": "act", "why": "CU journey: middle drag receipt", "action": "drag", "x": x, "y": y, "to_x": to_x, "to_y": to_y, "button": "middle" })),
    ];
    for (step, capability, declared, request) in button_steps {
        if declared {
            journey.run(step, capability, true, true, request);
        } else {
            journey.skip(step, capability, "verb is not implemented on this cell");
        }
    }

    // 0.0.7 richer actuation: one multi-segment drag and both wheel axes.
    // Every point is a convex combination of the two already-admitted
    // pointer coordinates, so the whole path stays inside the virtual
    // desktop on every runner. The last point is the readback target.
    let mid_x = (x + to_x) / 2;
    let mid_y = (y + to_y) / 2;
    let richer_steps: Vec<(&str, &str, bool, Value)> = vec![
        ("act_drag_path", "act:drag", caps.act_drag, json!({ "op": "act", "why": "CU journey: multi-point drag path", "action": "drag", "x": x, "y": y, "path": [[x, y], [mid_x, mid_y], [to_x, to_y]] })),
        ("act_scroll_x", "act:scroll", caps.act_scroll, json!({ "op": "act", "why": "CU journey: horizontal wheel axis", "action": "scroll", "x": x, "y": y, "delta_x": 120, "delta_y": 0 })),
        ("act_scroll_y", "act:scroll", caps.act_scroll, json!({ "op": "act", "why": "CU journey: vertical wheel axis with the new spelling", "action": "scroll", "x": x, "y": y, "delta_x": 0, "delta_y": -120 })),
    ];
    for (step, capability, declared, request) in richer_steps {
        if declared {
            journey.run(step, capability, true, true, request);
        } else {
            journey.skip(step, capability, "verb is not implemented on this cell");
        }
    }

    if caps.act_type {
        journey.run(
            "act_type",
            "act:type",
            true,
            true,
            json!({ "op": "act", "why": "CU journey: typed text", "action": "type", "text": "cu 0.0.7" }),
        );
    } else {
        journey.skip("act_type", "act:type", "verb is not implemented on this cell");
    }
    if caps.act_key {
        journey.run(
            "act_key",
            "act:key",
            true,
            true,
            json!({ "op": "act", "why": "CU journey: key combo", "action": "key", "keys": "ctrl+a" }),
        );
    } else {
        journey.skip("act_key", "act:key", "verb is not implemented on this cell");
    }

    // 0.0.8 held-key modifiers: hold one whitelisted modifier, read the
    // host's own state back, release it, and prove the set is empty again.
    // A modifier is keyboard state, never a button actuation receipt.
    if caps.act_key {
        let held_steps: Vec<(&str, &str, Value)> = vec![
            ("act_key_hold", "act:key", json!({ "op": "act", "why": "CU journey: hold one modifier", "action": "key", "keys": "ctrl", "hold": true })),
            ("disclose_held", "disclose", json!({ "op": "disclose" })),
            ("act_key_release", "act:key", json!({ "op": "act", "why": "CU journey: release held modifiers", "action": "key", "keys": "ctrl", "release": true })),
            ("disclose_released", "disclose", json!({ "op": "disclose" })),
        ];
        for (step, capability, request) in held_steps {
            journey.run(step, capability, true, true, request);
        }
    } else {
        for step in ["act_key_hold", "disclose_held", "act_key_release", "disclose_released"] {
            journey.skip(step, "act:key", "verb is not implemented on this cell");
        }
    }

    if caps.act_focus {
        if let Some(window) = &title {
            journey.run(
                "act_focus",
                "act:focus",
                true,
                true,
                json!({ "op": "act", "why": "CU journey: focus readback", "action": "focus", "window": window }),
            );
        } else {
            journey.skip("act_focus", "act:focus", "the session has no named window to focus");
        }
    } else {
        journey.skip("act_focus", "act:focus", "verb is not implemented on this cell");
    }

    if caps.act_move {
        if let Some(window) = &title {
            journey.run(
                "act_move",
                "act:move",
                true,
                true,
                json!({ "op": "act", "why": "CU journey: window rect readback", "action": "move", "window": window, "x": dx + 40, "y": dy + 40, "width": 520, "height": 360 }),
            );
        } else {
            journey.skip("act_move", "act:move", "the session has no named window to move");
        }
    } else {
        journey.skip("act_move", "act:move", "verb is not implemented on this cell");
    }

    // The typed control arm is Win32-only. It is recorded best-effort: a
    // session whose foreground window draws its own widgets honestly reports
    // zero controls, and that is a named result, not a skipped assertion.
    if caps.controls {
        let scanned = journey.run(
            "observe_controls",
            "observe",
            true,
            false,
            json!({ "op": "observe", "controls": true }),
        );
        let label = scanned["evidence"]["controls"]
            .as_array()
            .and_then(|controls| controls.first())
            .and_then(|control| control["label"].as_str())
            .map(str::to_owned);
        if let Some(label) = label {
            journey.run(
                "act_control",
                "act:control",
                true,
                false,
                json!({ "op": "act", "why": "CU journey: named control target", "action": "control", "control": label.clone() }),
            );
            // A real press on the named control, not only a hover. Windows is
            // the only cell that declares act:control; its edit control is a
            // harmless target. If the runner has no child window this step is
            // skipped by a declared capability and the reader fails the gate.
            journey.run(
                "act_control_press",
                "act:control",
                true,
                true,
                json!({ "op": "act", "why": "CU journey: real click on a named control", "action": "control", "control": label, "press": true }),
            );
        } else {
            journey.skip("act_control", "act:control", "no typed child control in this session");
        }
    } else {
        journey.skip("observe_controls", "observe", "controls arm is not implemented on this cell");
        journey.skip("act_control", "act:control", "verb is not implemented on this cell");
    }

    // Right-click goes last among clicks: a context menu may open. A refocus
    // plus a live final observation prove the session survived it.
    if caps.act_click {
        journey.run(
            "act_click_right",
            "act:click",
            true,
            true,
            json!({ "op": "act", "why": "CU journey: right button receipt", "action": "click", "x": x, "y": y, "button": "right" }),
        );
    } else {
        journey.skip("act_click_right", "act:click", "verb is not implemented on this cell");
    }
    if caps.act_focus {
        if let Some(window) = &title {
            journey.run(
                "act_refocus_after_right_click",
                "act:focus",
                true,
                true,
                json!({ "op": "act", "why": "CU journey: restore focus after a context menu", "action": "focus", "window": window }),
            );
        }
    }

    journey.run(
        "observe_final",
        "observe",
        caps.observe,
        true,
        json!({ "op": "observe" }),
    );

    // The journey must end with an empty held set, not with a modifier stuck
    // on a shared CI runner. `CuHost::drop` releases again after this read.
    journey.run(
        "disclose_final",
        "disclose",
        true,
        true,
        json!({ "op": "disclose" }),
    );

    // 0.0.9: a read-only, non-elevating disclosure of the session's own
    // preconditions. It requests no permission, posts no input and publishes
    // no identifying strings, so it stays required on every OS even when an
    // OS grant blocks a later effect. The independent reader recomputes the
    // environment shape and every typed grant row from this record.
    journey.run(
        "disclose_grants",
        "disclose",
        true,
        true,
        json!({ "op": "disclose" }),
    );

    // 0.0.10: a read-only disclosure of each display's coordinate basis. It
    // changes no display mode or DPI and posts no input, so it stays required
    // on all three OSes even when an OS grant blocks a later effect. The
    // writer only records the rows the host already published -- it decides
    // no verdict. The independent reader recomputes every typed scale, the
    // exact physical = logical * scale relation and the reason rule.
    let scales = journey.run(
        "disclose_scales",
        "disclose",
        true,
        true,
        json!({ "op": "disclose" }),
    );
    if let Some(rows) = scales["evidence"]["environment"]["display_scales"].as_array() {
        for row in rows {
            eprintln!(
                "cu-journey disclose_scales index={} scale={}",
                row["index"], row["scale"]
            );
        }
    }

    // The writer does not compute a verdict. The independent reader recomputes
    // everything from `records`; `summary` is informational only.
    let performed = journey
        .records
        .iter()
        .filter(|record| record["verdict"] == "performed")
        .count();
    let document = json!({
        "schema": 1,
        "platform": platform_name(),
        "root": root.to_string_lossy(),
        "matrix": matrix,
        "desktop": { "x": dx, "y": dy, "width": dw, "height": dh },
        "pointer": { "x": x, "y": y, "to_x": to_x, "to_y": to_y },
        "records": journey.records,
        "summary": { "records": performed },
    });
    std::fs::write(&out, serde_json::to_string_pretty(&document)?)?;
    println!(
        "cu-journey platform={} records={} performed={} out={}",
        platform_name(),
        document["records"].as_array().map(Vec::len).unwrap_or(0),
        performed,
        out.display()
    );
    Ok(())
}
