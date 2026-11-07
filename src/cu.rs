//! # computer-use base: a versioned host capability contract
//!
//! `screen.rs` owns the native mechanisms. This module owns the **contract**
//! above them: a versioned ABI, a discoverable capability manifest, typed
//! inputs and outputs, observation generations and an evidence receipt per
//! call. It is the seam a later dynamic-logic engine (`unisacc`/`ujs`, per
//! `minicon/prd/PRD_02_31_01_04_dynamic_logic.md` CONTRACT) can bind to
//! without being granted any authority of its own.
//!
//! The host invariants, non-negotiable here:
//!
//! 1. **The host owns authority.** `--allow-screen` and platform qualification
//!    are checked by the host before any native effect. A caller cannot widen
//!    them by supplying a different ABI level, a capability name or an
//!    `as_of` generation.
//! 2. **Everything is disclosed before it is used.** `op=disclose` returns the
//!    ABI, qualification, grant, capability manifest and recent receipts.
//! 3. **A call is typed, named and bounded.** Unknown capabilities, bad
//!    parameter types and oversized requests are refused before a device
//!    context is created (CU0 in `minicon/prd/PRD_02_38_computer_use.md`).
//! 4. **An observation has an age.** Each successful observe creates a new
//!    generation. An `act` may pin `as_of`; a generation that no longer
//!    matches is `stale` and is never replayed (CU2: no ambiguous replay).
//! 5. **A result is one of performed / not performed / unknown.** A native
//!    API returning is not evidence; readback is. An input whose foreground
//!    changed is `unknown`, not a success.
//!
//! No crate is added: `serde_json` is already one of the two dependencies,
//! and the state is plain `std::sync::Mutex`. Non-Windows cells keep the same
//! typed contract and refuse by name -- the ABI does not promise a desktop.

use crate::screen::{
    resolve_a11y_control, resolve_control, Act, Actuation, A11yTarget, CaptureTarget,
    ClickButton, ControlInfo, ControlTarget, FocusTarget,
    DisplayScaleProbe, EnvironmentProbe, GrantProbe, Modifier, ScreenSnapshot, ScreenTool,
    WindowInfo, SCREEN_TOOL_HELD_MODIFIERS, SCREEN_TOOL_MAX_CAPTURE_SCALE,
    SCREEN_TOOL_MAX_HELD_MODIFIERS, SCREEN_TOOL_MAX_TYPE_CHARS,
};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Major mismatch is refused before activation. A newer minor level is
/// accepted but cannot widen authority: unknown optional fields are ignored.
pub const CU_ABI_MAJOR: u32 = 1;
pub const CU_ABI_MINOR: u32 = 0;

/// Receipts retained in memory per invocation. A disclosure is a snapshot,
/// not an unbounded log.
pub const CU_MAX_RECEIPTS: usize = 32;

/// 0.0.13 D4 ceilings for the bounded `observe.wait_ms` parameter. A single
/// sleep is bounded, the cumulative budget is per host, and a refused wait
/// sleeps zero milliseconds.
const SCREEN_TOOL_MAX_WAIT_MS: u32 = 2000;
const SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS: u64 = 8000;
const SCREEN_TOOL_MAX_WAIT_CALLS: u32 = 8;
const SCREEN_TOOL_WAIT_OVERHEAD_MS: u64 = 500;

/// Which half of the contract a capability belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CuKind {
    Observe,
    Act,
    Disclose,
}

impl CuKind {
    fn tag(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Act => "act",
            Self::Disclose => "disclose",
        }
    }
}

/// The typed inputs a capability accepts. This is the shape a dynamic caller
/// validates against; the JSON wirecodec is only an adapter over it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CuType {
    Bool,
    Int,
    String,
    /// `true`, a window title, or a `{x,y,width,height}` object.
    Capture,
    /// `true` scans the foreground window, a string names one window.
    ControlScan,
    /// `true` scans the foreground window, an integer names a live hwnd.
    A11yScan,
    Button,
    /// An array of `[x,y]` integer pairs, bounded by the drag path limit.
    Path,
    /// 0.0.16: an `effect` object naming one a11y node identity plus a
    /// pre-declared `expect` whitelist. It is evidence, not a new verb.
    Effect,
}

impl CuType {
    fn tag(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::String => "string",
            Self::Capture => "capture",
            Self::ControlScan => "control-scan",
            Self::A11yScan => "a11y-scan",
            Self::Button => "button",
            Self::Path => "int-pair-array",
            Self::Effect => "effect-object",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CuParam {
    pub name: &'static str,
    pub ty: CuType,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CuCapability {
    pub name: &'static str,
    pub kind: CuKind,
    pub summary: &'static str,
    pub params: &'static [CuParam],
}

const P_WINDOW: CuParam = CuParam { name: "window", ty: CuType::String, required: false };
const P_CAPTURE: CuParam = CuParam { name: "capture", ty: CuType::Capture, required: false };
const P_CONTROLS: CuParam =
    CuParam { name: "controls", ty: CuType::ControlScan, required: false };
const P_A11Y: CuParam = CuParam { name: "a11y", ty: CuType::A11yScan, required: false };
const P_CONTROL: CuParam = CuParam { name: "control", ty: CuType::String, required: true };
const P_PRESS: CuParam = CuParam { name: "press", ty: CuType::Bool, required: false };
// 0.0.15: `source` and `role` are additive parameters of act:control. A
// semantic MSAA node is addressed by identity from the current generation's
// a11y tree; `controls` stays the default so every 0.0.6-0.0.14 call is
// byte-for-byte unchanged.
const P_SOURCE: CuParam = CuParam { name: "source", ty: CuType::String, required: false };
const P_ROLE: CuParam = CuParam { name: "role", ty: CuType::String, required: false };
const P_SCALE: CuParam = CuParam { name: "scale", ty: CuType::Int, required: false };
const P_WAIT_MS: CuParam = CuParam { name: "wait_ms", ty: CuType::Int, required: false };
/// 0.0.16: one predicted effect, declared before the act it verifies.
const P_EFFECT: CuParam = CuParam { name: "effect", ty: CuType::Effect, required: false };
const P_WHY: CuParam = CuParam { name: "why", ty: CuType::String, required: true };
const P_X: CuParam = CuParam { name: "x", ty: CuType::Int, required: true };
const P_Y: CuParam = CuParam { name: "y", ty: CuType::Int, required: true };
// `to_x`/`to_y` are required only when `path` is absent; `parse_request`
// enforces that conditional, so disclose must not advertise them as required.
const P_TO_X: CuParam = CuParam { name: "to_x", ty: CuType::Int, required: false };
const P_TO_Y: CuParam = CuParam { name: "to_y", ty: CuType::Int, required: false };
const P_WIDTH: CuParam = CuParam { name: "width", ty: CuType::Int, required: true };
const P_HEIGHT: CuParam = CuParam { name: "height", ty: CuType::Int, required: true };
// `act:focus` accepts exactly one of `window` or `hwnd`; parse_request
// enforces that mutual exclusion, so disclose must not advertise either as
// unconditionally required.
const P_TITLE: CuParam = CuParam { name: "window", ty: CuType::String, required: false };
const P_HWND: CuParam = CuParam { name: "hwnd", ty: CuType::Int, required: false };
const P_BUTTON: CuParam = CuParam { name: "button", ty: CuType::Button, required: false };
const P_DOUBLE: CuParam = CuParam { name: "double", ty: CuType::Bool, required: false };
const P_TEXT: CuParam = CuParam { name: "text", ty: CuType::String, required: true };
const P_KEYS: CuParam = CuParam { name: "keys", ty: CuType::String, required: true };
// `delta` stays as the vertical alias; either spelling (or only `delta_x`)
// is admitted, so none of the three is unconditionally required.
const P_DELTA: CuParam = CuParam { name: "delta", ty: CuType::Int, required: false };
const P_DELTA_X: CuParam = CuParam { name: "delta_x", ty: CuType::Int, required: false };
const P_DELTA_Y: CuParam = CuParam { name: "delta_y", ty: CuType::Int, required: false };
const P_PATH: CuParam = CuParam { name: "path", ty: CuType::Path, required: false };
// `hold` / `release` are additive parameters of `act:key`, not new verbs.
const P_HOLD: CuParam = CuParam { name: "hold", ty: CuType::Bool, required: false };
const P_RELEASE: CuParam = CuParam { name: "release", ty: CuType::Bool, required: false };

/// The single source of truth for what computer use can do here. The wire
/// tool definition's action enum is tested against this table so the two
/// cannot drift.
pub const CAPABILITIES: &[CuCapability] = &[
    CuCapability {
        name: "observe",
        kind: CuKind::Observe,
        summary: "Read virtual-desktop geometry, displays and windows; optionally publish one bounded PNG, scan one window's typed controls, take one bounded wait before a fresh snapshot, or compare one a11y node against a pre-declared effect expectation.",
        params: &[P_WINDOW, P_CAPTURE, P_CONTROLS, P_A11Y, P_SCALE, P_WAIT_MS, P_EFFECT],
    },
    CuCapability {
        name: "disclose",
        kind: CuKind::Disclose,
        summary: "Report the ABI, qualification, grant, capability manifest, generation and receipts.",
        params: &[],
    },
    CuCapability {
        name: "act:focus",
        kind: CuKind::Act,
        summary: "Bring one window to the foreground by a unique title or a live hwnd; readback is the desktop's foreground title and target_hwnd.",
        params: &[P_WHY, P_TITLE, P_HWND],
    },
    CuCapability {
        name: "act:move",
        kind: CuKind::Act,
        summary: "Move and resize one window; readback is the rectangle the desktop reports.",
        params: &[P_WHY, P_TITLE, P_X, P_Y, P_WIDTH, P_HEIGHT],
    },
    CuCapability {
        name: "act:click",
        kind: CuKind::Act,
        summary: "One click (optionally double, right or middle); readback is the cursor position.",
        params: &[P_WHY, P_X, P_Y, P_BUTTON, P_DOUBLE],
    },
    CuCapability {
        name: "act:hover",
        kind: CuKind::Act,
        summary: "Place the pointer without pressing; readback is the cursor position.",
        params: &[P_WHY, P_X, P_Y],
    },
    CuCapability {
        name: "act:drag",
        kind: CuKind::Act,
        summary: "Press, move in twelve steps per segment, release; `path[]` traverses several points and the receipt counts them. Without `path`, `to_x`/`to_y` are required and 0.0.6 behaviour is unchanged.",
        params: &[P_WHY, P_X, P_Y, P_TO_X, P_TO_Y, P_BUTTON, P_PATH],
    },
    CuCapability {
        name: "act:type",
        kind: CuKind::Act,
        summary: "Deliver literal text to the foreground window; evidence is the foreground before and after.",
        params: &[P_WHY, P_TEXT],
    },
    CuCapability {
        name: "act:key",
        kind: CuKind::Act,
        summary: "Press one key combination; `hold` keeps a whitelisted modifier down across calls and `release` lets it go. Keyboard state is disclosed and is never a button receipt.",
        params: &[P_WHY, P_KEYS, P_HOLD, P_RELEASE],
    },
    CuCapability {
        name: "act:scroll",
        kind: CuKind::Act,
        summary: "Two-axis wheel turn at a position; `delta` remains the vertical alias of `delta_y`. The axes are published as structured evidence; readback keeps the cursor position.",
        params: &[P_WHY, P_X, P_Y, P_DELTA, P_DELTA_X, P_DELTA_Y],
    },
    CuCapability {
        name: "act:control",
        kind: CuKind::Act,
        summary: "Resolve one control by name from the last observe and place the pointer at its centre; readback must land inside the control's rectangle. `source` defaults to `controls` (Win32); `source:\"a11y\"` resolves a named MSAA node from the same generation's normalised a11y tree, with an optional `role` filter. press=false by default (no click).",
        params: &[P_WHY, P_CONTROL, P_PRESS, P_BUTTON, P_DOUBLE, P_SOURCE, P_ROLE],
    },
];

pub fn capability(name: &str) -> Option<&'static CuCapability> {
    CAPABILITIES.iter().find(|entry| entry.name == name)
}

/// The compiled backend's capability matrix as typed evidence. It travels with
/// every platform refusal and with `disclose`, so a caller can see exactly
/// which verbs this cell implements instead of inferring it from the OS name.
pub fn capability_matrix(caps: &crate::screen::PlatformCapabilities) -> Value {
    Value::Array(
        caps.rows()
            .into_iter()
            .map(|(name, implemented)| {
                json!({ "capability": name, "implemented": implemented })
            })
            .collect(),
    )
}

/// `evidence["environment"]` for `op=disclose`. Only the coarse session kind
/// is published: no display number, user name, path or window title ever
/// leaves the probe.
pub fn environment_json(probe: &EnvironmentProbe, scales: &[DisplayScaleProbe]) -> Value {
    json!({
        "platform": probe.platform,
        "session": probe.session.tag(),
        "display_count": probe.display_count,
        "virtual_desktop": {
            "x": probe.virtual_desktop.0,
            "y": probe.virtual_desktop.1,
            "width": probe.virtual_desktop.2,
            "height": probe.virtual_desktop.3,
        },
        "foreground_present": probe.foreground_present,
        // 0.0.10: the coordinate basis is session state, so it nests under
        // environment rather than becoming a new capability or verdict.
        "display_scales": display_scales_json(scales),
    })
}

/// `evidence["environment"]["display_scales"]`. Every row is bounded, ordered
/// by index and admitted by `screen::normalize_display_scales`; `reason` is
/// omitted rather than published as an empty string.
pub fn display_scales_json(scales: &[DisplayScaleProbe]) -> Value {
    fn rect_json(rect: crate::screen::DisplayRect) -> Value {
        json!({
            "x": rect.0, "y": rect.1,
            "width": rect.2, "height": rect.3,
        })
    }
    Value::Array(
        scales
            .iter()
            .map(|row| {
                let mut value = json!({
                    "index": row.index,
                    "logical_rect": rect_json(row.logical_rect),
                    "physical_rect": rect_json(row.physical_rect),
                    "scale": row.scale.tag(),
                });
                if let Some(reason) = &row.reason {
                    if let Value::Object(fields) = &mut value {
                        fields.insert("reason".to_owned(), json!(reason));
                    }
                }
                value
            })
            .collect(),
    )
}

/// `evidence["grants"]` for `op=disclose`. The rows are already bounded,
/// sorted and de-duplicated by `screen::normalize_grant_probes`; `reason` is
/// omitted rather than published as an empty string.
pub fn grants_json(probes: &[GrantProbe]) -> Value {
    Value::Array(
        probes
            .iter()
            .map(|probe| {
                let mut row = json!({ "name": probe.name, "status": probe.status.tag() });
                if let Some(reason) = &probe.reason {
                    if let Value::Object(fields) = &mut row {
                        fields.insert("reason".to_owned(), json!(reason));
                    }
                }
                row
            })
            .collect(),
    )
}

/// Action names without the `act:` prefix, in manifest order.
pub fn act_names() -> Vec<&'static str> {
    CAPABILITIES
        .iter()
        .filter(|entry| entry.kind == CuKind::Act)
        .filter_map(|entry| entry.name.strip_prefix("act:"))
        .collect()
}

/// CU2's three outcomes, plus the three ways a call never reaches an effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CuVerdict {
    Performed,
    NotPerformed,
    Unknown,
    Refused,
    Unsupported,
    Stale,
}

impl CuVerdict {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Performed => "performed",
            Self::NotPerformed => "not_performed",
            Self::Unknown => "unknown",
            Self::Refused => "refused",
            Self::Unsupported => "unsupported",
            Self::Stale => "stale",
        }
    }

}

/// One addressable target after `act:control` resolution. 0.0.15 gives both
/// arms the same shape: a name, a coarse role, a rectangle and — only for the
/// a11y arm — the normalised node the independent reader recomputes. Pure host
/// data; no native handle or coordinate from a previous frame is retained.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedTarget {
    label: String,
    role: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    a11y_node: Option<Value>,
}

impl ResolvedTarget {
    fn center(&self) -> Option<(i32, i32)> {
        if self.width <= 0 || self.height <= 0 {
            return None;
        }
        Some((self.x + self.width / 2, self.y + self.height / 2))
    }
}

/// One observation bound to a generation. `facts_as_of` is the age a caller
/// must pin, not a wall-clock promise about the desktop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CuFrame {
    pub generation: u64,
    pub facts_as_of_ms: u64,
    pub snapshot: ScreenSnapshot,
}

impl CuFrame {
    pub fn to_json(&self) -> Value {
        let displays: Vec<Value> = self
            .snapshot
            .displays
            .iter()
            .enumerate()
            .map(|(index, display)| {
                // 0.0.10: the coordinate basis travels with each display. A
                // cell whose scale probe was refused reports `unknown`,
                // never a plausible `1` the caller might click through.
                let scale = self
                    .snapshot
                    .display_scales
                    .get(index)
                    .map(|row| row.scale.tag())
                    .unwrap_or("unknown");
                json!({
                    "x": display.x, "y": display.y,
                    "width": display.width, "height": display.height,
                    "primary": display.primary,
                    "scale": scale,
                })
            })
            .collect();
        let windows: Vec<Value> = self.snapshot.windows.iter().map(window_json).collect();
        let controls: Vec<Value> = self.snapshot.controls.iter().map(control_json).collect();
        let a11y_controls: Vec<Value> = self
            .snapshot
            .a11y_controls
            .iter()
            .map(a11y_control_json)
            .collect();
        json!({
            "generation": self.generation,
            "facts_as_of_ms": self.facts_as_of_ms,
            "platform": self.snapshot.platform,
            "qualified": self.snapshot.qualified,
            "desktop": {
                "x": self.snapshot.desktop.0, "y": self.snapshot.desktop.1,
                "width": self.snapshot.desktop.2, "height": self.snapshot.desktop.3,
            },
            "displays": displays,
            "windows": windows,
            "controls": controls,
            "controls_source": self.snapshot.controls_source,
            "controls_truncated": self.snapshot.controls_truncated,
            "controls_role_basis": "win32-class",
            "a11y_controls": a11y_controls,
            "a11y_source": self.snapshot.a11y_source,
            "a11y_reason": self.snapshot.a11y_reason,
            "a11y_truncated": self.snapshot.a11y_truncated,
            "a11y_role_basis": "msaa-role",
            "a11y_node_schema": "role-name-state-value-bounds-depth",
            "capture": self.snapshot.capture.as_ref().map(|outcome| json!({
                "path": outcome.path, "bytes": outcome.bytes,
                "width": outcome.width, "height": outcome.height,
                "scale": outcome.scale, "source": outcome.source,
            })),
        })
    }
}

fn control_json(control: &ControlInfo) -> Value {
    json!({
        "hwnd": control.handle, "parent": control.parent,
        "label": control.label, "class": control.class, "role": control.role,
        "text": control.text,
        "rect": [control.x, control.y, control.width, control.height],
        "visible": control.visible, "enabled": control.enabled,
    })
}

fn a11y_control_json(control: &crate::screen::A11yControl) -> Value {
    json!({
        "name": control.name,
        "value": control.value,
        "role": control.role,
        "role_name": control.role_name,
        "state": control.state,
        "state_names": crate::screen::a11y_state_names(control.state),
        "rect": [control.x, control.y, control.width, control.height],
        "depth": control.depth,
        "child_id": control.child_id,
    })
}

/// 0.0.16: the only four fields an effect may predict or report. Sorted so
/// the host and the independent reader derive the same ordered whitelist.
const EFFECT_FIELDS: [&str; 4] = ["name", "rect", "state", "value"];

fn normalise_expect(expect: &[String]) -> Result<Vec<String>, &'static str> {
    let mut rows: Vec<String> = Vec::new();
    for field in expect {
        if !EFFECT_FIELDS.contains(&field.as_str()) {
            return Err("invalid_expect");
        }
        if !rows.contains(field) {
            rows.push(field.clone());
        }
    }
    rows.sort();
    Ok(rows)
}

/// The published shape of one node on one side of the comparison. `name` is
/// included so the reader can recompute the `name` difference by itself.
fn a11y_effect_node_json(node: &crate::screen::A11yControl) -> Value {
    json!({
        "name": node.name,
        "value": node.value,
        "state": node.state,
        "state_names": crate::screen::a11y_state_names(node.state),
        "rect": [node.x, node.y, node.width, node.height],
        "depth": node.depth,
        "child_id": node.child_id,
        "role_name": node.role_name,
    })
}

fn a11y_effect_observed(
    before: &crate::screen::A11yControl,
    after: &crate::screen::A11yControl,
) -> Vec<String> {
    let mut rows = Vec::new();
    if before.value != after.value {
        rows.push("value".to_owned());
    }
    if before.state != after.state {
        rows.push("state".to_owned());
    }
    if [before.x, before.y, before.width, before.height]
        != [after.x, after.y, after.width, after.height]
    {
        rows.push("rect".to_owned());
    }
    if before.name != after.name {
        rows.push("name".to_owned());
    }
    rows.sort();
    rows
}

/// Every refusal publishes the same empty row. No success is ever taken from
/// the request, and a refused scan cannot leak a plausible `observed`.
fn effect_refusal(reason: &str) -> Value {
    json!({
        "reason": reason,
        "hwnd": Value::Null,
        "name": Value::Null,
        "role_name": Value::Null,
        "before": Value::Null,
        "after": Value::Null,
        "expect": [],
        "observed": [],
        "verdict": "refused",
        "before_generation": Value::Null,
        "after_generation": Value::Null,
    })
}

/// Pure comparison: no tool call, no native surface, no clock. `before` is
/// the frame read **before** `state.frame` was overwritten, so its generation
/// is the one the act was dispatched against (TOCTOU re-validation).
fn a11y_effect_evidence(
    before_frame: Option<&CuFrame>,
    dispatch_generation: u64,
    after: &ScreenSnapshot,
    request: &EffectRequest,
) -> Value {
    let expect = match normalise_expect(&request.expect) {
        Ok(rows) => rows,
        Err(reason) => return effect_refusal(reason),
    };
    let frame = match before_frame {
        Some(frame) => frame,
        None => return effect_refusal("no_frame"),
    };
    if frame.generation != dispatch_generation {
        return effect_refusal("stale");
    }
    if frame.snapshot.a11y_source.is_none() || frame.snapshot.a11y_controls.is_empty() {
        return effect_refusal("no_a11y_tree");
    }
    if after.a11y_source.is_none() || after.a11y_controls.is_empty() {
        return effect_refusal("no_a11y_tree");
    }
    let before_matches: Vec<&crate::screen::A11yControl> = frame
        .snapshot
        .a11y_controls
        .iter()
        .filter(|node| {
            node.name == request.name
                && request
                    .role
                    .as_deref()
                    .map_or(true, |role| role == node.role_name)
                && request
                    .child_id
                    .map_or(true, |child_id| child_id == node.child_id)
        })
        .collect();
    let before_node = match before_matches.len() {
        1 => before_matches[0],
        0 => return effect_refusal("no_matching_node"),
        _ => return effect_refusal("ambiguous_identity"),
    };
    // The after side is matched by the *full* published identity, never by a
    // coordinate or a fresh name lookup that could resolve a different node.
    let after_matches: Vec<&crate::screen::A11yControl> = after
        .a11y_controls
        .iter()
        .filter(|node| {
            node.name == before_node.name
                && node.role_name == before_node.role_name
                && node.child_id == before_node.child_id
                && node.depth == before_node.depth
        })
        .collect();
    let after_node = match after_matches.len() {
        1 => Some(after_matches[0]),
        0 => None,
        _ => return effect_refusal("ambiguous_identity"),
    };
    let (after_json, observed, verdict) = match after_node {
        None => (Value::Null, Vec::new(), "vanished"),
        Some(node) => {
            let observed = a11y_effect_observed(before_node, node);
            let verdict = if !observed.is_empty() && observed == expect {
                "confirmed"
            } else if observed.is_empty() && expect.is_empty() {
                "unchanged"
            } else {
                "mismatch"
            };
            (a11y_effect_node_json(node), observed, verdict)
        }
    };
    json!({
        "reason": Value::Null,
        "hwnd": request.hwnd,
        "name": before_node.name,
        "role_name": before_node.role_name,
        "before": a11y_effect_node_json(before_node),
        "after": after_json,
        "expect": expect,
        "observed": observed,
        "verdict": verdict,
        "before_generation": frame.generation,
        "after_generation": dispatch_generation + 1,
    })
}

fn window_json(window: &WindowInfo) -> Value {
    json!({
        "hwnd": window.handle, "pid": window.pid,
        "rect": [window.x, window.y, window.width, window.height],
        "class": window.class_name, "title": window.title,
        "minimized": window.minimized, "maximized": window.maximized,
        "focused": window.focused,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CuReceipt {
    pub generation: u64,
    pub capability: String,
    pub verdict: CuVerdict,
    pub facts_as_of_ms: u64,
    pub readback: String,
}

impl CuReceipt {
    fn to_json(&self) -> Value {
        json!({
            "generation": self.generation,
            "capability": self.capability,
            "verdict": self.verdict.tag(),
            "facts_as_of_ms": self.facts_as_of_ms,
            "readback": self.readback,
        })
    }

    fn to_line(&self) -> String {
        format!(
            "receipt: generation={} capability={} verdict={} readback={}",
            self.generation,
            self.capability,
            self.verdict.tag(),
            self.readback
        )
    }
}

/// 0.0.15: which inventory `act:control` resolves against. `Controls` keeps
/// the 0.0.6 Win32 behaviour; `A11y` addresses the normalised MSAA tree from
/// the very same frame. Pure data, so every cell can type-check both arms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlSource {
    Controls,
    A11y,
}

impl ControlSource {
    fn tag(self) -> &'static str {
        match self {
            Self::Controls => "controls",
            Self::A11y => "a11y",
        }
    }

    fn parse(raw: Option<&str>) -> Result<Self, String> {
        match raw.unwrap_or("controls") {
            "controls" => Ok(Self::Controls),
            "a11y" => Ok(Self::A11y),
            other => Err(format!(
                "screen: refused: unknown `source` {other:?}; act:control accepts only \
                 \"controls\" or \"a11y\""
            )),
        }
    }
}

/// The typed operation a caller asked the host to perform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CuAction {
    Focus { target: FocusTarget },
    Move { title: String, x: i32, y: i32, width: i32, height: i32 },
    Click { x: i32, y: i32, button: ClickButton, double: bool },
    Hover { x: i32, y: i32 },
    Drag {
        x: i32,
        y: i32,
        to_x: i32,
        to_y: i32,
        button: ClickButton,
        /// `None` is the 0.0.6 single-leg drag; `Some` is the bounded path.
        path: Option<Vec<(i32, i32)>>,
    },
    Type { text: String },
    /// `hold`/`release` are mutually exclusive; see `KeyboardState`.
    Key { keys: String, hold: bool, release: bool },
    Scroll { x: i32, y: i32, delta_x: i32, delta_y: i32 },
    /// Resolve a control from the current frame **by name**; see
    /// [`CuHost::act_control`]. `press` is opt-in so a semantic target can be
    /// qualified on a real desktop without clicking anything. `source` selects
    /// the Win32 inventory (default) or the 0.0.15 normalised a11y tree;
    /// `role` is an optional identity filter for the a11y arm only.
    Control {
        control: String,
        press: bool,
        button: ClickButton,
        double: bool,
        source: ControlSource,
        role: Option<String>,
    },
}

impl CuAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Focus { .. } => "act:focus",
            Self::Move { .. } => "act:move",
            Self::Click { .. } => "act:click",
            Self::Hover { .. } => "act:hover",
            Self::Drag { .. } => "act:drag",
            Self::Type { .. } => "act:type",
            Self::Key { .. } => "act:key",
            Self::Scroll { .. } => "act:scroll",
            Self::Control { .. } => "act:control",
        }
    }
}

/// 0.0.16: one predicted effect, declared **before** the act it verifies.
/// `expect` is kept in request order here; `normalise_expect` validates and
/// sorts it at dispatch so a refused request still publishes a named empty
/// row. An identity is never inferred from a coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRequest {
    pub hwnd: u64,
    pub name: String,
    pub role: Option<String>,
    pub child_id: Option<i32>,
    pub expect: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CuCall {
    Observe {
        window: Option<String>,
        capture: Option<CaptureTarget>,
        controls: Option<ControlTarget>,
        a11y: Option<A11yTarget>,
        scale: u32,
        wait_ms: Option<u32>,
        effect: Option<EffectRequest>,
    },
    Act {
        why: String,
        action: CuAction,
    },
    Disclose,
}

impl CuCall {
    fn name(&self) -> &'static str {
        match self {
            Self::Observe { .. } => "observe",
            Self::Act { action, .. } => action.name(),
            Self::Disclose => "disclose",
        }
    }
}

/// Everything a dynamic caller may say. Missing ABI fields default to the
/// host's own level, so a plain model tool call needs no extra ceremony.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CuRequest {
    pub abi_major: u32,
    pub abi_minor: u32,
    /// Generation this call was reasoned from. `None` means unpinned.
    pub as_of: Option<u64>,
    pub call: CuCall,
}

/// The typed result. `text` is the disclosure for a text-only model;
/// `evidence` is the same result for a dynamic caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CuResult {
    pub verdict: CuVerdict,
    pub capability: String,
    pub generation: u64,
    pub facts_as_of_ms: u64,
    pub evidence: Value,
    pub text: String,
}

struct CuState {
    generation: u64,
    frame: Option<CuFrame>,
    receipts: Vec<CuReceipt>,
    keyboard: KeyboardState,
    /// 0.0.13: actual elapsed milliseconds and performed wait calls. These are
    /// pure host state, never a model loop or a native surface change.
    wait_cumulative_ms: u64,
    wait_calls: u32,
}

/// Evidence for one performed bounded wait. `elapsed_ms` is measured with a
/// monotonic clock, not copied from the request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitObservation {
    requested_ms: u32,
    elapsed_ms: u64,
    cumulative_ms: u64,
    wait_calls: u32,
}

/// PRD 01.10: the held set is owned by the host, not by a native global.
/// Pure data, so every bound and the foreground-loss rule is unit-testable
/// without posting a real keystroke on a CI runner.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct KeyboardState {
    held: Vec<Modifier>,
    /// Foreground title when the last hold was posted. `None` on a fresh host.
    foreground: Option<String>,
}

impl KeyboardState {
    fn tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.held.iter().map(|key| key.tag().to_owned()).collect();
        tags.sort();
        tags
    }

    fn hold(&mut self, key: Modifier) -> Result<(), String> {
        if !self.held.contains(&key) {
            if self.held.len() >= SCREEN_TOOL_MAX_HELD_MODIFIERS {
                return Err(format!(
                    "screen: refused: at most {SCREEN_TOOL_MAX_HELD_MODIFIERS} held modifiers are allowed ({}); release one first",
                    SCREEN_TOOL_HELD_MODIFIERS.join(", ")
                ));
            }
            self.held.push(key);
        }
        Ok(())
    }

    fn release(&mut self) -> Vec<String> {
        let released = self.tags();
        self.held.clear();
        self.foreground = None;
        released
    }

    /// Before every call: a changed foreground, a lost grant or an unknown
    /// baseline releases everything held. Returns what was let go.
    fn reconcile(&mut self, current_foreground: &str, grant_lost: bool) -> Vec<String> {
        if self.held.is_empty() {
            return Vec::new();
        }
        let changed = self.foreground.as_deref() != Some(current_foreground);
        if changed || grant_lost {
            self.release()
        } else {
            Vec::new()
        }
    }
}

/// Canonical foreground identity for the keyboard state machine. A session
/// with no foreground window is still a stable target, never a guess.
fn keyboard_foreground() -> String {
    crate::screen::foreground_title()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "desktop session".to_owned())
}

/// Evidence for a keyboard state change. It is deliberately separate from
/// `PRD_01_07`'s button actuation receipt: `actuation` is always null here.
fn keyboard_evidence(readback: &str, generation: u64, held: &[String], released: &[String]) -> Value {
    json!({
        "action": "key",
        "window": Value::Null,
        "readback": readback,
        "verdict": "performed",
        "generation": generation,
        "actuation": Value::Null,
        "scroll": Value::Null,
        "keyboard": { "held": held, "released": released },
        "held_keys": held,
    })
}

/// The host side of the contract. It owns the grant and the native tool; a
/// caller supplies typed requests and receives typed results only.
pub struct CuHost {
    tool: ScreenTool,
    state: Mutex<CuState>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

/// One bounded sleep, measured with a monotonic clock. `thread::sleep`
/// guarantees at least the requested duration; `max` protects against a
/// sub-millisecond truncation of `Duration::as_millis()`, never from a short
/// sleep.
fn measure_wait(requested_ms: u32) -> u64 {
    let started = Instant::now();
    std::thread::sleep(Duration::from_millis(requested_ms as u64));
    started.elapsed().as_millis().max(requested_ms as u128) as u64
}

/// Admit one wait and reserve its worst honest cost before any sleep. A
/// refused wait changes no state and posts no delay.
fn reserve_wait(state: &mut CuState, requested_ms: u32) -> Result<u32, String> {
    if state.wait_calls >= SCREEN_TOOL_MAX_WAIT_CALLS {
        return Err(format!(
            "screen: refused: the observe wait call budget of {SCREEN_TOOL_MAX_WAIT_CALLS} \
             calls is exhausted; no sleep was performed"
        ));
    }
    let reserved = requested_ms as u64 + SCREEN_TOOL_WAIT_OVERHEAD_MS;
    if state
        .wait_cumulative_ms
        .saturating_add(reserved)
        > SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS
    {
        return Err(format!(
            "screen: refused: wait_ms={requested_ms} would exceed the cumulative observe wait \
             budget of {SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS} ms; no sleep was performed"
        ));
    }
    state.wait_calls += 1;
    state.wait_cumulative_ms += reserved;
    Ok(state.wait_calls)
}

/// Replace the reserved worst-case cost with the measured elapsed time.
fn settle_wait(state: &mut CuState, requested_ms: u32, elapsed_ms: u64) -> u64 {
    let reserved = requested_ms as u64 + SCREEN_TOOL_WAIT_OVERHEAD_MS;
    state.wait_cumulative_ms = state
        .wait_cumulative_ms
        .saturating_sub(reserved)
        .saturating_add(elapsed_ms);
    state.wait_cumulative_ms
}

impl CuHost {
    pub fn new(tool: ScreenTool) -> Self {
        Self {
            tool,
            state: Mutex::new(CuState {
                generation: 0,
                frame: None,
                receipts: Vec::new(),
                keyboard: KeyboardState::default(),
                wait_cumulative_ms: 0,
                wait_calls: 0,
            }),
        }
    }

    pub fn granted(&self) -> bool {
        self.tool.enabled()
    }

    /// Teardown releases every modifier, whether or not the caller did.
    /// `release_all_modifiers` is idempotent on all four backends; an error
    /// (no display, no OS grant) is deliberately ignored here because there
    /// is no further effect to protect after the host is gone.
    fn release_on_teardown(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.keyboard.release();
        let _ = crate::screen::release_all_modifiers();
    }

    /// The JSON-facing entrance a dynamic-logic module would bind to. Parse
    /// failures are typed refusals, never Rust errors that abort the run.
    pub fn invoke_json(&self, arguments: &Value) -> CuResult {
        match parse_request(arguments) {
            Ok(request) => self.invoke(request),
            Err(refusal) => {
                // A typed act refusal still names the capability it belongs to;
                // an observe/disclose refusal falls back to `op`. "unknown" is
                // reserved for a request with no advertised verb.
                let name = arguments
                    .get("action")
                    .and_then(Value::as_str)
                    .map(|action| format!("act:{action}"))
                    .filter(|candidate| capability(candidate).is_some())
                    .or_else(|| {
                        arguments
                            .get("op")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .filter(|candidate| capability(candidate).is_some())
                    })
                    .unwrap_or_else(|| "unknown".to_owned());
                self.refusal(&name, refusal)
            }
        }
    }

    /// The **named** entrance a dynamic caller binds to: it discovers a
    /// capability from `disclose`, then calls it by name with typed arguments.
    /// A name the host does not advertise is `unsupported` before any effect;
    /// a caller cannot invent `op` values or widen its grant here.
    pub fn invoke_capability(&self, name: &str, arguments: &Value) -> CuResult {
        if capability(name).is_none() {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            return self.result_with_state(
                &mut state,
                CuVerdict::Unsupported,
                name,
                format!(
                    "cu: unsupported: capability {name:?} is not advertised; disclose to list \
                     what this host offers"
                ),
                json!({ "capability": name }),
            );
        }
        let Some(object) = arguments.as_object() else {
            return self.refusal(
                name,
                "screen: refused: capability arguments must be a JSON object".to_owned(),
            );
        };
        let mut adapted = object.clone();
        match name {
            "observe" | "disclose" => {
                adapted.insert("op".to_owned(), json!(name));
            }
            action => {
                adapted.insert("op".to_owned(), json!("act"));
                adapted.insert(
                    "action".to_owned(),
                    json!(action.strip_prefix("act:").unwrap_or(action)),
                );
            }
        }
        self.invoke_json(&Value::Object(adapted))
    }

    fn refusal(&self, capability: &str, reason: String) -> CuResult {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let generation = state.generation;
        let text = format!(
            "{reason}\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={generation} \
             verdict=refused capability={capability}"
        );
        CuResult {
            verdict: CuVerdict::Refused,
            capability: capability.to_owned(),
            generation,
            facts_as_of_ms: now_ms(),
            evidence: json!({ "verdict": "refused", "reason": reason, "generation": generation }),
            text,
        }
    }

    /// The single entrance for every computer-use effect.
    pub fn invoke(&self, request: CuRequest) -> CuResult {
        let capability = request.call.name().to_owned();
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let generation = state.generation;

        // 1. ABI compatibility: refused before activation, never a widening.
        if request.abi_major != CU_ABI_MAJOR {
            return self.result_with_state(
                &mut state,
                CuVerdict::Refused,
                &capability,
                format!(
                    "cu: refused: ABI major {} is incompatible with host {CU_ABI_MAJOR}; no \
                     capability was activated",
                    request.abi_major
                ),
                json!({ "abi_major": request.abi_major, "host_major": CU_ABI_MAJOR }),
            );
        }
        // 2. Capability exists and is one this host advertises.
        let advertised = crate::cu::capability(&capability);
        if advertised.is_none() {
            return self.result_with_state(
                &mut state,
                CuVerdict::Unsupported,
                &capability,
                format!(
                    "cu: unsupported: capability {capability:?} is not advertised; disclose to \
                     list what this host offers"
                ),
                json!({ "capability": capability }),
            );
        }
        // 3. The grant is the host's, not the caller's. Disclosure is pure
        //    information: a caller may discover what it is *not* allowed to
        //    do even when computer use was never granted.
        let kind = advertised.map(|entry| entry.kind).unwrap_or(CuKind::Act);
        // 3a. PRD 01.10: before every call, a foreground change or a lost
        //     grant releases everything held and publishes an `unknown`
        //     result. No new effect is delivered in that call.
        let auto_released = state.keyboard.reconcile(
            &keyboard_foreground(),
            !self.tool.enabled(),
        );
        if !auto_released.is_empty() {
            return self.result_with_state(
                &mut state,
                CuVerdict::Unknown,
                &capability,
                format!(
                    "cu: unknown: the foreground changed or the grant was lost while modifiers were held; released {} and no new effect was delivered",
                    auto_released.join(", ")
                ),
                json!({
                    "keyboard": { "held": [], "released": auto_released },
                    "held_keys": [],
                    "actuation": Value::Null,
                }),
            );
        }
        if kind != CuKind::Disclose && !self.tool.enabled() {
            return self.result_with_state(
                &mut state,
                CuVerdict::Refused,
                &capability,
                "screen: refused: computer use is not granted; pass --allow-screen to let this \
                 invocation observe or operate the desktop"
                    .to_owned(),
                json!({ "grant": "denied" }),
            );
        }
        // 3b. Platform capability is a host fact, not a native surprise. A
        //     cell with no implemented effect refuses by naming its OS; a cell
        //     that implements some verbs refuses only the missing ones by name.
        let caps = crate::screen::platform_capabilities();
        if kind != CuKind::Disclose && !caps.any_effect_implemented() {
            return self.result_with_state(
                &mut state,
                CuVerdict::Refused,
                &capability,
                format!(
                    "screen: refused: no computer-use capability is implemented on {}; disclose \
                     reports this cell's capability matrix, and a refusal is the honest answer",
                    crate::screen::platform_name()
                ),
                json!({
                    "qualified": false,
                    "platform": crate::screen::platform_name(),
                    "platform_capabilities": capability_matrix(&caps),
                }),
            );
        }
        if kind != CuKind::Disclose && !caps.supports(&capability) {
            return self.result_with_state(
                &mut state,
                CuVerdict::Unsupported,
                &capability,
                format!(
                    "cu: unsupported: capability {capability:?} is not implemented by the {} \
                     backend; disclose reports the matrix; no effect was performed",
                    crate::screen::platform_name()
                ),
                json!({
                    "capability": capability,
                    "platform": crate::screen::platform_name(),
                    "platform_capabilities": capability_matrix(&caps),
                }),
            );
        }
        // `observe` has two optional arms that are separate capabilities: a
        // backend may read geometry but have no pixel or control mechanism.
        for (arm, implemented, requested) in [
            (
                "capture",
                caps.capture,
                matches!(&request.call, CuCall::Observe { capture: Some(_), .. }),
            ),
            (
                "controls",
                caps.controls,
                matches!(&request.call, CuCall::Observe { controls: Some(_), .. }),
            ),
        ] {
            if requested && !implemented {
                return self.result_with_state(
                    &mut state,
                    CuVerdict::Unsupported,
                    &capability,
                    format!(
                        "cu: unsupported: observe arm {arm:?} is not implemented by the {} \
                         backend; no image or control scan was produced",
                        crate::screen::platform_name()
                    ),
                    json!({
                        "observe_arm": arm,
                        "platform": crate::screen::platform_name(),
                        "platform_capabilities": capability_matrix(&caps),
                    }),
                );
            }
        }
        // 4. Age: a call reasoned from a superseded observation is never
        //    replayed against the desktop.
        if let Some(as_of) = request.as_of {
            if as_of != generation {
                return self.result_with_state(
                    &mut state,
                    CuVerdict::Stale,
                    &capability,
                    format!(
                        "cu: stale: this call is pinned to generation {as_of}, the host is at \
                         {generation}; observe again, the operation was not performed"
                    ),
                    json!({ "as_of": as_of, "current_generation": generation }),
                );
            }
        }
        // 5. Native effect. The lock is dropped before touching the desktop:
        //    a display enumeration must never hold host state hostage.
        drop(state);
        match request.call {
            CuCall::Observe { window, capture, controls, a11y, scale, wait_ms, effect } => {
                self.observe(
                    &capability, window, capture, controls, a11y, scale, wait_ms, effect,
                )
            }
            CuCall::Act { why, action } => match action {
                CuAction::Control { control, press, button, double, source, role } => {
                    self.act_control(
                        &capability, why, control, press, button, double, source, role,
                    )
                }
                CuAction::Key { keys, hold, release } if hold || release => {
                    self.act_key_modifier(&capability, why, keys, release)
                }
                action => self.act(&capability, why, action),
            },
            CuCall::Disclose => self.disclose(&capability, request.abi_minor),
        }
    }

    fn observe(
        &self,
        capability: &str,
        window: Option<String>,
        capture: Option<CaptureTarget>,
        controls: Option<ControlTarget>,
        a11y: Option<A11yTarget>,
        scale: u32,
        wait_ms: Option<u32>,
        effect: Option<EffectRequest>,
    ) -> CuResult {
        // 0.0.13: one bounded sleep before the fresh snapshot. Ceilings are
        // admitted under the lock; the sleep itself never holds host state.
        let wait = match wait_ms {
            None => None,
            Some(requested_ms) => {
                let wait_calls = {
                    let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                    match reserve_wait(&mut *state, requested_ms) {
                        Ok(calls) => calls,
                        Err(reason) => {
                            return self.result_with_state(
                                &mut state,
                                CuVerdict::Refused,
                                capability,
                                reason,
                                json!({}),
                            );
                        }
                    }
                };
                let elapsed_ms = measure_wait(requested_ms);
                let cumulative_ms = {
                    let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                    settle_wait(&mut *state, requested_ms, elapsed_ms)
                };
                Some(WaitObservation {
                    requested_ms,
                    elapsed_ms,
                    cumulative_ms,
                    wait_calls,
                })
            }
        };
        match self.tool.snapshot(window.as_deref(), capture, scale, controls, a11y) {
            Ok(snapshot) => {
                let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                // 0.0.16: the previous frame is the *only* admissible
                // `before` side. It must be read before the overwrite and
                // bound to the generation the act was dispatched against.
                let before_frame = state.frame.clone();
                let dispatch_generation = state.generation;
                state.generation += 1;
                let frame = CuFrame {
                    generation: state.generation,
                    facts_as_of_ms: now_ms(),
                    snapshot: snapshot.clone(),
                };
                let mut evidence = frame.to_json();
                state.frame = Some(frame);
                let generation = state.generation;
                if let Some(effect) = &effect {
                    evidence["a11y_effect"] = a11y_effect_evidence(
                        before_frame.as_ref(),
                        dispatch_generation,
                        &snapshot,
                        effect,
                    );
                }
                let mut text = snapshot.to_text();
                if let Some(wait) = &wait {
                    evidence["wait"] = json!({
                        "requested_ms": wait.requested_ms,
                        "elapsed_ms": wait.elapsed_ms,
                        "cumulative_ms": wait.cumulative_ms,
                        "wait_calls": wait.wait_calls,
                        "generation": generation,
                    });
                    text.push_str(&format!(
                        "\nwait: requested_ms={} elapsed_ms={} cumulative_ms={} wait_calls={}",
                        wait.requested_ms,
                        wait.elapsed_ms,
                        wait.cumulative_ms,
                        wait.wait_calls
                    ));
                }
                text.push_str(&format!(
                    "\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={generation} \
                     verdict=performed capability={capability}"
                ));
                let readback = format!(
                    "{} windows, {} displays{}",
                    snapshot.windows.len(),
                    snapshot.displays.len(),
                    snapshot
                        .controls_source
                        .as_ref()
                        .map(|source| format!(
                            ", {} controls from {source}{}",
                            snapshot.controls.len(),
                            if snapshot.controls_truncated { " (truncated)" } else { "" }
                        ))
                        .unwrap_or_default()
                );
                self.record(
                    &mut state,
                    CuReceipt {
                        generation,
                        capability: capability.to_owned(),
                        verdict: CuVerdict::Performed,
                        facts_as_of_ms: now_ms(),
                        readback,
                    },
                );
                CuResult {
                    verdict: CuVerdict::Performed,
                    capability: capability.to_owned(),
                    generation,
                    facts_as_of_ms: now_ms(),
                    evidence,
                    text,
                }
            }
            Err(error) => {
                let (verdict, reason) = classify(error);
                self.effect_failure(capability, verdict, reason)
            }
        }
    }

    fn act(&self, capability: &str, why: String, action: CuAction) -> CuResult {
        let outcome = match self.call_act(&action) {
            Ok(outcome) => outcome,
            Err(error) => {
                let (verdict, reason) = classify(error);
                return self.effect_failure(capability, verdict, reason);
            }
        };
        // A changed foreground means the input landed but its effect is not
        // confirmed. CU2 requires that to be `unknown`, not `performed`.
        let verdict = if outcome.readback.contains("changed during input") {
            CuVerdict::Unknown
        } else {
            CuVerdict::Performed
        };
        let verdict = verify_actuation(
            expected_dispatch(&action),
            outcome.actuation.as_ref(),
            verdict,
        );
        let window = outcome.window.clone();
        let readback = outcome.readback.clone();
        let mut text = format!(
            "why: {why}\nscreen: act={}{}\nreadback: {readback}",
            outcome.action,
            window
                .as_ref()
                .map(|title| format!(" window={title:?}"))
                .unwrap_or_default(),
        );
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        text.push_str(&format!(
            "\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={} verdict={} \
             capability={capability}",
            state.generation,
            verdict.tag()
        ));
        let generation = state.generation;
        let evidence = json!({
            "action": outcome.action,
            "window": window,
            "readback": readback,
            "verdict": verdict.tag(),
            "generation": generation,
            "actuation": outcome.actuation.as_ref().map(actuation_json).unwrap_or(Value::Null),
            "scroll": outcome.scroll.as_ref().map(|axes| json!({
                "delta_x": axes.delta_x, "delta_y": axes.delta_y,
            })).unwrap_or(Value::Null),
            "target_hwnd": outcome.target_hwnd,
            "target_hwnd_reason": outcome.target_hwnd_reason.clone(),
        });
        self.record(
            &mut state,
            CuReceipt {
                generation,
                capability: capability.to_owned(),
                verdict,
                facts_as_of_ms: now_ms(),
                readback,
            },
        );
        CuResult {
            verdict,
            capability: capability.to_owned(),
            generation,
            facts_as_of_ms: now_ms(),
            evidence,
            text,
        }
    }

    /// One `act:key` call that holds or releases whitelisted modifiers.
    /// It never goes through `Act::Key` (one atomic down/up pair) and never
    /// produces a button actuation receipt: its evidence is the host's own
    /// keyboard state after the native effect.
    fn act_key_modifier(
        &self,
        capability: &str,
        why: String,
        keys: String,
        release: bool,
    ) -> CuResult {
        let before = crate::screen::foreground_title();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());

        let modifier = if release {
            None
        } else {
            match Modifier::parse(keys.trim()) {
                Some(key) => Some(key),
                None => {
                    return self.result_with_state(
                        &mut state,
                        CuVerdict::Refused,
                        capability,
                        format!(
                            "screen: refused: `hold` accepts exactly one whitelisted modifier ({}); got {keys:?}",
                            SCREEN_TOOL_HELD_MODIFIERS.join(", ")
                        ),
                        json!({ "bound": "SCREEN_TOOL_HELD_MODIFIERS", "keys": keys }),
                    )
                }
            }
        };
        if let Some(key) = modifier {
            if !state.keyboard.held.contains(&key)
                && state.keyboard.held.len() >= SCREEN_TOOL_MAX_HELD_MODIFIERS
            {
                let held_before = state.keyboard.tags();
                return self.result_with_state(
                    &mut state,
                    CuVerdict::Refused,
                    capability,
                    format!(
                        "screen: refused: at most {SCREEN_TOOL_MAX_HELD_MODIFIERS} held modifiers are allowed; release one first"
                    ),
                    json!({ "bound": "SCREEN_TOOL_MAX_HELD_MODIFIERS", "held": held_before }),
                );
            }
        }

        let outcome = match modifier {
            Some(key) => crate::screen::set_modifier(key, true),
            None => crate::screen::release_all_modifiers(),
        };
        if let Err(error) = outcome {
            // A failed release is still reported with an empty host set; the
            // next teardown retries. Nothing is claimed as held.
            let released = state.keyboard.release();
            let _ = crate::screen::release_all_modifiers();
            let (verdict, reason) = classify(error);
            return self.result_with_state(
                &mut state,
                verdict,
                capability,
                reason,
                json!({
                    "keyboard": { "held": [], "released": released },
                    "held_keys": [],
                    "actuation": Value::Null,
                }),
            );
        }

        let released = if release {
            state.keyboard.release()
        } else {
            // The bound was checked above; holding the same modifier twice is
            // idempotent and does not post a second native key-down.
            let _ = state.keyboard.hold(modifier.unwrap());
            Vec::new()
        };
        let held = state.keyboard.tags();
        let after = crate::screen::foreground_title();
        let foreground = match crate::screen::foreground_evidence(before, after) {
            Ok(text) => text,
            Err(error) => {
                let auto = state.keyboard.release();
                let _ = crate::screen::release_all_modifiers();
                return self.result_with_state(
                    &mut state,
                    CuVerdict::Unknown,
                    capability,
                    error,
                    json!({
                        "keyboard": { "held": [], "released": auto },
                        "held_keys": [],
                        "actuation": Value::Null,
                    }),
                );
            }
        };
        if foreground.contains("changed during input") {
            let auto = state.keyboard.release();
            let _ = crate::screen::release_all_modifiers();
            return self.result_with_state(
                &mut state,
                CuVerdict::Unknown,
                capability,
                format!("cu: unknown: the foreground changed during a keyboard effect; {foreground}; all held modifiers were released"),
                json!({
                    "keyboard": { "held": [], "released": auto },
                    "held_keys": [],
                    "actuation": Value::Null,
                }),
            );
        }
        if !release {
            state.keyboard.foreground = Some(keyboard_foreground());
        }
        let released_text = if released.is_empty() {
            "no held modifiers".to_owned()
        } else {
            released.join(", ")
        };
        let readback = if release {
            format!("released {released_text} {foreground}")
        } else {
            format!("pressed {keys} (held) {foreground}")
        };
        let generation = state.generation;
        let evidence = keyboard_evidence(&readback, generation, &held, &released);
        let mut text = format!("why: {why}\nscreen: act=key {readback}\nheld_keys={}", held.join(","));
        text.push_str(&format!(
            "\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={generation} verdict=performed \
             capability={capability}"
        ));
        self.record(
            &mut state,
            CuReceipt {
                generation,
                capability: capability.to_owned(),
                verdict: CuVerdict::Performed,
                facts_as_of_ms: now_ms(),
                readback: readback.clone(),
            },
        );
        CuResult {
            verdict: CuVerdict::Performed,
            capability: capability.to_owned(),
            generation,
            facts_as_of_ms: now_ms(),
            evidence,
            text,
        }
    }

    /// The first **semantic** action. A control is addressed by the name the
    /// current generation disclosed -- never by a hwnd carried across calls,
    /// never by a coordinate the caller computed from an older frame. Every
    /// refusal happens before a device context or a pointer move exists.
    fn act_control(
        &self,
        capability: &str,
        why: String,
        control: String,
        press: bool,
        button: ClickButton,
        double: bool,
        source: ControlSource,
        role: Option<String>,
    ) -> CuResult {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let generation = state.generation;
        let frame = match state.frame.clone() {
            Some(frame) => frame,
            None => {
                return self.result_with_state(
                    &mut state,
                    CuVerdict::Refused,
                    capability,
                    "screen: refused: there is no observation yet; call op=observe with \
                     `controls` before using act:control"
                        .to_owned(),
                    json!({ "reason": "no_frame" }),
                )
            }
        };
        if frame.generation != generation {
            return self.result_with_state(
                &mut state,
                CuVerdict::Stale,
                capability,
                format!(
                    "cu: stale: the control inventory belongs to generation {}, the host is at \
                     {generation}; observe again, the operation was not performed",
                    frame.generation
                ),
                json!({ "frame_generation": frame.generation, "current_generation": generation }),
            );
        }
        // 0.0.15: both arms resolve a name into the same bounded target. The
        // a11y arm never falls back to Win32 rows or to a coordinate the
        // caller computed from an older frame.
        let resolved = match source {
            ControlSource::Controls => {
                if frame.snapshot.controls.is_empty() {
                    return self.result_with_state(
                        &mut state,
                        CuVerdict::Refused,
                        capability,
                        "screen: refused: the last observation contains no controls; observe with \
                         `controls` set to true or a window title. Self-drawn UIs expose no Win32 \
                         child windows, and an empty inventory is reported, not guessed"
                            .to_owned(),
                        json!({ "reason": "no_controls", "controls_source": frame.snapshot.controls_source }),
                    );
                }
                match resolve_control(&frame.snapshot.controls, &control) {
                    Ok(control) => {
                        if !control.visible || !control.enabled {
                            return self.result_with_state(
                                &mut state,
                                CuVerdict::Refused,
                                capability,
                                format!(
                                    "screen: refused: control {:?} is {} and cannot be operated; \
                                     the operation was not performed",
                                    control.label,
                                    if !control.visible { "hidden" } else { "disabled" }
                                ),
                                json!({ "control": control.label, "visible": control.visible, "enabled": control.enabled }),
                            );
                        }
                        ResolvedTarget {
                            label: control.label.clone(),
                            role: control.role.to_owned(),
                            x: control.x,
                            y: control.y,
                            width: control.width,
                            height: control.height,
                            a11y_node: None,
                        }
                    }
                    Err(reason) => {
                        return self.result_with_state(
                            &mut state,
                            CuVerdict::Refused,
                            capability,
                            reason,
                            json!({ "control": control }),
                        )
                    }
                }
            }
            ControlSource::A11y => {
                if frame.snapshot.a11y_controls.is_empty() {
                    return self.result_with_state(
                        &mut state,
                        CuVerdict::Refused,
                        capability,
                        "screen: refused: the current generation has no a11y nodes; call \
                         op=observe with `a11y` on a live window. A semantic target is never \
                         guessed from Win32 rows or from raw coordinates"
                            .to_owned(),
                        json!({
                            "reason": "no_a11y_tree",
                            "a11y_source": frame.snapshot.a11y_source,
                            "a11y_reason": frame.snapshot.a11y_reason,
                        }),
                    );
                }
                match resolve_a11y_control(
                    &frame.snapshot.a11y_controls,
                    &control,
                    role.as_deref(),
                ) {
                    Ok(node) => ResolvedTarget {
                        label: node.name.clone(),
                        role: node.role_name.to_owned(),
                        x: node.x,
                        y: node.y,
                        width: node.width,
                        height: node.height,
                        a11y_node: Some(json!({
                            "name": node.name,
                            "value": node.value,
                            "role": node.role,
                            "role_name": node.role_name,
                            "state": node.state,
                            "state_names": crate::screen::a11y_state_names(node.state),
                            "rect": [node.x, node.y, node.width, node.height],
                            "depth": node.depth,
                            "child_id": node.child_id,
                        })),
                    },
                    Err(reason) => {
                        return self.result_with_state(
                            &mut state,
                            CuVerdict::Refused,
                            capability,
                            reason,
                            json!({ "control": control, "source": "a11y", "role": role }),
                        )
                    }
                }
            }
        };
        let (x, y) = match resolved.center() {
            Some(point) => point,
            None => {
                return self.result_with_state(
                    &mut state,
                    CuVerdict::Refused,
                    capability,
                    format!(
                        "screen: refused: control {:?} has a {}x{} rectangle with no centre; \
                         nothing was clicked",
                        resolved.label, resolved.width, resolved.height
                    ),
                    json!({ "control": resolved.label }),
                )
            }
        };
        let (vx, vy, vw, vh) = frame.snapshot.desktop;
        if x < vx || y < vy || x >= vx + vw || y >= vy + vh {
            return self.result_with_state(
                &mut state,
                CuVerdict::Refused,
                capability,
                format!(
                    "screen: refused: control {:?} centre {x},{y} lies outside the virtual \
                     desktop {vx},{vy} {vw}x{vh}; the pointer was not moved",
                    resolved.label
                ),
                json!({ "center": [x, y], "desktop": [vx, vy, vw, vh] }),
            );
        }
        drop(state);

        let act = if press {
            Act::Click { x, y, button, double }
        } else {
            Act::Hover { x, y }
        };
        let outcome = match self.tool.act(act) {
            Ok(outcome) => outcome,
            Err(error) => {
                let (verdict, reason) = classify(error);
                return self.effect_failure(capability, verdict, reason);
            }
        };
        // The pointer readback must land inside the *control*, not merely near
        // the requested centre. A control that moved, collapsed or was covered
        // between observe and act is therefore `not_performed`/`unknown`, never
        // a success echoed from the request.
        let (verdict, detail) = match pointer_from_readback(&outcome.readback) {
            Some((px, py))
                if px >= resolved.x
                    && px < resolved.x + resolved.width
                    && py >= resolved.y
                    && py < resolved.y + resolved.height =>
            {
                (CuVerdict::Performed, "inside control rect")
            }
            Some((px, py)) => (
                CuVerdict::NotPerformed,
                if px == x && py == y {
                    "pointer matched the request but the control rect is stale"
                } else {
                    "pointer landed outside the control rect"
                },
            ),
            None => (CuVerdict::Unknown, "pointer readback could not be parsed"),
        };
        let verdict = verify_actuation(
            if press {
                Some((button.tag(), if double { 2 } else { 1 }, double, 0))
            } else {
                None
            },
            outcome.actuation.as_ref(),
            verdict,
        );
        let readback = format!(
            "{}; control={:?} role={} rect={},{} {}x{} {detail}",
            outcome.readback,
            resolved.label,
            resolved.role,
            resolved.x,
            resolved.y,
            resolved.width,
            resolved.height
        );
        let mut text = format!(
            "why: {why}\nscreen: act={} control={:?} press={press}\nreadback: {readback}",
            outcome.action, resolved.label
        );
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        text.push_str(&format!(
            "\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={} verdict={} capability={capability}",
            state.generation,
            verdict.tag()
        ));
        let generation = state.generation;
        let mut evidence = json!({
            "action": outcome.action,
            "control": resolved.label,
            "role": resolved.role,
            "rect": [resolved.x, resolved.y, resolved.width, resolved.height],
            "center": [x, y],
            "press": press,
            "readback": readback,
            "verdict": verdict.tag(),
            "generation": generation,
            "actuation": outcome.actuation.as_ref().map(actuation_json).unwrap_or(Value::Null),
            "scroll": outcome.scroll.as_ref().map(|axes| json!({
                "delta_x": axes.delta_x, "delta_y": axes.delta_y,
            })).unwrap_or(Value::Null),
            "target_hwnd": outcome.target_hwnd,
            "target_hwnd_reason": outcome.target_hwnd_reason.clone(),
            // 0.0.15: additive rows. Old calls publish source="controls" and
            // a11y_node=null; the reader recomputes state_names from `state`.
            "source": source.tag(),
            "a11y_node": resolved.a11y_node.clone().unwrap_or(Value::Null),
        });
        if source == ControlSource::A11y {
            evidence["a11y_node_schema"] = json!("role-name-state-value-bounds-depth");
        }
        self.record(
            &mut state,
            CuReceipt {
                generation,
                capability: capability.to_owned(),
                verdict,
                facts_as_of_ms: now_ms(),
                readback: readback.clone(),
            },
        );
        CuResult {
            verdict,
            capability: capability.to_owned(),
            generation,
            facts_as_of_ms: now_ms(),
            evidence,
            text,
        }
    }

    fn call_act(&self, action: &CuAction) -> Result<crate::screen::ActOutcome, String> {
        let act = match action {
            CuAction::Focus { target } => Act::Focus { target: target.clone() },
            CuAction::Move { title, x, y, width, height } => Act::Move {
                title: title.clone(),
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            },
            CuAction::Click { x, y, button, double } => Act::Click {
                x: *x,
                y: *y,
                button: *button,
                double: *double,
            },
            CuAction::Hover { x, y } => Act::Hover { x: *x, y: *y },
            CuAction::Drag { x, y, to_x, to_y, button, path } => Act::Drag {
                x: *x,
                y: *y,
                to_x: *to_x,
                to_y: *to_y,
                button: *button,
                path: path.clone(),
            },
            CuAction::Type { text } => Act::Type { text },
            CuAction::Key { keys, hold: false, release: false } => {
                Act::Key { keys: keys.clone() }
            }
            CuAction::Key { .. } => {
                return Err(
                    "screen: refused: held-key requests must go through the host keyboard state machine"
                        .to_owned(),
                )
            }
            CuAction::Scroll { x, y, delta_x, delta_y } => Act::Scroll {
                x: *x, y: *y, delta_x: *delta_x, delta_y: *delta_y,
            },
            // `invoke` routes named targets to `act_control`, which resolves
            // the frame first. Reaching here would mean a caller skipped that
            // admission step; refuse rather than invent coordinates.
            CuAction::Control { .. } => {
                return Err(
                    "screen: refused: a named control target must be resolved against the \
                     current observation"
                        .to_owned(),
                )
            }
        };
        self.tool.act(act)
    }

    fn disclose(&self, capability: &str, abi_minor: u32) -> CuResult {
        // The probes run before the host state is locked: a read-only probe
        // must never hold the receipt log hostage, and a malformed probe set is
        // a contract violation that publishes no partial disclosure at all.
        let environment = crate::screen::environment_probe();
        let grants = crate::screen::grant_probes();
        let scales = crate::screen::display_scale_probes();
        self.disclose_with(capability, abi_minor, &environment, grants, scales)
    }

    /// The disclosure proper, with the probe set supplied. Split out so the
    /// contract-violation path (a malformed probe set) is unit-testable without
    /// a backend that can be made to lie.
    fn disclose_with(
        &self,
        capability: &str,
        abi_minor: u32,
        environment: &EnvironmentProbe,
        grants: Result<Vec<GrantProbe>, String>,
        scales: Result<Vec<DisplayScaleProbe>, String>,
    ) -> CuResult {
        let grants = match grants {
            Ok(grants) => grants,
            Err(reason) => {
                let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                return self.result_with_state(
                    &mut state,
                    CuVerdict::NotPerformed,
                    capability,
                    reason,
                    json!({ "contract": "grants" }),
                );
            }
        };
        // 0.0.10: a malformed scale set is refused whole, exactly like a
        // malformed grant set. No partial `environment` is published.
        let scales = match scales {
            Ok(scales) => scales,
            Err(reason) => {
                let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                return self.result_with_state(
                    &mut state,
                    CuVerdict::NotPerformed,
                    capability,
                    reason,
                    json!({ "contract": "display_scales" }),
                );
            }
        };
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let mut evidence = self.manifest(&state, abi_minor);
        if let Value::Object(fields) = &mut evidence {
            // Additive evidence on the existing `disclose` capability: no new
            // capability row, no ABI change, and no 0.0.8 key is touched.
            fields.insert(
                "environment".to_owned(),
                environment_json(environment, &scales),
            );
            fields.insert("grants".to_owned(), grants_json(&grants));
            fields.insert(
                "wait_budget".to_owned(),
                json!({
                    "cumulative_ms": state.wait_cumulative_ms,
                    "wait_calls": state.wait_calls,
                    "max_cumulative_ms": SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS,
                    "max_wait_calls": SCREEN_TOOL_MAX_WAIT_CALLS,
                    "remaining_ms": SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS
                        .saturating_sub(state.wait_cumulative_ms),
                    "remaining_calls": SCREEN_TOOL_MAX_WAIT_CALLS
                        .saturating_sub(state.wait_calls),
                }),
            );
        }
        // Qualification comes from the native backend, never from a guess:
        // on an unqualified cell the disclosure says so.
        let mut text = format!(
            "cu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} platform={} qualified={} grant={} \
             generation={} capabilities={} receipts={} held_keys={}",
            crate::screen::platform_name(),
            crate::screen::platform_qualified(),
            if self.tool.enabled() { "granted" } else { "denied" },
            state.generation,
            CAPABILITIES.len(),
            state.receipts.len(),
            state.keyboard.tags().join(",")
        );
        for entry in CAPABILITIES {
            let params: Vec<String> = entry
                .params
                .iter()
                .map(|param| {
                    format!(
                        "{}{}:{}",
                        param.name,
                        if param.required { "!" } else { "?" },
                        param.ty.tag()
                    )
                })
                .collect();
            text.push_str(&format!(
                "\ncapability={} kind={} params={}",
                entry.name,
                entry.kind.tag(),
                params.join(",")
            ));
        }
        for (name, implemented) in crate::screen::platform_capabilities().rows() {
            text.push_str(&format!("\nplatform_verb={name} implemented={implemented}"));
        }
        text.push_str(&format!(
            "\nenvironment platform={} session={} displays={} desktop={},{} {}x{} foreground={}",
            environment.platform,
            environment.session.tag(),
            environment.display_count,
            environment.virtual_desktop.0,
            environment.virtual_desktop.1,
            environment.virtual_desktop.2,
            environment.virtual_desktop.3,
            environment.foreground_present
        ));
        for probe in &grants {
            text.push_str(&format!(
                "\ngrant name={} status={}{}",
                probe.name,
                probe.status.tag(),
                probe
                    .reason
                    .as_ref()
                    .map(|reason| format!(" reason={reason:?}"))
                    .unwrap_or_default()
            ));
        }
        for row in &scales {
            let (lx, ly, lw, lh) = row.logical_rect;
            let (px, py, pw, ph) = row.physical_rect;
            text.push_str(&format!(
                "\ndisplay_scale index={} logical={lx},{ly} {lw}x{lh} \
                 physical={px},{py} {pw}x{ph} scale={}{}",
                row.index,
                row.scale.tag(),
                row.reason
                    .as_ref()
                    .map(|reason| format!(" reason={reason:?}"))
                    .unwrap_or_default()
            ));
        }
        for receipt in state.receipts.iter().rev().take(CU_MAX_RECEIPTS) {
            text.push('\n');
            text.push_str(&receipt.to_line());
        }
        text.push_str(&format!(
            "\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={} verdict=performed \
             capability={capability}",
            state.generation
        ));
        CuResult {
            verdict: CuVerdict::Performed,
            capability: capability.to_owned(),
            generation: state.generation,
            facts_as_of_ms: now_ms(),
            evidence,
            text,
        }
    }

    fn manifest(&self, state: &CuState, abi_minor: u32) -> Value {
        let capabilities: Vec<Value> = CAPABILITIES
            .iter()
            .map(|entry| {
                let params: Vec<Value> = entry
                    .params
                    .iter()
                    .map(|param| {
                        json!({
                            "name": param.name, "type": param.ty.tag(),
                            "required": param.required,
                        })
                    })
                    .collect();
                json!({
                    "name": entry.name, "kind": entry.kind.tag(),
                    "summary": entry.summary, "params": params,
                })
            })
            .collect();
        let receipts: Vec<Value> = state.receipts.iter().rev().map(CuReceipt::to_json).collect();
        json!({
            "abi": { "major": CU_ABI_MAJOR, "minor": CU_ABI_MINOR },
            "caller_minor": abi_minor,
            "minor_degraded": abi_minor > CU_ABI_MINOR,
            "qualified": crate::screen::platform_qualified(),
            "platform": crate::screen::platform_name(),
            "platform_capabilities": capability_matrix(&crate::screen::platform_capabilities()),
            "grant": if self.tool.enabled() { "granted" } else { "denied" },
            "held_keys": state.keyboard.tags(),
            "generation": state.generation,
            "frame": state.frame.as_ref().map(CuFrame::to_json),
            "capabilities": capabilities,
            "receipts": receipts,
        })
    }

    fn effect_failure(&self, capability: &str, verdict: CuVerdict, reason: String) -> CuResult {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let generation = state.generation;
        let text = format!(
            "{reason}\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={generation} verdict={} \
             capability={capability}",
            verdict.tag()
        );
        let evidence = json!({
            "verdict": verdict.tag(), "reason": reason, "generation": generation,
        });
        self.record(
            &mut state,
            CuReceipt {
                generation,
                capability: capability.to_owned(),
                verdict,
                facts_as_of_ms: now_ms(),
                readback: reason.clone(),
            },
        );
        CuResult {
            verdict,
            capability: capability.to_owned(),
            generation,
            facts_as_of_ms: now_ms(),
            evidence,
            text,
        }
    }

    fn result_with_state(
        &self,
        state: &mut CuState,
        verdict: CuVerdict,
        capability: &str,
        reason: String,
        mut evidence: Value,
    ) -> CuResult {
        // Every admission refusal is evidence a typed caller can branch on;
        // the verdict must be in the JSON as well as in the text, not just
        // implied by which field happened to be present.
        if let Value::Object(fields) = &mut evidence {
            fields.insert("verdict".to_owned(), json!(verdict.tag()));
        }
        let generation = state.generation;
        let mut text = reason.clone();
        text.push_str(&format!(
            "\ncu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} generation={generation} verdict={} \
             capability={capability}",
            verdict.tag()
        ));
        self.record(
            state,
            CuReceipt {
                generation,
                capability: capability.to_owned(),
                verdict,
                facts_as_of_ms: now_ms(),
                readback: reason,
            },
        );
        CuResult {
            verdict,
            capability: capability.to_owned(),
            generation,
            facts_as_of_ms: now_ms(),
            evidence,
            text,
        }
    }

    fn record(&self, state: &mut CuState, receipt: CuReceipt) {
        state.receipts.push(receipt);
        if state.receipts.len() > CU_MAX_RECEIPTS {
            let excess = state.receipts.len() - CU_MAX_RECEIPTS;
            state.receipts.drain(..excess);
        }
    }
}

impl Drop for CuHost {
    fn drop(&mut self) {
        self.release_on_teardown();
    }
}

/// `place_pointer`/`click` report `pointer at x,y`. Pure and tested on every
/// cell: a named target's evidence is the desktop's own coordinates, not the
/// request that was sent to it.
fn pointer_from_readback(readback: &str) -> Option<(i32, i32)> {
    let pair = readback.strip_prefix("pointer at ")?;
    let (x, y) = pair.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// The button dispatch a request promises, or `None` when that action posts
/// no button event. Used only to *check* the native receipt, never to create
/// one: a receipt must come from the backend that posted the events.
fn expected_dispatch(action: &CuAction) -> Option<(&'static str, u32, bool, u32)> {
    match action {
        CuAction::Click { button, double, .. } => {
            Some((button.tag(), if *double { 2 } else { 1 }, *double, 0))
        }
        CuAction::Drag { button, path, .. } => Some((
            button.tag(),
            1,
            false,
            path.as_ref().map_or(1, |points| points.len() as u32),
        )),
        _ => None,
    }
}

/// A performed button action without a matching native receipt is not a
/// performed interaction. Missing evidence is unconfirmed (`unknown`); a
/// receipt that disagrees with the request is `not_performed`. A non-button
/// action, or a call that already failed its readback, keeps its verdict.
fn verify_actuation(
    expected: Option<(&'static str, u32, bool, u32)>,
    actual: Option<&Actuation>,
    base: CuVerdict,
) -> CuVerdict {
    let (button, downs, double, path_points) = match expected {
        Some(expected) => expected,
        None => return base,
    };
    if base != CuVerdict::Performed {
        return base;
    }
    match actual {
        None => CuVerdict::Unknown,
        Some(receipt)
            if receipt.button != button
                || receipt.downs != downs
                || receipt.ups != downs
                || receipt.ups != receipt.downs
                || receipt.double != double
                || receipt.path_points != path_points
                || receipt.target.trim().is_empty() =>
        {
            CuVerdict::NotPerformed
        }
        Some(_) => base,
    }
}

fn actuation_json(receipt: &Actuation) -> Value {
    json!({
        "button": receipt.button,
        "downs": receipt.downs,
        "ups": receipt.ups,
        "double": receipt.double,
        "target": receipt.target,
        "path_points": receipt.path_points,
    })
}

fn classify(error: String) -> (CuVerdict, String) {
    if error.starts_with("screen: failed:") {
        (CuVerdict::NotPerformed, error)
    } else if error.starts_with("screen: stale:") {
        (CuVerdict::Stale, error)
    } else if error.starts_with("screen: refused:") {
        (CuVerdict::Refused, error)
    } else if error.starts_with("cu: unsupported:") {
        // A backend that cannot perform an effect (missing library, missing
        // OS grant, missing mechanism) reports a named unsupported refusal.
        // It is not an unconfirmed outcome: no effect happened, by contract.
        (CuVerdict::Unsupported, error)
    } else {
        // Anything else is an unconfirmed native outcome. Never report a
        // success for it, and never replay it.
        (CuVerdict::Unknown, error)
    }
}

/// One integer wheel axis. A missing or null axis is zero; anything else must
/// be an `i32`, never a truncated `i64` or a float.
fn scroll_axis(name: &str, value: Option<&Value>) -> Result<i32, String> {
    match value {
        None | Some(Value::Null) => Ok(0),
        Some(other) => other
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .ok_or_else(|| {
                format!("screen: refused: `{name}` must be a 32-bit integer, got {other}")
            }),
    }
}

/// A `path[]` is admitted as a whole before any pointer or button effect. The
/// desktop-geometry half of the admission lives in `ScreenTool::act`, which
/// owns the native `virtual_screen()` reading.
fn parse_drag_path(arguments: &Value) -> Result<Option<Vec<(i32, i32)>>, String> {
    let Some(value) = arguments.get("path") else {
        return Ok(None);
    };
    let array = value.as_array().ok_or_else(|| {
        format!(
            "screen: refused: `path` must be an array of [x,y] integer pairs, got {value}"
        )
    })?;
    if array.len() < 2 {
        return Err(format!(
            "screen: refused: a `path` needs at least 2 points, got {}",
            array.len()
        ));
    }
    let bound = crate::screen::SCREEN_TOOL_MAX_DRAG_PATH_POINTS;
    if array.len() > bound {
        return Err(format!(
            "screen: refused: a `path` may have at most {bound} points, got {}",
            array.len()
        ));
    }
    let mut points = Vec::with_capacity(array.len());
    for (index, point) in array.iter().enumerate() {
        let pair = point.as_array().filter(|pair| pair.len() == 2).ok_or_else(|| {
            format!(
                "screen: refused: `path[{index}]` must be a two-integer [x,y] pair, got {point}"
            )
        })?;
        let mut xy = [0i32; 2];
        for (axis, component) in pair.iter().enumerate() {
            xy[axis] = component
                .as_i64()
                .and_then(|number| i32::try_from(number).ok())
                .ok_or_else(|| {
                    format!(
                        "screen: refused: `path[{index}][{axis}]` must be a 32-bit integer, \
                         got {component}"
                    )
                })?;
        }
        points.push((xy[0], xy[1]));
    }
    Ok(Some(points))
}

/// JSON adapter: the wire codec is the only place raw `Value` enters. Every
/// refusal here names the missing or mistyped field, so a model can correct
/// the call instead of probing blindly.
/// A typed boolean flag: absent or null is false, anything else but a bool
/// is a named refusal before any native effect.
fn bool_flag(name: &str, value: Option<&Value>) -> Result<bool, String> {
    match value {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(other) => Err(format!(
            "screen: refused: {name} must be a boolean, got {other}"
        )),
    }
}

/// 0.0.16: admission only. The `expect` whitelist is validated at dispatch
/// (not here) so even an invalid expectation publishes a named empty effect
/// row instead of a bare parse error that could be mistaken for success.
fn parse_effect_request(value: &Value) -> Result<EffectRequest, String> {
    let object = value.as_object().ok_or_else(|| {
        "screen: refused: `effect` must be an object with hwnd, name and expect".to_owned()
    })?;
    let hwnd = object
        .get("hwnd")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "screen: refused: `effect.hwnd` must be a live window handle".to_owned()
        })?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "screen: refused: `effect.name` must name one a11y node".to_owned())?
        .to_owned();
    let role = match object.get("role") {
        None | Some(Value::Null) => None,
        Some(Value::String(role)) if !role.trim().is_empty() => Some(role.clone()),
        Some(_) => {
            return Err(
                "screen: refused: `effect.role` must be a non-empty role name or null".to_owned(),
            )
        }
    };
    let child_id = match object.get("child_id") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_i64()
                .map(|id| id as i32)
                .ok_or_else(|| {
                    "screen: refused: `effect.child_id` must be an integer or null".to_owned()
                })?,
        ),
    };
    let expect = match object.get("expect") {
        Some(Value::Array(rows)) => rows
            .iter()
            .map(|row| {
                row.as_str().map(str::to_owned).ok_or_else(|| {
                    "screen: refused: `effect.expect` must be an array of field names".to_owned()
                })
            })
            .collect::<Result<Vec<String>, String>>()?,
        Some(_) => {
            return Err("screen: refused: `effect.expect` must be an array".to_owned())
        }
        None => {
            return Err(
                "screen: refused: `effect` needs a pre-declared `expect` array; use [] to \
                 predict no change"
                    .to_owned(),
            )
        }
    };
    Ok(EffectRequest { hwnd, name, role, child_id, expect })
}

pub fn parse_request(arguments: &Value) -> Result<CuRequest, String> {
    let abi_major = match arguments.get("abi_major") {
        None => CU_ABI_MAJOR,
        Some(value) => value.as_u64().map(|v| v as u32).ok_or_else(|| {
            "screen: refused: abi_major must be a non-negative integer".to_owned()
        })?,
    };
    let abi_minor = match arguments.get("abi_minor") {
        None => 0,
        Some(value) => value.as_u64().map(|v| v as u32).ok_or_else(|| {
            "screen: refused: abi_minor must be a non-negative integer".to_owned()
        })?,
    };
    let as_of = match arguments.get("as_of") {
        None => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| {
            "screen: refused: as_of must be the generation returned by observe".to_owned()
        })?),
    };
    let op = arguments
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "screen: refused: no `op` argument; use \"observe\", \"act\" or \"disclose\""
                .to_owned()
        })?;
    let call = match op {
        "observe" => {
            let window = arguments
                .get("window")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned);
            let capture = match arguments.get("capture") {
                Some(Value::Bool(true)) => Some(CaptureTarget::Desktop),
                Some(Value::String(title)) if !title.trim().is_empty() => {
                    Some(CaptureTarget::Window(title.clone()))
                }
                Some(Value::Object(region)) => {
                    let rect_number = |name: &str| -> Result<i32, String> {
                        region
                            .get(name)
                            .and_then(Value::as_i64)
                            .map(|value| value as i32)
                            .ok_or_else(|| {
                                format!(
                                    "screen: refused: a capture region needs an integer {name}"
                                )
                            })
                    };
                    Some(CaptureTarget::Region {
                        x: rect_number("x")?,
                        y: rect_number("y")?,
                        width: rect_number("width")?,
                        height: rect_number("height")?,
                    })
                }
                _ => None,
            };
            let controls = match arguments.get("controls") {
                Some(Value::Bool(true)) => Some(ControlTarget::Foreground),
                Some(Value::String(title)) if !title.trim().is_empty() => {
                    Some(ControlTarget::Window(title.clone()))
                }
                Some(Value::Bool(false)) | Some(Value::Null) | None => None,
                Some(other) => {
                    return Err(format!(
                        "screen: refused: `controls` must be true (foreground window) or a \
                         window title, got {other}"
                    ))
                }
            };
            // 0.0.11: a structured accessibility scan. `true` follows the
            // foreground window, an integer names a live hwnd from a prior
            // observe (the only unambiguous identity when titles collide).
            let a11y = match arguments.get("a11y") {
                Some(Value::Bool(true)) => Some(A11yTarget::Foreground),
                Some(value) if value.as_u64().is_some() => {
                    Some(A11yTarget::Handle(value.as_u64().unwrap_or(0)))
                }
                Some(Value::Bool(false)) | Some(Value::Null) | None => None,
                Some(other) => {
                    return Err(format!(
                        "screen: refused: `a11y` must be true or a live window hwnd, got {other}"
                    ))
                }
            };
            let scale = match arguments.get("scale") {
                None => 0,
                Some(value) => value
                    .as_u64()
                    .filter(|scale| *scale <= SCREEN_TOOL_MAX_CAPTURE_SCALE as u64)
                    .map(|scale| scale as u32)
                    .ok_or_else(|| {
                        format!(
                            "screen: refused: scale must be an integer 0..={}, 0 chooses \
                             automatically",
                            SCREEN_TOOL_MAX_CAPTURE_SCALE
                        )
                    })?,
            };
            let wait_ms = match arguments.get("wait_ms") {
                None => None,
                Some(value) => Some(
                    value
                        .as_u64()
                        .filter(|ms| *ms >= 1 && *ms <= SCREEN_TOOL_MAX_WAIT_MS as u64)
                        .map(|ms| ms as u32)
                        .ok_or_else(|| {
                            format!(
                                "screen: refused: wait_ms must be an integer 1..={}, no sleep \
                                 was performed",
                                SCREEN_TOOL_MAX_WAIT_MS
                            )
                        })?,
                ),
            };
            // 0.0.16: an effect is additive evidence on `observe`, not a new
            // verb. The expectation is declared here, before any act.
            let effect = match arguments.get("effect") {
                None | Some(Value::Null) => None,
                Some(value) => Some(parse_effect_request(value)?),
            };
            CuCall::Observe { window, capture, controls, a11y, scale, wait_ms, effect }
        }
        "act" => {
            let why = arguments
                .get("why")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    "screen: refused: the `why` argument is required: one short line saying \
                     what this operation is for"
                        .to_owned()
                })?
                .to_owned();
            let action = arguments.get("action").and_then(Value::as_str).ok_or_else(|| {
                "screen: refused: act needs an `action` (focus, move, click, hover, drag, type, \
                 key, scroll, control)"
                    .to_owned()
            })?;
            let number = |name: &str| -> Result<i32, String> {
                arguments
                    .get(name)
                    .and_then(Value::as_i64)
                    .map(|value| value as i32)
                    .ok_or_else(|| {
                        format!("screen: refused: action {action:?} needs an integer {name}")
                    })
            };
            let optional_number = |name: &str| -> Result<i32, String> {
                scroll_axis(name, arguments.get(name))
            };
            let title = || -> Result<String, String> {
                arguments
                    .get("window")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        format!("screen: refused: action {action:?} needs a `window` title")
                    })
            };
            let focus_target = || -> Result<FocusTarget, String> {
                let title = arguments.get("window").filter(|value| !value.is_null());
                let hwnd = arguments.get("hwnd").filter(|value| !value.is_null());
                match (title, hwnd) {
                    (Some(value), None) => value
                        .as_str()
                        .map(|text| FocusTarget::Title(text.to_owned()))
                        .ok_or_else(|| {
                            "screen: refused: focus `window` must be a string title".to_owned()
                        }),
                    (None, Some(value)) => value
                        .as_u64()
                        .filter(|handle| *handle != 0)
                        .map(FocusTarget::Handle)
                        .ok_or_else(|| {
                            "screen: refused: focus `hwnd` must be a positive integer from a \
                             prior observe"
                                .to_owned()
                        }),
                    (Some(_), Some(_)) => Err(
                        "screen: refused: focus needs exactly one of `window` or `hwnd`; giving \
                         both is ambiguous"
                            .to_owned(),
                    ),
                    (None, None) => Err(
                        "screen: refused: focus needs exactly one of `window` or `hwnd`"
                            .to_owned(),
                    ),
                }
            };
            let button = || match arguments.get("button").and_then(Value::as_str) {
                Some("right") => ClickButton::Right,
                Some("middle") => ClickButton::Middle,
                _ => ClickButton::Left,
            };
            let act = match action {
                "focus" => CuAction::Focus { target: focus_target()? },
                "move" => CuAction::Move {
                    title: title()?,
                    x: number("x")?,
                    y: number("y")?,
                    width: number("width")?,
                    height: number("height")?,
                },
                "click" => CuAction::Click {
                    x: number("x")?,
                    y: number("y")?,
                    button: button(),
                    double: arguments
                        .get("double")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
                "hover" => CuAction::Hover {
                    x: number("x")?,
                    y: number("y")?,
                },
                "drag" => {
                    let path = parse_drag_path(arguments)?;
                    // A path replaces the single destination. Without one,
                    // 0.0.6's typed refusal for a missing `to_x`/`to_y` stays.
                    let (to_x, to_y) = if path.is_some() {
                        (optional_number("to_x")?, optional_number("to_y")?)
                    } else {
                        (number("to_x")?, number("to_y")?)
                    };
                    CuAction::Drag {
                        x: number("x")?,
                        y: number("y")?,
                        to_x,
                        to_y,
                        button: button(),
                        path,
                    }
                }
                "type" => CuAction::Type {
                    text: arguments
                        .get("text")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            "screen: refused: action \"type\" needs a `text` argument".to_owned()
                        })?
                        .to_owned(),
                },
                "key" => {
                    let hold = bool_flag("hold", arguments.get("hold"))?;
                    let release = bool_flag("release", arguments.get("release"))?;
                    if hold && release {
                        return Err(
                            "screen: refused: `hold` and `release` cannot be supplied in one call; they are opposite keyboard effects"
                                .to_owned(),
                        );
                    }
                    let keys = match arguments.get("keys") {
                        Some(value) => value.as_str().ok_or_else(|| {
                            "screen: refused: action \"key\" needs a string `keys` argument"
                                .to_owned()
                        })?.to_owned(),
                        None if release => String::new(),
                        None => {
                            return Err(
                                "screen: refused: action \"key\" needs a `keys` argument, e.g. \
                                 \"ctrl+s\" or \"win+left\""
                                    .to_owned(),
                            )
                        }
                    };
                    if hold
                        && (keys.contains('+') || Modifier::parse(keys.trim()).is_none())
                    {
                        return Err(format!(
                            "screen: refused: `hold` accepts exactly one whitelisted modifier ({}); got {keys:?}",
                            SCREEN_TOOL_HELD_MODIFIERS.join(", ")
                        ));
                    }
                    if release && keys.contains('+') {
                        return Err(
                            "screen: refused: `release` takes no key combination; it releases every held modifier"
                                .to_owned(),
                        );
                    }
                    CuAction::Key { keys, hold, release }
                }
                "scroll" => {
                    let alias = arguments.get("delta").filter(|value| !value.is_null());
                    let explicit_y = arguments.get("delta_y").filter(|value| !value.is_null());
                    if alias.is_some() && explicit_y.is_some() {
                        return Err(
                            "screen: refused: `delta` and `delta_y` cannot both be supplied; \
                             `delta` is the vertical alias"
                                .to_owned(),
                        );
                    }
                    let delta_x = scroll_axis("delta_x", arguments.get("delta_x"))?;
                    let delta_y = if explicit_y.is_some() {
                        scroll_axis("delta_y", arguments.get("delta_y"))?
                    } else {
                        scroll_axis("delta", arguments.get("delta"))?
                    };
                    crate::screen::check_scroll_axes(delta_x, delta_y)?;
                    CuAction::Scroll {
                        x: number("x")?,
                        y: number("y")?,
                        delta_x,
                        delta_y,
                    }
                }
                "control" => {
                    // 0.0.15: `source` and `role` are admitted here, before any
                    // pointer exists. A mistyped or misapplied parameter is a
                    // named refusal, never a silent fallback to coordinates.
                    let source = match arguments.get("source") {
                        None => ControlSource::Controls,
                        Some(Value::String(value)) => ControlSource::parse(Some(value))?,
                        Some(_) => {
                            return Err(
                                "screen: refused: `source` must be a string: \"controls\" \
                                 or \"a11y\""
                                    .to_owned(),
                            )
                        }
                    };
                    let role = match arguments.get("role") {
                        None => None,
                        Some(Value::String(value)) if !value.trim().is_empty() => {
                            Some(value.to_owned())
                        }
                        Some(Value::String(_)) => {
                            return Err(
                                "screen: refused: `role` must name a non-empty a11y role"
                                    .to_owned(),
                            )
                        }
                        Some(_) => {
                            return Err(
                                "screen: refused: `role` must be a string naming an a11y role"
                                    .to_owned(),
                            )
                        }
                    };
                    if role.is_some() && source != ControlSource::A11y {
                        return Err(
                            "screen: refused: `role` applies only to source=\"a11y\"; \
                             the operation was not performed"
                                .to_owned(),
                        );
                    }
                    CuAction::Control {
                        control: arguments
                            .get("control")
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .ok_or_else(|| {
                                "screen: refused: action \"control\" needs a `control` name \
                                 from the last observe"
                                    .to_owned()
                            })?
                            .to_owned(),
                        // Hover is the default: resolving a named target must
                        // not be able to click a button just because a caller
                        // omitted a flag.
                        press: arguments
                            .get("press")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        button: button(),
                        double: arguments
                            .get("double")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        source,
                        role,
                    }
                }
                other => {
                    return Err(format!(
                        "screen: refused: unknown action {other:?}; act accepts focus, move, \
                         click, hover, drag, type, key, scroll, control"
                    ))
                }
            };
            // The type bound belongs to admission, not to the keystroke loop:
            // refuse before the desktop is touched at all.
            if let CuAction::Type { text } = &act {
                if text.chars().count() > SCREEN_TOOL_MAX_TYPE_CHARS {
                    return Err(format!(
                        "screen: refused: {} characters is over the type bound of \
                         {SCREEN_TOOL_MAX_TYPE_CHARS}; split the text",
                        text.chars().count()
                    ));
                }
            }
            CuCall::Act { why, action: act }
        }
        "disclose" => CuCall::Disclose,
        other => {
            return Err(format!(
                "screen: refused: op must be \"observe\", \"act\" or \"disclose\", got {other:?}"
            ))
        }
    };
    Ok(CuRequest { abi_major, abi_minor, as_of, call })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{ControlInfo, ScreenSnapshot, ScreenTool};

    fn host(granted: bool) -> CuHost {
        CuHost::new(ScreenTool::new(std::path::Path::new("."), granted))
    }

    fn request(call: CuCall) -> CuRequest {
        CuRequest {
            abi_major: CU_ABI_MAJOR,
            abi_minor: 0,
            as_of: None,
            call,
        }
    }

    #[test]
    fn platform_capability_matrix_is_disclosed_and_admits_by_name() {
        let caps = crate::screen::platform_capabilities();
        let rows = caps.rows();
        // CI runs this test on linux, macOS and Windows; the printed matrix is
        // the per-OS evidence that the compiled backend declared its own verbs.
        eprintln!(
            "cu platform={} qualified={} matrix={:?}",
            crate::screen::platform_name(),
            caps.any_effect_implemented(),
            rows
        );
        assert_eq!(rows.len(), 12, "one row per implemented-or-not verb");
        for entry in CAPABILITIES {
            if entry.kind == CuKind::Disclose {
                continue;
            }
            let row = rows
                .iter()
                .find(|(name, _)| *name == entry.name)
                .unwrap_or_else(|| panic!("{} has no platform matrix row", entry.name));
            assert_eq!(
                caps.supports(entry.name),
                row.1,
                "{} admission disagrees with the disclosed matrix",
                entry.name
            );
        }
        assert!(caps.supports("disclose"), "disclosure is always available");
        assert!(!caps.supports("act:invented"));

        let disclosed = host(false).invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(disclosed.verdict, CuVerdict::Performed);
        assert_eq!(
            disclosed.evidence["platform"],
            json!(crate::screen::platform_name())
        );
        assert_eq!(
            disclosed.evidence["qualified"],
            json!(caps.any_effect_implemented())
        );
        assert_eq!(
            disclosed.evidence["platform_capabilities"].as_array().unwrap().len(),
            rows.len()
        );
        assert!(
            disclosed.text.contains("platform_verb=observe implemented="),
            "{}",
            disclosed.text
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn a_cell_with_no_backend_refuses_by_naming_its_own_os() {
        // The observe `controls` arm is a named refusal on every non-Windows
        // cell: it cannot reach a desktop even where other verbs are real.
        let caps = crate::screen::platform_capabilities();
        let result = host(true).invoke_json(&json!({ "op": "observe", "controls": true }));
        assert!(
            matches!(result.verdict, CuVerdict::Refused | CuVerdict::Unsupported),
            "{}",
            result.text
        );
        assert!(
            result.text.contains(crate::screen::platform_name()),
            "{}",
            result.text
        );
        assert!(!result.text.contains("Windows only"), "{}", result.text);
        assert!(!caps.supports("act:control"));
        assert_eq!(
            result.evidence["platform_capabilities"].as_array().unwrap().len(),
            12
        );
    }

    /// A host with a synthetic frame, so every named-target refusal can be
    /// tested without driving the real pointer. Only the ignored real-desktop
    /// journey performs the effect.
    fn host_with_frame(
        controls: Vec<ControlInfo>,
        frame_generation: u64,
        current_generation: u64,
        desktop: (i32, i32, i32, i32),
    ) -> CuHost {
        let host = host(true);
        let snapshot = ScreenSnapshot {
            platform: "windows".to_owned(),
            qualified: true,
            desktop,
            displays: Vec::new(),
            display_scales: Vec::new(),
            windows: Vec::new(),
            controls_source: Some("window:synthetic".to_owned()),
            controls_truncated: false,
            controls,
            a11y_controls: Vec::new(),
            a11y_source: None,
            a11y_reason: None,
            a11y_truncated: false,
            capture: None,
        };
        let mut state = host.state.lock().unwrap();
        state.generation = current_generation;
        state.frame = Some(CuFrame {
            generation: frame_generation,
            facts_as_of_ms: 0,
            snapshot,
        });
        drop(state);
        host
    }

    /// 0.0.15: a frame carrying a normalised a11y tree instead of Win32 rows.
    fn host_with_a11y_frame(
        a11y_controls: Vec<crate::screen::A11yControl>,
        frame_generation: u64,
        current_generation: u64,
        desktop: (i32, i32, i32, i32),
    ) -> CuHost {
        let host = host(true);
        let snapshot = ScreenSnapshot {
            platform: "windows".to_owned(),
            qualified: true,
            desktop,
            displays: Vec::new(),
            display_scales: Vec::new(),
            windows: Vec::new(),
            controls_source: None,
            controls_truncated: false,
            controls: Vec::new(),
            a11y_controls,
            a11y_source: Some("handle:33689578 window:\"WorkBuddy\"".to_owned()),
            a11y_reason: None,
            a11y_truncated: false,
            capture: None,
        };
        let mut state = host.state.lock().unwrap();
        state.generation = current_generation;
        state.frame = Some(CuFrame {
            generation: frame_generation,
            facts_as_of_ms: 0,
            snapshot,
        });
        drop(state);
        host
    }

    fn synthetic_a11y_node(
        name: &str,
        role: i32,
        state: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> crate::screen::A11yControl {
        crate::screen::A11yControl {
            name: name.to_owned(),
            value: String::new(),
            role,
            role_name: crate::screen::a11y_role_name(role),
            state,
            x,
            y,
            width,
            height,
            depth: 6,
            child_id: 0,
        }
    }

    fn synthetic_control(label: &str, x: i32, y: i32, width: i32, height: i32) -> ControlInfo {
        ControlInfo {
            handle: 7,
            parent: 1,
            label: label.to_owned(),
            class: "Button".to_owned(),
            role: "button",
            text: label.to_owned(),
            x,
            y,
            width,
            height,
            visible: true,
            enabled: true,
        }
    }

    #[test]
    fn a11y_role_names_are_a_fixed_table_not_a_guess() {
        use crate::screen::a11y_role_name;
        assert_eq!(a11y_role_name(42), "textbox");
        assert_eq!(a11y_role_name(43), "button");
        assert_eq!(a11y_role_name(41), "label");
        assert_eq!(a11y_role_name(20), "group");
        assert_eq!(a11y_role_name(9999), "unknown");
    }

    #[test]
    fn observe_parses_a11y_foreground_and_handle_and_rejects_junk() {
        let foreground = parse_request(&json!({ "op": "observe", "a11y": true })).unwrap();
        assert!(matches!(
            foreground.call,
            CuCall::Observe { a11y: Some(A11yTarget::Foreground), .. }
        ));
        let handle = parse_request(&json!({ "op": "observe", "a11y": 59771304 })).unwrap();
        assert!(matches!(
            handle.call,
            CuCall::Observe { a11y: Some(A11yTarget::Handle(59771304)), .. }
        ));
        assert!(parse_request(&json!({ "op": "observe", "a11y": "WorkBuddy" })).is_err());
        assert!(parse_request(&json!({ "op": "observe", "a11y": -1 })).is_err());
        assert!(matches!(
            parse_request(&json!({ "op": "observe" })).unwrap().call,
            CuCall::Observe { a11y: None, .. }
        ));
    }

    #[test]
    fn a11y_frame_publishes_its_basis_source_and_reason() {
        let snapshot = ScreenSnapshot {
            platform: "windows".to_owned(),
            qualified: true,
            desktop: (0, 0, 200, 150),
            displays: Vec::new(),
            display_scales: Vec::new(),
            windows: Vec::new(),
            controls: Vec::new(),
            controls_source: None,
            controls_truncated: false,
            a11y_controls: Vec::new(),
            a11y_source: Some("handle:7".to_owned()),
            a11y_reason: Some("no a11y tree on this cell".to_owned()),
            a11y_truncated: false,
            capture: None,
        };
        let evidence = CuFrame { generation: 1, facts_as_of_ms: 0, snapshot }.to_json();
        assert_eq!(evidence["a11y_role_basis"], "msaa-role");
        assert_eq!(evidence["a11y_source"], "handle:7");
        assert_eq!(evidence["a11y_reason"], "no a11y tree on this cell");
        assert!(evidence["a11y_controls"].as_array().unwrap().is_empty());
    }

    #[test]
    fn pointer_readback_is_parsed_as_evidence_not_as_a_request() {
        assert_eq!(pointer_from_readback("pointer at 12,34"), Some((12, 34)));
        assert_eq!(pointer_from_readback("pointer at -5, 7"), Some((-5, 7)));
        assert_eq!(pointer_from_readback("pointer at nope"), None);
        assert_eq!(pointer_from_readback("clicked"), None);
    }

    #[test]
    fn observe_parses_a_control_scan_and_rejects_mistyped_ones() {
        let foreground = parse_request(&json!({ "op": "observe", "controls": true })).unwrap();
        assert_eq!(
            foreground.call,
            CuCall::Observe {
                window: None,
                capture: None,
                controls: Some(ControlTarget::Foreground),
                a11y: None,
                scale: 0,
                wait_ms: None,
                effect: None,
            }
        );
        let named = parse_request(&json!({ "op": "observe", "controls": "Notepad" })).unwrap();
        assert!(matches!(
            named.call,
            CuCall::Observe { controls: Some(ControlTarget::Window(ref title)), .. }
                if title == "Notepad"
        ));
        let plain = parse_request(&json!({ "op": "observe" })).unwrap();
        assert!(matches!(plain.call, CuCall::Observe { controls: None, .. }));
        let mistyped = parse_request(&json!({ "op": "observe", "controls": 42 })).unwrap_err();
        assert!(mistyped.contains("must be true"), "{mistyped}");
    }

    #[test]
    fn a_named_target_is_hover_by_default_and_needs_a_name() {
        let parsed = parse_request(&json!({
            "op": "act", "why": "find the button", "action": "control", "control": "OK"
        }))
        .unwrap();
        assert!(matches!(
            parsed.call,
            CuCall::Act {
                action: CuAction::Control { ref control, press: false, .. },
                ..
            } if control == "OK"
        ));
        let clicked = parse_request(&json!({
            "op": "act", "why": "press it", "action": "control",
            "control": "OK", "press": true, "button": "right"
        }))
        .unwrap();
        assert!(matches!(
            clicked.call,
            CuCall::Act {
                action: CuAction::Control {
                    press: true, button: ClickButton::Right, ..
                },
                ..
            }
        ));
        let missing = parse_request(&json!({
            "op": "act", "why": "x", "action": "control"
        }))
        .unwrap_err();
        assert!(missing.contains("needs a `control` name"), "{missing}");
    }

    #[test]
    fn a_named_target_refuses_without_an_observation() {
        let host = host(true);
        let result = host.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "OK"
        }));
        // A cell without an `act:control` backend admits nothing. A cell that
        // implements other verbs answers `unsupported`; a cell with no effect
        // at all answers `refused` and reports qualified=false. Both name the
        // OS and publish the matrix -- see PRD 01.06.
        let caps = crate::screen::platform_capabilities();
        if !caps.supports("act:control") {
            assert!(result.text.contains(crate::screen::platform_name()), "{}", result.text);
            if caps.any_effect_implemented() {
                assert_eq!(result.verdict, CuVerdict::Unsupported);
                assert_eq!(result.evidence["platform_capabilities"].as_array().unwrap().len(), 12);
            } else {
                assert_eq!(result.verdict, CuVerdict::Refused);
                assert_eq!(result.evidence["qualified"], json!(false));
            }
            return;
        }
        assert_eq!(result.verdict, CuVerdict::Refused);
        assert!(result.text.contains("no observation yet"), "{}", result.text);
        assert_eq!(result.evidence["reason"], "no_frame");
    }

    #[test]
    fn a_named_target_never_guesses_ambiguous_stale_or_unusable_controls() {
        let caps = crate::screen::platform_capabilities();
        if !caps.supports("act:control") {
            let refused = host_with_frame(Vec::new(), 1, 1, (0, 0, 800, 600)).invoke_json(
                &json!({ "op": "act", "why": "x", "action": "control", "control": "OK" }),
            );
            assert!(refused.text.contains(crate::screen::platform_name()), "{}", refused.text);
            if caps.any_effect_implemented() {
                assert_eq!(refused.verdict, CuVerdict::Unsupported);
            } else {
                assert_eq!(refused.verdict, CuVerdict::Refused);
            }
            assert_eq!(refused.evidence["platform_capabilities"].as_array().unwrap().len(), 12);
            return;
        }
        let empty = host_with_frame(Vec::new(), 1, 1, (0, 0, 800, 600));
        let no_controls = empty.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "OK"
        }));
        assert_eq!(no_controls.verdict, CuVerdict::Refused);
        assert!(no_controls.text.contains("no controls"), "{}", no_controls.text);

        let stale = host_with_frame(
            vec![synthetic_control("button:OK", 10, 10, 40, 20)],
            1,
            2,
            (0, 0, 800, 600),
        );
        let stale_result = stale.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "button:OK"
        }));
        assert_eq!(stale_result.verdict, CuVerdict::Stale, "{}", stale_result.text);

        let ambiguous = host_with_frame(
            vec![
                synthetic_control("button:OK", 10, 10, 40, 20),
                synthetic_control("button:OK again", 60, 10, 40, 20),
            ],
            1,
            1,
            (0, 0, 800, 600),
        );
        let ambiguous_result = ambiguous.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "OK"
        }));
        assert_eq!(ambiguous_result.verdict, CuVerdict::Refused);
        assert!(ambiguous_result.text.contains("matches 2 controls"), "{}", ambiguous_result.text);

        let mut hidden = synthetic_control("button:OK", 10, 10, 40, 20);
        hidden.visible = false;
        let hidden_host = host_with_frame(vec![hidden], 1, 1, (0, 0, 800, 600));
        let hidden_result = hidden_host.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "button:OK"
        }));
        assert_eq!(hidden_result.verdict, CuVerdict::Refused);
        assert!(hidden_result.text.contains("hidden"), "{}", hidden_result.text);

        // A control whose centre is outside the virtual desktop is refused
        // before the OS can clamp the pointer to a different point.
        let offscreen = host_with_frame(
            vec![synthetic_control("button:OK", 900, 10, 40, 20)],
            1,
            1,
            (0, 0, 800, 600),
        );
        let offscreen_result = offscreen.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "button:OK"
        }));
        assert_eq!(offscreen_result.verdict, CuVerdict::Refused);
        assert!(offscreen_result.text.contains("outside the virtual desktop"), "{}", offscreen_result.text);
    }

    #[test]
    fn the_frame_serialises_the_control_inventory_for_typed_callers() {
        let host = host_with_frame(
            vec![synthetic_control("button:OK", 10, 10, 40, 20)],
            1,
            1,
            (0, 0, 800, 600),
        );
        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        let frame = &disclosed.evidence["frame"];
        assert_eq!(frame["controls"][0]["label"], "button:OK");
        assert_eq!(frame["controls"][0]["role"], "button");
        assert_eq!(frame["controls"][0]["rect"], json!([10, 10, 40, 20]));
        assert_eq!(frame["controls_role_basis"], "win32-class");
        assert_eq!(frame["controls_truncated"], false);
        let observe = disclosed.evidence["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "observe")
            .unwrap();
        assert!(observe["params"]
            .as_array()
            .unwrap()
            .iter()
            .any(|param| param["name"] == "controls"));
        assert!(disclosed.evidence["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "act:control"));
    }

    #[test]
    fn abi_major_mismatch_is_refused_before_any_effect() {
        let host = host(true);
        let mut mismatched = request(CuCall::Disclose);
        mismatched.abi_major = CU_ABI_MAJOR + 1;
        let result = host.invoke(mismatched);
        assert_eq!(result.verdict, CuVerdict::Refused);
        assert!(result.text.contains("ABI major"), "{}", result.text);
        // No observation generation was created.
        assert_eq!(host.invoke(request(CuCall::Disclose)).generation, 0);
    }

    #[test]
    fn unknown_capability_is_unsupported_and_named() {
        let host = host(true);
        let invented = host.invoke_capability("act:teleport", &json!({}));
        assert_eq!(invented.verdict, CuVerdict::Unsupported);
        assert!(invented.text.contains("not advertised"), "{}", invented.text);
        let mistyped = host.invoke_capability("act:click", &json!([]));
        assert_eq!(mistyped.verdict, CuVerdict::Refused);
        let unknown_op = host.invoke_json(&json!({ "op": "invented" }));
        assert_eq!(unknown_op.verdict, CuVerdict::Refused);
        assert!(unknown_op.text.contains("op must be"), "{}", unknown_op.text);
        assert_eq!(capability("act:teleport"), None);
        assert!(capability("act:click").is_some());
    }

    #[test]
    fn a_named_capability_call_is_adapted_to_the_typed_entrance() {
        let host = host(false);
        // Not granted, so no desktop effect: this proves the name resolved to
        // the right capability and reached admission, not that it acted.
        let result = host.invoke_capability(
            "act:hover",
            &json!({ "why": "point at the button", "x": 10, "y": 10 }),
        );
        assert_eq!(result.verdict, CuVerdict::Refused);
        assert!(result.text.contains("--allow-screen"), "{}", result.text);
    }

    #[test]
    fn capability_manifest_and_wire_action_enum_agree() {
        let definitions = crate::wire::tool_definitions();
        let screen = definitions
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["function"]["name"] == "screen")
            .expect("screen tool definition");
        let actions = screen["function"]["parameters"]["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(actions, act_names());
        let ops = screen["function"]["parameters"]["properties"]["op"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(ops, vec!["observe", "act", "disclose"]);
    }

    #[test]
    fn richer_actuation_parameters_are_disclosed_and_in_the_wire_schema() {
        let definitions = crate::wire::tool_definitions();
        let screen = definitions
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["function"]["name"] == "screen")
            .expect("screen tool definition");
        let props = &screen["function"]["parameters"]["properties"];
        assert_eq!(props["path"]["type"], "array");
        assert_eq!(props["path"]["minItems"], 2);
        assert_eq!(props["path"]["maxItems"], 16);
        assert_eq!(props["delta_x"]["type"], "integer");
        assert_eq!(props["delta_y"]["type"], "integer");
        assert_eq!(props["delta"]["type"], "integer");

        let drag = capability("act:drag").unwrap();
        assert!(drag.params.iter().any(|p| p.name == "path" && p.ty == CuType::Path));
        assert!(drag.params.iter().any(|p| p.name == "to_x" && !p.required));
        let scroll = capability("act:scroll").unwrap();
        for name in ["delta", "delta_x", "delta_y"] {
            assert!(
                scroll.params.iter().any(|p| p.name == name && !p.required),
                "missing or required parameter {name}"
            );
        }
    }

    #[test]
    fn richer_actuation_adds_parameters_not_capabilities_or_abi() {
        assert_eq!(
            act_names(),
            vec!["focus", "move", "click", "hover", "drag", "type", "key", "scroll", "control"]
        );
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
        assert_eq!(crate::screen::SCREEN_TOOL_MAX_DRAG_PATH_POINTS, 16);
        assert_eq!(crate::screen::SCREEN_TOOL_MAX_DRAG_STEPS, 12);
        assert_eq!(crate::screen::SCREEN_TOOL_MAX_SCROLL_DELTA, 1440);
    }

    #[test]
    fn focus_target_is_exactly_one_of_title_or_live_handle() {
        let by_title = parse_request(&json!({
            "op": "act", "why": "focus by title", "action": "focus",
            "window": "WorkBuddy"
        }))
        .unwrap();
        assert_eq!(
            by_title.call,
            CuCall::Act {
                why: "focus by title".to_owned(),
                action: CuAction::Focus {
                    target: FocusTarget::Title("WorkBuddy".to_owned())
                }
            }
        );
        let by_handle = parse_request(&json!({
            "op": "act", "why": "focus by handle", "action": "focus",
            "hwnd": 59771304
        }))
        .unwrap();
        assert_eq!(
            by_handle.call,
            CuCall::Act {
                why: "focus by handle".to_owned(),
                action: CuAction::Focus { target: FocusTarget::Handle(59771304) }
            }
        );
        for (label, arguments) in [
            (
                "both",
                json!({ "op": "act", "why": "focus", "action": "focus", "window": "WorkBuddy", "hwnd": 1 }),
            ),
            (
                "neither",
                json!({ "op": "act", "why": "focus", "action": "focus" }),
            ),
            (
                "negative",
                json!({ "op": "act", "why": "focus", "action": "focus", "hwnd": -1 }),
            ),
            (
                "zero",
                json!({ "op": "act", "why": "focus", "action": "focus", "hwnd": 0 }),
            ),
            (
                "string handle",
                json!({ "op": "act", "why": "focus", "action": "focus", "hwnd": "59771304" }),
            ),
        ] {
            let error = parse_request(&arguments).unwrap_err();
            assert!(error.starts_with("screen: refused:"), "{label}: {error}");
            assert!(
                error.contains("focus") || error.contains("window") || error.contains("hwnd"),
                "{label}: {error}"
            );
        }
    }

    #[test]
    fn handle_target_is_an_additive_focus_parameter() {
        let focus = capability("act:focus").unwrap();
        assert!(focus.params.iter().any(|p| p.name == "hwnd"
            && p.ty == CuType::Int
            && !p.required));
        assert!(focus.params.iter().any(|p| p.name == "window" && !p.required));
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
        let definitions = crate::wire::tool_definitions();
        let screen = definitions
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["function"]["name"] == "screen")
            .expect("screen tool definition");
        let props = &screen["function"]["parameters"]["properties"];
        assert_eq!(props["hwnd"]["type"], "integer");
        assert!(props["hwnd"]["description"]
            .as_str()
            .unwrap()
            .contains("mutually exclusive"));
    }

    #[test]
    fn focus_target_refusal_names_its_capability() {
        let result = host(false).invoke_json(&json!({
            "op": "act", "why": "ambiguous focus", "action": "focus",
            "window": "WorkBuddy", "hwnd": 59771304
        }));
        assert_eq!(result.verdict, CuVerdict::Refused);
        assert_eq!(result.capability, "act:focus");
        assert!(result.text.contains("exactly one"), "{}", result.text);
    }

    #[test]
    fn stale_handle_error_is_stale_not_unknown() {
        let (verdict, reason) = classify(
            "screen: stale: hwnd 1 is not in the live window list; observe again".to_owned(),
        );
        assert_eq!(verdict, CuVerdict::Stale);
        assert!(reason.contains("observe again"));
    }

    #[test]
    fn ungranted_calls_are_refused_with_the_flag_named() {
        let host = host(false);
        for op in ["observe", "act"] {
            let arguments = match op {
                "act" => json!({ "op": "act", "why": "x", "action": "hover", "x": 1, "y": 1 }),
                other => json!({ "op": other }),
            };
            let result = host.invoke_json(&arguments);
            assert_eq!(result.verdict, CuVerdict::Refused, "{op}");
            assert!(result.text.contains("--allow-screen"), "{op}: {}", result.text);
        }
        // Disclosure is information, not an effect: it still works and says
        // plainly that the grant is denied.
        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(disclosed.verdict, CuVerdict::Performed);
        assert!(disclosed.text.contains("grant=denied"), "{}", disclosed.text);
        assert_eq!(disclosed.evidence["grant"], "denied");
    }

    #[test]
    fn stale_generation_is_never_replayed() {
        let host = host(true);
        // An unqualified/qualified host with no desktop still advances no
        // generation on a refused observe, so pin an impossible generation.
        let mut pinned = request(CuCall::Disclose);
        pinned.as_of = Some(7);
        let result = host.invoke(pinned);
        assert_eq!(result.verdict, CuVerdict::Stale);
        assert!(result.text.contains("was not performed"), "{}", result.text);
        assert_eq!(result.generation, 0);
    }

    #[test]
    fn typed_parameters_are_refused_by_name_before_effects() {
        let host = host(false);
        let cases: &[(&str, Value, &str)] = &[
            ("missing height", json!({ "op": "observe", "capture": { "x": 0, "y": 0, "width": 10 } }), "integer height"),
            ("bad scale", json!({ "op": "observe", "scale": 99 }), "scale must be an integer"),
            ("missing to_y", json!({ "op": "act", "why": "x", "action": "drag", "x": 1, "y": 2, "to_x": 3 }), "integer to_y"),
            ("missing why", json!({ "op": "act", "action": "hover", "x": 1, "y": 1 }), "`why` argument is required"),
            ("bad as_of", json!({ "op": "observe", "as_of": -1 }), "as_of must be"),
            ("bad abi", json!({ "op": "observe", "abi_major": "one" }), "abi_major must be"),
        ];
        for (label, arguments, expected) in cases {
            let result = host.invoke_json(arguments);
            assert_eq!(result.verdict, CuVerdict::Refused, "{label}");
            assert!(result.text.contains(expected), "{label}: {}", result.text);
        }
    }

    #[test]
    fn verdict_classification_never_calls_an_ambiguous_failure_a_success() {
        assert_eq!(
            classify("screen: failed: asked for rect=...".into()).0,
            CuVerdict::NotPerformed
        );
        assert_eq!(
            classify("screen: refused: no window".into()).0,
            CuVerdict::Refused
        );
        assert_eq!(classify("native effect uncertain".into()).0, CuVerdict::Unknown);
    }

    /// The contract against a real desktop: generations, pinned calls, stale
    /// refusal without replay, and receipts a disclosure can account for.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs a signed-in Windows desktop session"]
    fn real_desktop_contract_generations_and_receipts() {
        let root = std::env::temp_dir().join(format!(
            "minicon-harness-cu-real-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        let host = CuHost::new(ScreenTool::new(&root, true));

        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(disclosed.verdict, CuVerdict::Performed);
        assert!(disclosed.text.contains("qualified=true"), "{}", disclosed.text);
        assert!(disclosed.text.contains("grant=granted"), "{}", disclosed.text);
        assert_eq!(
            disclosed.evidence["capabilities"].as_array().unwrap().len(),
            CAPABILITIES.len()
        );

        let observed = host.invoke_json(&json!({ "op": "observe" }));
        assert_eq!(observed.verdict, CuVerdict::Performed, "{}", observed.text);
        assert_eq!(observed.generation, 1);
        assert_eq!(observed.evidence["generation"], 1);
        assert!(observed.evidence["windows"].as_array().unwrap().len() > 0);
        assert!(observed.evidence["displays"].as_array().unwrap().len() > 0);
        let x = observed.evidence["desktop"]["x"].as_i64().unwrap() + 100;
        let y = observed.evidence["desktop"]["y"].as_i64().unwrap() + 100;

        let pinned = host.invoke_json(&json!({
            "op": "act", "why": "verify a pinned pointer action",
            "action": "hover", "x": x, "y": y, "as_of": 1
        }));
        assert_eq!(pinned.verdict, CuVerdict::Performed, "{}", pinned.text);
        assert!(pinned.evidence["readback"].as_str().unwrap().starts_with("pointer at "));

        let stale = host.invoke_json(&json!({
            "op": "act", "why": "this must never reach the desktop",
            "action": "hover", "x": x, "y": y, "as_of": 0
        }));
        assert_eq!(stale.verdict, CuVerdict::Stale, "{}", stale.text);
        assert!(stale.text.contains("was not performed"), "{}", stale.text);

        let second = host.invoke_json(&json!({ "op": "observe" }));
        assert_eq!(second.generation, 2);
        let final_disclosure = host.invoke_json(&json!({ "op": "disclose" }));
        let receipts = final_disclosure.evidence["receipts"].as_array().unwrap();
        assert!(receipts.len() >= 4, "{}", final_disclosure.text);
        assert!(final_disclosure.text.contains("capability=act:hover"));
        assert!(final_disclosure.text.contains("verdict=stale"));
        // Publish the typed evidence alongside the assertions, so an
        // independent reader can check the contract without trusting Rust's
        // own asserts (the same discipline as the Pillow PNG decode).
        std::fs::write(
            root.join("cu-observe.json"),
            serde_json::to_string_pretty(&observed.evidence).unwrap(),
        )
        .expect("write observe evidence");
        std::fs::write(
            root.join("cu-disclose.json"),
            serde_json::to_string_pretty(&final_disclosure.evidence).unwrap(),
        )
        .expect("write disclosure evidence");
        eprintln!("cu evidence: {}", root.display());
    }

    /// A deterministic Win32 fixture inside the test process: a real parent
    /// window with a real BUTTON child. No detached GUI process, no message
    /// pump, no focus steal (SW_SHOWNOACTIVATE + WS_EX_NOACTIVATE). This makes
    /// the named-target success path verifiable on any signed-in desktop,
    /// instead of depending on whatever app happens to be in front.
    #[cfg(windows)]
    mod fixture {
        use std::ffi::c_void;

        type Hwnd = *mut c_void;
        const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
        const WS_CHILD: u32 = 0x4000_0000;
        const WS_VISIBLE: u32 = 0x1000_0000;
        const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
        const SW_SHOWNOACTIVATE: i32 = 4;

        #[link(name = "user32")]
        extern "system" {
            fn CreateWindowExW(
                style_ex: u32,
                class: *const u16,
                name: *const u16,
                style: u32,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                parent: Hwnd,
                menu: Hwnd,
                instance: Hwnd,
                param: *mut c_void,
            ) -> Hwnd;
            fn DestroyWindow(window: Hwnd) -> i32;
            fn ShowWindow(window: Hwnd, command: i32) -> i32;
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetModuleHandleW(name: *const u16) -> Hwnd;
        }

        fn wide_z(text: &str) -> Vec<u16> {
            text.encode_utf16().chain(std::iter::once(0)).collect()
        }

        pub const TITLE: &str = "minicon harness control fixture";

        pub struct Window {
            parent: Hwnd,
        }

        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.parent);
                }
            }
        }

        pub fn create() -> Option<Window> {
            let static_class = wide_z("STATIC");
            let button_class = wide_z("BUTTON");
            let title = wide_z(TITLE);
            let label = wide_z("OK");
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let parent = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    static_class.as_ptr(),
                    title.as_ptr(),
                    WS_OVERLAPPEDWINDOW,
                    200,
                    200,
                    320,
                    200,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null_mut(),
                )
            };
            if parent.is_null() {
                return None;
            }
            let button = unsafe {
                CreateWindowExW(
                    0,
                    button_class.as_ptr(),
                    label.as_ptr(),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    20,
                    90,
                    26,
                    parent,
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null_mut(),
                )
            };
            unsafe {
                ShowWindow(parent, SW_SHOWNOACTIVATE);
            }
            if button.is_null() {
                None
            } else {
                Some(Window { parent })
            }
        }
    }

    /// The semantic journey against a real desktop: a typed control inventory,
    /// a control addressed **by name**, and a pointer readback that lands
    /// inside that control's rectangle. Publishes `cu-controls.json` so an
    /// independent reader can check it.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs a signed-in Windows desktop session"]
    fn real_desktop_semantic_controls_named_target() {
        let root = std::env::temp_dir().join(format!(
            "minicon-harness-cu-controls-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        let host = CuHost::new(ScreenTool::new(&root, true));

        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(disclosed.evidence["abi"]["major"], CU_ABI_MAJOR);
        assert_eq!(disclosed.evidence["grant"], "granted");

        // The foreground window first: this is what a real caller would scan.
        let foreground = host.invoke_json(&json!({ "op": "observe", "controls": true }));
        assert_eq!(foreground.verdict, CuVerdict::Performed, "{}", foreground.text);
        assert!(foreground.text.contains("controls: source="), "{}", foreground.text);
        assert_eq!(foreground.evidence["controls_role_basis"], "win32-class");
        assert!(foreground.evidence["controls_truncated"].is_boolean());

        // Then the deterministic fixture, resolved by title rather than by
        // handle. `find` must discover it through the normal enumeration.
        let window = fixture::create().expect("create a Win32 control fixture");
        let observed = host.invoke_json(&json!({
            "op": "observe", "controls": fixture::TITLE
        }));
        assert_eq!(observed.verdict, CuVerdict::Performed, "{}", observed.text);
        let controls = observed.evidence["controls"].as_array().unwrap();
        assert!(!controls.is_empty(), "{}", observed.text);
        let labels: Vec<&str> = controls
            .iter()
            .filter_map(|entry| entry["label"].as_str())
            .collect();
        assert_eq!(
            labels.iter().collect::<std::collections::HashSet<_>>().len(),
            labels.len(),
            "{labels:?}"
        );
        let button = controls
            .iter()
            .find(|entry| entry["text"] == "OK" && entry["role"] == "button")
            .expect("the fixture exposes a Win32 BUTTON child");
        let label = button["label"].as_str().unwrap().to_owned();
        let rect = button["rect"].as_array().unwrap();

        // press defaults to false: this journey never clicks anything.
        let acted = host.invoke_json(&json!({
            "op": "act", "why": "verify a named control target",
            "action": "control", "control": label,
            "as_of": observed.generation
        }));
        assert_eq!(acted.verdict, CuVerdict::Performed, "{}", acted.text);
        assert_eq!(acted.evidence["press"], false);
        assert_eq!(acted.evidence["control"], label);
        assert_eq!(acted.evidence["rect"], json!(rect));
        assert!(acted.evidence["readback"]
            .as_str()
            .unwrap()
            .contains("inside control rect"));
        assert!(acted.text.contains("press=false"), "{}", acted.text);

        // A stale pinned call must not replay the named target either.
        let stale = host.invoke_json(&json!({
            "op": "act", "why": "must not replay",
            "action": "control", "control": label,
            "as_of": observed.generation - 1
        }));
        assert_eq!(stale.verdict, CuVerdict::Stale, "{}", stale.text);

        std::fs::write(
            root.join("cu-controls.json"),
            serde_json::to_string_pretty(&json!({
                "abi": disclosed.evidence["abi"],
                "foreground": foreground.evidence,
                "observe": observed.evidence,
                "target": {
                    "label": label, "rect": rect,
                    "center": acted.evidence["center"],
                },
                "acted": acted.evidence,
                "stale": stale.evidence,
            }))
            .unwrap(),
        )
        .expect("write control evidence");
        eprintln!("cu-controls evidence: {}", root.display());
        drop(window);
    }

    fn receipt(button: &'static str, downs: u32, ups: u32, double: bool) -> Actuation {
        receipt_path(button, downs, ups, double, 0)
    }

    fn receipt_path(
        button: &'static str,
        downs: u32,
        ups: u32,
        double: bool,
        path_points: u32,
    ) -> Actuation {
        Actuation {
            button,
            downs,
            ups,
            double,
            target: "Notepad".to_owned(),
            path_points,
        }
    }

    #[test]
    fn actuation_receipt_matching_the_request_stays_performed() {
        let expected = expected_dispatch(&CuAction::Click {
            x: 10, y: 10, button: crate::screen::ClickButton::Middle, double: false,
        });
        assert_eq!(expected, Some(("middle", 1, false, 0)));
        assert_eq!(
            verify_actuation(expected, Some(&receipt("middle", 1, 1, false)), CuVerdict::Performed),
            CuVerdict::Performed
        );
        let double = expected_dispatch(&CuAction::Click {
            x: 10, y: 10, button: crate::screen::ClickButton::Left, double: true,
        });
        assert_eq!(
            verify_actuation(double, Some(&receipt("left", 2, 2, true)), CuVerdict::Performed),
            CuVerdict::Performed
        );
        assert_eq!(
            expected_dispatch(&CuAction::Drag {
                x: 0, y: 0, to_x: 5, to_y: 5, button: crate::screen::ClickButton::Right,
                path: None,
            }),
            Some(("right", 1, false, 1))
        );
    }

    #[test]
    fn actuation_receipt_missing_is_unknown_never_performed() {
        let expected = expected_dispatch(&CuAction::Click {
            x: 1, y: 1, button: crate::screen::ClickButton::Left, double: false,
        });
        assert_eq!(verify_actuation(expected, None, CuVerdict::Performed), CuVerdict::Unknown);
        // A failed readback stays failed; the receipt cannot upgrade it.
        assert_eq!(
            verify_actuation(expected, Some(&receipt("left", 1, 1, false)), CuVerdict::NotPerformed),
            CuVerdict::NotPerformed
        );
    }

    #[test]
    fn actuation_receipt_mismatch_is_not_performed() {
        let expected = Some(("left", 1, false, 0));
        for bad in [
            receipt("right", 1, 1, false),
            receipt("left", 2, 2, false),
            receipt("left", 1, 0, false),
            receipt("left", 1, 1, true),
            Actuation { target: "  ".to_owned(), ..receipt("left", 1, 1, false) },
        ] {
            assert_eq!(
                verify_actuation(expected, Some(&bad), CuVerdict::Performed),
                CuVerdict::NotPerformed,
                "bad receipt: {bad:?}"
            );
        }
    }

    #[test]
    fn non_button_action_carries_no_receipt_and_keeps_its_verdict() {
        assert_eq!(expected_dispatch(&CuAction::Hover { x: 1, y: 1 }), None);
        assert_eq!(
            verify_actuation(None, None, CuVerdict::Performed),
            CuVerdict::Performed
        );
        let value = actuation_json(&receipt("left", 1, 1, false));
        assert_eq!(value["button"], "left");
        assert_eq!(value["downs"], 1);
        assert_eq!(value["ups"], 1);
        assert_eq!(value["double"], false);
        assert_eq!(value["target"], "Notepad");
        assert_eq!(value["path_points"], 0);
    }

    #[test]
    fn path_drag_receipt_counts_the_points_actually_traversed() {
        let path = vec![(10, 10), (20, 20), (30, 30)];
        let expected = expected_dispatch(&CuAction::Drag {
            x: 0,
            y: 0,
            to_x: 0,
            to_y: 0,
            button: crate::screen::ClickButton::Left,
            path: Some(path),
        });
        assert_eq!(expected, Some(("left", 1, false, 3)));
        assert_eq!(
            verify_actuation(
                expected,
                Some(&receipt_path("left", 1, 1, false, 3)),
                CuVerdict::Performed
            ),
            CuVerdict::Performed
        );
        // A receipt that omits the path (0.0.6 shape) or claims another
        // length cannot qualify a performed path drag.
        assert_eq!(
            verify_actuation(expected, Some(&receipt("left", 1, 1, false)), CuVerdict::Performed),
            CuVerdict::NotPerformed
        );
        assert_eq!(
            verify_actuation(expected, None, CuVerdict::Performed),
            CuVerdict::Unknown
        );
    }

    #[test]
    fn drag_path_admission_is_typed_and_bounded_before_effects() {
        let host = host(false);
        let long: Value = Value::Array((0..17).map(|i| json!([i, i])).collect());
        let cases: &[(&str, Value, &str)] = &[
            ("one point", json!({ "op": "act", "why": "x", "action": "drag", "x": 1, "y": 2, "path": [[3, 4]] }), "needs at least 2 points"),
            ("too long", json!({ "op": "act", "why": "x", "action": "drag", "x": 1, "y": 2, "path": long }), "at most 16 points"),
            ("not an array", json!({ "op": "act", "why": "x", "action": "drag", "x": 1, "y": 2, "path": "3,4" }), "must be an array"),
            ("short pair", json!({ "op": "act", "why": "x", "action": "drag", "x": 1, "y": 2, "path": [[3, 4], [5]] }), "two-integer [x,y] pair"),
            ("non integer", json!({ "op": "act", "why": "x", "action": "drag", "x": 1, "y": 2, "path": [[3, 4], [5, "six"]] }), "32-bit integer"),
        ];
        for (label, arguments, expected) in cases {
            let result = host.invoke_json(arguments);
            assert_eq!(result.verdict, CuVerdict::Refused, "{label}");
            assert!(result.text.contains(expected), "{label}: {}", result.text);
        }
    }

    #[test]
    fn scroll_axes_are_admitted_before_effects() {
        let host = host(false);
        let cases: &[(&str, Value, &str)] = &[
            ("both zero", json!({ "op": "act", "why": "x", "action": "scroll", "x": 1, "y": 1, "delta_x": 0, "delta_y": 0 }), "both scroll axes are zero"),
            ("alias conflict", json!({ "op": "act", "why": "x", "action": "scroll", "x": 1, "y": 1, "delta": 120, "delta_y": 120 }), "cannot both be supplied"),
            ("axis bound", json!({ "op": "act", "why": "x", "action": "scroll", "x": 1, "y": 1, "delta_x": 1441 }), "+-1440"),
            ("bad axis type", json!({ "op": "act", "why": "x", "action": "scroll", "x": 1, "y": 1, "delta_y": "up" }), "32-bit integer"),
        ];
        for (label, arguments, expected) in cases {
            let result = host.invoke_json(arguments);
            assert_eq!(result.verdict, CuVerdict::Refused, "{label}");
            assert!(result.text.contains(expected), "{label}: {}", result.text);
        }
    }

    #[test]
    fn the_vertical_alias_and_path_destinations_are_additive() {
        let host = host(false);
        // Both spellings parse cleanly and then stop at the grant gate: the
        // 0.0.6 call shape still works, and a path needs no to_x/to_y.
        let legacy = host.invoke_json(&json!({
            "op": "act", "why": "x", "action": "scroll",
            "x": 1, "y": 1, "delta": 120,
        }));
        assert_eq!(legacy.verdict, CuVerdict::Refused);
        assert!(legacy.text.contains("--allow-screen"), "{}", legacy.text);
        let path = host.invoke_json(&json!({
            "op": "act", "why": "x", "action": "drag",
            "x": 1, "y": 1, "path": [[2, 2], [3, 3]],
        }));
        assert_eq!(path.verdict, CuVerdict::Refused);
        assert!(path.text.contains("--allow-screen"), "{}", path.text);
    }

    #[test]
    fn disclosure_reports_abi_grant_capabilities_and_receipts() {
        let host = host(false);
        let refused = host.invoke_json(&json!({ "op": "observe" }));
        assert_eq!(refused.verdict, CuVerdict::Refused);
        // A disclosure must be possible even when the desktop is not granted:
        // the manifest is information, not an effect.
        let manifest = host.manifest(
            &CuState {
                generation: 0,
                frame: None,
                receipts: Vec::new(),
                keyboard: KeyboardState::default(),
                wait_cumulative_ms: 0,
                wait_calls: 0,
            },
            0,
        );
        assert_eq!(manifest["abi"]["major"], CU_ABI_MAJOR);
        assert_eq!(manifest["grant"], "denied");
        assert_eq!(
            manifest["capabilities"].as_array().unwrap().len(),
            CAPABILITIES.len()
        );
        assert_eq!(manifest["minor_degraded"], false);
    }

    #[test]
    fn held_modifier_whitelist_is_small_and_canonical() {
        assert_eq!(
            Modifier::parse("ctrl"),
            Some(Modifier::Ctrl)
        );
        assert_eq!(
            Modifier::parse("control"),
            Some(Modifier::Ctrl)
        );
        assert_eq!(
            Modifier::parse("option"),
            Some(Modifier::Alt)
        );
        assert_eq!(
            Modifier::parse("cmd"),
            Some(Modifier::Win)
        );
        assert_eq!(Modifier::parse("enter"), None);
        assert_eq!(Modifier::parse("ctrl+s"), None);
        assert_eq!(Modifier::ALL.len(), SCREEN_TOOL_MAX_HELD_MODIFIERS);
        assert_eq!(SCREEN_TOOL_HELD_MODIFIERS, ["ctrl", "shift", "alt", "win/cmd"]);
    }

    #[test]
    fn held_key_state_bounds_four_modifiers_and_is_idempotent() {
        let mut state = KeyboardState::default();
        for key in Modifier::ALL {
            state.hold(key).unwrap();
        }
        state.hold(Modifier::Ctrl).unwrap(); // holding twice is not a fifth
        assert_eq!(state.tags(), vec!["alt", "ctrl", "shift", "win"]);
        // There are exactly four whitelisted modifiers, so the bound equals
        // the size of the whitelist and a full set is still idempotent.
        assert_eq!(state.held.len(), SCREEN_TOOL_MAX_HELD_MODIFIERS);
        assert_eq!(Modifier::ALL.len(), SCREEN_TOOL_MAX_HELD_MODIFIERS);
        let released = state.release();
        assert_eq!(released, vec!["alt", "ctrl", "shift", "win"]);
        assert!(state.tags().is_empty());
        assert_eq!(state.foreground, None);
        assert!(state.release().is_empty()); // teardown is idempotent
    }

    #[test]
    fn held_keys_are_released_on_foreground_change_or_lost_grant() {
        let mut state = KeyboardState::default();
        state.hold(Modifier::Ctrl).unwrap();
        state.foreground = Some("Editor".to_owned());
        assert!(state.reconcile("Editor", false).is_empty());
        assert_eq!(state.tags(), vec!["ctrl"]);
        let released = state.reconcile("Save dialog", false);
        assert_eq!(released, vec!["ctrl"]);
        assert!(state.tags().is_empty());
        assert_eq!(state.foreground, None);

        state.hold(Modifier::Shift).unwrap();
        state.foreground = Some("Editor".to_owned());
        let lost = state.reconcile("Editor", true);
        assert_eq!(lost, vec!["shift"]);
        assert!(state.tags().is_empty());
        // An empty held set never invents an automatic release.
        assert!(state.reconcile("Another window", true).is_empty());
    }

    #[test]
    fn held_key_flags_are_typed_bounded_and_mutually_exclusive() {
        let call = |value: Value| parse_request(&value);
        let ok = call(json!({
            "op": "act", "why": "hold shift while selecting",
            "action": "key", "keys": "shift", "hold": true
        })).unwrap();
        assert_eq!(
            ok.call,
            CuCall::Act {
                why: "hold shift while selecting".to_owned(),
                action: CuAction::Key { keys: "shift".to_owned(), hold: true, release: false },
            }
        );
        let release = call(json!({
            "op": "act", "why": "let go", "action": "key", "release": true
        })).unwrap();
        assert_eq!(
            release.call,
            CuCall::Act {
                why: "let go".to_owned(),
                action: CuAction::Key { keys: String::new(), hold: false, release: true },
            }
        );
        for arguments in [
            json!({ "op": "act", "why": "x", "action": "key", "keys": "ctrl", "hold": true, "release": true }),
            json!({ "op": "act", "why": "x", "action": "key", "keys": "ctrl+s", "hold": true }),
            json!({ "op": "act", "why": "x", "action": "key", "keys": "f5", "hold": true }),
            json!({ "op": "act", "why": "x", "action": "key", "keys": "ctrl+s", "release": true }),
            json!({ "op": "act", "why": "x", "action": "key", "keys": "ctrl", "hold": "yes" }),
            json!({ "op": "act", "why": "x", "action": "key", "hold": true }),
        ] {
            let error = call(arguments).unwrap_err();
            assert!(error.starts_with("screen: refused:"), "{error}");
        }
    }

    #[test]
    fn held_keys_are_disclosed_and_never_get_a_button_receipt() {
        let host = host(false);
        let state = CuState {
            generation: 3,
            frame: None,
            receipts: Vec::new(),
            keyboard: KeyboardState {
                held: vec![Modifier::Ctrl, Modifier::Shift],
                foreground: Some("Editor".to_owned()),
            },
            wait_cumulative_ms: 0,
            wait_calls: 0,
        };
        let manifest = host.manifest(&state, 0);
        assert_eq!(manifest["held_keys"], json!(["ctrl", "shift"]));
        let evidence = keyboard_evidence(
            "pressed shift (held) foreground=\"Editor\"",
            3,
            &["ctrl".to_owned(), "shift".to_owned()],
            &[],
        );
        assert_eq!(evidence["keyboard"]["held"], json!(["ctrl", "shift"]));
        assert_eq!(evidence["actuation"], Value::Null);
        assert_eq!(evidence["scroll"], Value::Null);
        let key = CuAction::Key { keys: "ctrl".to_owned(), hold: true, release: false };
        assert_eq!(key.name(), "act:key");
        assert_eq!(expected_dispatch(&key), None);
        // A stray button receipt on a keyboard step must never upgrade or
        // downgrade a keyboard verdict through the button checker.
        assert_eq!(verify_actuation(None, None, CuVerdict::Performed), CuVerdict::Performed);
    }

    /// 0.0.9 / PRD 01.11: the status whitelist is exactly these four values,
    /// and a string outside it never becomes a status.
    #[test]
    fn grant_status_whitelist_is_exactly_four_and_round_trips() {
        use crate::screen::GrantStatus;
        let tags: Vec<&str> = GrantStatus::ALL.iter().map(|status| status.tag()).collect();
        assert_eq!(tags, vec!["granted", "denied", "unknown", "not_applicable"]);
        assert_eq!(
            tags.iter().collect::<std::collections::HashSet<_>>().len(),
            4,
            "a duplicate tag would make the whitelist ambiguous: {tags:?}"
        );
        for status in GrantStatus::ALL {
            assert_eq!(GrantStatus::parse(status.tag()), Some(status));
        }
        for outside in ["", "ok", "granted ", "GRANTED", "maybe", "unavailable"] {
            assert_eq!(GrantStatus::parse(outside), None, "{outside:?}");
        }
        // Only the two non-answers must carry a reason.
        assert!(!GrantStatus::Granted.needs_reason());
        assert!(!GrantStatus::Denied.needs_reason());
        assert!(GrantStatus::Unknown.needs_reason());
        assert!(GrantStatus::NotApplicable.needs_reason());
    }

    /// The reason rule: an unanswerable row without a reason is a contract
    /// violation, and a probe set is refused whole rather than published
    /// partially.
    #[test]
    fn grant_reasons_are_required_sorted_unique_and_bounded() {
        use crate::screen::{normalize_grant_probes, GrantStatus};
        let missing = normalize_grant_probes(vec![GrantProbe::unknown("screen_capture", "")])
            .unwrap_err();
        assert!(missing.contains("without a reason"), "{missing}");
        let blank = normalize_grant_probes(vec![GrantProbe {
            name: "input_posting",
            status: GrantStatus::NotApplicable,
            reason: Some("   ".to_owned()),
        }])
        .unwrap_err();
        assert!(blank.contains("without a reason"), "{blank}");
        // `granted` / `denied` may omit it.
        assert!(normalize_grant_probes(vec![GrantProbe::granted("screen_capture")]).is_ok());
        assert!(normalize_grant_probes(vec![GrantProbe::denied("input_posting")]).is_ok());

        // Sorted by name, and a duplicate name is refused, not de-duplicated.
        let sorted = normalize_grant_probes(vec![
            GrantProbe::granted("screen_capture"),
            GrantProbe::granted("accessibility"),
            GrantProbe::unknown("input_posting", "no symbol"),
        ])
        .unwrap();
        let names: Vec<&str> = sorted.iter().map(|probe| probe.name).collect();
        assert_eq!(names, vec!["accessibility", "input_posting", "screen_capture"]);
        let duplicate = normalize_grant_probes(vec![
            GrantProbe::granted("screen_capture"),
            GrantProbe::granted("screen_capture"),
        ])
        .unwrap_err();
        assert!(duplicate.contains("published twice"), "{duplicate}");

        // The probe bound is enforced, never silently clipped.
        let over: Vec<GrantProbe> = crate::screen::GrantStatus::ALL
            .iter()
            .flat_map(|_| ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8"])
            .take(crate::screen::SCREEN_TOOL_MAX_GRANT_PROBES + 1)
            .map(GrantProbe::granted)
            .collect();
        let too_many = normalize_grant_probes(over).unwrap_err();
        assert!(too_many.contains("exceed the bound"), "{too_many}");
        assert!(too_many.contains(&crate::screen::SCREEN_TOOL_MAX_GRANT_PROBES.to_string()));
    }

    /// A long reason is cut and marked; the marker is inside the bound, so a
    /// published reason is never longer than the constant.
    #[test]
    fn a_long_grant_reason_is_truncated_with_a_marker() {
        use crate::screen::{
            truncate_grant_reason, SCREEN_TOOL_GRANT_REASON_MARKER, SCREEN_TOOL_MAX_GRANT_REASON,
        };
        let short = "no X11 display is reachable";
        assert_eq!(truncate_grant_reason(short), short);
        assert_eq!(truncate_grant_reason("  padded  "), "padded");
        let long = "x".repeat(SCREEN_TOOL_MAX_GRANT_REASON + 40);
        let cut = truncate_grant_reason(&long);
        assert_eq!(cut.chars().count(), SCREEN_TOOL_MAX_GRANT_REASON);
        assert_eq!(cut.chars().last(), Some(SCREEN_TOOL_GRANT_REASON_MARKER));
        // A reason of exactly the bound is not marked.
        let exact: String = "y".repeat(SCREEN_TOOL_MAX_GRANT_REASON);
        assert_eq!(truncate_grant_reason(&exact), exact);
    }

    /// The live probe set of whichever backend this test compiled in must
    /// already satisfy the contract. This is the assertion that catches a
    /// backend publishing a `denied` with no reason or an unsorted array.
    #[test]
    fn this_cells_native_grant_probes_satisfy_the_contract() {
        use crate::screen::{GrantStatus, SCREEN_TOOL_MAX_GRANT_PROBES};
        let probes = crate::screen::grant_probes().expect("this cell's probe set is well formed");
        // The printed rows are the per-OS evidence that each backend answered
        // its own probes; CI keeps them beside the matrix line above.
        eprintln!(
            "cu grants platform={} rows={:?}",
            crate::screen::platform_name(),
            probes
                .iter()
                .map(|probe| (probe.name, probe.status.tag()))
                .collect::<Vec<_>>()
        );

        assert!(!probes.is_empty(), "a disclosure with no grants cannot be recomputed");
        assert!(probes.len() <= SCREEN_TOOL_MAX_GRANT_PROBES);
        assert!(probes.len() >= 2, "every cell names at least two probes");
        let names: Vec<&str> = probes.iter().map(|probe| probe.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "rows are sorted by name");
        assert_eq!(
            names.iter().collect::<std::collections::HashSet<_>>().len(),
            names.len(),
            "rows have unique names: {names:?}"
        );
        for probe in &probes {
            assert_eq!(GrantStatus::parse(probe.status.tag()), Some(probe.status));
            if probe.status.needs_reason() {
                let reason = probe.reason.as_deref().unwrap_or_default();
                assert!(!reason.trim().is_empty(), "{} has no reason", probe.name);
            }
            if let Some(reason) = &probe.reason {
                assert!(
                    reason.chars().count() <= crate::screen::SCREEN_TOOL_MAX_GRANT_REASON,
                    "{} reason is over the bound",
                    probe.name
                );
            }
        }
        // A probe that names an effect this cell implements must never be
        // reported as something a reader could read as "no permission".
        if crate::screen::platform_capabilities().capture {
            let capture = probes
                .iter()
                .find(|probe| probe.name == "screen_capture")
                .expect("a capture-capable cell names screen_capture");
            assert_ne!(capture.status, GrantStatus::Denied, "{}", capture.name);
        }
    }

    /// The environment probe is a shape, not a title: coarse session kind, a
    /// non-negative geometry and a boolean.
    #[test]
    fn the_environment_probe_publishes_shape_not_values() {
        use crate::screen::{environment_probe, SessionKind};
        let probe = environment_probe();
        assert!(!probe.platform.trim().is_empty());
        assert_eq!(probe.platform, crate::screen::platform_name());
        assert!(SessionKind::ALL.iter().any(|kind| kind.tag() == probe.session.tag()));
        let (x, y, width, height) = probe.virtual_desktop;
        assert!(width >= 0 && height >= 0, "{x},{y} {width}x{height}");
        // A rect with no area is honest on a headless cell; a made-up one is
        // not. Only an unreadable geometry may be zero.
        if probe.display_count > 0 && probe.session != SessionKind::Unavailable {
            assert!(width > 0 && height > 0, "{width}x{height}");
        }
        let scales = crate::screen::display_scale_probes().unwrap_or_default();
        let value = environment_json(&probe, &scales);
        eprintln!("cu environment platform={} evidence={value}", probe.platform);
        assert_eq!(value["platform"], json!(probe.platform));
        assert_eq!(value["session"], json!(probe.session.tag()));
        assert_eq!(value["display_count"], json!(probe.display_count));
        assert_eq!(value["virtual_desktop"]["width"], json!(width));
        assert_eq!(value["virtual_desktop"]["height"], json!(height));
        assert_eq!(value["foreground_present"], json!(probe.foreground_present));
        // No field may carry a title, a user name or a path.
        for key in ["title", "window", "user", "path", "display"] {
            assert!(value.get(key).is_none(), "{key} must not be published");
        }
    }

    /// Disclosure stays available without `--allow-screen`, and it now carries
    /// both halves: a caller may read its own preconditions before it is
    /// allowed to act.
    #[test]
    fn disclosure_publishes_environment_and_grants_without_a_grant() {
        let host = host(false);
        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(disclosed.verdict, CuVerdict::Performed, "{}", disclosed.text);
        assert_eq!(disclosed.evidence["grant"], "denied");
        let environment = disclosed.evidence["environment"]
            .as_object()
            .expect("environment is an object");
        for key in ["platform", "session", "display_count", "virtual_desktop", "foreground_present"] {
            assert!(environment.contains_key(key), "environment is missing {key}");
        }
        let grants = disclosed.evidence["grants"].as_array().expect("grants is an array");
        assert!(!grants.is_empty());
        for row in grants {
            assert!(row["name"].as_str().is_some_and(|name| !name.trim().is_empty()));
            let status = row["status"].as_str().expect("every row has a status");
            let parsed = crate::screen::GrantStatus::parse(status)
                .unwrap_or_else(|| panic!("status {status:?} is outside the whitelist"));
            if parsed.needs_reason() {
                assert!(
                    row["reason"].as_str().is_some_and(|reason| !reason.trim().is_empty()),
                    "{status:?} row {} has no reason",
                    row["name"]
                );
            }
        }
        assert!(
            disclosed.text.contains("\nenvironment platform="),
            "{}",
            disclosed.text
        );
        assert!(disclosed.text.contains("\ngrant name="), "{}", disclosed.text);
        // 0.0.6..0.0.8 evidence keys are untouched by the addition.
        for absent in ["actuation", "keyboard", "scroll"] {
            assert!(disclosed.evidence.get(absent).is_none(), "{absent} must stay absent");
        }
        assert_eq!(disclosed.evidence["held_keys"], json!([]));
    }

    /// 0.0.9 adds evidence only: no 13th capability, no ABI bump and no new
    /// matrix row. This test is the guard the plan names for W1.
    #[test]
    fn disclosure_adds_evidence_not_a_capability_row_or_abi() {
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
        assert_eq!(crate::screen::platform_capabilities().rows().len(), 12);
        assert_eq!(
            act_names(),
            vec!["focus", "move", "click", "hover", "drag", "type", "key", "scroll", "control"]
        );
        let disclose = capability("disclose").unwrap();
        assert!(disclose.params.is_empty(), "disclose still takes no typed parameter");
        assert_eq!(disclose.kind, CuKind::Disclose);
        // The additions are evidence keys, not capability names.
        for invented in ["environment", "grants", "act:environment", "act:grants"] {
            assert_eq!(capability(invented), None, "{invented} must not be a capability");
        }
        let host = host(false);
        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(
            disclosed.evidence["capabilities"].as_array().unwrap().len(),
            CAPABILITIES.len()
        );
        assert_eq!(
            disclosed.evidence["platform_capabilities"].as_array().unwrap().len(),
            12
        );
    }

    #[test]
    fn a_malformed_probe_set_is_not_performed_and_publishes_no_disclosure() {
        let host = host(false);
        let environment = crate::screen::environment_probe();
        let scales = crate::screen::display_scale_probes();
        let violation = host.disclose_with(
            "disclose",
            0,
            &environment,
            Err(
                "screen: refused: grant probe \"screen_capture\" is published twice; a reader \
                 cannot recompute a disclosure with a duplicated name"
                    .to_owned(),
            ),
            scales.clone(),
        );
        assert_eq!(violation.verdict, CuVerdict::NotPerformed, "{}", violation.text);
        assert!(violation.text.contains("published twice"), "{}", violation.text);
        assert_eq!(violation.evidence["contract"], "grants");
        // No partial disclosure: a reader that saw `grants` would infer a shape
        // the host never finished.
        assert!(violation.evidence.get("grants").is_none());
        assert!(violation.evidence.get("environment").is_none());
        // The same host still discloses normally for a well-formed set.
        let fine = host.disclose_with(
            "disclose",
            0,
            &environment,
            crate::screen::grant_probes(),
            scales,
        );
        assert_eq!(fine.verdict, CuVerdict::Performed, "{}", fine.text);
        assert!(fine.evidence["grants"].as_array().is_some_and(|rows| !rows.is_empty()));
    }

    #[test]
    fn held_keys_add_parameters_not_capabilities_or_abi() {
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!(
            act_names(),
            vec!["focus", "move", "click", "hover", "drag", "type", "key", "scroll", "control"]
        );
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
        let key = capability("act:key").unwrap();
        for name in ["hold", "release"] {
            let param = key.params.iter().find(|param| param.name == name).unwrap();
            assert_eq!(param.ty, CuType::Bool);
            assert!(!param.required);
        }
        let definitions = crate::wire::tool_definitions();
        let screen = definitions
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["function"]["name"] == "screen")
            .unwrap();
        let props = &screen["function"]["parameters"]["properties"];
        assert_eq!(props["hold"]["type"], "boolean");
        assert_eq!(props["release"]["type"], "boolean");
    }

    // -- 0.0.10 / PRD 01.13: display scale disclosure --------------------

    /// The scale whitelist is exactly these six tags. `1.0` is deliberately
    /// outside it: no cell may publish a plausible-looking default.
    #[test]
    fn display_scale_whitelist_is_exactly_six_and_round_trips() {
        use crate::screen::DisplayScale;
        let tags: Vec<&str> = DisplayScale::ALL.iter().map(|scale| scale.tag()).collect();
        assert_eq!(tags, vec!["1", "1.25", "1.5", "2", "3", "unknown"]);
        assert_eq!(
            tags.iter().collect::<std::collections::HashSet<_>>().len(),
            6,
            "a duplicate tag would make the whitelist ambiguous: {tags:?}"
        );
        for scale in DisplayScale::ALL {
            assert_eq!(DisplayScale::parse(scale.tag()), Some(scale));
        }
        for outside in ["", "1.0", "0", "4", "granted", "UNKNOWN", "1.250", "unknown "] {
            assert_eq!(DisplayScale::parse(outside), None, "{outside:?}");
        }
        for (millis, expected) in [
            (1000, DisplayScale::One),
            (1250, DisplayScale::OneQuarter),
            (1500, DisplayScale::OneHalf),
            (2000, DisplayScale::Two),
            (3000, DisplayScale::Three),
        ] {
            assert_eq!(DisplayScale::from_millis(millis), Some(expected));
        }
        for outside in [0, 1100, 1333, 1750, 2500, 4000, -1000] {
            assert_eq!(DisplayScale::from_millis(outside), None, "{outside}");
        }
        assert!(!DisplayScale::One.needs_reason());
        assert!(DisplayScale::Unknown.needs_reason());
        assert_eq!(DisplayScale::Unknown.ratio(), None);
    }

    /// An unanswerable scale must name its OS and the symbol that failed; a
    /// long reason is cut and marked, never published whole.
    #[test]
    fn display_scale_reasons_are_required_and_bounded() {
        use crate::screen::{
            display_scale_row, normalize_display_scales, truncate_scale_reason, DisplayScale,
            ScaleRectBasis, SCREEN_TOOL_MAX_SCALE_REASON,
        };
        let missing = display_scale_row(
            0,
            (0, 0, 100, 100),
            ScaleRectBasis::Physical,
            DisplayScale::Unknown,
            "",
        );
        let err = normalize_display_scales(vec![missing]).unwrap_err();
        assert!(err.contains("without a reason"), "{err}");
        let blank = display_scale_row(
            0,
            (0, 0, 100, 100),
            ScaleRectBasis::Physical,
            DisplayScale::Unknown,
            "   ",
        );
        assert!(normalize_display_scales(vec![blank])
            .unwrap_err()
            .contains("without a reason"));

        let long = "x".repeat(SCREEN_TOOL_MAX_SCALE_REASON + 40);
        let cut = truncate_scale_reason(&long);
        assert_eq!(cut.chars().count(), SCREEN_TOOL_MAX_SCALE_REASON);
        assert_eq!(cut.chars().last(), Some('…'));
        let exact = "y".repeat(SCREEN_TOOL_MAX_SCALE_REASON);
        assert_eq!(truncate_scale_reason(&exact), exact);

        // A known scale derives logical_rect and needs no reason.
        let known = display_scale_row(
            0,
            (0, 0, 200, 150),
            ScaleRectBasis::Physical,
            DisplayScale::OneQuarter,
            "unused",
        );
        assert_eq!(known.logical_rect, (0, 0, 160, 120));
        assert_eq!(known.physical_rect, (0, 0, 200, 150));
        assert_eq!(known.reason, None);
        let unknown = display_scale_row(
            1,
            (160, 0, 200, 150),
            ScaleRectBasis::Physical,
            DisplayScale::Unknown,
            "windows: GetDpiForSystem returned 0",
        );
        let rows = normalize_display_scales(vec![known, unknown]).unwrap();
        assert_eq!(rows[1].scale, DisplayScale::Unknown);
        assert!(rows[1]
            .reason
            .as_deref()
            .unwrap()
            .contains("GetDpiForSystem"));
    }

    /// Bound, index order and the recomputable relation. A violation refuses
    /// the whole set instead of publishing a partial scale disclosure.
    #[test]
    fn display_scale_rows_are_bounded_ordered_and_recomputable() {
        use crate::screen::{
            display_scale_row, normalize_display_scales, DisplayScale, ScaleRectBasis,
            SCREEN_TOOL_MAX_DISPLAY_SCALES,
        };
        let row = |index| {
            display_scale_row(
                index,
                (0, 0, 100, 100),
                ScaleRectBasis::Physical,
                DisplayScale::One,
                "unused",
            )
        };
        let over: Vec<_> = (0..=SCREEN_TOOL_MAX_DISPLAY_SCALES).map(row).collect();
        let too_many = normalize_display_scales(over).unwrap_err();
        assert!(too_many.contains("exceed the bound"), "{too_many}");
        assert!(too_many.contains(&SCREEN_TOOL_MAX_DISPLAY_SCALES.to_string()));
        assert!(normalize_display_scales(Vec::new())
            .unwrap_err()
            .contains("one named"));

        let mut swapped = vec![row(0), row(1)];
        swapped[1].index = 7;
        assert!(normalize_display_scales(swapped)
            .unwrap_err()
            .contains("displays[] order"));

        // 600x300 is divisible by every scale in the typed set.
        for scale in [
            DisplayScale::One,
            DisplayScale::OneQuarter,
            DisplayScale::OneHalf,
            DisplayScale::Two,
            DisplayScale::Three,
        ] {
            let good = display_scale_row(
                0,
                (0, 0, 600, 300),
                ScaleRectBasis::Physical,
                scale,
                "unused",
            );
            assert_eq!(good.scale, scale, "{} divides exactly", scale.tag());
            assert!(normalize_display_scales(vec![good.clone()]).is_ok());
            let mut bad = good;
            bad.physical_rect.2 += 1;
            assert!(normalize_display_scales(vec![bad])
                .unwrap_err()
                .contains("not logical_rect"));
        }
        // A rect that does not divide exactly is downgraded to `unknown`.
        let odd = display_scale_row(
            0,
            (0, 0, 101, 100),
            ScaleRectBasis::Physical,
            DisplayScale::OneQuarter,
            "windows: GetDpiForSystem",
        );
        assert_eq!(odd.scale, DisplayScale::Unknown);
        assert!(odd.reason.is_some());
    }

    /// The live probe set of whichever backend compiled here must already
    /// satisfy the contract. This catches a cell that published a plausible
    /// `1`, an unsorted row or an `unknown` without a named symbol.
    #[test]
    fn this_cells_native_display_scales_satisfy_the_contract() {
        use crate::screen::{
            DisplayScale, SCREEN_TOOL_MAX_DISPLAY_SCALES, SCREEN_TOOL_MAX_SCALE_REASON,
        };
        let rows = crate::screen::display_scale_probes()
            .expect("this cell's scale rows are well formed");
        eprintln!(
            "cu display_scales platform={} rows={:?}",
            crate::screen::platform_name(),
            rows.iter()
                .map(|row| (row.index, row.scale.tag()))
                .collect::<Vec<_>>()
        );
        assert!(!rows.is_empty(), "a cell that cannot probe must publish one `unknown` row");
        assert!(rows.len() <= SCREEN_TOOL_MAX_DISPLAY_SCALES);
        let symbols = [
            "GetDpiForSystem",
            "GetDpiForWindow",
            "GetSystemMetricsForDpi",
            "backingScaleFactor",
            "NSScreen",
            "Xft.dpi",
            "XResourceManagerString",
            "XRRGetScreenResources",
            "XDisplayWidth",
            "CGGetActiveDisplayList",
        ];
        for (position, row) in rows.iter().enumerate() {
            assert_eq!(row.index, position, "rows follow displays[] order");
            assert_eq!(DisplayScale::parse(row.scale.tag()), Some(row.scale));
            if row.scale.needs_reason() {
                let reason = row.reason.as_deref().unwrap_or_default();
                assert!(!reason.trim().is_empty(), "row {position} has no reason");
                assert!(
                    reason.contains(crate::screen::platform_name()),
                    "reason must name the OS: {reason}"
                );
                assert!(
                    symbols.iter().any(|symbol| reason.contains(symbol)),
                    "reason must name the failed symbol: {reason}"
                );
                assert!(reason.chars().count() <= SCREEN_TOOL_MAX_SCALE_REASON);
            }
            if let Some((num, den)) = row.scale.ratio() {
                for (logical, physical) in [
                    (row.logical_rect.0, row.physical_rect.0),
                    (row.logical_rect.1, row.physical_rect.1),
                    (row.logical_rect.2, row.physical_rect.2),
                    (row.logical_rect.3, row.physical_rect.3),
                ] {
                    assert_eq!(
                        logical as i64 * num,
                        physical as i64 * den,
                        "physical_rect must equal logical_rect * {}",
                        row.scale.tag()
                    );
                }
            }
        }
    }

    /// `observe` publishes the scale on `displays[i]`, and a refused probe set
    /// becomes `unknown` rather than a plausible `1`.
    #[test]
    fn observe_displays_carry_a_typed_scale() {
        use crate::screen::{display_scale_row, DisplayInfo, DisplayScale, ScaleRectBasis};
        let snapshot = ScreenSnapshot {
            platform: "windows".to_owned(),
            qualified: true,
            desktop: (0, 0, 200, 150),
            displays: vec![DisplayInfo {
                x: 0,
                y: 0,
                width: 200,
                height: 150,
                primary: true,
            }],
            display_scales: vec![display_scale_row(
                0,
                (0, 0, 200, 150),
                ScaleRectBasis::Physical,
                DisplayScale::OneQuarter,
                "unused",
            )],
            windows: Vec::new(),
            controls: Vec::new(),
            controls_source: None,
            controls_truncated: false,
            a11y_controls: Vec::new(),
            a11y_source: None,
            a11y_reason: None,
            a11y_truncated: false,
            capture: None,
        };
        let frame = CuFrame { generation: 1, facts_as_of_ms: 0, snapshot };
        let frame_json = frame.to_json();
        let displays = frame_json["displays"].as_array().unwrap();
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0]["scale"], "1.25");
        assert!(frame.snapshot.to_text().contains("scale=1.25"));
        let mut frame = frame;
        frame.snapshot.display_scales.clear();
        assert_eq!(frame.to_json()["displays"][0]["scale"], "unknown");
    }

    /// Disclosure stays available without `--allow-screen`, and now carries
    /// the coordinate basis nested under `environment` rather than as a new
    /// capability, verdict or top-level evidence key.
    #[test]
    fn disclosure_publishes_display_scales_without_a_grant() {
        use crate::screen::DisplayScale;
        let host = host(false);
        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(disclosed.verdict, CuVerdict::Performed, "{}", disclosed.text);
        assert_eq!(disclosed.evidence["grant"], "denied");
        let environment = disclosed.evidence["environment"]
            .as_object()
            .expect("environment is an object");
        for key in [
            "platform",
            "session",
            "display_count",
            "virtual_desktop",
            "foreground_present",
            "display_scales",
        ] {
            assert!(environment.contains_key(key), "environment is missing {key}");
        }
        let scales = disclosed.evidence["environment"]["display_scales"]
            .as_array()
            .expect("display_scales is an array");
        assert!(!scales.is_empty());
        for (position, row) in scales.iter().enumerate() {
            assert_eq!(row["index"].as_u64(), Some(position as u64));
            for key in ["logical_rect", "physical_rect"] {
                for field in ["x", "y", "width", "height"] {
                    assert!(row[key][field].is_i64(), "{key}.{field} is an integer");
                }
            }
            let scale = row["scale"].as_str().expect("every row has a scale");
            let parsed = DisplayScale::parse(scale)
                .unwrap_or_else(|| panic!("scale {scale:?} is outside the whitelist"));
            if parsed.needs_reason() {
                assert!(
                    row["reason"]
                        .as_str()
                        .is_some_and(|reason| !reason.trim().is_empty()),
                    "{scale:?} row {position} has no reason"
                );
            }
        }
        assert!(
            disclosed.text.contains("\ndisplay_scale index="),
            "{}",
            disclosed.text
        );
        // Nested only: no new top-level evidence key.
        assert!(disclosed.evidence.get("display_scales").is_none());
        // 0.0.6..0.0.9 evidence keys are untouched by the addition.
        for absent in ["actuation", "keyboard", "scroll"] {
            assert!(disclosed.evidence.get(absent).is_none(), "{absent} must stay absent");
        }
        assert_eq!(disclosed.evidence["held_keys"], json!([]));
    }

    /// 0.0.10 adds evidence only: no 13th capability, no ABI bump, no new
    /// matrix row and no change to the six false cells.
    #[test]
    fn display_scales_add_no_capability_row_or_abi_bump() {
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
        assert_eq!(crate::screen::platform_capabilities().rows().len(), 12);
        assert_eq!(
            act_names(),
            vec!["focus", "move", "click", "hover", "drag", "type", "key", "scroll", "control"]
        );
        for invented in [
            "display_scales",
            "act:display_scale",
            "scale",
            "op:disclose_scales",
        ] {
            assert_eq!(capability(invented), None, "{invented} must not be a capability");
        }
        let host = host(false);
        let disclosed = host.invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(
            disclosed.evidence["capabilities"].as_array().unwrap().len(),
            CAPABILITIES.len()
        );
        assert_eq!(
            disclosed.evidence["platform_capabilities"].as_array().unwrap().len(),
            12
        );
    }

    /// A malformed scale set publishes no partial disclosure at all, even
    /// though the grant half of the same call was already well formed.
    #[test]
    fn a_malformed_scale_set_is_not_performed_and_publishes_no_disclosure() {
        let host = host(false);
        let environment = crate::screen::environment_probe();
        let violation = host.disclose_with(
            "disclose",
            0,
            &environment,
            crate::screen::grant_probes(),
            Err(
                "screen: refused: display scale row 0 is unknown without a reason; the \
                 disclosure was not published"
                    .to_owned(),
            ),
        );
        assert_eq!(violation.verdict, CuVerdict::NotPerformed, "{}", violation.text);
        assert_eq!(violation.evidence["contract"], "display_scales");
        assert!(violation.evidence.get("environment").is_none());
        assert!(violation.evidence.get("grants").is_none());
    }

    // -- 0.0.13 / PRD 01.16: bounded observation wait ----------------------

    #[test]
    fn wait_ms_is_an_additive_observe_parameter() {
        let parsed = parse_request(&json!({ "op": "observe", "wait_ms": 125 })).unwrap();
        assert!(matches!(
            parsed.call,
            CuCall::Observe { wait_ms: Some(125), .. }
        ));
        for bad in [
            json!({ "op": "observe", "wait_ms": 0 }),
            json!({ "op": "observe", "wait_ms": 2001 }),
            json!({ "op": "observe", "wait_ms": -1 }),
            json!({ "op": "observe", "wait_ms": "125" }),
            json!({ "op": "observe", "wait_ms": Value::Null }),
        ] {
            let error = parse_request(&bad).unwrap_err();
            assert!(error.contains("1..=2000"), "{error}");
        }
        let observe = capability("observe").unwrap();
        assert!(observe.params.iter().any(|p| p.name == "wait_ms"
            && p.ty == CuType::Int
            && !p.required));
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
        let definitions = crate::wire::tool_definitions();
        let screen = definitions
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["function"]["name"] == "screen")
            .expect("screen tool definition");
        assert_eq!(
            screen["function"]["parameters"]["properties"]["wait_ms"]["type"],
            "integer"
        );
    }

    #[test]
    fn wait_refusals_name_observe_and_publish_no_wait_evidence() {
        let junk = host(false).invoke_json(&json!({ "op": "observe", "wait_ms": 2001 }));
        assert_eq!(junk.verdict, CuVerdict::Refused);
        assert_eq!(junk.capability, "observe");
        assert!(junk.text.contains("1..=2000"), "{}", junk.text);
        assert!(junk.evidence.get("wait").is_none());

        let calls = host(true);
        {
            let mut state = calls.state.lock().unwrap();
            state.wait_calls = SCREEN_TOOL_MAX_WAIT_CALLS;
        }
        let refused = calls.invoke_json(&json!({ "op": "observe", "wait_ms": 1 }));
        assert_eq!(refused.verdict, CuVerdict::Refused);
        assert!(refused.text.contains("call budget"), "{}", refused.text);
        assert!(refused.evidence.get("wait").is_none());

        let cumulative = host(true);
        {
            let mut state = cumulative.state.lock().unwrap();
            state.wait_cumulative_ms = SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS;
        }
        let refused = cumulative.invoke_json(&json!({ "op": "observe", "wait_ms": 1 }));
        assert_eq!(refused.verdict, CuVerdict::Refused);
        assert!(refused.text.contains("cumulative"), "{}", refused.text);
        assert!(refused.evidence.get("wait").is_none());
    }

    #[test]
    fn bounded_wait_accounting_is_pure_host_state() {
        let mut state = CuState {
            generation: 0,
            frame: None,
            receipts: Vec::new(),
            keyboard: KeyboardState::default(),
            wait_cumulative_ms: 0,
            wait_calls: 0,
        };
        let calls1 = reserve_wait(&mut state, 5).unwrap();
        let elapsed1 = measure_wait(5);
        let cumulative1 = settle_wait(&mut state, 5, elapsed1);
        assert_eq!(calls1, 1);
        assert!(elapsed1 >= 5 && elapsed1 <= 5 + SCREEN_TOOL_WAIT_OVERHEAD_MS);
        assert_eq!(cumulative1, elapsed1);

        let calls2 = reserve_wait(&mut state, 5).unwrap();
        let elapsed2 = measure_wait(5);
        let cumulative2 = settle_wait(&mut state, 5, elapsed2);
        assert_eq!(calls2, 2);
        assert!(elapsed2 >= 5 && elapsed2 <= 5 + SCREEN_TOOL_WAIT_OVERHEAD_MS);
        assert_eq!(cumulative2, elapsed1 + elapsed2);
        assert_eq!(state.wait_cumulative_ms, elapsed1 + elapsed2);

        state.wait_calls = SCREEN_TOOL_MAX_WAIT_CALLS;
        let too_many = reserve_wait(&mut state, 1).unwrap_err();
        assert!(too_many.contains("call budget"), "{too_many}");
        state.wait_calls = 0;
        state.wait_cumulative_ms = SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS;
        let too_long = reserve_wait(&mut state, 1).unwrap_err();
        assert!(too_long.contains("cumulative"), "{too_long}");
    }

    #[test]
    fn disclose_publishes_the_initial_wait_budget() {
        let result = host(false).invoke_json(&json!({ "op": "disclose" }));
        assert_eq!(result.verdict, CuVerdict::Performed);
        let budget = &result.evidence["wait_budget"];
        assert_eq!(budget["cumulative_ms"], 0);
        assert_eq!(budget["wait_calls"], 0);
        assert_eq!(budget["max_cumulative_ms"], SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS);
        assert_eq!(budget["max_wait_calls"], SCREEN_TOOL_MAX_WAIT_CALLS);
        assert_eq!(budget["remaining_ms"], SCREEN_TOOL_MAX_CUMULATIVE_WAIT_MS);
        assert_eq!(budget["remaining_calls"], SCREEN_TOOL_MAX_WAIT_CALLS);
    }

    #[test]
    fn a11y_state_names_are_a_fixed_bit_table_not_a_guess() {
        assert!(crate::screen::a11y_state_names(0).is_empty());
        assert_eq!(
            crate::screen::a11y_state_names(0x4 | 0x10),
            vec!["focused", "checked"]
        );
        assert_eq!(
            crate::screen::a11y_state_names(0x4000_0000 | 0x100000),
            vec!["focusable", "haspopup"]
        );
        // An unknown bit stays in the raw integer but is never named.
        assert!(crate::screen::a11y_state_names(0x1).is_empty());
    }

    #[test]
    fn a11y_node_json_publishes_the_normalised_schema() {
        let control = crate::screen::A11yControl {
            name: "Send".to_owned(),
            value: "send".to_owned(),
            role: 43,
            role_name: "button",
            state: 0x4 | 0x100000,
            x: 1,
            y: 2,
            width: 3,
            height: 4,
            depth: 1,
            child_id: 0,
        };
        let row = a11y_control_json(&control);
        assert_eq!(row["name"], "Send");
        assert_eq!(row["value"], "send");
        assert_eq!(row["role"], 43);
        assert_eq!(row["role_name"], "button");
        assert_eq!(row["state"], 0x4 | 0x100000);
        assert_eq!(row["state_names"], json!(["focused", "focusable"]));
        assert_eq!(row["rect"], json!([1, 2, 3, 4]));
        assert_eq!(row["depth"], 1);
        assert_eq!(row["child_id"], 0);
        assert_eq!(CAPABILITIES.len(), 11);
        assert_eq!((CU_ABI_MAJOR, CU_ABI_MINOR), (1, 0));
    }

    // ---- 0.0.15: a11y-addressed actuation --------------------------------

    #[test]
    fn control_source_is_admitted_before_any_pointer_exists() {
        let plain = parse_request(&json!({
            "op": "act", "why": "x", "action": "control", "control": "OK"
        }))
        .unwrap();
        assert!(matches!(
            plain.call,
            CuCall::Act {
                action: CuAction::Control {
                    source: ControlSource::Controls,
                    role: None,
                    ..
                },
                ..
            }
        ));
        let semantic = parse_request(&json!({
            "op": "act", "why": "x", "action": "control", "control": "新建任务",
            "source": "a11y", "role": "button"
        }))
        .unwrap();
        assert!(matches!(
            semantic.call,
            CuCall::Act {
                action: CuAction::Control {
                    source: ControlSource::A11y,
                    role: Some(ref role),
                    ..
                },
                ..
            } if role == "button"
        ));
        let errors: Vec<String> = [
            json!({ "op": "act", "why": "x", "action": "control", "control": "OK", "source": "uia" }),
            json!({ "op": "act", "why": "x", "action": "control", "control": "OK", "source": 1 }),
            json!({ "op": "act", "why": "x", "action": "control", "control": "OK", "role": "button" }),
            json!({ "op": "act", "why": "x", "action": "control", "control": "OK", "source": "a11y", "role": 7 }),
            json!({ "op": "act", "why": "x", "action": "control", "control": "OK", "source": "a11y", "role": "  " }),
        ]
        .iter()
        .map(|request| parse_request(request).unwrap_err())
        .collect();
        assert!(errors[0].contains("unknown `source`"), "{}", errors[0]);
        assert!(errors[1].contains("`source` must be a string"), "{}", errors[1]);
        assert!(errors[2].contains("`role` applies only"), "{}", errors[2]);
        assert!(errors[3].contains("`role` must be a string"), "{}", errors[3]);
        assert!(errors[4].contains("non-empty a11y role"), "{}", errors[4]);
        assert_eq!(ControlSource::Controls.tag(), "controls");
        assert_eq!(ControlSource::A11y.tag(), "a11y");
    }

    #[test]
    fn control_capability_discloses_the_two_additive_params() {
        let entry = capability("act:control").unwrap();
        let params: Vec<&str> = entry.params.iter().map(|param| param.name).collect();
        assert!(params.contains(&"source"), "{params:?}");
        assert!(params.contains(&"role"), "{params:?}");
        assert!(!entry.params.iter().any(|param| param.name == "source" && param.required));
        assert_eq!(CAPABILITIES.len(), 11, "source is a parameter, not a new capability");
        let schema = crate::wire::tool_definitions();
        let rendered = serde_json::to_string(&schema).unwrap();
        assert!(rendered.contains("\"enum\":[\"controls\",\"a11y\"]"), "{rendered}");
    }

    #[test]
    fn an_empty_a11y_tree_is_a_named_refusal_not_a_coordinate_fallback() {
        let caps = crate::screen::platform_capabilities();
        let empty = host_with_a11y_frame(Vec::new(), 1, 1, (0, 0, 800, 600));
        let result = empty.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control",
            "control": "新建任务", "source": "a11y"
        }));
        assert_ne!(result.verdict, CuVerdict::Performed, "{}", result.text);
        if !caps.supports("act:control") {
            // A cell whose matrix declares the verb unsupported names that
            // boundary at admission, before any frame is consulted. The pure
            // a11y resolver is separately tested on all four configurations.
            assert!(
                matches!(result.verdict, CuVerdict::Refused | CuVerdict::Unsupported),
                "{}",
                result.text
            );
            assert!(result.text.contains(crate::screen::platform_name()), "{}", result.text);
            assert!(result.text.contains("act:control"), "{}", result.text);
            let reason = crate::screen::resolve_a11y_control(&[], "新建任务", None)
                .unwrap_err();
            assert!(reason.contains("no a11y node matches"), "{reason}");
            return;
        }
        assert_eq!(result.evidence["reason"], "no_a11y_tree");
        assert!(result.text.contains("no a11y nodes"), "{}", result.text);
        // A stale a11y frame is stale, exactly like a Win32 frame.
        let stale = host_with_a11y_frame(
            vec![synthetic_a11y_node("新建任务", 43, 0x100000, 500, 16, 32, 32)],
            1,
            2,
            (0, 0, 800, 600),
        );
        let stale_result = stale.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control",
            "control": "新建任务", "source": "a11y"
        }));
        assert_eq!(stale_result.verdict, CuVerdict::Stale, "{}", stale_result.text);
    }

    #[test]
    fn an_a11y_target_never_guesses_ambiguous_or_inoperable_nodes() {
        let caps = crate::screen::platform_capabilities();
        let ambiguous_rows = vec![
            synthetic_a11y_node("OK", 43, 0x100000, 10, 10, 40, 20),
            synthetic_a11y_node("OK", 43, 0x100000, 60, 10, 40, 20),
        ];
        let vetoed_rows =
            vec![synthetic_a11y_node("关闭", 43, 0x8000, 10, 10, 40, 20)];
        if !caps.supports("act:control") {
            // Admission names the capability boundary; the backend-independent
            // resolver still has to refuse ambiguity and an inoperable node on
            // this cell. Both are pure functions, so no desktop is needed.
            let reason = crate::screen::resolve_a11y_control(&ambiguous_rows, "OK", None)
                .unwrap_err();
            assert!(reason.contains("matches 2"), "{reason}");
            let reason = crate::screen::resolve_a11y_control(&vetoed_rows, "关闭", None)
                .unwrap_err();
            assert!(reason.contains("invisible"), "{reason}");
            return;
        }
        let host = host_with_a11y_frame(ambiguous_rows, 1, 1, (0, 0, 800, 600));
        let ambiguous = host.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "OK", "source": "a11y"
        }));
        assert_ne!(ambiguous.verdict, CuVerdict::Performed);
        assert!(ambiguous.text.contains("matches 2"), "{}", ambiguous.text);

        let vetoed = host_with_a11y_frame(
            vec![synthetic_a11y_node("关闭", 43, 0x8000, 10, 10, 40, 20)],
            1,
            1,
            (0, 0, 800, 600),
        );
        let refused = vetoed.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "关闭", "source": "a11y"
        }));
        assert_ne!(refused.verdict, CuVerdict::Performed);
        assert!(refused.text.contains("invisible"), "{}", refused.text);

        let outside = host_with_a11y_frame(
            vec![synthetic_a11y_node("最大化", 43, 0x100000, 900, 10, 40, 20)],
            1,
            1,
            (0, 0, 800, 600),
        );
        let out = outside.invoke_json(&json!({
            "op": "act", "why": "x", "action": "control", "control": "最大化", "source": "a11y"
        }));
        assert_ne!(out.verdict, CuVerdict::Performed);
        assert!(out.text.contains("outside the virtual desktop"), "{}", out.text);
    }

    #[test]
    fn a_resolved_a11y_target_publishes_recomputable_evidence() {
        let host = host_with_a11y_frame(
            vec![synthetic_a11y_node("新建任务", 43, 0x4 | 0x100000, 500, 16, 32, 32)],
            1,
            1,
            (0, 0, 1680, 1050),
        );
        let result = host.invoke_json(&json!({
            "op": "act", "why": "CU journey: semantic target",
            "action": "control", "control": "新建任务", "source": "a11y", "role": "button"
        }));
        // On a real Windows desktop this performs a hover; on a cell without
        // act:control it is a named platform refusal. Neither may invent rows.
        if result.verdict == CuVerdict::Performed {
            assert_eq!(result.evidence["source"], "a11y");
            assert_eq!(result.evidence["a11y_node_schema"], "role-name-state-value-bounds-depth");
            let node = &result.evidence["a11y_node"];
            assert_eq!(node["name"], "新建任务");
            assert_eq!(node["role_name"], "button");
            assert_eq!(node["state"], 0x4 | 0x100000);
            assert_eq!(node["state_names"], json!(["focused", "focusable"]));
            assert_eq!(node["rect"], json!([500, 16, 32, 32]));
            assert_eq!(result.evidence["rect"], json!([500, 16, 32, 32]));
            assert_eq!(result.evidence["center"], json!([516, 32]));
            assert_eq!(result.evidence["press"], false);
        } else {
            assert!(
                matches!(result.verdict, CuVerdict::Refused | CuVerdict::Unsupported),
                "{}",
                result.text
            );
        }
    }

    // ---- 0.0.16: a11y effect readback (expect-vs-observed) ----

    fn effect_node(
        name: &str,
        value: &str,
        state: i32,
        rect: (i32, i32, i32, i32),
    ) -> crate::screen::A11yControl {
        crate::screen::A11yControl {
            name: name.to_owned(),
            value: value.to_owned(),
            role: 43,
            role_name: "textbox",
            state,
            x: rect.0,
            y: rect.1,
            width: rect.2,
            height: rect.3,
            depth: 6,
            child_id: 0,
        }
    }

    fn effect_snapshot(rows: Vec<crate::screen::A11yControl>) -> ScreenSnapshot {
        ScreenSnapshot {
            platform: "windows".to_owned(),
            qualified: true,
            desktop: (0, 0, 800, 600),
            displays: Vec::new(),
            display_scales: Vec::new(),
            windows: Vec::new(),
            controls: Vec::new(),
            controls_source: None,
            controls_truncated: false,
            a11y_controls: rows,
            a11y_source: Some("handle:33689578 window:\"WorkBuddy\"".to_owned()),
            a11y_reason: None,
            a11y_truncated: false,
            capture: None,
        }
    }

    fn effect_frame(generation: u64, snapshot: ScreenSnapshot) -> CuFrame {
        CuFrame { generation, facts_as_of_ms: 0, snapshot }
    }

    fn effect_request(name: &str, expect: &[&str]) -> EffectRequest {
        EffectRequest {
            hwnd: 33689578,
            name: name.to_owned(),
            role: Some("textbox".to_owned()),
            child_id: None,
            expect: expect.iter().map(|field| (*field).to_owned()).collect(),
        }
    }

    #[test]
    fn expect_is_sorted_deduplicated_and_whitelisted() {
        assert_eq!(
            normalise_expect(&["value".to_owned(), "state".to_owned(), "value".to_owned()]),
            Ok(vec!["state".to_owned(), "value".to_owned()])
        );
        assert_eq!(normalise_expect(&[]), Ok(vec![]));
        assert_eq!(normalise_expect(&["pixels".to_owned()]), Err("invalid_expect"));
    }

    #[test]
    fn effect_confirmed_only_when_observed_equals_the_pre_declared_expect() {
        let before = effect_frame(
            7,
            effect_snapshot(vec![effect_node("Text Editor", "a", 0x100000, (10, 10, 40, 20))]),
        );
        let after = effect_snapshot(vec![effect_node(
            "Text Editor", "b", 0x100000, (10, 10, 40, 20),
        )]);
        let row = a11y_effect_evidence(
            Some(&before), 7, &after, &effect_request("Text Editor", &["value"]),
        );
        assert_eq!(row["verdict"], json!("confirmed"));
        assert_eq!(row["expect"], json!(["value"]));
        assert_eq!(row["observed"], json!(["value"]));
        assert_eq!(row["before"]["value"], json!("a"));
        assert_eq!(row["after"]["value"], json!("b"));
        assert_eq!(row["before_generation"], json!(7));
        assert_eq!(row["after_generation"], json!(8));
        assert_eq!(
            row["before"]["state_names"],
            json!(crate::screen::a11y_state_names(0x100000))
        );
        assert_eq!(
            row["after"]["state_names"],
            json!(crate::screen::a11y_state_names(0x100000))
        );
    }

    #[test]
    fn a_no_op_is_unchanged_and_an_unmet_expectation_is_mismatch() {
        let before = effect_frame(
            3,
            effect_snapshot(vec![effect_node("Text Editor", "", 0x100040, (1, 2, 30, 10))]),
        );
        let after = effect_snapshot(vec![effect_node(
            "Text Editor", "", 0x100040, (1, 2, 30, 10),
        )]);
        let unchanged = a11y_effect_evidence(
            Some(&before), 3, &after, &effect_request("Text Editor", &[]),
        );
        assert_eq!(unchanged["verdict"], json!("unchanged"));
        assert_eq!(unchanged["observed"], json!([]));
        let mismatch = a11y_effect_evidence(
            Some(&before), 3, &after, &effect_request("Text Editor", &["value"]),
        );
        assert_eq!(mismatch["verdict"], json!("mismatch"));
        assert_eq!(mismatch["observed"], json!([]));
        assert_eq!(mismatch["expect"], json!(["value"]));
    }

    #[test]
    fn observed_fields_are_recomputed_and_sorted_not_echoed() {
        let before = effect_frame(
            1,
            effect_snapshot(vec![effect_node("Text Editor", "a", 0, (1, 2, 30, 10))]),
        );
        let after = effect_snapshot(vec![effect_node(
            "Text Editor", "b", 0x4, (1, 2, 31, 10),
        )]);
        let row = a11y_effect_evidence(
            Some(&before), 1, &after,
            &effect_request("Text Editor", &["rect", "state", "value"]),
        );
        assert_eq!(row["observed"], json!(["rect", "state", "value"]));
        assert_eq!(row["verdict"], json!("confirmed"));
    }

    #[test]
    fn a_vanished_node_is_not_reported_as_a_change() {
        let before = effect_frame(
            2,
            effect_snapshot(vec![effect_node("Text Editor", "", 0, (1, 2, 30, 10))]),
        );
        let after = effect_snapshot(vec![effect_node("Other", "", 0, (1, 2, 30, 10))]);
        let row = a11y_effect_evidence(
            Some(&before), 2, &after, &effect_request("Text Editor", &["value"]),
        );
        assert_eq!(row["verdict"], json!("vanished"));
        assert_eq!(row["after"], Value::Null);
        assert_eq!(row["observed"], json!([]));
    }

    #[test]
    fn effect_refusals_are_named_and_never_publish_a_plausible_row() {
        let node = effect_node("Text Editor", "", 0, (1, 2, 30, 10));
        let frame = effect_frame(4, effect_snapshot(vec![node.clone()]));
        let after = effect_snapshot(vec![node.clone()]);
        let req = effect_request("Text Editor", &[]);
        let mut empty_before = effect_frame(4, after.clone());
        empty_before.snapshot.a11y_controls.clear();
        let mut empty_after = after.clone();
        empty_after.a11y_controls.clear();
        let cases: Vec<(Option<CuFrame>, u64, ScreenSnapshot, EffectRequest, &str)> = vec![
            (None, 4, after.clone(), req.clone(), "no_frame"),
            (Some(frame.clone()), 5, after.clone(), req.clone(), "stale"),
            (Some(empty_before), 4, after.clone(), req.clone(), "no_a11y_tree"),
            (Some(frame.clone()), 4, empty_after, req.clone(), "no_a11y_tree"),
            (
                Some(frame.clone()), 4, after.clone(),
                effect_request("missing", &[]), "no_matching_node",
            ),
            (
                Some(effect_frame(4, effect_snapshot(vec![node.clone(), node.clone()]))),
                4, after.clone(), req.clone(), "ambiguous_identity",
            ),
            (
                Some(frame.clone()), 4, after.clone(),
                effect_request("Text Editor", &["pixels"]), "invalid_expect",
            ),
        ];
        for (before, generation, snapshot, request, reason) in cases {
            let row = a11y_effect_evidence(before.as_ref(), generation, &snapshot, &request);
            assert_eq!(row["verdict"], json!("refused"), "{reason}: {row}");
            assert_eq!(row["reason"], json!(reason), "{reason}: {row}");
            assert_eq!(row["observed"], json!([]), "{reason}: {row}");
            assert_eq!(row["before"], Value::Null, "{reason}: {row}");
            assert_eq!(row["after"], Value::Null, "{reason}: {row}");
        }
    }

    #[test]
    fn effect_is_admitted_before_the_act_and_disclosed_as_a_param() {
        let parsed = parse_request(&json!({
            "op": "observe", "a11y": 33689578,
            "effect": {
                "hwnd": 33689578, "name": "Text Editor",
                "role": "textbox", "expect": ["value"]
            }
        }))
        .unwrap();
        assert_eq!(
            parsed.call,
            CuCall::Observe {
                window: None,
                capture: None,
                controls: None,
                a11y: Some(A11yTarget::Handle(33689578)),
                scale: 0,
                wait_ms: None,
                effect: Some(EffectRequest {
                    hwnd: 33689578,
                    name: "Text Editor".to_owned(),
                    role: Some("textbox".to_owned()),
                    child_id: None,
                    expect: vec!["value".to_owned()],
                }),
            }
        );
        // The expectation must be declared up front; a post-hoc one cannot be
        // attached to a result, and a non-string field is refused.
        assert!(parse_request(&json!({
            "op": "observe", "effect": { "hwnd": 1, "name": "x" }
        }))
        .is_err());
        assert!(parse_request(&json!({
            "op": "observe", "effect": { "hwnd": 1, "name": "x", "expect": [7] }
        }))
        .is_err());
        let observe = CAPABILITIES.iter().find(|cap| cap.name == "observe").unwrap();
        assert!(observe
            .params
            .iter()
            .any(|param| param.name == "effect" && param.ty == CuType::Effect));
        assert_eq!(CuType::Effect.tag(), "effect-object");
    }

}
