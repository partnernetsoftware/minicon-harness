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
    resolve_control, Act, Actuation, CaptureTarget, ClickButton, ControlInfo, ControlTarget,
    ScreenSnapshot, ScreenTool, WindowInfo, SCREEN_TOOL_MAX_CAPTURE_SCALE,
    SCREEN_TOOL_MAX_TYPE_CHARS,
};
use serde_json::{json, Value};
use std::sync::Mutex;

/// Major mismatch is refused before activation. A newer minor level is
/// accepted but cannot widen authority: unknown optional fields are ignored.
pub const CU_ABI_MAJOR: u32 = 1;
pub const CU_ABI_MINOR: u32 = 0;

/// Receipts retained in memory per invocation. A disclosure is a snapshot,
/// not an unbounded log.
pub const CU_MAX_RECEIPTS: usize = 32;

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
    Button,
    /// An array of `[x,y]` integer pairs, bounded by the drag path limit.
    Path,
}

impl CuType {
    fn tag(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::String => "string",
            Self::Capture => "capture",
            Self::ControlScan => "control-scan",
            Self::Button => "button",
            Self::Path => "int-pair-array",
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
const P_CONTROL: CuParam = CuParam { name: "control", ty: CuType::String, required: true };
const P_PRESS: CuParam = CuParam { name: "press", ty: CuType::Bool, required: false };
const P_SCALE: CuParam = CuParam { name: "scale", ty: CuType::Int, required: false };
const P_WHY: CuParam = CuParam { name: "why", ty: CuType::String, required: true };
const P_X: CuParam = CuParam { name: "x", ty: CuType::Int, required: true };
const P_Y: CuParam = CuParam { name: "y", ty: CuType::Int, required: true };
// `to_x`/`to_y` are required only when `path` is absent; `parse_request`
// enforces that conditional, so disclose must not advertise them as required.
const P_TO_X: CuParam = CuParam { name: "to_x", ty: CuType::Int, required: false };
const P_TO_Y: CuParam = CuParam { name: "to_y", ty: CuType::Int, required: false };
const P_WIDTH: CuParam = CuParam { name: "width", ty: CuType::Int, required: true };
const P_HEIGHT: CuParam = CuParam { name: "height", ty: CuType::Int, required: true };
const P_TITLE: CuParam = CuParam { name: "window", ty: CuType::String, required: true };
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

/// The single source of truth for what computer use can do here. The wire
/// tool definition's action enum is tested against this table so the two
/// cannot drift.
pub const CAPABILITIES: &[CuCapability] = &[
    CuCapability {
        name: "observe",
        kind: CuKind::Observe,
        summary: "Read virtual-desktop geometry, displays and windows; optionally publish one bounded PNG or scan one window's typed controls.",
        params: &[P_WINDOW, P_CAPTURE, P_CONTROLS, P_SCALE],
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
        summary: "Bring one uniquely named window to the foreground; readback is the title.",
        params: &[P_WHY, P_TITLE],
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
        summary: "Press one key combination; evidence is the foreground before and after.",
        params: &[P_WHY, P_KEYS],
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
        summary: "Resolve one control by name from the last observe and place the pointer at its centre; readback must land inside the control's rectangle. press=false by default (no click).",
        params: &[P_WHY, P_CONTROL, P_PRESS, P_BUTTON, P_DOUBLE],
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
            .map(|display| {
                json!({
                    "x": display.x, "y": display.y,
                    "width": display.width, "height": display.height,
                    "primary": display.primary,
                })
            })
            .collect();
        let windows: Vec<Value> = self.snapshot.windows.iter().map(window_json).collect();
        let controls: Vec<Value> = self.snapshot.controls.iter().map(control_json).collect();
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

/// The typed operation a caller asked the host to perform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CuAction {
    Focus { title: String },
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
    Key { keys: String },
    Scroll { x: i32, y: i32, delta_x: i32, delta_y: i32 },
    /// Resolve a control from the current frame **by name**; see
    /// [`CuHost::act_control`]. `press` is opt-in so a semantic target can be
    /// qualified on a real desktop without clicking anything.
    Control { control: String, press: bool, button: ClickButton, double: bool },
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CuCall {
    Observe {
        window: Option<String>,
        capture: Option<CaptureTarget>,
        controls: Option<ControlTarget>,
        scale: u32,
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

impl CuHost {
    pub fn new(tool: ScreenTool) -> Self {
        Self {
            tool,
            state: Mutex::new(CuState {
                generation: 0,
                frame: None,
                receipts: Vec::new(),
            }),
        }
    }

    pub fn granted(&self) -> bool {
        self.tool.enabled()
    }

    /// The JSON-facing entrance a dynamic-logic module would bind to. Parse
    /// failures are typed refusals, never Rust errors that abort the run.
    pub fn invoke_json(&self, arguments: &Value) -> CuResult {
        match parse_request(arguments) {
            Ok(request) => self.invoke(request),
            Err(refusal) => self.refusal("unknown", refusal),
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
            CuCall::Observe { window, capture, controls, scale } => {
                self.observe(&capability, window, capture, controls, scale)
            }
            CuCall::Act { why, action } => match action {
                CuAction::Control { control, press, button, double } => {
                    self.act_control(&capability, why, control, press, button, double)
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
        scale: u32,
    ) -> CuResult {
        match self.tool.snapshot(window.as_deref(), capture, scale, controls) {
            Ok(snapshot) => {
                let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                state.generation += 1;
                let frame = CuFrame {
                    generation: state.generation,
                    facts_as_of_ms: now_ms(),
                    snapshot: snapshot.clone(),
                };
                let evidence = frame.to_json();
                state.frame = Some(frame);
                let generation = state.generation;
                let mut text = snapshot.to_text();
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
        let resolved = match resolve_control(&frame.snapshot.controls, &control) {
            Ok(control) => control.clone(),
            Err(reason) => {
                return self.result_with_state(&mut state, CuVerdict::Refused, capability, reason, json!({ "control": control }));
            }
        };
        if !resolved.visible || !resolved.enabled {
            return self.result_with_state(
                &mut state,
                CuVerdict::Refused,
                capability,
                format!(
                    "screen: refused: control {:?} is {} and cannot be operated; the operation \
                     was not performed",
                    resolved.label,
                    if !resolved.visible { "hidden" } else { "disabled" }
                ),
                json!({ "control": resolved.label, "visible": resolved.visible, "enabled": resolved.enabled }),
            );
        }
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
        let evidence = json!({
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
        });
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
            CuAction::Focus { title } => Act::Focus { title: title.clone() },
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
            CuAction::Key { keys } => Act::Key { keys: keys.clone() },
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
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let manifest = self.manifest(&state, abi_minor);
        // Qualification comes from the native backend, never from a guess:
        // on an unqualified cell the disclosure says so.
        let mut text = format!(
            "cu-abi={CU_ABI_MAJOR}.{CU_ABI_MINOR} platform={} qualified={} grant={} \
             generation={} capabilities={} receipts={}",
            crate::screen::platform_name(),
            crate::screen::platform_qualified(),
            if self.tool.enabled() { "granted" } else { "denied" },
            state.generation,
            CAPABILITIES.len(),
            state.receipts.len()
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
            evidence: manifest,
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
            CuCall::Observe { window, capture, controls, scale }
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
            let button = || match arguments.get("button").and_then(Value::as_str) {
                Some("right") => ClickButton::Right,
                Some("middle") => ClickButton::Middle,
                _ => ClickButton::Left,
            };
            let act = match action {
                "focus" => CuAction::Focus { title: title()? },
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
                "key" => CuAction::Key {
                    keys: arguments
                        .get("keys")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            "screen: refused: action \"key\" needs a `keys` argument, e.g. \
                             \"ctrl+s\" or \"win+left\""
                                .to_owned()
                        })?
                        .to_owned(),
                },
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
                "control" => CuAction::Control {
                    control: arguments
                        .get("control")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| {
                            "screen: refused: action \"control\" needs a `control` name from \
                             the last observe"
                                .to_owned()
                        })?
                        .to_owned(),
                    // Hover is the default: resolving a named target must not
                    // be able to click a button just because a caller omitted
                    // a flag.
                    press: arguments
                        .get("press")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    button: button(),
                    double: arguments
                        .get("double")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
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
            windows: Vec::new(),
            controls_source: Some("window:synthetic".to_owned()),
            controls_truncated: false,
            controls,
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
                scale: 0,
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
}
