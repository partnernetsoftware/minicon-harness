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

/// One observation, as typed state. The CU host binds a generation to this
/// value; nothing here is a rendered prompt string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenSnapshot {
    pub platform: String,
    pub qualified: bool,
    /// Virtual desktop origin and size.
    pub desktop: (i32, i32, i32, i32),
    pub displays: Vec<DisplayInfo>,
    pub windows: Vec<WindowInfo>,
    /// Empty when this observe did not request a control scan.
    pub controls: Vec<ControlInfo>,
    /// Which window was scanned, when one was. `None` means no scan happened.
    pub controls_source: Option<String>,
    /// True when [`SCREEN_TOOL_MAX_CONTROLS`] stopped the enumeration.
    pub controls_truncated: bool,
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
        let capture = match capture {
            Some(target) => Some(self.capture(target, scale)?),
            None => None,
        };
        Ok(ScreenSnapshot {
            platform: native::PLATFORM.to_owned(),
            qualified: platform_qualified(),
            desktop: virtual_screen,
            displays,
            windows,
            controls,
            controls_source,
            controls_truncated,
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
        Ok(self.snapshot(title, capture, scale, controls)?.to_text())
    }

    /// Performs exactly one operation. Nothing here retries: an operation is
    /// attempted once and its outcome is read back from the desktop.
    pub fn act(&self, act: Act<'_>) -> Result<ActOutcome, String> {
        self.admit()?;
        match act {
            Act::Focus { title } => {
                let window = native::find(&title)?;
                native::handle_is_live(window.handle, &window.title)?;
                native::focus(window.handle)?;
                let readback = native::windows()?
                    .into_iter()
                    .find(|candidate| candidate.focused)
                    .map(|candidate| format!("foreground={:?}", candidate.title))
                    .unwrap_or_else(|| "foreground=<none>".to_owned());
                Ok(ActOutcome {
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
                    action: "move".to_owned(),
                    window: Some(after.title),
                    readback: format!(
                        "rect={},{} {}x{}",
                        after.x, after.y, after.width, after.height
                    ),
                })
            }
            Act::Click { x, y, button, double } => {
                let (px, py) = native::click(x, y, button, double)?;
                Ok(ActOutcome {
                    action: "click".to_owned(),
                    window: None,
                    readback: format!("pointer at {px},{py}"),
                })
            }
            Act::Hover { x, y } => {
                let (px, py) = native::place_pointer(x, y)?;
                Ok(ActOutcome {
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
            } => {
                let (px, py) = native::drag(x, y, to_x, to_y, button)?;
                Ok(ActOutcome {
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
                    action: "key".to_owned(),
                    window: None,
                    readback: format!("pressed {keys} {evidence}"),
                })
            }
            Act::Scroll { x, y, delta } => {
                let (px, py) = native::scroll(x, y, delta)?;
                Ok(ActOutcome {
                    action: "scroll".to_owned(),
                    window: None,
                    readback: format!("scrolled {delta} at {px},{py}"),
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
    Focus { title: String },
    Move { title: String, x: i32, y: i32, width: i32, height: i32 },
    Click { x: i32, y: i32, button: ClickButton, double: bool },
    Hover { x: i32, y: i32 },
    Drag { x: i32, y: i32, to_x: i32, to_y: i32, button: ClickButton },
    Type { text: &'a str },
    Key { keys: String },
    Scroll { x: i32, y: i32, delta: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickButton {
    Left,
    Right,
    Middle,
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
fn foreground_evidence(before: Option<String>, after: Option<String>) -> Result<String, String> {
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

#[cfg(windows)]
mod native {
    //! Hand-written `user32`/`gdi32` bindings. No crate, no COM, no service:
    //! the harness process talks to the desktop and exits.

    use super::{
        control_role, Captured, ClickButton, ControlInfo, PlatformCapabilities, WindowInfo,
        SCREEN_TOOL_CONTROL_TIMEOUT_MS, SCREEN_TOOL_MAX_CONTROLS, SCREEN_TOOL_MAX_CONTROL_TEXT,
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
    ) -> Result<(i32, i32), String> {
        let position = place_pointer(x, y)?;
        std::thread::sleep(std::time::Duration::from_millis(40));
        let (down, up) = button_flags(button);
        unsafe { mouse_event(down, 0, 0, 0, 0) };
        unsafe { mouse_event(up, 0, 0, 0, 0) };
        if double {
            std::thread::sleep(std::time::Duration::from_millis(60));
            unsafe { mouse_event(down, 0, 0, 0, 0) };
            unsafe { mouse_event(up, 0, 0, 0, 0) };
        }
        Ok(position)
    }

    /// A press, a stepped move and a release. Stepped motion matters: most
    /// drag-and-drop implementations need intermediate WM_MOUSEMOVE messages,
    /// and a single jump to the destination is never a drag.
    pub fn drag(
        x: i32,
        y: i32,
        to_x: i32,
        to_y: i32,
        button: ClickButton,
    ) -> Result<(i32, i32), String> {
        const STEPS: i64 = 12;
        check_point(to_x, to_y)?;
        place_pointer(x, y)?;
        let (down, up) = button_flags(button);
        unsafe { mouse_event(down, 0, 0, 0, 0) };
        for step in 1..=STEPS {
            let px = x as i64 + (to_x as i64 - x as i64) * step / STEPS;
            let py = y as i64 + (to_y as i64 - y as i64) * step / STEPS;
            unsafe { SetCursorPos(px as Int, py as Int) };
            std::thread::sleep(std::time::Duration::from_millis(12));
        }
        unsafe { mouse_event(up, 0, 0, 0, 0) };
        pointer_readback(to_x, to_y)
    }

    pub fn scroll(x: i32, y: i32, delta: i32) -> Result<(i32, i32), String> {
        let position = place_pointer(x, y)?;
        std::thread::sleep(std::time::Duration::from_millis(40));
        unsafe { mouse_event(MOUSEEVENTF_WHEEL, 0, 0, delta as Dword, 0) };
        Ok(position)
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
            Captured, ClickButton, ControlInfo, DisplayInfo, PlatformCapabilities, WindowInfo,
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
        pub fn virtual_screen() -> Result<(i32, i32, i32, i32), String> {
            Err(unsupported("observe virtual screen"))
        }
        pub fn displays() -> Result<Vec<DisplayInfo>, String> {
            Err(unsupported("observe displays"))
        }
        pub fn foreground_title() -> Option<String> {
            None
        }
        pub fn controls(_handle: u64) -> Result<(Vec<ControlInfo>, bool), String> {
            Err(unsupported("observe controls"))
        }
        pub fn handle_is_live(_handle: u64, _title: &str) -> Result<(), String> {
            Err(unsupported("window liveness"))
        }
        pub fn place_pointer(_x: i32, _y: i32) -> Result<(i32, i32), String> {
            Err(unsupported("act:hover"))
        }
        pub fn drag(
            _x: i32, _y: i32, _to_x: i32, _to_y: i32, _button: ClickButton,
        ) -> Result<(i32, i32), String> {
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
        ) -> Result<(i32, i32), String> {
            Err(unsupported("act:click"))
        }
        pub fn scroll(_x: i32, _y: i32, _delta: i32) -> Result<(i32, i32), String> {
            Err(unsupported("act:scroll"))
        }
        pub fn type_text(_text: &str) -> Result<(), String> {
            Err(unsupported("act:type"))
        }
        pub fn press_key(_combo: &str) -> Result<(), String> {
            Err(unsupported("act:key"))
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
