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
    Act, CaptureTarget, ClickButton, ScreenSnapshot, ScreenTool, WindowInfo,
    SCREEN_TOOL_MAX_CAPTURE_SCALE, SCREEN_TOOL_MAX_TYPE_CHARS,
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
    Button,
}

impl CuType {
    fn tag(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::String => "string",
            Self::Capture => "capture",
            Self::Button => "button",
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
const P_SCALE: CuParam = CuParam { name: "scale", ty: CuType::Int, required: false };
const P_WHY: CuParam = CuParam { name: "why", ty: CuType::String, required: true };
const P_X: CuParam = CuParam { name: "x", ty: CuType::Int, required: true };
const P_Y: CuParam = CuParam { name: "y", ty: CuType::Int, required: true };
const P_TO_X: CuParam = CuParam { name: "to_x", ty: CuType::Int, required: true };
const P_TO_Y: CuParam = CuParam { name: "to_y", ty: CuType::Int, required: true };
const P_WIDTH: CuParam = CuParam { name: "width", ty: CuType::Int, required: true };
const P_HEIGHT: CuParam = CuParam { name: "height", ty: CuType::Int, required: true };
const P_TITLE: CuParam = CuParam { name: "window", ty: CuType::String, required: true };
const P_BUTTON: CuParam = CuParam { name: "button", ty: CuType::Button, required: false };
const P_DOUBLE: CuParam = CuParam { name: "double", ty: CuType::Bool, required: false };
const P_TEXT: CuParam = CuParam { name: "text", ty: CuType::String, required: true };
const P_KEYS: CuParam = CuParam { name: "keys", ty: CuType::String, required: true };
const P_DELTA: CuParam = CuParam { name: "delta", ty: CuType::Int, required: true };

/// The single source of truth for what computer use can do here. The wire
/// tool definition's action enum is tested against this table so the two
/// cannot drift.
pub const CAPABILITIES: &[CuCapability] = &[
    CuCapability {
        name: "observe",
        kind: CuKind::Observe,
        summary: "Read virtual-desktop geometry, displays and windows; optionally publish one bounded PNG.",
        params: &[P_WINDOW, P_CAPTURE, P_SCALE],
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
        summary: "Press, move in twelve steps, release; readback is the final cursor position.",
        params: &[P_WHY, P_X, P_Y, P_TO_X, P_TO_Y, P_BUTTON],
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
        summary: "One wheel turn at a position; readback is the cursor position.",
        params: &[P_WHY, P_X, P_Y, P_DELTA],
    },
];

pub fn capability(name: &str) -> Option<&'static CuCapability> {
    CAPABILITIES.iter().find(|entry| entry.name == name)
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
            "capture": self.snapshot.capture.as_ref().map(|outcome| json!({
                "path": outcome.path, "bytes": outcome.bytes,
                "width": outcome.width, "height": outcome.height,
                "scale": outcome.scale, "source": outcome.source,
            })),
        })
    }
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
    Drag { x: i32, y: i32, to_x: i32, to_y: i32, button: ClickButton },
    Type { text: String },
    Key { keys: String },
    Scroll { x: i32, y: i32, delta: i32 },
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CuCall {
    Observe {
        window: Option<String>,
        capture: Option<CaptureTarget>,
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
        // Platform qualification is a host fact, not a native surprise: an
        // unqualified cell refuses by name before a device context exists.
        if kind != CuKind::Disclose && !crate::screen::platform_qualified() {
            return self.result_with_state(
                &mut state,
                CuVerdict::Refused,
                &capability,
                format!(
                    "screen: refused: computer use is not qualified on {} (v1 is Windows only); \
                     a refusal is the honest answer, an empty observation would not be",
                    crate::screen::platform_name()
                ),
                json!({ "qualified": false, "platform": crate::screen::platform_name() }),
            );
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
            CuCall::Observe { window, capture, scale } => {
                self.observe(&capability, window, capture, scale)
            }
            CuCall::Act { why, action } => self.act(&capability, why, action),
            CuCall::Disclose => self.disclose(&capability, request.abi_minor),
        }
    }

    fn observe(
        &self,
        capability: &str,
        window: Option<String>,
        capture: Option<CaptureTarget>,
        scale: u32,
    ) -> CuResult {
        match self.tool.snapshot(window.as_deref(), capture, scale) {
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
                    "{} windows, {} displays",
                    snapshot.windows.len(),
                    snapshot.displays.len()
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
            CuAction::Drag { x, y, to_x, to_y, button } => Act::Drag {
                x: *x,
                y: *y,
                to_x: *to_x,
                to_y: *to_y,
                button: *button,
            },
            CuAction::Type { text } => Act::Type { text },
            CuAction::Key { keys } => Act::Key { keys: keys.clone() },
            CuAction::Scroll { x, y, delta } => Act::Scroll { x: *x, y: *y, delta: *delta },
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
        evidence: Value,
    ) -> CuResult {
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

fn classify(error: String) -> (CuVerdict, String) {
    if error.starts_with("screen: failed:") {
        (CuVerdict::NotPerformed, error)
    } else if error.starts_with("screen: refused:") {
        (CuVerdict::Refused, error)
    } else {
        // Anything else is an unconfirmed native outcome. Never report a
        // success for it, and never replay it.
        (CuVerdict::Unknown, error)
    }
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
            CuCall::Observe { window, capture, scale }
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
                 key, scroll)"
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
                "drag" => CuAction::Drag {
                    x: number("x")?,
                    y: number("y")?,
                    to_x: number("to_x")?,
                    to_y: number("to_y")?,
                    button: button(),
                },
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
                "scroll" => CuAction::Scroll {
                    x: number("x")?,
                    y: number("y")?,
                    delta: number("delta")?,
                },
                other => {
                    return Err(format!(
                        "screen: refused: unknown action {other:?}; act accepts focus, move, \
                         click, hover, drag, type, key, scroll"
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
    use crate::screen::ScreenTool;

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
