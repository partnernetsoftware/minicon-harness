//! The third tool: `screen` (computer use), in two halves that never blur:
//! `observe` reads the desktop and changes nothing, `act` performs exactly one
//! operation and reports what it did.
//!
//! The shape follows the contract already frozen by the sibling repositories
//! (`minicon/prd/PRD_02_38_computer_use.md` CU0..CU2 and
//! `agenterm/docs/cu-gaps-analysis.md`): observation and actuation are
//! separate target-bound choices, an operation names its target explicitly, and
//! a platform that has not implemented a verb refuses it by name instead of
//! pretending to work.
//!
//! Deliberately **no third-party crate and no link-time platform library**.
//! Each `target_os` compiles exactly one `native` backend -- hand-written
//! `user32`/`gdi32` FFI on Windows, `CoreGraphics`/`CoreFoundation` via
//! `dlopen` on macOS, and `X11`/`XTest` via `dlopen` on Linux
//! (`plan/plan-0.0.5-cross-platform-cu.md`). The store-mode PNG encoder,
//! bounds, downsample and readback rules are shared by all three: the harness
//! ships as one auditable executable per cell, and the six cells are
//! cross-compiled on a single runner with no foreign SDK to link against.
//!
//! Windows implements every verb. Linux implements all verbs except the
//! control arm; macOS deliberately leaves `act:focus` / `act:move` to the
//! Accessibility API and reports them as named refusals. A missing library or
//! a missing OS grant (Screen Recording / Input Monitoring) is a *named
//! refusal*, never a black PNG or a silent no-op. Observation has two arms --
//! the pixel arm
//! (virtual-desktop/display geometry, window inventory, a PNG written under
//! the task root) and, since 0.0.4, a **typed control arm** (child windows of
//! one resolved window, with class-derived roles, text, rectangle and state;
//! `prd/PRD_01_05_semantic_controls.md`). The control arm is Win32
//! `EnumChildWindows` + `WM_GETTEXT`, **not** a UIA accessibility tree: it is
//! deliberately smaller, and a window with no child windows is reported as
//! such rather than as a successful semantic read.
//!
//! Every action is verified by readback -- cursor position, window rectangle
//! or foreground title -- rather than by the API call having returned.

use std::path::{Path, PathBuf};

/// Most windows one `observe` returns. The list becomes a model prompt, so the
/// bound belongs to the read, exactly like the file tool's.
pub const SCREEN_TOOL_MAX_WINDOWS: usize = 64;

/// Most bytes one captured image may occupy before it is refused. A capture is
/// written under the root and never enters the prompt, so this bound exists to
/// stop a runaway virtual desktop, not to bound a prompt.
pub const SCREEN_TOOL_MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

/// Largest integer downsample factor one capture may use. Store-mode PNG is
/// uncompressed, so a 4K desktop is ~25 MiB and would otherwise be refused;
/// scaling the pixels is honest as long as the factor is reported back.
pub const SCREEN_TOOL_MAX_CAPTURE_SCALE: u32 = 8;

/// How many capture PNGs one task root keeps. Captures are tool-produced
/// artifacts, not model output; the oldest ones are pruned after each write.
pub const SCREEN_TOOL_MAX_CAPTURE_FILES: usize = 12;

/// Largest rectangle any single capture may read, in pixels. A region request
/// larger than this is refused before a device context is created.
pub const SCREEN_TOOL_MAX_REGION_PIXELS: i32 = 8192;

/// Most characters one `act:type` may deliver in a single call.
pub const SCREEN_TOOL_MAX_TYPE_CHARS: usize = 4096;

/// How far a requested window rectangle may end up from the one the desktop
/// reports before the move counts as not performed. Window managers apply
/// minimum sizes, invisible borders and snapped layouts; a difference beyond a
/// few pixels means the system kept its own geometry, and reporting that as a
/// success would be the exact lie this harness refuses.
pub const SCREEN_TOOL_MOVE_TOLERANCE_PX: i32 = 4;

/// How far the pointer may end up from the requested position before an input
/// action counts as not performed. The same rule as the move readback: the
/// desktop's own coordinates are the evidence, not `SetCursorPos` returning.
pub const SCREEN_TOOL_POINTER_TOLERANCE_PX: i32 = 4;

/// Directory (relative to the task root) every capture is published into. The
/// model never names a capture path; the tool composes it, so a capture cannot
/// become an arbitrary write.
pub const SCREEN_TOOL_CAPTURE_DIR: &str = ".harness/capture";

/// Most child controls one control scan returns. Hitting the bound is
/// **reported** (`truncated=true`), never silently clipped: a model that sees
/// 128 controls must know there were more.
pub const SCREEN_TOOL_MAX_CONTROLS: usize = 128;

/// Most UTF-16 code units retained from one control's text. Long edit buffers
/// are truncated with an ellipsis marker in the text, not in the frame.
pub const SCREEN_TOOL_MAX_CONTROL_TEXT: usize = 256;

/// Most points one `act:drag` path may carry. A longer path is refused, never
/// truncated: silently dropping the end of a drag changes where it lands.
pub const SCREEN_TOOL_MAX_DRAG_PATH_POINTS: usize = 16;

/// Stepped intermediate moves **per segment** of a path. Drag-and-drop targets
/// need intermediate motion; this keeps even a 16-point path bounded.
pub const SCREEN_TOOL_MAX_DRAG_STEPS: i64 = 12;

/// Absolute bound on one scroll axis: twelve wheel notches of 120. Matches the
/// clamp the native backends already applied to the single vertical delta.
pub const SCREEN_TOOL_MAX_SCROLL_DELTA: i32 = 1440;

/// The four modifiers a CU host may hold across calls (PRD 01.10). A held
/// modifier is keyboard state, never a button actuation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Win,
}

impl Modifier {
    pub const ALL: [Modifier; 4] = [Self::Ctrl, Self::Shift, Self::Alt, Self::Win];

    /// Wire aliases are accepted here, but evidence always uses `tag()` so the
    /// independent reader sees exactly one spelling on every OS.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Self::Ctrl),
            "shift" => Some(Self::Shift),
            "alt" | "option" => Some(Self::Alt),
            "win" | "cmd" | "command" | "super" | "meta" => Some(Self::Win),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Ctrl => "ctrl",
            Self::Shift => "shift",
            Self::Alt => "alt",
            Self::Win => "win",
        }
    }
}

/// Most modifiers one host may hold at once. A fifth is a named refusal,
/// never a silently dropped key.
pub const SCREEN_TOOL_MAX_HELD_MODIFIERS: usize = 4;

/// The disclosed whitelist, in canonical spelling.
pub const SCREEN_TOOL_HELD_MODIFIERS: [&str; 4] = ["ctrl", "shift", "alt", "win/cmd"];

/// Timeout for one cross-process `WM_GETTEXT`. A hung foreign window must not
/// be able to stall a harness invocation that only wanted to *observe* it.
pub const SCREEN_TOOL_CONTROL_TIMEOUT_MS: u32 = 200;

/// One display in the virtual desktop, in virtual-screen coordinates. Secondary
/// monitors can sit at negative x/y; reporting them is what makes a click on a
/// secondary monitor expressible at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayInfo {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

impl DisplayInfo {
    pub fn to_line(&self, index: usize) -> String {
        format!(
            "display={index} rect={},{} {}x{}{}",
            self.x,
            self.y,
            self.width,
            self.height,
            if self.primary { " primary" } else { "" }
        )
    }
}

/// One child window, as a typed semantic observation. `role` is a heuristic
/// derived from the Win32 **class name** -- it is not a UIA role, and a window
/// that draws its own widgets exposes no children at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlInfo {
    pub handle: u64,
    pub parent: u64,
    /// Stable, unique, human-readable name a caller addresses the control by.
    pub label: String,
    pub class: String,
    pub role: &'static str,
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
    pub enabled: bool,
}

impl ControlInfo {
    pub fn to_line(&self, index: usize) -> String {
        format!(
            "control={index} label={} role={} class={} text={:?} rect={},{} {}x{}{}{} hwnd={}",
            self.label,
            self.role,
            self.class,
            self.text,
            self.x,
            self.y,
            self.width,
            self.height,
            if self.visible { "" } else { " hidden" },
            if self.enabled { "" } else { " disabled" },
            self.handle
        )
    }

    /// The screen point `act:control` aims at. A zero-sized control has no
    /// meaningful centre and is refused before any pointer moves.
    pub fn center(&self) -> Option<(i32, i32)> {
        if self.width <= 0 || self.height <= 0 {
            return None;
        }
        Some((self.x + self.width / 2, self.y + self.height / 2))
    }
}

/// Which window a control scan reads. `Foreground` follows the desktop's own
/// focus; a title is resolved by the same unique-name rule every act uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlTarget {
    Foreground,
    Window(String),
}

/// Which window a structured accessibility scan reads. A raw handle is the
/// only unambiguous identity when two windows share a title.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum A11yTarget {
    Foreground,
    Handle(u64),
}

/// Which window an `act:focus` is bound to. A raw handle is the only
/// unambiguous identity when two windows share a title; a title remains the
/// 0.0.11-compatible spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FocusTarget {
    Title(String),
    Handle(u64),
}

/// Bounds for one accessibility scan. Both are published with the frame so a
/// reader can tell a bounded inventory from a complete one.
pub const SCREEN_TOOL_MAX_A11Y_CONTROLS: usize = 400;
pub const SCREEN_TOOL_MAX_A11Y_DEPTH: usize = 12;

/// One node of a read-only MSAA/IAccessible tree. Coordinates are virtual
/// desktop pixels, the same basis as `windows[]` and the PNG capture arm.
/// 0.0.14 adds the raw `STATE_SYSTEM_*` bit mask and `accValue` so a node
/// carries the normalised `role / name / state / value / bounds / depth`
/// schema a cross-platform agent needs; the raw integer is always kept so an
/// independent reader can recompute `state_names` from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct A11yControl {
    pub name: String,
    pub value: String,
    pub role: i32,
    pub role_name: &'static str,
    pub state: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub depth: u32,
    pub child_id: i32,
}

/// MSAA `ROLE_SYSTEM_*` values -> the same coarse vocabulary as Win32 classes.
/// Unknown roles stay `unknown` instead of being guessed.
pub fn a11y_role_name(role: i32) -> &'static str {
    match role {
        2 => "menubar",
        9 => "window",
        10 => "client",
        12 => "menuitem",
        15 => "document",
        18 => "dialog",
        20 => "group",
        21 => "separator",
        22 => "toolbar",
        33 => "list",
        34 => "listitem",
        37 => "tab",
        40 => "graphic",
        41 => "label",
        42 => "textbox",
        43 => "button",
        44 => "checkbox",
        46 => "combobox",
        47 => "slider",
        50 => "tabitem",
        _ => "unknown",
    }
}

/// MSAA `STATE_SYSTEM_*` bits -> a fixed, bounded vocabulary. The table is
/// pure so the independent journey reader can recompute it from the published
/// integer on every cell, including cells that never qualify a desktop.
/// Unknown bits stay in `state` but are never named by guess.
pub fn a11y_state_names(state: i32) -> Vec<&'static str> {
    const BITS: [(i32, &str); 16] = [
        (0x2, "selected"),
        (0x4, "focused"),
        (0x8, "pressed"),
        (0x10, "checked"),
        (0x20, "mixed"),
        (0x40, "readonly"),
        (0x100, "default"),
        (0x200, "expanded"),
        (0x400, "collapsed"),
        (0x800, "busy"),
        (0x8000, "invisible"),
        (0x10000, "offscreen"),
        (0x100000, "focusable"),
        (0x200000, "selectable"),
        (0x400000, "linked"),
        (0x4000_0000, "haspopup"),
    ];
    BITS
        .iter()
        .filter(|(bit, _)| state & bit != 0)
        .map(|(_, name)| *name)
        .collect()
}

/// Win32 class name -> a coarse role. Deliberately a fixed table: no UIA, no
/// provider registry, no guessing about third-party widget toolkits. Pure so
/// every cell can test it, including cells that never qualify a desktop.
pub fn control_role(class: &str) -> &'static str {
    match class {
        "Button" => "button",
        "Edit" => "textbox",
        "Static" => "label",
        "ComboBox" | "ComboBoxEx32" => "combobox",
        "ListBox" => "list",
        "SysListView32" => "list",
        "SysTreeView32" => "tree",
        "SysTabControl32" => "tab",
        "msctls_progress32" => "progress",
        "msctls_trackbar32" => "slider",
        "ScrollBar" => "scrollbar",
        "ToolbarWindow32" => "toolbar",
        "SysHeader32" => "header",
        _ => "unknown",
    }
}

/// Build stable labels. A control with no text is named by its role and index;
/// duplicate names gain `#index`, so no two controls in one frame share a
/// label and an ambiguous `act:control` cannot be caused by the renderer.
pub fn label_controls(controls: &mut [ControlInfo]) {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (index, control) in controls.iter_mut().enumerate() {
        let base = if control.text.trim().is_empty() {
            format!("{}#{}", control.role, index)
        } else {
            format!("{}:{}", control.role, control.text.trim())
        };
        let count = seen.entry(base.clone()).or_insert(0);
        *count += 1;
        control.label = if *count == 1 {
            base
        } else {
            format!("{base}#{}", index)
        };
    }
}

/// Resolve a control **by name** from one generation's frame. Exact label wins;
/// otherwise a case-insensitive substring must match exactly one control. Any
/// ambiguity is refused with the candidates named -- a guessed target would be
/// a click on the wrong button.
pub fn resolve_control<'a>(
    controls: &'a [ControlInfo],
    needle: &str,
) -> Result<&'a ControlInfo, String> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Err("screen: refused: `control` must name a control from the last observe".to_owned());
    }
    if let Some(exact) = controls.iter().find(|control| control.label == needle) {
        return Ok(exact);
    }
    let lower = needle.to_lowercase();
    let matches: Vec<&ControlInfo> = controls
        .iter()
        .filter(|control| {
            control.label.to_lowercase().contains(&lower)
                || control.text.to_lowercase().contains(&lower)
        })
        .collect();
    match matches.len() {
        0 => Err(format!(
            "screen: refused: no control matches {needle:?} in the current frame; call \
             op=observe with `controls` first"
        )),
        1 => Ok(matches[0]),
        _ => Err(format!(
            "screen: refused: {needle:?} matches {} controls ({}); name one exactly, the \
             operation was not performed",
            matches.len(),
            matches
                .iter()
                .take(6)
                .map(|control| control.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// `STATE_SYSTEM_*` bits that make a node inoperable. `UNAVAILABLE` (0x1) is
/// deliberately absent from `a11y_state_names`: it stays in the raw published
/// mask, but here it is a veto rather than a guessed name.
pub const A11Y_STATE_UNAVAILABLE: i32 = 0x1;
pub const A11Y_STATE_INVISIBLE: i32 = 0x8000;
pub const A11Y_STATE_OFFSCREEN: i32 = 0x10000;

/// Pure: a node with any of these bits is never a target for an act. The
/// independent reader can recompute the same predicate from `state`.
pub fn a11y_node_operable(state: i32) -> bool {
    state & (A11Y_STATE_UNAVAILABLE | A11Y_STATE_INVISIBLE | A11Y_STATE_OFFSCREEN) == 0
}

/// The coarse role vocabulary, as a fixed table so an unknown `role` filter
/// is refused instead of silently ignored. Mirrors `a11y_role_name` above and
/// the independent reader's role whitelist; `unknown` stays addressable.
pub const A11Y_ROLE_NAMES: &[&str] = &[
    "menubar", "window", "client", "menuitem", "document", "dialog", "group",
    "separator", "toolbar", "list", "listitem", "tab", "graphic", "label",
    "textbox", "button", "checkbox", "combobox", "slider", "tabitem", "unknown",
];

/// 0.0.15: resolve one semantic MSAA node from a normalised a11y tree by
/// identity (name, optionally narrowed by `role`), never by a coordinate.
/// Pure and back-end independent, so all four configurations type-check it and
/// every refusal is unit-testable without touching a desktop.
pub fn resolve_a11y_control<'a>(
    rows: &'a [A11yControl],
    needle: &str,
    role: Option<&str>,
) -> Result<&'a A11yControl, String> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Err(
            "screen: refused: `control` must name an a11y node from the last observe".to_owned(),
        );
    }
    let role_filter = match role {
        None => None,
        Some(raw) => {
            let wanted = raw.trim().to_lowercase();
            if !A11Y_ROLE_NAMES.contains(&wanted.as_str()) {
                return Err(format!(
                    "screen: refused: unknown a11y role {wanted:?}; expected one of {}",
                    A11Y_ROLE_NAMES.join(", ")
                ));
            }
            Some(wanted)
        }
    };
    let pool: Vec<&A11yControl> = rows
        .iter()
        .filter(|control| {
            role_filter.as_deref().map_or(true, |wanted| control.role_name == wanted)
        })
        .collect();
    let exact: Vec<&A11yControl> =
        pool.iter().copied().filter(|control| control.name == needle).collect();
    let matched: Vec<&A11yControl> = if !exact.is_empty() {
        exact
    } else {
        let lower = needle.to_lowercase();
        pool.iter()
            .copied()
            .filter(|control| {
                control.name.to_lowercase().contains(&lower)
                    || control.value.to_lowercase().contains(&lower)
            })
            .collect()
    };
    if matched.is_empty() {
        return Err(format!(
            "screen: refused: no a11y node matches {needle:?} in the current frame; call \
             op=observe with `a11y` first",
        ));
    }
    let operable: Vec<&A11yControl> = matched
        .iter()
        .copied()
        .filter(|control| a11y_node_operable(control.state))
        .collect();
    if operable.is_empty() {
        return Err(format!(
            "screen: refused: a11y node {:?} is unavailable, invisible or offscreen \
             (state={}); no operation was performed",
            matched[0].name, matched[0].state,
        ));
    }
    if operable.len() > 1 {
        return Err(format!(
            "screen: refused: {needle:?} matches {} operable a11y nodes ({}); name one exactly \
             or narrow it with `role`, the operation was not performed",
            operable.len(),
            operable
                .iter()
                .take(6)
                .map(|control| format!("{}:{}", control.role_name, control.name))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(operable[0])
}

/// One observation, as typed state. The CU host binds a generation to this
/// value; nothing here is a rendered prompt string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenSnapshot {
    pub platform: String,
    pub qualified: bool,
    /// Virtual desktop origin and size.
    pub desktop: (i32, i32, i32, i32),
    pub displays: Vec<DisplayInfo>,
    /// One read-only scale row per display, in `displays` order (0.0.10). An
    /// empty set means the probe contract refused the rows; `observe` still
    /// publishes each display as `unknown` rather than guessing `1`.
    pub display_scales: Vec<DisplayScaleProbe>,
    pub windows: Vec<WindowInfo>,
    /// Empty when this observe did not request a control scan.
    pub controls: Vec<ControlInfo>,
    /// Which window was scanned, when one was. `None` means no scan happened.
    pub controls_source: Option<String>,
    /// True when [`SCREEN_TOOL_MAX_CONTROLS`] stopped the enumeration.
    pub controls_truncated: bool,
    /// Structured accessibility tree, empty when the observe did not request
    /// one. This is an additive arm: the Win32 `controls[]` above is unchanged.
    pub a11y_controls: Vec<A11yControl>,
    /// Which window the scan resolved to, or None when no scan happened.
    pub a11y_source: Option<String>,
    /// A named reason when the scan could not be produced. An empty tree is
    /// never silently presented as a successful read.
    pub a11y_reason: Option<String>,
    /// True when [`SCREEN_TOOL_MAX_A11Y_CONTROLS`] stopped the walk.
    pub a11y_truncated: bool,
    pub capture: Option<CaptureOutcome>,
}

impl ScreenSnapshot {
    /// The deterministic, model-facing text. Two calls against an unchanged
    /// desktop read the same, and the window list is bounded here rather than
    /// in the snapshot so typed consumers can see the full inventory.
    pub fn to_text(&self) -> String {
        let total = self.windows.len();
        let listed = total.min(SCREEN_TOOL_MAX_WINDOWS);
        let mut text = format!("platform={} windows={total}", self.platform);
        if total > listed {
            text.push_str(&format!(" (listed {listed}, bound {SCREEN_TOOL_MAX_WINDOWS})"));
        }
        text.push_str(&format!(
            "\ndesktop={},{} {}x{} displays={}",
            self.desktop.0, self.desktop.1, self.desktop.2, self.desktop.3,
            self.displays.len()
        ));
        for (index, display) in self.displays.iter().enumerate() {
            text.push('\n');
            text.push_str(&display.to_line(index));
            // 0.0.10: the coordinate basis travels with the display, not in a
            // side channel a caller would have to correlate by hand.
            text.push_str(&format!(
                " scale={}",
                self.display_scales
                    .get(index)
                    .map(|row| row.scale.tag())
                    .unwrap_or("unknown")
            ));
        }
        for window in self.windows.iter().take(listed) {
            text.push('\n');
            text.push_str(&window.to_line());
        }
        if let Some(source) = &self.controls_source {
            text.push_str(&format!(
                "\ncontrols: source={} count={}{}",
                source,
                self.controls.len(),
                if self.controls_truncated {
                    format!(" truncated=true bound={SCREEN_TOOL_MAX_CONTROLS}")
                } else {
                    String::new()
                }
            ));
            if self.controls.is_empty() {
                text.push_str(
                    "\ncontrols: this window exposes no child windows; roles come from Win32 \
                     class names, not UIA, and self-drawn UIs report none",
                );
            }
            for (index, control) in self.controls.iter().enumerate() {
                text.push('\n');
                text.push_str(&control.to_line(index));
            }
        }
        if let Some(outcome) = &self.capture {
            text.push_str(&format!(
                "\ncapture: {} bytes={} image={}x{} scale={} source={}",
                outcome.path,
                outcome.bytes,
                outcome.width,
                outcome.height,
                outcome.scale,
                outcome.source
            ));
        }
        text
    }
}

/// One window as `observe` reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowInfo {
    pub handle: u64,
    pub title: String,
    pub class_name: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub minimized: bool,
    pub maximized: bool,
    pub focused: bool,
}

impl WindowInfo {
    /// One line of the observe result. Fixed field order and no padding words:
    /// this text is read by a model, not by a human.
    pub fn to_line(&self) -> String {
        format!(
            "hwnd={} pid={} rect={},{} {}x{}{}{}{} class={} title={}",
            self.handle,
            self.pid,
            self.x,
            self.y,
            self.width,
            self.height,
            if self.minimized { " minimized" } else { "" },
            if self.maximized { " maximized" } else { "" },
            if self.focused { " focused" } else { "" },
            self.class_name,
            self.title
        )
    }
}

/// What the native backend actually posted, counted at each dispatch site.
/// It is produced *after* the down/up calls, so the host can compare it with
/// the request instead of echoing the request back as evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ButtonDispatch {
    /// Cross-platform normalisation; an independent reader never depends on
    /// native constants (X11 button 2, `MOUSEEVENTF_MIDDLEDOWN`, CG centre).
    pub button: &'static str,
    pub downs: u32,
    pub ups: u32,
    pub double: bool,
    /// Destination points actually traversed by a drag. A legacy single-leg
    /// drag reports 1; a click posts no path and reports 0.
    pub path_points: u32,
}

/// The two wheel axes a native backend actually posted, normalised so that
/// positive `delta_y` is up and positive `delta_x` is right on every OS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollDispatch {
    pub delta_x: i32,
    pub delta_y: i32,
}

/// The actuation receipt published by `CuHost` in `evidence["actuation"]`.
/// `target` is the foreground window title at dispatch time, or the literal
/// `desktop session` when the session exposes no foreground window: a non-empty
/// statement of where the event was posted, never a guessed handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Actuation {
    pub button: &'static str,
    pub downs: u32,
    pub ups: u32,
    pub double: bool,
    pub target: String,
    pub path_points: u32,
}

impl Actuation {
    pub fn from_dispatch(dispatch: ButtonDispatch, target: String) -> Self {
        Self {
            button: dispatch.button,
            downs: dispatch.downs,
            ups: dispatch.ups,
            double: dispatch.double,
            target,
            path_points: dispatch.path_points,
        }
    }
}

/// Reject a path point that the desktop cannot contain. Both the point and
/// the virtual-desktop rectangle are named; native backends must never clip a
/// coordinate into a different place and then report a success.
pub fn check_drag_path(
    points: &[(i32, i32)],
    desktop: (i32, i32, i32, i32),
) -> Result<(), String> {
    let (vx, vy, vw, vh) = desktop;
    for &(px, py) in points {
        if px < vx || px >= vx + vw || py < vy || py >= vy + vh {
            return Err(format!(
                "screen: refused: drag path point {px},{py} is outside the virtual desktop \
                 {vx},{vy} {vw}x{vh}; no button event was posted"
            ));
        }
    }
    Ok(())
}

/// Admission for the two wheel axes, applied before any native wheel event.
/// Using comparisons instead of `abs()` keeps `i32::MIN` from panicking.
pub fn check_scroll_axes(delta_x: i32, delta_y: i32) -> Result<(), String> {
    let bound = SCREEN_TOOL_MAX_SCROLL_DELTA;
    if delta_x < -bound
        || delta_x > bound
        || delta_y < -bound
        || delta_y > bound
    {
        return Err(format!(
            "screen: refused: each scroll axis must be within +-{bound}, got \
             delta_x={delta_x} delta_y={delta_y}; no wheel event was posted"
        ));
    }
    if delta_x == 0 && delta_y == 0 {
        return Err(
            "screen: refused: both scroll axes are zero; a no-op is never reported as \
             a performed wheel event"
                .to_owned(),
        );
    }
    Ok(())
}

/// The session an input event is posted to. A missing foreground title is
/// reported as the session, not silently replaced with a plausible window.
fn dispatch_target() -> String {
    native::foreground_title()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "desktop session".to_owned())
}

/// What one `act` did, as evidence rather than as an intention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActOutcome {
    /// The operation actually performed, echoed back.
    pub action: String,
    /// The window the operation was aimed at, when it named one.
    pub window: Option<String>,
    /// What the desktop says afterwards: the target's rect for window moves,
    /// the foreground window's title for input.
    pub readback: String,
    /// Present exactly when the operation dispatched a button event: click,
    /// drag, or a named control with `press=true`. Hover, scroll, typing,
    /// focus and move post no button event and carry no receipt. `target_hwnd`
    /// is separate focus evidence, never a button receipt.
    pub actuation: Option<Actuation>,
    /// Present exactly when the operation posted a wheel event. A scroll has
    /// no button receipt; its two axes are its own typed evidence.
    pub scroll: Option<ScrollDispatch>,
    /// The desktop's focused window handle at dispatch time, read from the
    /// live window list and never echoed from the request. `None` with a
    /// named reason when the session exposes no foreground window.
    pub target_hwnd: Option<u64>,
    pub target_hwnd_reason: Option<String>,
}

/// The `screen` tool: observe the desktop, or perform one operation on it.
#[derive(Clone, Debug)]
pub struct ScreenTool {
    root: PathBuf,
    /// `--allow-screen`. Off by default: a tool that can move the mouse is not
    /// something a caller gets without asking, and an off tool still answers
    /// every call with a refusal that names the flag, so a model cannot tell
    /// "not granted yet" from "tool absent" by probing.
    enabled: bool,
}

impl ScreenTool {
    pub fn new(root: &Path, enabled: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            enabled,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Refuses before any native effect when the tool was not granted. Named,
    /// never silent.
    fn admit(&self) -> Result<(), String> {
        if !self.enabled {
            return Err(
                "screen: refused: computer use is not granted; pass --allow-screen to let this \
                 invocation observe or operate the desktop"
                    .to_owned(),
            );
        }
        if !platform_qualified() {
            return Err(format!(
                "screen: refused: the {} backend implements no computer-use capability; disclose \
                 reports its capability matrix, and the tool refuses instead of reporting an \
                 empty success",
                native::PLATFORM
            ));
        }
        Ok(())
    }

    /// Reads the desktop into a structured snapshot. `title` filters by
    /// substring when given; `capture` writes one PNG under
    /// [`SCREEN_TOOL_CAPTURE_DIR`]. `scale` 0 means choose the smallest factor
    /// that keeps a store-mode PNG inside the byte bound. `controls` requests a
    /// **separate** typed scan of one window; without it no child window is
    /// enumerated, so a plain observe costs the same as before.
    ///
    /// The snapshot is the single source of truth: [`ScreenTool::observe`]
    /// renders it for the model, and the CU host consumes it as typed state.
    pub fn snapshot(
        &self,
        title: Option<&str>,
        capture: Option<CaptureTarget>,
        scale: u32,
        controls: Option<ControlTarget>,
        a11y: Option<A11yTarget>,
    ) -> Result<ScreenSnapshot, String> {
        self.admit()?;
        if scale > SCREEN_TOOL_MAX_CAPTURE_SCALE {
            return Err(format!(
                "screen: refused: capture scale {scale} is over the bound of \
                 {SCREEN_TOOL_MAX_CAPTURE_SCALE}; use 0 for automatic"
            ));
        }
        let virtual_screen = native::virtual_screen()?;
        let displays = native::displays()?;
        let mut windows = native::windows()?;
        if let Some(needle) = title {
            let needle = needle.to_lowercase();
            windows.retain(|window| window.title.to_lowercase().contains(&needle));
            if windows.is_empty() {
                return Err(format!(
                    "screen: refused: no window title contains {needle:?}; call op=observe with \
                     no title to list what is there"
                ));
            }
        }
        // Deterministic order for every consumer of the snapshot: focused
        // first, then top-left to bottom-right. Sorting here (not only in the
        // text renderer) keeps the typed frame and the prompt in agreement.
        windows.sort_by_key(|window| {
            (
                !window.focused,
                window.minimized,
                window.y,
                window.x,
                window.handle,
            )
        });
        // A control scan is a separate, explicitly requested read. It resolves
        // its window by the same rules an action would, so a stale handle is
        // never carried across calls.
        let (controls, controls_source, controls_truncated) = match controls {
            None => (Vec::new(), None, false),
            Some(ControlTarget::Foreground) => {
                let focused = windows.iter().find(|window| window.focused).ok_or_else(|| {
                    "screen: refused: the desktop has no foreground window to scan for controls; \
                     name a window with `controls` instead"
                        .to_owned()
                })?;
                let (mut list, truncated) = native::controls(focused.handle)?;
                label_controls(&mut list);
                (list, Some(format!("foreground:{:?}", focused.title)), truncated)
            }
            Some(ControlTarget::Window(title)) => {
                let window = native::find(&title)?;
                let (mut list, truncated) = native::controls(window.handle)?;
                label_controls(&mut list);
                (list, Some(format!("window:{:?}", window.title)), truncated)
            }
        };
        // A structured scan is a separate, explicitly requested read. Its
        // handle is re-validated against the live window list on every call;
        // a stale handle is a named reason, never a silent best effort.
        let (mut a11y_controls, mut a11y_source, mut a11y_reason, mut a11y_truncated) =
            (Vec::new(), None, None, false);
        if let Some(target) = a11y {
            let live = native::windows().unwrap_or_default();
            let resolved = match target {
                A11yTarget::Foreground => live
                    .iter()
                    .find(|window| window.focused)
                    .map(|window| (window.handle, window.title.clone())),
                A11yTarget::Handle(handle) => live
                    .iter()
                    .find(|window| window.handle == handle)
                    .map(|window| (window.handle, window.title.clone())),
            };
            match resolved {
                None => {
                    a11y_reason = Some(
                        "screen: unknown: no live window matched the a11y target; observe again"
                            .to_owned(),
                    );
                }
                Some((handle, title)) => {
                    a11y_source = Some(format!("handle:{handle} window:{title:?}"));
                    match native::a11y_controls(handle) {
                        Ok((rows, truncated)) => {
                            a11y_controls = rows;
                            a11y_truncated = truncated;
                        }
                        Err(reason) => a11y_reason = Some(reason),
                    }
                }
            }
        }
        let capture = match capture {
            Some(target) => Some(self.capture(target, scale)?),
            None => None,
        };
        // A scale probe is pure session state; it is captured with the frame
        // so a later serialization cannot pair rows with a changed display set.
        let display_scales = display_scale_probes().unwrap_or_default();
        Ok(ScreenSnapshot {
            platform: native::PLATFORM.to_owned(),
            qualified: platform_qualified(),
            desktop: virtual_screen,
            displays,
            display_scales,
            windows,
            controls,
            controls_source,
            controls_truncated,
            a11y_controls,
            a11y_source,
            a11y_reason,
            a11y_truncated,
            capture,
        })
    }

    /// Model-facing rendering of [`ScreenTool::snapshot`]. Kept as a thin
    /// wrapper so an existing caller cannot drift from the typed state.
    pub fn observe(
        &self,
        title: Option<&str>,
        capture: Option<CaptureTarget>,
        scale: u32,
        controls: Option<ControlTarget>,
    ) -> Result<String, String> {
        Ok(self.snapshot(title, capture, scale, controls, None)?.to_text())
    }

    /// Performs exactly one operation. Nothing here retries: an operation is
    /// attempted once and its outcome is read back from the desktop.
    pub fn act(&self, act: Act<'_>) -> Result<ActOutcome, String> {
        self.admit()?;
        match act {
            Act::Focus { target } => {
                let window = match target {
                    FocusTarget::Title(title) => native::find(&title)?,
                    FocusTarget::Handle(handle) => {
                        native::window_by_handle(handle).ok_or_else(|| {
                            format!(
                                "screen: stale: hwnd {handle} is not in the live window list; \
                                 observe again before operating on it"
                            )
                        })?
                    }
                };
                // The resolved row comes from a fresh live window list, never
                // from a cached frame. Re-check the title so a reused handle
                // cannot silently become a different window.
                native::handle_is_live(window.handle, &window.title)?;
                native::focus(window.handle)?;
                let focused = native::windows()?
                    .into_iter()
                    .find(|candidate| candidate.focused);
                let (readback, target_hwnd, target_hwnd_reason) = match focused {
                    Some(candidate) => (
                        format!(
                            "foreground={:?} target_hwnd={}",
                            candidate.title, candidate.handle
                        ),
                        Some(candidate.handle),
                        None,
                    ),
                    None => (
                        "foreground=<none> target_hwnd=<none>".to_owned(),
                        None,
                        Some(
                            "screen: unknown: no foreground window exposed a handle; observe \
                             again"
                                .to_owned(),
                        ),
                    ),
                };
                Ok(ActOutcome {
                    target_hwnd,
                    target_hwnd_reason,
                    scroll: None,
                    actuation: None,
                    action: "focus".to_owned(),
                    window: Some(window.title),
                    readback,
                })
            }
            Act::Move {
                title,
                x,
                y,
                width,
                height,
            } => {
                let window = native::find(&title)?;
                native::handle_is_live(window.handle, &window.title)?;
                native::move_window(window.handle, x, y, width, height)?;
                let after = native::find(&title)?;
                // The desktop's own rectangle is the evidence, not the request
                // having returned. A maximized or size-constrained window keeps
                // its geometry, and saying otherwise would be a fake success.
                let off = (after.x - x).abs().max((after.y - y).abs())
                    > SCREEN_TOOL_MOVE_TOLERANCE_PX
                    || (after.width - width).abs() > SCREEN_TOOL_MOVE_TOLERANCE_PX
                    || (after.height - height).abs() > SCREEN_TOOL_MOVE_TOLERANCE_PX;
                if off {
                    return Err(format!(
                        "screen: failed: asked for rect={x},{y} {width}x{height} on {:?}, the \
                         desktop reports rect={},{} {}x{}; the window manager kept its own \
                         geometry (a maximized or size-constrained window does). Nothing else \
                         was attempted",
                        after.title, after.x, after.y, after.width, after.height
                    ));
                }
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: None,
                    actuation: None,
                    action: "move".to_owned(),
                    window: Some(after.title),
                    readback: format!(
                        "rect={},{} {}x{}",
                        after.x, after.y, after.width, after.height
                    ),
                })
            }
            Act::Click { x, y, button, double } => {
                let (px, py, dispatch) = native::click(x, y, button, double)?;
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: None,
                    actuation: Some(Actuation::from_dispatch(dispatch, dispatch_target())),
                    action: "click".to_owned(),
                    window: None,
                    readback: format!("pointer at {px},{py}"),
                })
            }
            Act::Hover { x, y } => {
                let (px, py) = native::place_pointer(x, y)?;
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: None,
                    actuation: None,
                    action: "hover".to_owned(),
                    window: None,
                    readback: format!("pointer at {px},{py}"),
                })
            }
            Act::Drag {
                x,
                y,
                to_x,
                to_y,
                button,
                path,
            } => {
                let points = match path {
                    Some(points) => {
                        if points.len() < 2
                            || points.len() > SCREEN_TOOL_MAX_DRAG_PATH_POINTS
                        {
                            return Err(format!(
                                "screen: refused: a drag path needs 2..={} points, got {}; \
                                 no button event was posted",
                                SCREEN_TOOL_MAX_DRAG_PATH_POINTS,
                                points.len()
                            ));
                        }
                        // The whole path is admitted against the *desktop's*
                        // geometry, not just its endpoints, and before any
                        // native pointer move or button press exists.
                        check_drag_path(&points, native::virtual_screen()?)?;
                        points
                    }
                    None => vec![(to_x, to_y)],
                };
                let (px, py, dispatch) = native::drag(x, y, &points, button)?;
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: None,
                    actuation: Some(Actuation::from_dispatch(dispatch, dispatch_target())),
                    action: "drag".to_owned(),
                    window: None,
                    readback: format!("pointer at {px},{py}"),
                })
            }
            Act::Type { text } => {
                let chars = text.chars().count();
                if chars > SCREEN_TOOL_MAX_TYPE_CHARS {
                    return Err(format!(
                        "screen: refused: {chars} characters is over the type bound of \
                         {SCREEN_TOOL_MAX_TYPE_CHARS}; split the text"
                    ));
                }
                let before = native::foreground_title();
                native::type_text(text)?;
                let evidence = foreground_evidence(before, native::foreground_title())?;
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: None,
                    actuation: None,
                    action: "type".to_owned(),
                    window: None,
                    readback: format!("typed {chars} characters {evidence}"),
                })
            }
            Act::Key { keys } => {
                let before = native::foreground_title();
                native::press_key(&keys)?;
                let evidence = foreground_evidence(before, native::foreground_title())?;
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: None,
                    actuation: None,
                    action: "key".to_owned(),
                    window: None,
                    readback: format!("pressed {keys} {evidence}"),
                })
            }
            Act::Scroll {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                check_scroll_axes(delta_x, delta_y)?;
                let (px, py, dispatch) = native::scroll(x, y, delta_x, delta_y)?;
                Ok(ActOutcome {
                    target_hwnd: None,
                    target_hwnd_reason: None,
                    scroll: Some(dispatch),
                    actuation: None,
                    action: "scroll".to_owned(),
                    window: None,
                    // `dx=.. dy=..` carries no comma, so the independent
                    // reader's `x,y` pointer assertion still reads the tail.
                    readback: format!("scrolled dx={delta_x} dy={delta_y} at {px},{py}"),
                })
            }
        }
    }

    /// Captures the desktop, one window, or one explicit region; publishes it
    /// under the root. The path is composed here, never supplied by the model.
    fn capture(&self, target: CaptureTarget, scale: u32) -> Result<CaptureOutcome, String> {
        let (source, image) = match target {
            CaptureTarget::Desktop => ("desktop".to_owned(), native::capture_desktop(scale)?),
            CaptureTarget::Window(title) => {
                let window = native::find(&title)?;
                native::handle_is_live(window.handle, &window.title)?;
                let image = native::capture_window(window.handle, scale)?;
                (format!("{:?}", window.title), image)
            }
            CaptureTarget::Region {
                x,
                y,
                width,
                height,
            } => {
                let (vx, vy, vw, vh) = native::virtual_screen()?;
                if width <= 0
                    || height <= 0
                    || width > SCREEN_TOOL_MAX_REGION_PIXELS
                    || height > SCREEN_TOOL_MAX_REGION_PIXELS
                {
                    return Err(format!(
                        "screen: refused: region {width}x{height} is outside 1..=\
                         {SCREEN_TOOL_MAX_REGION_PIXELS} on each side"
                    ));
                }
                // Refuse rather than clip: a clipped image would silently show
                // a different part of the desktop than the model asked for.
                let right = x as i64 + width as i64;
                let bottom = y as i64 + height as i64;
                if (x as i64) < vx as i64
                    || (y as i64) < vy as i64
                    || right > vx as i64 + vw as i64
                    || bottom > vy as i64 + vh as i64
                {
                    return Err(format!(
                        "screen: refused: region {x},{y} {width}x{height} is outside the \
                         virtual desktop {vx},{vy} {vw}x{vh}; observe without a capture to \
                         read the display geometry"
                    ));
                }
                (
                    format!("region {x},{y} {width}x{height}"),
                    native::capture_region(x, y, width, height, scale)?,
                )
            }
        };
        if image.bytes.len() > SCREEN_TOOL_MAX_CAPTURE_BYTES {
            return Err(format!(
                "screen: refused: the capture is {} bytes at scale {}, over the bound of {}; \
                 capture a region or ask for a larger scale",
                image.bytes.len(),
                image.scale,
                SCREEN_TOOL_MAX_CAPTURE_BYTES
            ));
        }
        let directory = self.root.join(SCREEN_TOOL_CAPTURE_DIR);
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("screen: capture directory failed: {error}"))?;
        // A monotonic tail keeps two captures in the same millisecond from
        // overwriting each other.
        let sequence = CAPTURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!(
            "capture-{}-{}-{sequence}.png",
            std::process::id(),
            native::now_millis()
        );
        let path = directory.join(&name);
        std::fs::write(&path, &image.bytes)
            .map_err(|error| format!("screen: capture write failed: {error}"))?;
        prune_captures(&directory);
        Ok(CaptureOutcome {
            path: format!("{SCREEN_TOOL_CAPTURE_DIR}/{name}"),
            bytes: image.bytes.len(),
            width: image.width,
            height: image.height,
            scale: image.scale,
            source,
        })
    }
}

static CAPTURE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// One published capture, fully described so the model knows the pixels are
/// downsampled when they are.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureOutcome {
    pub path: String,
    pub bytes: usize,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub source: String,
}

/// Smallest power-of-two downsample factor whose store-mode PNG fits the byte
/// bound. `requested > 0` is honoured verbatim (the byte check still applies).
/// Pure arithmetic, so it is testable on every cell.
pub fn capture_scale(width: u32, height: u32, requested: u32) -> u32 {
    if requested > 0 {
        return requested;
    }
    let budget = (SCREEN_TOOL_MAX_CAPTURE_BYTES as u64).saturating_sub(4096);
    let mut scale = 1u32;
    while scale < SCREEN_TOOL_MAX_CAPTURE_SCALE {
        let factor = scale as u64;
        let w = (width as u64 + factor - 1) / factor;
        let h = (height as u64 + factor - 1) / factor;
        // 3 bytes per pixel, one filter byte per row, zlib/deflate overhead.
        if w.saturating_mul(h)
            .saturating_mul(3)
            .saturating_add(h)
            .saturating_add(1024)
            <= budget
        {
            break;
        }
        scale *= 2;
    }
    scale
}

/// Box-average downsample in BGRA. Averaging (rather than nearest neighbour)
/// keeps thin UI text legible when a 4K desktop has to be reduced to fit the
/// byte bound. Pure arithmetic: testable on every cell, including the ones
/// where computer use is unqualified.
pub fn downsample(
    bgra: &[u8],
    width: u32,
    height: u32,
    scale: u32,
) -> (Vec<u8>, u32, u32) {
    if scale <= 1 {
        return (bgra.to_vec(), width, height);
    }
    let step = scale as usize;
    let out_width = (width as usize + step - 1) / step;
    let out_height = (height as usize + step - 1) / step;
    let mut out = vec![0u8; out_width * out_height * 4];
    for row in 0..out_height {
        for column in 0..out_width {
            let mut sum = [0u64; 3];
            let mut count = 0u64;
            for dy in 0..step {
                let source_row = row * step + dy;
                if source_row >= height as usize {
                    break;
                }
                for dx in 0..step {
                    let source_column = column * step + dx;
                    if source_column >= width as usize {
                        break;
                    }
                    let pixel = (source_row * width as usize + source_column) * 4;
                    for channel in 0..3 {
                        sum[channel] += bgra[pixel + channel] as u64;
                    }
                    count += 1;
                }
            }
            let target = (row * out_width + column) * 4;
            if count > 0 {
                for channel in 0..3 {
                    out[target + channel] = (sum[channel] / count) as u8;
                }
            }
            out[target + 3] = 255;
        }
    }
    (out, out_width as u32, out_height as u32)
}

/// Best-effort retention: keep the newest tool-produced PNGs. Only names this
/// tool composes are considered; a failure to prune never fails a capture.
fn prune_captures(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut captures: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("capture-") || !name.ends_with(".png") {
                return None;
            }
            let time = entry.metadata().and_then(|meta| meta.modified()).ok()?;
            Some((time, entry.path()))
        })
        .collect();
    if captures.len() <= SCREEN_TOOL_MAX_CAPTURE_FILES {
        return;
    }
    captures.sort_by(|(left_time, left_path), (right_time, right_path)| {
        right_time
            .cmp(left_time)
            .then_with(|| right_path.cmp(left_path))
    });
    for (_, path) in captures
        .into_iter()
        .skip(SCREEN_TOOL_MAX_CAPTURE_FILES)
    {
        let _ = std::fs::remove_file(path);
    }
}

/// Which surface one capture reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureTarget {
    Desktop,
    /// Substring match, resolved through the same admission as `act`.
    Window(String),
    /// Explicit rectangle in virtual-desktop coordinates; never silently
    /// clipped to the display.
    Region {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
}

/// One operation the model asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Act<'a> {
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
        /// Optional multi-point path. `None` keeps the 0.0.6 single-leg drag.
        path: Option<Vec<(i32, i32)>>,
    },
    Type { text: &'a str },
    Key { keys: String },
    Scroll { x: i32, y: i32, delta_x: i32, delta_y: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickButton {
    Left,
    Right,
    Middle,
}

impl ClickButton {
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

/// One encoded image on its way to disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Captured {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// 1 means the original pixels; >1 means a box downsample was applied and
    /// the reported dimensions are the scaled ones.
    pub scale: u32,
}

/// Evidence for a keystroke: which window actually owned the foreground before
/// and after the input. Losing the foreground entirely is a failure; a window
/// that changed as a *result* of the key (Enter opening a dialog) is reported,
/// not refused, because the input did reach the original target.
/// Hold or release exactly one whitelisted modifier. `CuHost` owns the held
/// set; the backend only posts the symmetric native event.
pub fn set_modifier(key: Modifier, down: bool) -> Result<(), String> {
    native::set_modifier(key, down)
}

/// Release every modifier, whether or not the host believes it is held.
/// Idempotent on every backend: an extra key-up cannot leave a stuck key.
pub fn release_all_modifiers() -> Result<(), String> {
    native::release_all_modifiers()
}

/// The foreground title as the native backend reads it right now.
pub fn foreground_title() -> Option<String> {
    native::foreground_title()
}

/// Shared by atomic `act:key` and the held-key state machine so the 0.0.6
/// foreground assertion keeps exactly one shape.
pub(crate) fn foreground_evidence(before: Option<String>, after: Option<String>) -> Result<String, String> {
    match (&before, &after) {
        (Some(title), None) => Err(format!(
            "screen: failed: the foreground window {title:?} disappeared during input; nothing \
             further was delivered"
        )),
        (Some(first), Some(second)) if first != second => Ok(format!(
            "foreground={second:?} (was {first:?}, changed during input)"
        )),
        (Some(title), Some(_)) => Ok(format!("foreground={title:?}")),
        (None, Some(title)) => Ok(format!("foreground={title:?} (no foreground before input)")),
        (None, None) => Ok("foreground=<none>".to_owned()),
    }
}

/// What this compiled cell actually implements. A capability is a declared
/// fact of the backend, never a guess: the CU host admits a verb only when the
/// matching flag is set, and every other verb is a named refusal. That is what
/// makes a partially implemented OS honest instead of "Windows only".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCapabilities {
    pub observe: bool,
    pub controls: bool,
    pub capture: bool,
    pub act_focus: bool,
    pub act_move: bool,
    pub act_click: bool,
    pub act_hover: bool,
    pub act_drag: bool,
    pub act_type: bool,
    pub act_key: bool,
    pub act_scroll: bool,
    pub act_control: bool,
}

impl PlatformCapabilities {
    pub const NONE: Self = Self {
        observe: false, controls: false, capture: false,
        act_focus: false, act_move: false, act_click: false, act_hover: false,
        act_drag: false, act_type: false, act_key: false, act_scroll: false,
        act_control: false,
    };
    pub const ALL: Self = Self {
        observe: true, controls: true, capture: true,
        act_focus: true, act_move: true, act_click: true, act_hover: true,
        act_drag: true, act_type: true, act_key: true, act_scroll: true,
        act_control: true,
    };

    /// Stable matrix for `disclose` evidence and tests.
    pub fn rows(&self) -> Vec<(&'static str, bool)> {
        vec![
            ("observe", self.observe),
            ("controls", self.controls),
            ("capture", self.capture),
            ("act:focus", self.act_focus),
            ("act:move", self.act_move),
            ("act:click", self.act_click),
            ("act:hover", self.act_hover),
            ("act:drag", self.act_drag),
            ("act:type", self.act_type),
            ("act:key", self.act_key),
            ("act:scroll", self.act_scroll),
            ("act:control", self.act_control),
        ]
    }

    /// Admission by the exact names in `cu::CAPABILITIES`.
    pub fn supports(&self, capability: &str) -> bool {
        match capability {
            "disclose" => true,
            "observe" => self.observe,
            "act:focus" => self.act_focus,
            "act:move" => self.act_move,
            "act:click" => self.act_click,
            "act:hover" => self.act_hover,
            "act:drag" => self.act_drag,
            "act:type" => self.act_type,
            "act:key" => self.act_key,
            "act:scroll" => self.act_scroll,
            "act:control" => self.act_control,
            _ => false,
        }
    }

    pub fn any_effect_implemented(&self) -> bool {
        self.rows().iter().any(|(_, implemented)| *implemented)
    }
}

/// Contract-level platform facts, readable by the CU host without going
/// through a native effect. Qualification is derived from the matrix, so an OS
/// with no implemented effect reports `false` instead of being called Windows.
pub fn platform_qualified() -> bool {
    native::CAPABILITIES.any_effect_implemented()
}

pub fn platform_capabilities() -> PlatformCapabilities {
    native::CAPABILITIES
}

pub fn platform_name() -> &'static str {
    native::PLATFORM
}

// ---------------------------------------------------------------------------
// grant and environment disclosure (0.0.9, prd/PRD_01_11_grant_disclosure.md)
// ---------------------------------------------------------------------------
//
// A probe is pure session state: it reads a precondition and reports a typed
// status. It never requests a permission, never posts an event and never
// elevates. `disclose` publishes the result; nothing here can make a verb
// available that the matrix refuses.

/// Most grant rows one disclosure may publish. More are refused by
/// [`normalize_grant_probes`], never silently clipped: a reader that saw seven
/// rows must not be told there were seven when there were nine.
pub const SCREEN_TOOL_MAX_GRANT_PROBES: usize = 8;

/// Characters of one reason, including the truncation marker. A longer reason
/// is cut and marked, never published whole: a reason is a short named fact,
/// not a log line.
pub const SCREEN_TOOL_MAX_GRANT_REASON: usize = 160;

/// Appended as the last character of a truncated reason. It is a character,
/// so it is inside [`SCREEN_TOOL_MAX_GRANT_REASON`] rather than after it.
pub const SCREEN_TOOL_GRANT_REASON_MARKER: char = '…';

/// The four statuses `disclose` may publish, and no others. `unknown` means
/// "could not be probed safely"; `not_applicable` means "this cell does not
/// implement the mechanism the probe names". Neither is a soft `denied`, and
/// `denied` is reported, never escalated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantStatus {
    Granted,
    Denied,
    Unknown,
    NotApplicable,
}

impl GrantStatus {
    pub const ALL: [Self; 4] = [Self::Granted, Self::Denied, Self::Unknown, Self::NotApplicable];

    pub fn tag(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not_applicable",
        }
    }

    /// The inverse of [`GrantStatus::tag`]. Used by the independent reader
    /// contract and by tests; an unrecognised string is `None`, never a guess.
    pub fn parse(tag: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|status| status.tag() == tag)
    }

    /// A status that cannot answer "does this work?" must say why. `granted`
    /// and `denied` are answers; the other two are the absence of one.
    pub fn needs_reason(self) -> bool {
        matches!(self, Self::Unknown | Self::NotApplicable)
    }
}

/// One named probe, as a backend reports it. `name` is a stable snake_case
/// identifier, spelled identically on every backend that can answer it, so a
/// reader can compare cells without a per-OS table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantProbe {
    pub name: &'static str,
    pub status: GrantStatus,
    /// Required for `unknown` / `not_applicable`; optional otherwise. It names
    /// a fact (a library, a symbol, a cell), never a secret.
    pub reason: Option<String>,
}

impl GrantProbe {
    pub fn granted(name: &'static str) -> Self {
        Self { name, status: GrantStatus::Granted, reason: None }
    }

    pub fn denied(name: &'static str) -> Self {
        Self { name, status: GrantStatus::Denied, reason: None }
    }

    pub fn unknown(name: &'static str, reason: impl Into<String>) -> Self {
        Self { name, status: GrantStatus::Unknown, reason: Some(reason.into()) }
    }

    pub fn not_applicable(name: &'static str, reason: impl Into<String>) -> Self {
        Self { name, status: GrantStatus::NotApplicable, reason: Some(reason.into()) }
    }

    /// A `granted` / `denied` row with an explanatory reason. Kept separate so
    /// a status that carries no reason cannot be confused with one that must.
    pub fn noted(
        name: &'static str,
        status: GrantStatus,
        reason: impl Into<String>,
    ) -> Self {
        Self { name, status, reason: Some(reason.into()) }
    }
}

/// The coarse session kind. Deliberately not an environment value: no display
/// number, user name, path or window title is ever published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionKind {
    Gui,
    X11,
    WaylandX11Compat,
    Headless,
    Unavailable,
}

impl SessionKind {
    pub const ALL: [Self; 5] = [
        Self::Gui,
        Self::X11,
        Self::WaylandX11Compat,
        Self::Headless,
        Self::Unavailable,
    ];

    pub fn tag(self) -> &'static str {
        match self {
            Self::Gui => "gui",
            Self::X11 => "x11",
            Self::WaylandX11Compat => "wayland-x11-compat",
            Self::Headless => "headless",
            Self::Unavailable => "unavailable",
        }
    }
}

/// What one read-only environment probe found. A cell that cannot read its own
/// geometry reports a zero rect and a zero display count instead of inventing
/// a desktop; `foreground_present` is a boolean, never a title.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentProbe {
    pub platform: &'static str,
    pub session: SessionKind,
    pub display_count: usize,
    pub virtual_desktop: (i32, i32, i32, i32),
    pub foreground_present: bool,
}

/// One constructor for all four backends, so the rect can never be published
/// with a negative size: an unreadable geometry is `0x0` at the origin.
pub fn environment_from(
    platform: &'static str,
    session: SessionKind,
    desktop: Option<(i32, i32, i32, i32)>,
    display_count: usize,
    foreground_present: bool,
) -> EnvironmentProbe {
    let (x, y, width, height) = desktop.unwrap_or((0, 0, 0, 0));
    EnvironmentProbe {
        platform,
        session,
        display_count,
        virtual_desktop: (x, y, width.max(0), height.max(0)),
        foreground_present,
    }
}

/// Cut a reason to the bound, marking the cut. The marker is the last
/// character of the result, so a truncated reason still fits the bound.
pub fn truncate_grant_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.chars().count() <= SCREEN_TOOL_MAX_GRANT_REASON {
        return trimmed.to_owned();
    }
    let keep = SCREEN_TOOL_MAX_GRANT_REASON - 1;
    let mut out: String = trimmed.chars().take(keep).collect();
    out.push(SCREEN_TOOL_GRANT_REASON_MARKER);
    out
}

/// The single admission rule for a probe set: bounded, whitelisted, sorted by
/// name, uniquely named, and every unanswerable row carries a reason. A set
/// that violates any rule is refused whole -- a disclosure is either a
/// complete, recomputable snapshot or it is not published at all.
pub fn normalize_grant_probes(probes: Vec<GrantProbe>) -> Result<Vec<GrantProbe>, String> {
    if probes.len() > SCREEN_TOOL_MAX_GRANT_PROBES {
        return Err(format!(
            "screen: refused: {} grant probes exceed the bound of {}; the disclosure was not \
             published",
            probes.len(),
            SCREEN_TOOL_MAX_GRANT_PROBES
        ));
    }
    let mut rows = Vec::with_capacity(probes.len());
    for probe in probes {
        if probe.name.trim().is_empty() {
            return Err(
                "screen: refused: a grant probe must name itself; the disclosure was not published"
                    .to_owned(),
            );
        }
        let reason = probe
            .reason
            .as_deref()
            .map(truncate_grant_reason)
            .filter(|reason| !reason.is_empty());
        if probe.status.needs_reason() && reason.is_none() {
            return Err(format!(
                "screen: refused: grant probe {:?} is {} without a reason; an unanswerable probe \
                 must say why, and the disclosure was not published",
                probe.name,
                probe.status.tag()
            ));
        }
        rows.push(GrantProbe { name: probe.name, status: probe.status, reason });
    }
    rows.sort_by(|left, right| left.name.cmp(right.name));
    for pair in rows.windows(2) {
        if pair[0].name == pair[1].name {
            return Err(format!(
                "screen: refused: grant probe {:?} is published twice; a reader cannot recompute \
                 a disclosure with a duplicated name",
                pair[0].name
            ));
        }
    }
    Ok(rows)
}

/// Read-only environment probe for `op=disclose`. No permission is requested
/// and no event is posted on any backend.
pub fn environment_probe() -> EnvironmentProbe {
    native::environment()
}

/// Read-only grant probes, admitted by [`normalize_grant_probes`] so a
/// disclosure can never publish a malformed set.
pub fn grant_probes() -> Result<Vec<GrantProbe>, String> {
    normalize_grant_probes(native::grants())
}

// ---------------------------------------------------------------------------
// display scale disclosure (0.0.10, prd/PRD_01_13_display_scale_disclosure.md)
// ---------------------------------------------------------------------------
//
// The coordinate basis: a screenshot a model reads is expressed in logical
// coordinates, a pointer event needs physical ones. Every probe below is a
// read-only query: no display-mode change, no DPI change, no mutation of the
// process's DPI awareness and no permission request. A cell that cannot
// answer says `unknown` with the OS and the failing symbol named -- never a
// plausible `1`.

/// Most scale rows one disclosure may publish. A larger set is refused whole
/// by [`normalize_display_scales`], never silently clipped.
pub const SCREEN_TOOL_MAX_DISPLAY_SCALES: usize = 16;

/// Characters of one scale reason, including the truncation marker.
pub const SCREEN_TOOL_MAX_SCALE_REASON: usize = 160;

/// `{x, y, width, height}`, the same shape `observe` publishes.
pub type DisplayRect = (i32, i32, i32, i32);

/// The bounded set of scales `op=disclose` may name. No measured ratio is
/// rounded to a neighbouring member: an out-of-set ratio is `unknown`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayScale {
    One,
    OneQuarter,
    OneHalf,
    Two,
    Three,
    Unknown,
}

impl DisplayScale {
    pub const ALL: [Self; 6] = [
        Self::One,
        Self::OneQuarter,
        Self::OneHalf,
        Self::Two,
        Self::Three,
        Self::Unknown,
    ];

    pub fn tag(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::OneQuarter => "1.25",
            Self::OneHalf => "1.5",
            Self::Two => "2",
            Self::Three => "3",
            Self::Unknown => "unknown",
        }
    }

    /// The inverse of [`DisplayScale::tag`]. An unrecognised string is `None`,
    /// never a guess at a scale.
    pub fn parse(tag: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|scale| scale.tag() == tag)
    }

    /// Only a non-answer must say why.
    pub fn needs_reason(self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// `(numerator, denominator)` of `physical / logical`. `Unknown` has no
    /// ratio, so no relation can be recomputed for that row.
    pub fn ratio(self) -> Option<(i64, i64)> {
        match self {
            Self::One => Some((1, 1)),
            Self::OneQuarter => Some((5, 4)),
            Self::OneHalf => Some((3, 2)),
            Self::Two => Some((2, 1)),
            Self::Three => Some((3, 1)),
            Self::Unknown => None,
        }
    }

    /// Exact match for `physical / logical` in thousandths. A ratio outside
    /// the set is `None` -- it is never rounded into a permissible scale.
    pub fn from_millis(millis: i64) -> Option<Self> {
        Self::ALL.iter().copied().find(|scale| {
            scale
                .ratio()
                .is_some_and(|(num, den)| millis * den == num * 1000)
        })
    }
}

/// Which coordinate basis a backend's `displays()` rect is already in.
/// Win32 `EnumDisplayMonitors` and X11 screen geometry report physical pixels;
/// CoreGraphics `CGDisplayBounds` reports logical points. Naming the basis
/// is what keeps `physical_rect == logical_rect * scale` honest on every cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScaleRectBasis {
    Physical,
    Logical,
}

/// One read-only scale row, indexed by the existing `displays[]` order. No
/// display id, device name, path or user name is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayScaleProbe {
    pub index: usize,
    pub logical_rect: DisplayRect,
    pub physical_rect: DisplayRect,
    pub scale: DisplayScale,
    /// Required for `unknown`; optional otherwise. Names an OS and a symbol.
    pub reason: Option<String>,
}

/// Exact integer scaling of a rect by `num / den`. A rect that does not divide
/// exactly yields `None` rather than a rounded rect that no reader could
/// recompute. Negative origins (secondary monitors) divide the same way.
fn scale_rect(rect: DisplayRect, num: i64, den: i64) -> Option<DisplayRect> {
    let mut out = [0i32; 4];
    for (slot, value) in [rect.0, rect.1, rect.2, rect.3].into_iter().enumerate() {
        let scaled = value as i64 * num;
        if scaled % den != 0 {
            return None;
        }
        out[slot] = i32::try_from(scaled / den).ok()?;
    }
    Some((out[0], out[1], out[2], out[3]))
}

/// Build one row from a backend rect and the scale that backend read. A cell
/// that cannot answer passes `DisplayScale::Unknown` and a reason naming its
/// OS and the symbol that failed; a scale that does not divide the rect exactly
/// is downgraded to `unknown` instead of publishing a fabricated relation.
pub fn display_scale_row(
    index: usize,
    rect: DisplayRect,
    basis: ScaleRectBasis,
    scale: DisplayScale,
    unknown_reason: &str,
) -> DisplayScaleProbe {
    if scale == DisplayScale::Unknown {
        return DisplayScaleProbe {
            index,
            logical_rect: rect,
            physical_rect: rect,
            scale,
            reason: Some(unknown_reason.to_owned()),
        };
    }
    let derived = scale.ratio().and_then(|(num, den)| match basis {
        ScaleRectBasis::Physical => {
            scale_rect(rect, den, num).map(|logical| (logical, rect))
        }
        ScaleRectBasis::Logical => {
            scale_rect(rect, num, den).map(|physical| (rect, physical))
        }
    });
    match derived {
        Some((logical_rect, physical_rect)) => DisplayScaleProbe {
            index,
            logical_rect,
            physical_rect,
            scale,
            reason: None,
        },
        None => DisplayScaleProbe {
            index,
            logical_rect: rect,
            physical_rect: rect,
            scale: DisplayScale::Unknown,
            reason: Some(format!(
                "{unknown_reason}; this ratio does not divide the display rect exactly"
            )),
        },
    }
}

/// Cut a reason to the bound, marking the cut with a final `…` character.
pub fn truncate_scale_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.chars().count() <= SCREEN_TOOL_MAX_SCALE_REASON {
        return trimmed.to_owned();
    }
    let mut out: String = trimmed
        .chars()
        .take(SCREEN_TOOL_MAX_SCALE_REASON - 1)
        .collect();
    out.push('…');
    out
}

/// The single admission rule for the scale rows: bounded, non-empty, indexed
/// in `displays[]` order from zero, every `unknown` carries a reason, and for
/// every known scale `physical_rect == logical_rect * scale`. A violation
/// refuses the whole set -- no partial scale disclosure is ever published.
pub fn normalize_display_scales(
    rows: Vec<DisplayScaleProbe>,
) -> Result<Vec<DisplayScaleProbe>, String> {
    if rows.len() > SCREEN_TOOL_MAX_DISPLAY_SCALES {
        return Err(format!(
            "screen: refused: {} display scale rows exceed the bound of {}; the disclosure was \
             not published",
            rows.len(),
            SCREEN_TOOL_MAX_DISPLAY_SCALES
        ));
    }
    if rows.is_empty() {
        return Err(
            "screen: refused: a cell that cannot probe a scale must publish one named `unknown` \
             row, not omit the scale; the disclosure was not published"
                .to_owned(),
        );
    }
    let mut out = Vec::with_capacity(rows.len());
    for (position, row) in rows.into_iter().enumerate() {
        if row.index != position {
            return Err(format!(
                "screen: refused: display scale row at {position} has index {}; rows must follow \
                 the displays[] order; the disclosure was not published",
                row.index
            ));
        }
        let reason = row
            .reason
            .as_deref()
            .map(truncate_scale_reason)
            .filter(|reason| !reason.is_empty());
        if row.scale.needs_reason() && reason.is_none() {
            return Err(format!(
                "screen: refused: display scale row {position} is {} without a reason; an \
                 unanswerable scale must name the OS and the failed symbol, and the disclosure \
                 was not published",
                row.scale.tag()
            ));
        }
        if let Some((num, den)) = row.scale.ratio() {
            if scale_rect(row.logical_rect, num, den) != Some(row.physical_rect) {
                return Err(format!(
                    "screen: refused: display scale row {position} publishes physical_rect that \
                     is not logical_rect * {}; the disclosure was not published",
                    row.scale.tag()
                ));
            }
        }
        out.push(DisplayScaleProbe {
            index: row.index,
            logical_rect: row.logical_rect,
            physical_rect: row.physical_rect,
            scale: row.scale,
            reason,
        });
    }
    Ok(out)
}

/// Read-only scale rows from the compiled backend, admitted by
/// [`normalize_display_scales`].
pub fn display_scale_probes() -> Result<Vec<DisplayScaleProbe>, String> {
    normalize_display_scales(native::display_scales())
}

pub mod png {
    use super::Captured;

    pub fn encode_rgb(bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
        // Filter byte 0 (None) plus RGB triples, which is what the raw
        // deflate payload below carries.
        let stride = width as usize * 3 + 1;
        let mut raw = Vec::with_capacity(stride * height as usize);
        for row in 0..height as usize {
            raw.push(0);
            let base = row * width as usize * 4;
            for column in 0..width as usize {
                let pixel = base + column * 4;
                raw.push(bgra[pixel + 2]);
                raw.push(bgra[pixel + 1]);
                raw.push(bgra[pixel]);
            }
        }
        let mut out = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 64);
        out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut header = Vec::with_capacity(13);
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &header);
        chunk(&mut out, b"IDAT", &zlib_store(&raw));
        chunk(&mut out, b"IEND", &[]);
        out
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        let mut crc_input = Vec::with_capacity(4 + payload.len());
        crc_input.extend_from_slice(kind);
        crc_input.extend_from_slice(payload);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    }

    /// zlib wrapper around stored (uncompressed) deflate blocks.
    fn zlib_store(raw: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
        out.extend_from_slice(&[0x78, 0x01]);
        let mut offset = 0usize;
        if raw.is_empty() {
            out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
        }
        while offset < raw.len() {
            let length = (raw.len() - offset).min(65535);
            let final_block = offset + length == raw.len();
            out.push(if final_block { 0x01 } else { 0x00 });
            out.extend_from_slice(&(length as u16).to_le_bytes());
            out.extend_from_slice(&(!(length as u16)).to_le_bytes());
            out.extend_from_slice(&raw[offset..offset + length]);
            offset += length;
        }
        out.extend_from_slice(&adler32(raw).to_be_bytes());
        out
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for byte in data {
            a = (a + *byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for byte in data {
            crc ^= *byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    #[allow(dead_code)]
    fn _type_check(_: Captured) {}
}

// ---------------------------------------------------------------------------
// platform backends
// ---------------------------------------------------------------------------

/// The identity of one published a11y row, for locating the live object of
/// an invoke. Mirrors the reader's key: name, role_name, child_id, depth.
#[derive(Clone, Debug)]
pub struct A11yIdentity {
    pub name: String,
    pub role_name: String,
    pub child_id: i32,
    pub depth: u32,
}

/// 0.0.17: invoke a resolved a11y node's MSAA default action by identity.
/// No pointer, no window activation. `Err` is a dispatch failure (named),
/// `Ok(Err(..))` is a failed invoke, `Ok(Ok(()))` is S_OK.
pub fn a11y_invoke_default_action(
    handle: u64,
    identity: A11yIdentity,
) -> Result<Result<(), String>, String> {
    native::a11y_invoke_default_action_inner(handle, identity)
}

mod native {
    //! Hand-written `user32`/`gdi32` bindings. No crate, no COM, no service:
    //! the harness process talks to the desktop and exits.

    use super::{
        control_role, ButtonDispatch, Captured, ClickButton, ControlInfo, Modifier,
        PlatformCapabilities, ScrollDispatch, WindowInfo, SCREEN_TOOL_CONTROL_TIMEOUT_MS,
        SCREEN_TOOL_MAX_CONTROLS, SCREEN_TOOL_MAX_CONTROL_TEXT, SCREEN_TOOL_MAX_DRAG_STEPS,
    };
    use std::ffi::c_void;

    pub const CAPABILITIES: PlatformCapabilities = PlatformCapabilities::ALL;
    pub const PLATFORM: &str = "windows";

    type Hwnd = *mut c_void;
    type Bool = i32;
    type Uint = u32;
    type Int = i32;
    type Dword = u32;
    type Hdc = *mut c_void;
    type Hbitmap = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        left: Int,
        top: Int,
        right: Int,
        bottom: Int,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: Dword,
        width: Int,
        height: Int,
        planes: u16,
        bit_count: u16,
        compression: Dword,
        size_image: Dword,
        x_pels_per_meter: Int,
        y_pels_per_meter: Int,
        clr_used: Dword,
        clr_important: Dword,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [Dword; 3],
    }

    #[repr(C)]
    struct Point {
        x: Int,
        y: Int,
    }

    #[repr(C)]
    struct MonitorInfo {
        size: Dword,
        monitor: Rect,
        work: Rect,
        flags: Dword,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KeyboardInput {
        vk: u16,
        scan: u16,
        flags: Dword,
        time: Dword,
        extra: usize,
    }

    #[repr(C)]
    union InputUnion {
        keyboard: KeyboardInput,
        // Size/alignment of the largest arm (MOUSEINPUT on x64 is 32 bytes).
        _pad: [u64; 4],
    }

    #[repr(C)]
    struct Input {
        kind: Dword,
        data: InputUnion,
    }

    const INPUT_KEYBOARD: Dword = 1;
    const KEYEVENTF_KEYUP: Dword = 0x0002;
    const KEYEVENTF_UNICODE: Dword = 0x0004;

    const SM_CXSCREEN: Int = 0;
    const SM_CYSCREEN: Int = 1;
    const SM_XVIRTUALSCREEN: Int = 76;
    const SM_YVIRTUALSCREEN: Int = 77;
    const SM_CXVIRTUALSCREEN: Int = 78;
    const SM_CYVIRTUALSCREEN: Int = 79;
    /// The 100% baseline: every other scale is a ratio against this DPI.
    const USER_DEFAULT_SCREEN_DPI: Uint = 96;
    const MONITORINFOF_PRIMARY: Dword = 1;
    const SW_RESTORE: Int = 9;
    const SW_SHOW: Int = 5;
    const SRCCOPY: Dword = 0x00CC_0020;
    const BI_RGB: Dword = 0;
    const DIB_RGB_COLORS: Uint = 0;
    const MOUSEEVENTF_LEFTDOWN: Dword = 0x0002;
    const MOUSEEVENTF_LEFTUP: Dword = 0x0004;
    const MOUSEEVENTF_RIGHTDOWN: Dword = 0x0008;
    const MOUSEEVENTF_RIGHTUP: Dword = 0x0010;
    const MOUSEEVENTF_WHEEL: Dword = 0x0800;
    const MOUSEEVENTF_HWHEEL: Dword = 0x1000;
    const MOUSEEVENTF_MIDDLEDOWN: Dword = 0x0020;
    const MOUSEEVENTF_MIDDLEUP: Dword = 0x0040;
    const WM_GETTEXT: Dword = 0x000D;
    const SMTO_ABORTIFHUNG: Dword = 0x0002;

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: extern "system" fn(Hwnd, isize) -> Bool, param: isize) -> Bool;
        fn EnumChildWindows(
            parent: Hwnd,
            callback: extern "system" fn(Hwnd, isize) -> Bool,
            param: isize,
        ) -> Bool;
        fn IsWindowEnabled(window: Hwnd) -> Bool;
        fn SendMessageTimeoutW(
            window: Hwnd,
            message: Dword,
            wparam: usize,
            lparam: isize,
            flags: Dword,
            timeout: Dword,
            result: *mut usize,
        ) -> isize;
        fn IsWindowVisible(window: Hwnd) -> Bool;
        fn IsIconic(window: Hwnd) -> Bool;
        fn IsZoomed(window: Hwnd) -> Bool;
        fn GetWindowTextW(window: Hwnd, buffer: *mut u16, size: Int) -> Int;
        fn GetClassNameW(window: Hwnd, buffer: *mut u16, size: Int) -> Int;
        fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> Bool;
        fn GetWindowThreadProcessId(window: Hwnd, pid: *mut Dword) -> Dword;
        fn GetForegroundWindow() -> Hwnd;
        fn ShowWindow(window: Hwnd, command: Int) -> Bool;
        fn SetForegroundWindow(window: Hwnd) -> Bool;
        fn SetWindowPos(
            window: Hwnd,
            after: Hwnd,
            x: Int,
            y: Int,
            width: Int,
            height: Int,
            flags: Uint,
        ) -> Bool;
        fn GetDC(window: Hwnd) -> Hdc;
        fn ReleaseDC(window: Hwnd, dc: Hdc) -> Int;
        fn SetCursorPos(x: Int, y: Int) -> Bool;
        fn mouse_event(flags: Dword, x: Dword, y: Dword, data: Dword, extra: usize);
        fn SendInput(count: Uint, inputs: *mut Input, size: Int) -> Uint;
        fn GetSystemMetrics(index: Int) -> Int;
        fn VkKeyScanW(character: u16) -> i16;
        fn GetCursorPos(point: *mut Point) -> Bool;
        fn IsWindow(window: Hwnd) -> Bool;
        fn EnumDisplayMonitors(
            dc: Hdc,
            clip: *mut Rect,
            callback: extern "system" fn(Hwnd, Hdc, *mut Rect, isize) -> Bool,
            param: isize,
        ) -> Bool;
        fn GetMonitorInfoW(monitor: Hwnd, info: *mut MonitorInfo) -> Bool;
        // 0.0.10 read-only DPI queries. None of these changes the system DPI
        // or this process's DPI awareness; a zero return is a named `unknown`.
        fn GetDpiForSystem() -> Uint;
        fn GetDpiForWindow(window: Hwnd) -> Uint;
        fn GetSystemMetricsForDpi(index: Int, dpi: Uint) -> Int;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(dc: Hdc) -> Hdc;
        fn CreateCompatibleBitmap(dc: Hdc, width: Int, height: Int) -> Hbitmap;
        fn SelectObject(dc: Hdc, object: Hbitmap) -> Hbitmap;
        fn DeleteObject(object: Hbitmap) -> Bool;
        fn DeleteDC(dc: Hdc) -> Bool;
        fn BitBlt(
            target: Hdc,
            x: Int,
            y: Int,
            width: Int,
            height: Int,
            source: Hdc,
            source_x: Int,
            source_y: Int,
            rop: Dword,
        ) -> Bool;
        fn GetDIBits(
            dc: Hdc,
            bitmap: Hbitmap,
            start: Uint,
            lines: Uint,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: Uint,
        ) -> Int;
    }

    fn wide(raw: &[u16]) -> String {
        let end = raw.iter().position(|unit| *unit == 0).unwrap_or(raw.len());
        String::from_utf16_lossy(&raw[..end])
    }

    struct Collector {
        windows: Vec<WindowInfo>,
        foreground: usize,
    }

    extern "system" fn collect(window: Hwnd, param: isize) -> Bool {
        // SAFETY: `param` is the `&mut Collector` handed to EnumWindows below,
        // and this callback is only reachable inside that call.
        let collector = unsafe { &mut *(param as *mut Collector) };
        if unsafe { IsWindowVisible(window) } == 0 {
            return 1;
        }
        let mut title = [0u16; 512];
        let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as Int) };
        if length <= 0 {
            return 1;
        }
        let mut class_name = [0u16; 256];
        unsafe { GetClassNameW(window, class_name.as_mut_ptr(), class_name.len() as Int) };
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(window, &mut rect) } == 0 {
            return 1;
        }
        let mut pid = 0 as Dword;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        let focused = window == unsafe { GetForegroundWindow() };
        if focused {
            collector.foreground = collector.windows.len();
        }
        collector.windows.push(WindowInfo {
            handle: window as usize as u64,
            title: wide(&title[..length as usize + 1]),
            class_name: wide(&class_name),
            pid,
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
            minimized: unsafe { IsIconic(window) } != 0,
            maximized: unsafe { IsZoomed(window) } != 0,
            focused,
        });
        1
    }

    /// The whole virtual desktop. The primary-monitor metrics (SM_CXSCREEN)
    /// silently drop every secondary display, including ones at negative x.
    pub fn virtual_screen() -> Result<(i32, i32, i32, i32), String> {
        let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if width > 0 && height > 0 {
            return Ok((x, y, width, height));
        }
        // Some remote/headless sessions report only the primary metrics.
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if width <= 0 || height <= 0 {
            return Err("screen: refused: the desktop has no readable dimensions".to_owned());
        }
        Ok((0, 0, width, height))
    }

    struct DisplayCollector {
        displays: Vec<super::DisplayInfo>,
    }

    extern "system" fn collect_display(
        monitor: Hwnd,
        _dc: Hdc,
        _clip: *mut Rect,
        param: isize,
    ) -> Bool {
        // SAFETY: `param` is the `&mut DisplayCollector` handed to
        // EnumDisplayMonitors below; the callback cannot outlive that call.
        let collector = unsafe { &mut *(param as *mut DisplayCollector) };
        let empty = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let mut info = MonitorInfo {
            size: size_of::<MonitorInfo>() as Dword,
            monitor: empty,
            work: empty,
            flags: 0,
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return 1;
        }
        collector.displays.push(super::DisplayInfo {
            x: info.monitor.left,
            y: info.monitor.top,
            width: info.monitor.right - info.monitor.left,
            height: info.monitor.bottom - info.monitor.top,
            primary: info.flags & MONITORINFOF_PRIMARY != 0,
        });
        1
    }

    pub fn displays() -> Result<Vec<super::DisplayInfo>, String> {
        let mut collector = DisplayCollector {
            displays: Vec::new(),
        };
        let ok = unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                collect_display,
                &mut collector as *mut DisplayCollector as isize,
            )
        };
        if ok == 0 {
            return Err("screen: refused: the displays could not be enumerated".to_owned());
        }
        collector
            .displays
            .sort_by_key(|display| (display.x, display.y, display.width, display.height));
        if collector.displays.is_empty() {
            let (x, y, width, height) = virtual_screen()?;
            collector.displays.push(super::DisplayInfo {
                x,
                y,
                width,
                height,
                primary: true,
            });
        }
        Ok(collector.displays)
    }

    pub fn foreground_title() -> Option<String> {
        let window = unsafe { GetForegroundWindow() };
        if window.is_null() {
            return None;
        }
        let mut title = [0u16; 512];
        let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as Int) };
        (length > 0).then(|| wide(&title[..length as usize + 1]))
    }

    /// Whether any window owns the foreground right now. The title is never
    /// published by a disclosure, only this boolean.
    fn foreground_present() -> bool {
        !unsafe { GetForegroundWindow() }.is_null()
    }

    /// Read-only environment probe. `GetSystemMetrics` and
    /// `EnumDisplayMonitors` are pure geometry queries: no TCC, no UAC, no
    /// input event.
    pub fn environment() -> super::EnvironmentProbe {
        let desktop = virtual_screen().ok();
        let display_count = displays().map(|list| list.len()).unwrap_or(0);
        let foreground_present = foreground_present();
        super::environment_from(
            PLATFORM,
            if desktop.is_some() || foreground_present {
                super::SessionKind::Gui
            } else {
                super::SessionKind::Headless
            },
            desktop,
            display_count,
            foreground_present,
        )
    }

    /// Read-only grant probes. Nothing here resolves a privilege: GDI capture
    /// and `EnumWindows` need no OS grant on Windows, and UIPI is decided per
    /// target window, so it is reported `unknown` with the reason named rather
    /// than guessed as `granted`.
    pub fn grants() -> Vec<super::GrantProbe> {
        let foreground = foreground_present();
        vec![
            super::GrantProbe::granted("screen_capture"),
            super::GrantProbe::granted("window_enumeration"),
            super::GrantProbe::unknown(
                "input_posting",
                "UIPI decides per target window integrity level; a read-only probe cannot answer \
                 it and this harness never prompts or elevates",
            ),
            if foreground {
                super::GrantProbe::granted("foreground_window")
            } else {
                super::GrantProbe::unknown(
                    "foreground_window",
                    "no window owns the foreground in this session",
                )
            },
        ]
    }

    /// Read-only display scale (0.0.10). `GetDpiForSystem` reports the
    /// system DPI without depending on this process's awareness; the window
    /// variant is a fallback when no system DPI is exposed.
    /// `GetSystemMetricsForDpi` supplies both the 96-DPI logical baseline and
    /// the physical width at the measured DPI, so the ratio is measured rather
    /// than inferred from a plausible `1`.
    fn windows_display_scale() -> Result<super::DisplayScale, String> {
        let system = unsafe { GetDpiForSystem() };
        let dpi = if system > 0 {
            system
        } else {
            let foreground = unsafe { GetForegroundWindow() };
            if foreground.is_null() {
                0
            } else {
                unsafe { GetDpiForWindow(foreground) }
            }
        };
        if dpi == 0 {
            return Err(
                "windows: neither GetDpiForSystem nor GetDpiForWindow returned a DPI; the scale \
                 cannot be read without changing DPI awareness"
                    .to_owned(),
            );
        }
        let physical = unsafe { GetSystemMetricsForDpi(SM_CXSCREEN, dpi) };
        let logical = unsafe { GetSystemMetricsForDpi(SM_CXSCREEN, USER_DEFAULT_SCREEN_DPI) };
        if physical <= 0 || logical <= 0 {
            return Err(
                "windows: GetSystemMetricsForDpi(SM_CXSCREEN) returned no width at either the \
                 measured DPI or the 96-DPI baseline; the scale cannot be read"
                    .to_owned(),
            );
        }
        let millis = physical as i64 * 1000 / logical as i64;
        super::DisplayScale::from_millis(millis).ok_or_else(|| {
            format!(
                "windows: GetDpiForSystem/GetSystemMetricsForDpi measured {millis}/1000, \
                 outside the bounded scale set; no plausible scale is guessed"
            )
        })
    }

    /// One row per display, in the same order as `displays()`. Every row is a
    /// read-only query: no display-mode change, no DPI change, no awareness
    /// mutation and no permission request.
    pub fn display_scales() -> Vec<super::DisplayScaleProbe> {
        let no_dpi = "windows: GetDpiForSystem/GetDpiForWindow/GetSystemMetricsForDpi could \
                      not be read in this session";
        let scale = windows_display_scale();
        let list = displays().unwrap_or_default();
        if list.is_empty() {
            return vec![super::display_scale_row(
                0,
                (0, 0, 0, 0),
                super::ScaleRectBasis::Physical,
                super::DisplayScale::Unknown,
                &format!(
                    "windows: EnumDisplayMonitors/GetMonitorInfoW enumerated no display; {no_dpi}"
                ),
            )];
        }
        list.iter()
            .enumerate()
            .map(|(index, display)| {
                let rect = (display.x, display.y, display.width, display.height);
                match &scale {
                    Ok(value) => super::display_scale_row(
                        index,
                        rect,
                        super::ScaleRectBasis::Physical,
                        *value,
                        no_dpi,
                    ),
                    Err(reason) => super::display_scale_row(
                        index,
                        rect,
                        super::ScaleRectBasis::Physical,
                        super::DisplayScale::Unknown,
                        reason,
                    ),
                }
            })
            .collect()
    }

    /// A requested pointer position must lie in the virtual desktop. Refusing
    /// here keeps an off-screen click from becoming a click on whatever the OS
    /// clamped it to.
    fn check_point(x: Int, y: Int) -> Result<(), String> {
        let (vx, vy, vw, vh) = virtual_screen()?;
        if (x as i64) < vx as i64
            || (y as i64) < vy as i64
            || (x as i64) >= vx as i64 + vw as i64
            || (y as i64) >= vy as i64 + vh as i64
        {
            return Err(format!(
                "screen: refused: pointer {x},{y} is outside the virtual desktop {vx},{vy} \
                 {vw}x{vh}; observe without a capture to read the display geometry"
            ));
        }
        Ok(())
    }

    /// The pointer readback every input action uses. `SetCursorPos` succeeding
    /// is not evidence; the position the desktop reports afterwards is.
    fn pointer_readback(x: Int, y: Int) -> Result<(Int, Int), String> {
        let mut point = Point { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut point) } == 0 {
            return Err("screen: refused: the pointer position could not be read back".to_owned());
        }
        let off = (point.x - x).abs().max((point.y - y).abs())
            > super::SCREEN_TOOL_POINTER_TOLERANCE_PX;
        if off {
            return Err(format!(
                "screen: failed: asked for pointer at {x},{y}, the desktop reports {},{}; the \
                 requested position was not reached (DPI scaling, clipping or a locked session \
                 can do this). Nothing else was attempted",
                point.x, point.y
            ));
        }
        Ok((point.x, point.y))
    }

    pub fn place_pointer(x: Int, y: Int) -> Result<(Int, Int), String> {
        check_point(x, y)?;
        if unsafe { SetCursorPos(x, y) } == 0 {
            return Err(format!(
                "screen: refused: the pointer could not be placed at {x},{y}"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        pointer_readback(x, y)
    }

    fn button_flags(button: ClickButton) -> (Dword, Dword) {
        match button {
            ClickButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            ClickButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            ClickButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        }
    }

    /// One control's text, bounded and made safe to put in a prompt. A control
    /// belonging to another process cannot be read with `GetWindowTextW`, so
    /// `WM_GETTEXT` is used with `SMTO_ABORTIFHUNG`: an unresponsive foreign
    /// window costs at most [`SCREEN_TOOL_CONTROL_TIMEOUT_MS`] and never stalls
    /// the invocation.
    fn control_text(child: Hwnd) -> String {
        let mut buffer = [0u16; 512];
        let direct = unsafe { GetWindowTextW(child, buffer.as_mut_ptr(), buffer.len() as Int) };
        if direct > 0 {
            return clean_text(wide(&buffer[..direct as usize]));
        }
        let mut written: usize = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                child,
                WM_GETTEXT,
                buffer.len(),
                buffer.as_mut_ptr() as isize,
                SMTO_ABORTIFHUNG,
                SCREEN_TOOL_CONTROL_TIMEOUT_MS,
                &mut written,
            )
        };
        if sent == 0 {
            return String::new();
        }
        let count = written.min(buffer.len());
        clean_text(wide(&buffer[..count]))
    }

    fn clean_text(text: String) -> String {
        let mut short = String::new();
        for character in text.chars() {
            if short.chars().count() >= SCREEN_TOOL_MAX_CONTROL_TEXT {
                short.push_str("...");
                break;
            }
            short.push(if character.is_control() { ' ' } else { character });
        }
        short.trim().to_owned()
    }

    struct ControlCollector {
        parent: u64,
        controls: Vec<ControlInfo>,
        truncated: bool,
    }

    extern "system" fn collect_control(child: Hwnd, param: isize) -> Bool {
        // SAFETY: `param` is the `&mut ControlCollector` handed to
        // EnumChildWindows below; the callback cannot outlive that call.
        let collector = unsafe { &mut *(param as *mut ControlCollector) };
        if collector.controls.len() >= SCREEN_TOOL_MAX_CONTROLS {
            collector.truncated = true;
            return 0;
        }
        let mut class_buffer = [0u16; 128];
        let class_len =
            unsafe { GetClassNameW(child, class_buffer.as_mut_ptr(), class_buffer.len() as Int) };
        let class = if class_len > 0 {
            wide(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(child, &mut rect) } == 0 {
            return 1;
        }
        collector.controls.push(ControlInfo {
            handle: child as usize as u64,
            parent: collector.parent,
            label: String::new(),
            role: control_role(&class),
            class,
            text: control_text(child),
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
            visible: unsafe { IsWindowVisible(child) } != 0,
            enabled: unsafe { IsWindowEnabled(child) } != 0,
        });
        1
    }

    /// The typed control arm of observation. One window only, bounded, and
    /// deterministic: two scans of an unchanged window read the same. A hit on
    /// the bound is reported by `truncated`, never hidden.
    pub fn controls(handle: u64) -> Result<(Vec<ControlInfo>, bool), String> {
        let window = handle as usize as Hwnd;
        if unsafe { IsWindow(window) } == 0 {
            return Err(
                "screen: refused: the window to scan is gone; observe again before acting"
                    .to_owned(),
            );
        }
        let mut collector = ControlCollector {
            parent: handle,
            controls: Vec::new(),
            truncated: false,
        };
        // A zero return can mean "no children" or "stopped at the bound"; the
        // collector says which. Neither is an error: an empty inventory is a
        // fact about a self-drawn UI, not a failed read.
        unsafe {
            EnumChildWindows(
                window,
                collect_control,
                &mut collector as *mut ControlCollector as isize,
            )
        };
        collector
            .controls
            .sort_by_key(|control| (control.y, control.x, control.handle));
        Ok((collector.controls, collector.truncated))
    }


    /// Read-only MSAA/IAccessible tree for self-drawn (Chromium/Electron)
    /// windows, where `EnumChildWindows` reports only the render widget.
    /// Hand-written COM bindings, no new crate, no OS grant request, no
    /// elevation, no writes to the target process.
    pub fn a11y_controls(handle: u64) -> Result<(Vec<super::A11yControl>, bool), String> {
        msaa::scan(handle)
    }

    pub(super) fn a11y_invoke_default_action_inner(
        handle: u64,
        identity: crate::screen::A11yIdentity,
    ) -> Result<Result<(), String>, String> {
        msaa::invoke_default_action(handle, identity)
    }


    mod msaa {
        use super::super::{
            a11y_role_name, A11yControl, SCREEN_TOOL_MAX_A11Y_CONTROLS,
            SCREEN_TOOL_MAX_A11Y_DEPTH,
        };
        use crate::screen::A11yIdentity;
        use std::ffi::c_void;

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct Guid {
            a: u32,
            b: u16,
            c: u16,
            d: [u8; 8],
        }

        /// Win64 VARIANT is 24 bytes: an 8-byte header plus a 16-byte union.
        /// A 16-byte definition corrupts the stack when COM writes through it.
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct Variant {
            vt: u16,
            r1: u16,
            r2: u16,
            r3: u16,
            data: [u8; 16],
        }

        impl Variant {
            fn i4(value: i32) -> Self {
                let mut v = Self { vt: 3, r1: 0, r2: 0, r3: 0, data: [0; 16] };
                v.data[..4].copy_from_slice(&value.to_ne_bytes());
                v
            }
            fn empty() -> Self {
                Self { vt: 0, r1: 0, r2: 0, r3: 0, data: [0; 16] }
            }
            fn dispatch(&self) -> *mut c_void {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&self.data[..8]);
                i64::from_ne_bytes(bytes) as isize as *mut c_void
            }
            fn i4_value(&self) -> i32 {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&self.data[..4]);
                i32::from_ne_bytes(bytes)
            }
        }

        const IID_IACCESSIBLE: Guid = Guid {
            a: 0x6187_36E0,
            b: 0x3C3D,
            c: 0x11CF,
            d: [0x81, 0x0C, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
        };
        const OBJID_CLIENT: u32 = 0xFFFF_FFFC;
        const VT_DISPATCH: u16 = 9;
        const VT_I4: u16 = 3;

        // `EnumChildWindows` and `GetClassNameW` are already declared by the
        // parent native module. Reusing them keeps one signature per symbol.
        #[link(name = "ole32")]
        extern "system" {
            fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
            fn CoUninitialize();
        }
        #[link(name = "oleacc")]
        extern "system" {
            fn AccessibleObjectFromWindow(
                window: *mut c_void,
                object_id: u32,
                iid: *const Guid,
                out: *mut *mut c_void,
            ) -> i32;
            fn AccessibleChildren(
                object: *mut c_void,
                start: i32,
                count: i32,
                out: *mut Variant,
                obtained: *mut i32,
            ) -> i32;
        }

        unsafe fn slot(object: *mut c_void, index: usize) -> usize {
            let table = *(object as *const *const usize);
            *table.add(index)
        }

        unsafe fn name(object: *mut c_void, child: Variant) -> String {
            type F = unsafe extern "system" fn(*mut c_void, Variant, *mut *mut u16) -> i32;
            let function: F = std::mem::transmute(slot(object, 10));
            let mut bstr: *mut u16 = std::ptr::null_mut();
            if function(object, child, &mut bstr) != 0 || bstr.is_null() {
                return String::new();
            }
            let bytes = *((bstr as *const u8).offset(-4) as *const u32) as usize;
            let text = std::slice::from_raw_parts(bstr, (bytes / 2).min(256));
            String::from_utf16_lossy(text)
        }

        unsafe fn role(object: *mut c_void, child: Variant) -> i32 {
            type F = unsafe extern "system" fn(*mut c_void, Variant, *mut Variant) -> i32;
            let function: F = std::mem::transmute(slot(object, 13));
            let mut value = Variant::empty();
            if function(object, child, &mut value) != 0 {
                -1
            } else {
                value.i4_value()
            }
        }

        /// `IAccessible::get_accValue` (vtable slot 11) returns a BSTR, the
        /// same shape as `get_accName`. A missing value is an empty string,
        /// never a guessed one.
        unsafe fn value(object: *mut c_void, child: Variant) -> String {
            type F = unsafe extern "system" fn(*mut c_void, Variant, *mut *mut u16) -> i32;
            let function: F = std::mem::transmute(slot(object, 11));
            let mut bstr: *mut u16 = std::ptr::null_mut();
            if function(object, child, &mut bstr) != 0 || bstr.is_null() {
                return String::new();
            }
            let bytes = *((bstr as *const u8).offset(-4) as *const u32) as usize;
            let text = std::slice::from_raw_parts(bstr, (bytes / 2).min(256));
            String::from_utf16_lossy(text)
        }

        /// `IAccessible::get_accState` (vtable slot 14) returns a VT_I4
        /// `STATE_SYSTEM_*` bit mask. The raw integer is published so the
        /// independent reader can recompute the names; a failed call is zero,
        /// which means "no state bits reported", not "no state".
        unsafe fn state(object: *mut c_void, child: Variant) -> i32 {
            type F = unsafe extern "system" fn(*mut c_void, Variant, *mut Variant) -> i32;
            let function: F = std::mem::transmute(slot(object, 14));
            let mut result = Variant::empty();
            if function(object, child, &mut result) != 0 || result.vt != VT_I4 {
                0
            } else {
                result.i4_value()
            }
        }

        unsafe fn rect(object: *mut c_void, child: Variant) -> [i32; 4] {
            type F = unsafe extern "system" fn(
                *mut c_void,
                *mut i32,
                *mut i32,
                *mut i32,
                *mut i32,
                Variant,
            ) -> i32;
            let function: F = std::mem::transmute(slot(object, 22));
            let (mut x, mut y, mut w, mut h) = (0i32, 0i32, 0i32, 0i32);
            if function(object, &mut x, &mut y, &mut w, &mut h, child) != 0 {
                [0, 0, 0, 0]
            } else {
                [x, y, w, h]
            }
        }

        unsafe fn child_count(object: *mut c_void) -> i32 {
            type F = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
            let function: F = std::mem::transmute(slot(object, 8));
            let mut count = 0i32;
            if function(object, &mut count) != 0 { 0 } else { count }
        }

        unsafe fn release(object: *mut c_void) {
            type F = unsafe extern "system" fn(*mut c_void) -> u32;
            let function: F = std::mem::transmute(slot(object, 2));
            function(object);
        }

        struct Walk {
            rows: Vec<A11yControl>,
            truncated: bool,
        }

        unsafe fn walk(
            object: *mut c_void,
            child: Variant,
            depth: usize,
            expand: bool,
            out: &mut Walk,
        ) {
            if out.rows.len() >= SCREEN_TOOL_MAX_A11Y_CONTROLS {
                out.truncated = true;
                return;
            }
            let role_value = role(object, child);
            let [x, y, width, height] = rect(object, child);
            out.rows.push(A11yControl {
                name: name(object, child),
                value: value(object, child),
                role: role_value,
                role_name: a11y_role_name(role_value),
                state: state(object, child),
                x,
                y,
                width,
                height,
                depth: depth as u32,
                child_id: child.i4_value(),
            });
            if !expand || depth >= SCREEN_TOOL_MAX_A11Y_DEPTH {
                return;
            }
            let count = child_count(object).min(SCREEN_TOOL_MAX_A11Y_CONTROLS as i32);
            if count <= 0 {
                return;
            }
            let mut buffer = vec![Variant::empty(); count as usize];
            let mut obtained = 0i32;
            if AccessibleChildren(
                object,
                0,
                count,
                buffer.as_mut_ptr(),
                &mut obtained,
            ) != 0
            {
                return;
            }
            for index in 0..obtained as usize {
                if out.rows.len() >= SCREEN_TOOL_MAX_A11Y_CONTROLS {
                    out.truncated = true;
                    return;
                }
                let item = buffer[index];
                if item.vt == VT_DISPATCH {
                    let pointer = item.dispatch();
                    if !pointer.is_null() {
                        walk(pointer, Variant::i4(0), depth + 1, true, out);
                        release(pointer);
                    }
                } else if item.vt == VT_I4 {
                    // A simple child has no vtable of its own: read it through
                    // its parent, but do not enumerate the parent's children
                    // again under that child id.
                    walk(object, item, depth + 1, false, out);
                }
            }
        }

        extern "system" fn collect_child(window: *mut c_void, param: isize) -> i32 {
            unsafe {
                let found = &mut *(param as *mut Vec<*mut c_void>);
                let mut class = [0u16; 128];
                let length = super::GetClassNameW(
                    window,
                    class.as_mut_ptr(),
                    class.len() as i32,
                );
                let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
                if class.contains("Chrome_RenderWidgetHostHWND") {
                    found.push(window);
                }
            }
            1
        }

        /// 0.0.17: invoke by identity. Walks the same tree `scan` publishes,
        /// locates the unique object whose published identity matches, and
        /// invokes *there* - Electron exposes named buttons as deep
        /// VT_DISPATCH children, so addressing a child_id on the window
        /// object would invoke the wrong target (verified by the W1 probe).
        pub(super) fn invoke_default_action(
            handle: u64,
            identity: A11yIdentity,
        ) -> Result<Result<(), String>, String> {
            let window = handle as usize as *mut c_void;
            unsafe {
                let initialized = CoInitializeEx(std::ptr::null_mut(), 2);
                if initialized < 0 {
                    return Err(format!(
                        "screen: unknown: COM could not be initialised for an invoke \
                         (HRESULT {initialized:#010x})"
                    ));
                }
                let outcome = invoke_by_identity(window, &identity);
                CoUninitialize();
                outcome
            }
        }

        unsafe fn invoke_by_identity(
            window: *mut c_void,
            identity: &A11yIdentity,
        ) -> Result<Result<(), String>, String> {
            let mut children: Vec<*mut c_void> = Vec::new();
            super::EnumChildWindows(
                window,
                collect_child,
                &mut children as *mut _ as isize,
            );
            if children.is_empty() {
                children.push(window);
            }
            let mut matches = 0usize;
            for host in children {
                let mut object: *mut c_void = std::ptr::null_mut();
                let hr = AccessibleObjectFromWindow(
                    host,
                    OBJID_CLIENT,
                    &IID_IACCESSIBLE,
                    &mut object,
                );
                if hr != 0 || object.is_null() {
                    continue;
                }
                let mut found: Option<(*mut c_void, Variant)> = None;
                walk_for_identity(
                    object,
                    Variant::i4(0),
                    0,
                    identity,
                    &mut matches,
                    &mut found,
                );
                if let Some((target, child)) = found {
                    let result = do_default_action(target, child);
                    if target != object {
                        release(target);
                    }
                    release(object);
                    if result == 0 {
                        return Ok(Ok(()));
                    }
                    return Ok(Err(format!(
                        "accDoDefaultAction returned HRESULT {result:#010x}"
                    )));
                }
                release(object);
            }
            if matches == 0 {
                Ok(Err("no accessible object matches the resolved identity".to_owned()))
            } else {
                Ok(Err("the resolved identity is ambiguous in the live tree".to_owned()))
            }
        }

        /// Bounded read-mostly walk; the only live effect is the invoke the
        /// caller performs on the found object after this returns.
        unsafe fn walk_for_identity(
            object: *mut c_void,
            child: Variant,
            depth: usize,
            identity: &A11yIdentity,
            matches: &mut usize,
            found: &mut Option<(*mut c_void, Variant)>,
        ) {
            if found.is_some() {
                return;
            }
            let node_role = role(object, child);
            if node_role != -1
                && child.i4_value() == identity.child_id
                && name(object, child) == identity.name
                && a11y_role_name(node_role) == identity.role_name
            {
                *matches += 1;
                if *matches == 1 {
                    *found = Some((object, child));
                }
                return;
            }
            if depth >= SCREEN_TOOL_MAX_A11Y_DEPTH {
                return;
            }
            type CF = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
            let child_count: CF = std::mem::transmute(slot(object, 8));
            let mut count = 0i32;
            let _ = child_count(object, &mut count);
            let count = count.min(SCREEN_TOOL_MAX_A11Y_CONTROLS as i32);
            if count <= 0 {
                return;
            }
            let mut buffer = vec![Variant::empty(); count as usize];
            let mut obtained = 0i32;
            if AccessibleChildren(
                object,
                0,
                count,
                buffer.as_mut_ptr(),
                &mut obtained,
            ) != 0
            {
                return;
            }
            for item in &buffer[..obtained.max(0) as usize] {
                if item.vt == VT_DISPATCH {
                    let pointer = item.dispatch();
                    if !pointer.is_null() {
                        walk_for_identity(
                            pointer,
                            Variant::i4(0),
                            depth + 1,
                            identity,
                            matches,
                            found,
                        );
                        if found.is_some() {
                            return;
                        }
                        release(pointer);
                    }
                } else {
                    walk_for_identity(
                        object,
                        *item,
                        depth + 1,
                        identity,
                        matches,
                        found,
                    );
                }
            }
        }

        /// `IAccessible::accDoDefaultAction` (vtable slot 25).
        unsafe fn do_default_action(object: *mut c_void, child: Variant) -> i32 {
            type F = unsafe extern "system" fn(*mut c_void, Variant) -> i32;
            let function: F = std::mem::transmute(slot(object, 25));
            function(object, child)
        }

        pub(super) fn scan(handle: u64) -> Result<(Vec<A11yControl>, bool), String> {
            let window = handle as usize as *mut c_void;
            unsafe {
                let initialized = CoInitializeEx(std::ptr::null_mut(), 2);
                if initialized < 0 {
                    return Err(format!(
                        "screen: unknown: COM could not be initialised for an a11y scan \
                         (HRESULT {initialized:#010x})"
                    ));
                }
                let mut children: Vec<*mut c_void> = Vec::new();
                super::EnumChildWindows(
                    window,
                    collect_child,
                    &mut children as *mut _ as isize,
                );
                // The render widget is the usual host; when it is absent still
                // try the top-level window itself.
                if children.is_empty() {
                    children.push(window);
                }
                let mut out = Walk { rows: Vec::new(), truncated: false };
                for child in children {
                    let mut object: *mut c_void = std::ptr::null_mut();
                    let hr = AccessibleObjectFromWindow(
                        child,
                        OBJID_CLIENT,
                        &IID_IACCESSIBLE,
                        &mut object,
                    );
                    if hr == 0 && !object.is_null() {
                        walk(object, Variant::i4(0), 0, true, &mut out);
                        release(object);
                    }
                }
                CoUninitialize();
                out.rows.sort_by_key(|row| (row.depth, row.y, row.x));
                Ok((out.rows, out.truncated))
            }
        }
    }

    pub fn windows() -> Result<Vec<WindowInfo>, String> {
        let mut collector = Collector {
            windows: Vec::new(),
            foreground: 0,
        };
        let ok = unsafe {
            EnumWindows(
                collect,
                &mut collector as *mut Collector as isize,
            )
        };
        if ok == 0 {
            return Err("screen: the desktop could not be enumerated".to_owned());
        }
        Ok(collector.windows)
    }

    /// Resolves one live handle from the same enumeration that `observe`
    /// publishes. A handle outside the fresh list is never guessed at.
    pub fn window_by_handle(handle: u64) -> Option<WindowInfo> {
        windows().ok()?.into_iter().find(|window| window.handle == handle)
    }

    /// Resolves one model-supplied title to a window. Exact match first, then a
    /// unique case-insensitive substring; an ambiguous request is refused with
    /// the candidates named, never resolved by guess.
    pub fn find(title: &str) -> Result<WindowInfo, String> {
        let all = windows()?;
        if let Some(exact) = all.iter().find(|window| window.title == title) {
            return Ok(exact.clone());
        }
        let needle = title.to_lowercase();
        let matches: Vec<&WindowInfo> = all
            .iter()
            .filter(|window| window.title.to_lowercase().contains(&needle))
            .collect();
        match matches.as_slice() {
            [] => Err(format!(
                "screen: refused: no window title contains {title:?}; observe first to see what \
                 is there"
            )),
            [only] => Ok((*only).clone()),
            many => Err(format!(
                "screen: refused: {title:?} matches {} windows ({}); give a longer, unique title",
                many.len(),
                many.iter()
                    .take(5)
                    .map(|window| format!("{:?}", window.title))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// A handle is never trusted across calls: the window may have been closed
    /// (and the handle reused) between the title lookup and the operation.
    pub fn handle_is_live(handle: u64, title: &str) -> Result<(), String> {
        let window = handle as usize as Hwnd;
        if window.is_null() || unsafe { IsWindow(window) } == 0 {
            return Err(format!(
                "screen: refused: the window {title:?} no longer exists; observe again before \
                 operating on it"
            ));
        }
        let mut current = [0u16; 512];
        let length = unsafe { GetWindowTextW(window, current.as_mut_ptr(), current.len() as Int) };
        if length <= 0 || wide(&current[..length as usize + 1]) != title {
            return Err(format!(
                "screen: refused: the window {title:?} changed identity between lookup and \
                 operation; observe again"
            ));
        }
        Ok(())
    }

    pub fn focus(handle: u64) -> Result<(), String> {
        let window = handle as usize as Hwnd;
        if unsafe { IsIconic(window) } != 0 {
            unsafe { ShowWindow(window, SW_RESTORE) };
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        if unsafe { SetForegroundWindow(window) } == 0 {
            // Windows refuses foreground steals from a background process; the
            // operation is reported as not performed rather than retried.
            return Err(
                "screen: refused: the window manager refused the foreground request; bring the \
                 window forward once by hand, then retry"
                    .to_owned(),
            );
        }
        Ok(())
    }

    pub fn move_window(handle: u64, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
        let window = handle as usize as Hwnd;
        if width <= 0 || height <= 0 {
            return Err("screen: refused: width and height must be positive".to_owned());
        }
        if unsafe { IsIconic(window) } != 0 {
            unsafe { ShowWindow(window, SW_RESTORE) };
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        // A maximized window ignores the requested rectangle unless it is
        // restored first; restoring is part of performing the move, not a
        // second operation.
        if unsafe { IsZoomed(window) } != 0 {
            unsafe { ShowWindow(window, SW_RESTORE) };
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        // SWP_NOZORDER (0x0004) | SWP_NOACTIVATE (0x0010): moving a window is
        // not a request to take focus.
        let ok = unsafe { SetWindowPos(window, std::ptr::null_mut(), x, y, width, height, 0x0014) };
        if ok == 0 {
            return Err("screen: refused: the window manager refused the move".to_owned());
        }
        Ok(())
    }

    pub fn click(
        x: i32,
        y: i32,
        button: ClickButton,
        double: bool,
    ) -> Result<(i32, i32, ButtonDispatch), String> {
        let position = place_pointer(x, y)?;
        std::thread::sleep(std::time::Duration::from_millis(40));
        let (down, up) = button_flags(button);
        let mut downs = 0;
        let mut ups = 0;
        unsafe { mouse_event(down, 0, 0, 0, 0) };
        downs += 1;
        unsafe { mouse_event(up, 0, 0, 0, 0) };
        ups += 1;
        if double {
            std::thread::sleep(std::time::Duration::from_millis(60));
            unsafe { mouse_event(down, 0, 0, 0, 0) };
            downs += 1;
            unsafe { mouse_event(up, 0, 0, 0, 0) };
            ups += 1;
        }
        Ok((position.0, position.1, ButtonDispatch {
            button: button.tag(), downs, ups, double, path_points: 0,
        }))
    }

    /// A press, a stepped move and a release. Stepped motion matters: most
    /// drag-and-drop implementations need intermediate WM_MOUSEMOVE messages,
    /// and a single jump to the destination is never a drag.
    pub fn drag(
        x: i32,
        y: i32,
        points: &[(i32, i32)],
        button: ClickButton,
    ) -> Result<(i32, i32, ButtonDispatch), String> {
        const STEPS: i64 = SCREEN_TOOL_MAX_DRAG_STEPS;
        let &(last_x, last_y) = points.last().ok_or_else(|| {
            "screen: refused: a drag needs at least one destination point".to_owned()
        })?;
        for &(px, py) in points {
            check_point(px, py)?;
        }
        place_pointer(x, y)?;
        let (down, up) = button_flags(button);
        let mut downs = 0;
        let mut ups = 0;
        unsafe { mouse_event(down, 0, 0, 0, 0) };
        downs += 1;
        let mut from_x = x as i64;
        let mut from_y = y as i64;
        for &(to_x, to_y) in points {
            for step in 1..=STEPS {
                let px = from_x + (to_x as i64 - from_x) * step / STEPS;
                let py = from_y + (to_y as i64 - from_y) * step / STEPS;
                unsafe { SetCursorPos(px as Int, py as Int) };
                std::thread::sleep(std::time::Duration::from_millis(12));
            }
            from_x = to_x as i64;
            from_y = to_y as i64;
        }
        unsafe { mouse_event(up, 0, 0, 0, 0) };
        ups += 1;
        let position = pointer_readback(last_x, last_y)?;
        Ok((position.0, position.1, ButtonDispatch {
            button: button.tag(), downs, ups, double: false,
            path_points: points.len() as u32,
        }))
    }

    pub fn scroll(
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
    ) -> Result<(i32, i32, ScrollDispatch), String> {
        let position = place_pointer(x, y)?;
        std::thread::sleep(std::time::Duration::from_millis(40));
        if delta_y != 0 {
            unsafe { mouse_event(MOUSEEVENTF_WHEEL, 0, 0, delta_y as Dword, 0) };
        }
        if delta_x != 0 {
            // HWHEEL is positive to the right; the signed convention is
            // normalised in the published evidence, not left to the reader.
            unsafe { mouse_event(MOUSEEVENTF_HWHEEL, 0, 0, delta_x as Dword, 0) };
        }
        Ok((position.0, position.1, ScrollDispatch { delta_x, delta_y }))
    }

    pub fn type_text(text: &str) -> Result<(), String> {
        for character in text.chars() {
            // A UTF-16 unit at a time: surrogates are delivered as the pair the
            // string already carries, so astral characters survive.
            let mut units = [0u16; 2];
            for unit in character.encode_utf16(&mut units) {
                for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
                    let input = Input {
                        kind: INPUT_KEYBOARD,
                        data: InputUnion {
                            keyboard: KeyboardInput {
                                vk: 0,
                                scan: *unit,
                                flags,
                                time: 0,
                                extra: 0,
                            },
                        },
                    };
                    let sent = unsafe {
                        SendInput(1, &input as *const Input as *mut Input, size_of::<Input>() as Int)
                    };
                    if sent != 1 {
                        return Err(
                            "screen: refused: the desktop did not accept the keystroke; nothing \
                             further was typed"
                                .to_owned(),
                        );
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Ok(())
    }

    const MODIFIERS: [(&str, u16); 4] = [
        ("ctrl", 0x11),
        ("alt", 0x12),
        ("shift", 0x10),
        ("win", 0x5B),
    ];

    const NAMED_KEYS: [(&str, u16); 32] = [
        ("enter", 0x0D),
        ("return", 0x0D),
        ("tab", 0x09),
        ("esc", 0x1B),
        ("escape", 0x1B),
        ("space", 0x20),
        ("backspace", 0x08),
        ("delete", 0x2E),
        ("home", 0x24),
        ("end", 0x23),
        ("pageup", 0x21),
        ("pagedown", 0x22),
        ("up", 0x26),
        ("down", 0x28),
        ("left", 0x25),
        ("right", 0x27),
        ("insert", 0x2D),
        ("printscreen", 0x2C),
        ("pause", 0x13),
        ("capslock", 0x14),
        ("f1", 0x70),
        ("f2", 0x71),
        ("f3", 0x72),
        ("f4", 0x73),
        ("f5", 0x74),
        ("f6", 0x75),
        ("f7", 0x76),
        ("f8", 0x77),
        ("f9", 0x78),
        ("f10", 0x79),
        ("f11", 0x7A),
        ("f12", 0x7B),
    ];

    pub fn press_key(combo: &str) -> Result<(), String> {
        let parts: Vec<String> = combo
            .split('+')
            .map(|part| part.trim().to_lowercase())
            .filter(|part| !part.is_empty())
            .collect();
        let (key, modifiers) = parts
            .split_last()
            .ok_or_else(|| "screen: refused: no key given".to_owned())?;
        let mut modifier_codes = Vec::with_capacity(modifiers.len());
        for modifier in modifiers {
            let code = MODIFIERS
                .iter()
                .find(|(name, _)| name == modifier)
                .map(|(_, code)| *code)
                .ok_or_else(|| format!("screen: refused: unknown modifier {modifier:?}"))?;
            modifier_codes.push(code);
        }
        let code = NAMED_KEYS
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, code)| *code)
            .or_else(|| {
                let mut chars = key.chars();
                let only = chars.next().filter(|_| chars.next().is_none())?;
                let mapped = unsafe { VkKeyScanW(only as u16) };
                (mapped != -1).then_some((mapped & 0xFF) as u16)
            })
            .ok_or_else(|| format!("screen: refused: unknown key {key:?}"))?;
        for modifier in &modifier_codes {
            key_event(*modifier, false)?;
        }
        key_event(code, false)?;
        key_event(code, true)?;
        for modifier in modifier_codes.iter().rev() {
            key_event(*modifier, true)?;
        }
        Ok(())
    }

    /// Virtual keys for the four held modifiers. `key_event(code, up)` uses
    /// `up=true` for key-up, so `set_modifier(.., down)` posts `!down`.
    const HELD_MODIFIER_CODES: [(Modifier, u16); 4] = [
        (Modifier::Ctrl, 0x11),
        (Modifier::Shift, 0x10),
        (Modifier::Alt, 0x12),
        (Modifier::Win, 0x5B),
    ];

    pub fn set_modifier(key: Modifier, down: bool) -> Result<(), String> {
        let (_, code) = HELD_MODIFIER_CODES
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .ok_or_else(|| format!("screen: refused: unknown modifier {key:?}"))?;
        key_event(*code, !down)
    }

    pub fn release_all_modifiers() -> Result<(), String> {
        for (_, code) in HELD_MODIFIER_CODES {
            key_event(code, true)?;
        }
        Ok(())
    }

    fn key_event(code: u16, up: bool) -> Result<(), String> {
        let input = Input {
            kind: INPUT_KEYBOARD,
            data: InputUnion {
                keyboard: KeyboardInput {
                    vk: code,
                    scan: 0,
                    flags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    extra: 0,
                },
            },
        };
        let sent = unsafe {
            SendInput(1, &input as *const Input as *mut Input, size_of::<Input>() as Int)
        };
        if sent != 1 {
            return Err(format!(
                "screen: refused: the desktop did not accept key event {code:#x}; nothing \
                 further was pressed"
            ));
        }
        Ok(())
    }

    /// Reads one rectangular region into 32-bit BGRA, top-down, then downsamples
    /// and encodes it. `x`/`y` are virtual-desktop coordinates and may be
    /// negative on a secondary display.
    fn grab(x: i32, y: i32, width: i32, height: i32, scale: u32) -> Result<Captured, String> {
        if width <= 0 || height <= 0 {
            return Err(format!(
                "screen: refused: the region is {width}x{height}; nothing to capture"
            ));
        }
        if width > super::SCREEN_TOOL_MAX_REGION_PIXELS
            || height > super::SCREEN_TOOL_MAX_REGION_PIXELS
        {
            return Err(format!(
                "screen: refused: the region is {width}x{height}, over the \
                 {bound} pixel bound on each side; capture a smaller region",
                bound = super::SCREEN_TOOL_MAX_REGION_PIXELS
            ));
        }
        let screen = unsafe { GetDC(std::ptr::null_mut()) };
        if screen.is_null() {
            return Err("screen: refused: no screen device context".to_owned());
        }
        let memory = unsafe { CreateCompatibleDC(screen) };
        let bitmap = unsafe { CreateCompatibleBitmap(screen, width, height) };
        if memory.is_null() || bitmap.is_null() {
            unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
            return Err("screen: refused: a capture surface could not be created".to_owned());
        }
        let previous = unsafe { SelectObject(memory, bitmap) };
        let copied = unsafe { BitBlt(memory, 0, 0, width, height, screen, x, y, SRCCOPY) };
        let mut info = BitmapInfo {
            header: BitmapInfoHeader {
                size: size_of::<BitmapInfoHeader>() as Dword,
                width,
                // Negative height asks for a top-down bitmap, which is the row
                // order PNG wants.
                height: -height,
                planes: 1,
                bit_count: 32,
                compression: BI_RGB,
                size_image: 0,
                x_pels_per_meter: 0,
                y_pels_per_meter: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0; 3],
        };
        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        let lines = unsafe {
            GetDIBits(
                memory,
                bitmap,
                0,
                height as Uint,
                pixels.as_mut_ptr() as *mut c_void,
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        unsafe { SelectObject(memory, previous) };
        unsafe { DeleteObject(bitmap) };
        unsafe { DeleteDC(memory) };
        unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
        if copied == 0 || lines != height {
            return Err("screen: refused: the capture could not be read back".to_owned());
        }
        let chosen = super::capture_scale(width as u32, height as u32, scale);
        let (pixels, out_width, out_height) =
            super::downsample(&pixels, width as u32, height as u32, chosen);
        Ok(Captured {
            bytes: png::encode_rgb(&pixels, out_width, out_height),
            width: out_width,
            height: out_height,
            scale: chosen,
        })
    }

    pub fn capture_desktop(scale: u32) -> Result<Captured, String> {
        let (x, y, width, height) = virtual_screen()?;
        grab(x, y, width, height, scale)
    }

    pub fn capture_region(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        scale: u32,
    ) -> Result<Captured, String> {
        grab(x, y, width, height, scale)
    }

    pub fn capture_window(handle: u64, scale: u32) -> Result<Captured, String> {
        let window = handle as usize as Hwnd;
        if unsafe { IsIconic(window) } != 0 {
            unsafe { ShowWindow(window, SW_SHOW) };
            unsafe { ShowWindow(window, SW_RESTORE) };
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(window, &mut rect) } == 0 {
            return Err("screen: refused: the window has no readable rectangle".to_owned());
        }
        grab(
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            scale,
        )
    }

    pub fn now_millis() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or(0)
    }

    /// Store-mode PNG: no compression, so no compressor has to be audited or
    /// linked. A capture is a local artifact; its size is bounded elsewhere.
    pub use super::png;
}

/// Runtime `dlopen`/`dlsym` shared by the Darwin and Linux backends. Windows
/// uses its import libraries and never compiles this module.
#[cfg(not(windows))]
#[path = "screen_dynlib.rs"]
mod dynlib;

/// One refusing backend per non-Windows target OS. W1 froze the contract and
/// the capability matrix; W2 replaced macOS and Linux with real backends.
/// This macro now serves only an OS outside the six-cell matrix. A backend
/// that has not implemented a verb must never report an empty success, and
/// must never be described as a different OS.
#[cfg(not(windows))]
#[allow(unused_macros)]
macro_rules! refusing_backend {
    ($platform:expr, $hint:expr) => {
        use super::{
            ButtonDispatch, Captured, ClickButton, ControlInfo, DisplayInfo, Modifier,
            PlatformCapabilities, ScrollDispatch, WindowInfo,
        };

        pub const PLATFORM: &str = $platform;
        pub const CAPABILITIES: PlatformCapabilities = PlatformCapabilities::NONE;

        fn unsupported(verb: &str) -> String {
            format!(
                "cu: unsupported: {verb} is not implemented by the {PLATFORM} backend; \
                 disclose reports this cell's capability matrix. {}. No effect was performed.",
                $hint
            )
        }

        pub fn windows() -> Result<Vec<WindowInfo>, String> {
            Err(unsupported("observe windows"))
        }
        pub fn window_by_handle(_handle: u64) -> Option<WindowInfo> {
            None
        }
        pub fn virtual_screen() -> Result<(i32, i32, i32, i32), String> {
            Err(unsupported("observe virtual screen"))
        }
        pub fn displays() -> Result<Vec<DisplayInfo>, String> {
            Err(unsupported("observe displays"))
        }
        pub fn foreground_title() -> Option<String> {
            None
        }
        /// No backend means no environment can be read. The disclosure names
        /// the session kind and the cell; it does not invent a desktop.
        pub fn environment() -> super::EnvironmentProbe {
            super::environment_from(
                PLATFORM,
                super::SessionKind::Unavailable,
                None,
                0,
                false,
            )
        }
        /// Every probe is `not_applicable` with a reason naming this cell, so a
        /// reader can tell "no backend" from "no permission".
        pub fn grants() -> Vec<super::GrantProbe> {
            let reason = format!(
                "this cell has no computer-use backend in the six-cell matrix ({})",
                PLATFORM
            );
            vec![
                super::GrantProbe::not_applicable("screen_capture", reason.clone()),
                super::GrantProbe::not_applicable("input_posting", reason),
            ]
        }
        /// No backend means no scale can be read. One `unknown` row names the
        /// cell and the platform symbols that are absent; it never guesses 1.
        pub fn display_scales() -> Vec<super::DisplayScaleProbe> {
            let reason = format!(
                "{PLATFORM}: this cell has no computer-use backend in the six-cell matrix, so \
                 GetDpiForSystem (windows), backingScaleFactor (macos) and Xft.dpi / \
                 XRRScreenResources (linux) are all absent; no scale can be probed"
            );
            vec![super::display_scale_row(
                0,
                (0, 0, 0, 0),
                super::ScaleRectBasis::Physical,
                super::DisplayScale::Unknown,
                &reason,
            )]
        }
        pub fn controls(_handle: u64) -> Result<(Vec<ControlInfo>, bool), String> {
            Err(unsupported("observe controls"))
        }
        pub fn a11y_controls(
            _handle: u64,
        ) -> Result<(Vec<super::A11yControl>, bool), String> {
            Err(unsupported("observe a11y"))
        }
        pub fn handle_is_live(_handle: u64, _title: &str) -> Result<(), String> {
            Err(unsupported("window liveness"))
        }
        pub fn place_pointer(_x: i32, _y: i32) -> Result<(i32, i32), String> {
            Err(unsupported("act:hover"))
        }
        pub fn drag(
            _x: i32, _y: i32, _points: &[(i32, i32)], _button: ClickButton,
        ) -> Result<(i32, i32, ButtonDispatch), String> {
            Err(unsupported("act:drag"))
        }
        pub fn find(_title: &str) -> Result<WindowInfo, String> {
            Err(unsupported("act:focus"))
        }
        pub fn focus(_handle: u64) -> Result<(), String> {
            Err(unsupported("act:focus"))
        }
        pub fn move_window(_h: u64, _x: i32, _y: i32, _w: i32, _h2: i32) -> Result<(), String> {
            Err(unsupported("act:move"))
        }
        pub fn click(
            _x: i32, _y: i32, _b: ClickButton, _d: bool,
        ) -> Result<(i32, i32, ButtonDispatch), String> {
            Err(unsupported("act:click"))
        }
        pub fn scroll(
            _x: i32, _y: i32, _delta_x: i32, _delta_y: i32,
        ) -> Result<(i32, i32, ScrollDispatch), String> {
            Err(unsupported("act:scroll"))
        }
        pub fn type_text(_text: &str) -> Result<(), String> {
            Err(unsupported("act:type"))
        }
        pub fn press_key(_combo: &str) -> Result<(), String> {
            Err(unsupported("act:key"))
        }
        pub fn set_modifier(_key: Modifier, _down: bool) -> Result<(), String> {
            Err(unsupported("act:key hold"))
        }
        pub fn release_all_modifiers() -> Result<(), String> {
            Err(unsupported("act:key release"))
        }
        pub fn capture_desktop(_scale: u32) -> Result<Captured, String> {
            Err(unsupported("capture desktop"))
        }
        pub fn capture_window(_handle: u64, _scale: u32) -> Result<Captured, String> {
            Err(unsupported("capture window"))
        }
        pub fn capture_region(
            _x: i32, _y: i32, _width: i32, _height: i32, _scale: u32,
        ) -> Result<Captured, String> {
            Err(unsupported("capture region"))
        }
        pub fn now_millis() -> u128 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis())
                .unwrap_or(0)
        }
    };
}

#[cfg(target_os = "macos")]
#[path = "screen_macos.rs"]
mod native;

#[cfg(target_os = "linux")]
#[path = "screen_linux.rs"]
mod native;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod native {
    refusing_backend!(
        std::env::consts::OS,
        "this target has no computer-use backend in the six-cell matrix"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0.0.15: a normalised MSAA node, pure data with no COM or desktop.
    fn node(name: &str, value: &str, role: i32, state: i32) -> A11yControl {
        A11yControl {
            name: name.to_owned(),
            value: value.to_owned(),
            role,
            role_name: a11y_role_name(role),
            state,
            x: 500,
            y: 16,
            width: 32,
            height: 32,
            depth: 6,
            child_id: 0,
        }
    }

    #[test]
    fn a11y_targets_are_resolved_by_identity_never_by_a_guess() {
        let rows = vec![
            node("新建任务", "", 43, 0x100000),
            node("分享", "", 43, 0x100000),
            node("", "search hint", 42, 0x100000),
        ];
        // Exact name wins, case-insensitive substring over name or value is
        // unique, and an unknown role is refused rather than ignored.
        assert_eq!(resolve_a11y_control(&rows, "新建任务", None).unwrap().name, "新建任务");
        assert_eq!(resolve_a11y_control(&rows, "分享", Some("button")).unwrap().name, "分享");
        assert_eq!(resolve_a11y_control(&rows, "SEARCH HINT", None).unwrap().role, 42);
        assert!(resolve_a11y_control(&rows, "新建任务", Some("invented"))
            .unwrap_err()
            .contains("unknown a11y role"));
        assert!(resolve_a11y_control(&rows, "新建任务", Some("textbox"))
            .unwrap_err()
            .contains("no a11y node matches"));
        assert!(resolve_a11y_control(&rows, "  ", None).unwrap_err().contains("must name"));
        assert!(resolve_a11y_control(&[], "新建任务", None)
            .unwrap_err()
            .contains("no a11y node matches"));
    }

    #[test]
    fn a11y_targets_refuse_ambiguity_and_inoperable_states() {
        let rows = vec![
            node("OK", "first", 43, 0x100000),
            node("OK", "second", 43, 0x100000),
        ];
        let ambiguous = resolve_a11y_control(&rows, "OK", None).unwrap_err();
        assert!(ambiguous.contains("matches 2 operable a11y nodes"), "{ambiguous}");
        // A role filter can disambiguate only when it actually separates them.
        let rows = vec![
            node("OK", "", 43, 0x100000),
            node("OK", "", 42, 0x100000),
        ];
        assert_eq!(resolve_a11y_control(&rows, "OK", Some("textbox")).unwrap().role, 42);

        for veto in [A11Y_STATE_UNAVAILABLE, A11Y_STATE_INVISIBLE, A11Y_STATE_OFFSCREEN] {
            let rows = vec![node("关闭", "", 43, veto)];
            let reason = resolve_a11y_control(&rows, "关闭", None).unwrap_err();
            assert!(reason.contains("unavailable, invisible or offscreen"), "{reason}");
            assert!(!a11y_node_operable(veto));
        }
        assert!(a11y_node_operable(0x100000));
        assert!(a11y_node_operable(0x4 | 0x40 | 0x100000));
        // `unavailable` stays in the raw mask but is never published as a name.
        assert!(a11y_state_names(A11Y_STATE_UNAVAILABLE).is_empty());
    }

    fn control(label: &str, text: &str, x: i32, y: i32) -> ControlInfo {
        ControlInfo {
            handle: 0,
            parent: 1,
            label: label.to_owned(),
            class: "Button".to_owned(),
            role: "button",
            text: text.to_owned(),
            x,
            y,
            width: 40,
            height: 20,
            visible: true,
            enabled: true,
        }
    }

    #[test]
    fn control_roles_come_from_a_fixed_win32_class_table() {
        assert_eq!(control_role("Button"), "button");
        assert_eq!(control_role("Edit"), "textbox");
        assert_eq!(control_role("Static"), "label");
        assert_eq!(control_role("SysListView32"), "list");
        assert_eq!(control_role("SysTreeView32"), "tree");
        // Third-party, self-drawn classes are unknown, not guessed.
        assert_eq!(control_role("Chrome_RenderWidgetHostHWND"), "unknown");
    }

    #[test]
    fn labels_are_stable_unique_and_never_blank() {
        let mut controls = vec![
            control("", "OK", 0, 0),
            control("", "OK", 40, 0),
            control("", "", 0, 20),
            control("", "Cancel", 40, 20),
        ];
        label_controls(&mut controls);
        let labels: Vec<&str> = controls.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels.len(), 4);
        assert_eq!(
            labels.iter().collect::<std::collections::HashSet<_>>().len(),
            4,
            "{labels:?}"
        );
        assert!(labels.iter().all(|label| !label.is_empty()));
        assert_eq!(labels[0], "button:OK");
        // The empty-text control is named by role and index, not by an
        // ambiguous empty string.
        assert_eq!(labels[2], "button#2");
        label_controls(&mut controls);
        assert_eq!(controls[0].label, "button:OK", "labelling is idempotent");
    }

    #[test]
    fn resolving_a_control_never_guesses_an_ambiguous_name() {
        let controls = vec![
            control("button:OK", "OK", 0, 0),
            control("button:Cancel", "Cancel", 40, 0),
            control("button:OK (2)", "OK too", 80, 0),
        ];
        assert_eq!(resolve_control(&controls, "button:OK").unwrap().text, "OK");
        assert_eq!(resolve_control(&controls, "cancel").unwrap().text, "Cancel");
        let ambiguous = resolve_control(&controls, "OK").unwrap_err();
        assert!(ambiguous.contains("matches 2 controls"), "{ambiguous}");
        assert!(ambiguous.contains("button:OK"), "{ambiguous}");
        assert!(ambiguous.contains("not performed"), "{ambiguous}");
        assert!(resolve_control(&controls, "nothing").unwrap_err().contains("no control matches"));
        assert!(resolve_control(&controls, "  ").unwrap_err().contains("must name"));
    }

    #[test]
    fn a_zero_sized_control_has_no_centre_to_target() {
        let mut empty = control("button:empty", "", 10, 10);
        empty.width = 0;
        assert_eq!(empty.center(), None);
        let normal = control("button:OK", "OK", 10, 20);
        assert_eq!(normal.center(), Some((30, 30)));
    }

    fn root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "minicon-harness-screen-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        root.canonicalize().expect("canonical fixture root")
    }

    #[test]
    fn screen_refuses_without_the_flag_and_names_it() {
        let tool = ScreenTool::new(&root("flag"), false);
        let error = tool.observe(None, None, 0, None).unwrap_err();
        assert!(error.contains("--allow-screen"), "{error}");
        assert!(error.contains("refused"), "{error}");
    }

    #[test]
    fn screen_type_bound_is_enforced_before_any_input() {
        let tool = ScreenTool::new(&root("type"), true);
        let text = "x".repeat(SCREEN_TOOL_MAX_TYPE_CHARS + 1);
        // Reaches the bound check before the platform is consulted, so the
        // refusal is the same on every host.
        if platform_qualified() {
            let error = tool.act(Act::Type { text: &text }).unwrap_err();
            assert!(error.contains("over the type bound"), "{error}");
        }
    }

    #[test]
    fn window_line_is_stable_and_ordered() {
        let window = WindowInfo {
            handle: 42,
            title: "Grok Bot".to_owned(),
            class_name: "Chrome_WidgetWin_1".to_owned(),
            pid: 7,
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            minimized: false,
            maximized: false,
            focused: true,
        };
        assert_eq!(
            window.to_line(),
            "hwnd=42 pid=7 rect=0,0 800x600 focused class=Chrome_WidgetWin_1 title=Grok Bot"
        );
    }

    #[test]
    fn drag_path_points_are_admitted_against_the_whole_desktop_rect() {
        let desktop = (-1920, -200, 3840, 1280);
        assert!(check_drag_path(&[(-1900, -100), (0, 0), (1900, 1000)], desktop).is_ok());
        let outside = check_drag_path(&[(0, 0), (1900, 1081)], desktop).unwrap_err();
        assert!(outside.starts_with("screen: refused:"), "{outside}");
        assert!(outside.contains("1900,1081"), "{outside}");
        assert!(outside.contains("-1920,-200 3840x1280"), "{outside}");
        assert!(outside.contains("no button event was posted"), "{outside}");
    }

    #[test]
    fn scroll_axes_are_bounded_and_zero_is_a_named_refusal() {
        assert!(check_scroll_axes(120, -120).is_ok());
        assert!(check_scroll_axes(SCREEN_TOOL_MAX_SCROLL_DELTA, 0).is_ok());
        let too_large = check_scroll_axes(SCREEN_TOOL_MAX_SCROLL_DELTA + 1, 0).unwrap_err();
        assert!(too_large.contains("+-1440"), "{too_large}");
        let zero = check_scroll_axes(0, 0).unwrap_err();
        assert!(zero.contains("both scroll axes are zero"), "{zero}");
        // i32::MIN must be refused, not panic in an abs() call.
        let minimum = check_scroll_axes(i32::MIN, 0).unwrap_err();
        assert!(minimum.contains("+-1440"), "{minimum}");
    }

    #[test]
    fn display_line_reports_negative_virtual_coordinates() {
        let display = DisplayInfo {
            x: -1920,
            y: -240,
            width: 1920,
            height: 1080,
            primary: false,
        };
        assert_eq!(
            display.to_line(1),
            "display=1 rect=-1920,-240 1920x1080"
        );
    }

    #[test]
    fn capture_scale_only_grows_when_store_mode_png_would_exceed_the_bound() {
        assert_eq!(capture_scale(1680, 1050, 0), 1);
        assert_eq!(capture_scale(3840, 2160, 0), 2);
        assert_eq!(capture_scale(7680, 4320, 0), 4);
        assert_eq!(capture_scale(65536, 65536, 0), SCREEN_TOOL_MAX_CAPTURE_SCALE);
        assert_eq!(capture_scale(3840, 2160, 3), 3);
        // Absurd dimensions must saturate, not overflow in a debug build.
        assert_eq!(capture_scale(u32::MAX, u32::MAX, 0), SCREEN_TOOL_MAX_CAPTURE_SCALE);
    }

    #[test]
    fn downsample_averages_a_block_and_keeps_alpha() {
        // 2x2 BGRA, all pure red (B=0, G=0, R=255).
        let mut red = Vec::new();
        for _ in 0..4 {
            red.extend_from_slice(&[0, 0, 255, 255]);
        }
        let (pixels, width, height) = downsample(&red, 2, 2, 2);
        assert_eq!((width, height), (1, 1));
        assert_eq!(pixels, vec![0, 0, 255, 255]);
        // Non-divisible dimensions round up, with the partial block averaged
        // over the samples that exist and alpha always opaque.
        let mut mixed = Vec::new();
        for _ in 0..3 {
            mixed.extend_from_slice(&[10, 20, 30, 0]);
        }
        let (pixels, width, height) = downsample(&mixed, 3, 1, 2);
        assert_eq!((width, height), (2, 1));
        assert_eq!(&pixels[4..8], &[10, 20, 30, 255]);
        assert_eq!(pixels[3], 255);
    }

    #[test]
    fn foreground_evidence_reports_a_change_and_refuses_a_loss() {
        assert_eq!(
            foreground_evidence(Some("Editor".into()), Some("Editor".into())).unwrap(),
            "foreground=\"Editor\""
        );
        let changed =
            foreground_evidence(Some("Editor".into()), Some("Save dialog".into())).unwrap();
        assert!(changed.contains("changed during input"), "{changed}");
        let lost = foreground_evidence(Some("Editor".into()), None).unwrap_err();
        assert!(lost.contains("disappeared"), "{lost}");
        assert_eq!(
            foreground_evidence(None, None).unwrap(),
            "foreground=<none>"
        );
    }

    #[test]
    fn capture_retention_keeps_only_tool_produced_pngs() {
        let root = root("retention");
        let directory = root.join(SCREEN_TOOL_CAPTURE_DIR);
        std::fs::create_dir_all(&directory).expect("capture directory");
        for index in 0..(SCREEN_TOOL_MAX_CAPTURE_FILES + 4) {
            std::fs::write(
                directory.join(format!("capture-1-{index}.png")),
                b"png",
            )
            .expect("fixture capture");
        }
        std::fs::write(directory.join("model-output.txt"), b"keep").expect("foreign file");
        prune_captures(&directory);
        let captures = std::fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("capture-"))
            .count();
        assert_eq!(captures, SCREEN_TOOL_MAX_CAPTURE_FILES);
        assert!(directory.join("model-output.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn png_encoder_produces_a_readable_container() {
        let bgra = [
            0u8, 0, 255, 255, // red
            0, 255, 0, 255, // green
            255, 0, 0, 255, // blue
            255, 255, 255, 255, // white
        ];
        let png = native::png::encode_rgb(&bgra, 2, 2);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..20], &2u32.to_be_bytes());
        assert_eq!(&png[20..24], &2u32.to_be_bytes());
        assert!(png.ends_with(b"IEND\xae\x42\x60\x82"));
    }

    /// The only honest qualification for a desktop tool: run it against a real
    /// desktop. Ignored by default because CI has no interactive session; run
    /// `cargo test -- --ignored` on a signed-in machine and read the evidence.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs a signed-in Windows desktop session"]
    fn real_desktop_observe_lists_windows_and_writes_a_png() {
        let root = root("real");
        let tool = ScreenTool::new(&root, true);
        let observed = tool
            .observe(None, Some(CaptureTarget::Desktop), 0, None)
            .expect("observe a real desktop");
        assert!(observed.starts_with("platform=windows"), "{observed}");
        assert!(observed.contains("windows="), "{observed}");
        assert!(observed.contains("desktop="), "{observed}");
        assert!(observed.contains("display=0 rect="), "{observed}");
        assert!(observed.contains("capture: .harness/capture/"), "{observed}");
        assert!(observed.contains("scale="), "{observed}");
        // Printed with --nocapture so a qualification run leaves human-readable
        // evidence of the display topology, not just assertions in code.
        for line in observed.lines().take_while(|line| !line.starts_with("hwnd=")) {
            eprintln!("observe: {line}");
        }

        let png = root.join(
            observed
                .lines()
                .find_map(|line| line.strip_prefix("capture: ")?.split(' ').next())
                .expect("capture path"),
        );
        let bytes = std::fs::read(&png).expect("capture readable");
        assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // IHDR width/height must be the real screen, not zero.
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        assert!(width >= 640 && height >= 480, "{width}x{height}");

        // Geometry, region capture, explicit scale, pointer readback and
        // out-of-bounds refusal: all against the same live desktop.
        let geometry = observed
            .lines()
            .find_map(|line| line.strip_prefix("desktop="))
            .expect("desktop geometry line")
            .split_whitespace()
            .next()
            .expect("origin");
        let mut origin = geometry.split(',');
        let vx: i32 = origin.next().expect("x").parse().expect("x integer");
        let vy: i32 = origin.next().expect("y").parse().expect("y integer");

        let region = tool
            .observe(
                None,
                Some(CaptureTarget::Region {
                    x: vx + 10,
                    y: vy + 10,
                    width: 200,
                    height: 120,
                }),
                2,
                None,
            )
            .expect("capture a real region at scale 2");
        assert!(region.contains("image=100x60 scale=2"), "{region}");
        let region_path = region
            .lines()
            .find_map(|line| line.strip_prefix("capture: ")?.split(' ').next())
            .expect("region capture path");
        let region_bytes = std::fs::read(root.join(region_path)).expect("region PNG");
        assert_eq!(
            u32::from_be_bytes([
                region_bytes[16], region_bytes[17], region_bytes[18], region_bytes[19]
            ]),
            100
        );
        assert_eq!(
            u32::from_be_bytes([
                region_bytes[20], region_bytes[21], region_bytes[22], region_bytes[23]
            ]),
            60
        );

        let hover = tool
            .act(Act::Hover {
                x: vx + 100,
                y: vy + 100,
            })
            .expect("place the pointer on the real desktop");
        assert!(hover.readback.starts_with("pointer at "), "{}", hover.readback);

        let outside = tool
            .act(Act::Click {
                x: i32::MAX,
                y: i32::MAX,
                button: ClickButton::Left,
                double: false,
            })
            .unwrap_err();
        assert!(outside.contains("outside the virtual desktop"), "{outside}");

        let bad_scale = tool.observe(None, None, 9, None).unwrap_err();
        assert!(bad_scale.contains("over the bound"), "{bad_scale}");

        // A move is the only operation that can be verified without typing
        // into someone's window: it changes a rectangle and reports it back.
        // Both outcomes are acceptable, and both must be evidence: either the
        // requested rectangle is what the desktop now reports, or the refusal
        // names the rectangle the desktop actually kept. What is never
        // acceptable is a bare `ok` that hides a window manager ignoring the
        // request.
        let Some(first) = observed
            .lines()
            .skip(1)
            .find(|line| line.starts_with("hwnd="))
        else {
            panic!("no windows to operate on: {observed}");
        };
        let title = first
            .split("title=")
            .nth(1)
            .expect("title field")
            .to_owned();
        match tool.act(Act::Move {
            title,
            x: 40,
            y: 40,
            width: 720,
            height: 520,
        }) {
            Ok(outcome) => {
                assert_eq!(outcome.action, "move");
                assert!(outcome.readback.contains("720x520"), "{}", outcome.readback);
            }
            Err(error) => {
                assert!(error.contains("the desktop reports rect="), "{error}");
                assert!(error.contains("720x520"), "{error}");
            }
        }
    }
}
