//! The third tool: `screen` (computer use), in two halves that never blur:
//! `observe` reads the desktop and changes nothing, `act` performs exactly one
//! operation and reports what it did.
//!
//! The shape follows the contract already frozen by the sibling repositories
//! (`minicon/prd/PRD_02_38_computer_use.md` CU0..CU2 and
//! `agenterm/docs/cu-gaps-analysis.md`): observation and actuation are
//! separate target-bound choices, an operation names its target explicitly, and
//! an unqualified platform refuses by name instead of pretending to work.
//!
//! Deliberately **no third-party crate**. The Windows path is hand-written FFI
//! to `user32`/`gdi32` and a store-mode PNG encoder, because the harness ships
//! as one auditable executable per cell.
//!
//! v1 qualification: Windows, pixel-level observation (window inventory plus a
//! PNG written under the task root). The accessibility tree is an explicit open
//! gap -- see `prd/PRD_02_01_computer_use.md` -- so nothing here claims a
//! semantic read of any window.

use std::path::{Path, PathBuf};

/// Most windows one `observe` returns. The list becomes a model prompt, so the
/// bound belongs to the read, exactly like the file tool's.
pub const SCREEN_TOOL_MAX_WINDOWS: usize = 64;

/// Most bytes one captured image may occupy before it is refused. A capture is
/// written under the root and never enters the prompt, so this bound exists to
/// stop a runaway virtual desktop, not to bound a prompt.
pub const SCREEN_TOOL_MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

/// Most characters one `act:type` may deliver in a single call.
pub const SCREEN_TOOL_MAX_TYPE_CHARS: usize = 4096;

/// How far a requested window rectangle may end up from the one the desktop
/// reports before the move counts as not performed. Window managers apply
/// minimum sizes, invisible borders and snapped layouts; a difference beyond a
/// few pixels means the system kept its own geometry, and reporting that as a
/// success would be the exact lie this harness refuses.
pub const SCREEN_TOOL_MOVE_TOLERANCE_PX: i32 = 4;

/// Directory (relative to the task root) every capture is published into. The
/// model never names a capture path; the tool composes it, so a capture cannot
/// become an arbitrary write.
pub const SCREEN_TOOL_CAPTURE_DIR: &str = ".harness/capture";

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
        if !native::QUALIFIED {
            return Err(format!(
                "screen: refused: computer use is not qualified on this platform ({}); the tool \
                 refuses instead of reporting an empty success",
                native::PLATFORM
            ));
        }
        Ok(())
    }

    /// Reads the desktop. `title` filters by substring when given; `capture`
    /// writes one PNG under [`SCREEN_TOOL_CAPTURE_DIR`].
    pub fn observe(
        &self,
        title: Option<&str>,
        capture: Option<CaptureTarget>,
    ) -> Result<String, String> {
        self.admit()?;
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
        // Deterministic order: focused first, then top-left to bottom-right, so
        // two calls against an unchanged desktop read the same.
        windows.sort_by_key(|window| {
            (
                !window.focused,
                window.minimized,
                window.y,
                window.x,
                window.handle,
            )
        });
        let total = windows.len();
        let listed = windows.len().min(SCREEN_TOOL_MAX_WINDOWS);
        let mut text = format!("platform={} windows={total}", native::PLATFORM);
        if total > listed {
            text.push_str(&format!(" (listed {listed}, bound {SCREEN_TOOL_MAX_WINDOWS})"));
        }
        for window in windows.iter().take(listed) {
            text.push('\n');
            text.push_str(&window.to_line());
        }
        if let Some(capture) = capture {
            let (window, path, bytes) = self.capture(capture)?;
            text.push_str(&format!(
                "\ncapture: {path} bytes={bytes} source={window}"
            ));
        }
        Ok(text)
    }

    /// Performs exactly one operation. Nothing here retries: an operation is
    /// attempted once and its outcome is read back from the desktop.
    pub fn act(&self, act: Act<'_>) -> Result<ActOutcome, String> {
        self.admit()?;
        match act {
            Act::Focus { title } => {
                let window = native::find(&title)?;
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
                native::click(x, y, button, double)?;
                Ok(ActOutcome {
                    action: "click".to_owned(),
                    window: None,
                    readback: format!("pointer at {x},{y}"),
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
                native::type_text(text)?;
                Ok(ActOutcome {
                    action: "type".to_owned(),
                    window: None,
                    readback: format!("typed {chars} characters"),
                })
            }
            Act::Key { keys } => {
                native::press_key(&keys)?;
                Ok(ActOutcome {
                    action: "key".to_owned(),
                    window: None,
                    readback: format!("pressed {keys}"),
                })
            }
            Act::Scroll { x, y, delta } => {
                native::scroll(x, y, delta)?;
                Ok(ActOutcome {
                    action: "scroll".to_owned(),
                    window: None,
                    readback: format!("scrolled {delta} at {x},{y}"),
                })
            }
        }
    }

    /// Captures the whole desktop or one window, publishes it under the root,
    /// and returns `(source, relative path, bytes)`. The path is composed here,
    /// never supplied by the model.
    fn capture(&self, target: CaptureTarget) -> Result<(String, String, usize), String> {
        let (source, image) = match &target {
            CaptureTarget::Desktop => ("desktop".to_owned(), native::capture_desktop()?),
            CaptureTarget::Window(title) => {
                let window = native::find(title)?;
                let image = native::capture_window(window.handle)?;
                (format!("{:?}", window.title), image)
            }
        };
        if image.bytes.len() > SCREEN_TOOL_MAX_CAPTURE_BYTES {
            return Err(format!(
                "screen: refused: the capture is {} bytes, over the bound of {}; capture one \
                 window instead of the desktop",
                image.bytes.len(),
                SCREEN_TOOL_MAX_CAPTURE_BYTES
            ));
        }
        let directory = self.root.join(SCREEN_TOOL_CAPTURE_DIR);
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("screen: capture directory failed: {error}"))?;
        let name = format!("capture-{}-{}.png", std::process::id(), native::now_millis());
        let path = directory.join(&name);
        std::fs::write(&path, &image.bytes)
            .map_err(|error| format!("screen: capture write failed: {error}"))?;
        Ok((
            source,
            format!("{SCREEN_TOOL_CAPTURE_DIR}/{name}"),
            image.bytes.len(),
        ))
    }
}

/// Which surface one capture reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureTarget {
    Desktop,
    /// Substring match, resolved through the same admission as `act`.
    Window(String),
}

/// One operation the model asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Act<'a> {
    Focus { title: String },
    Move { title: String, x: i32, y: i32, width: i32, height: i32 },
    Click { x: i32, y: i32, button: ClickButton, double: bool },
    Type { text: &'a str },
    Key { keys: String },
    Scroll { x: i32, y: i32, delta: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickButton {
    Left,
    Right,
}

/// One encoded image on its way to disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Captured {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// ---------------------------------------------------------------------------
// platform backends
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod native {
    //! Hand-written `user32`/`gdi32` bindings. No crate, no COM, no service:
    //! the harness process talks to the desktop and exits.

    use super::{Captured, ClickButton, WindowInfo};
    use std::ffi::c_void;

    pub const QUALIFIED: bool = true;
    pub const PLATFORM: &str = "windows";

    type Hwnd = *mut c_void;
    type Bool = i32;
    type Uint = u32;
    type Int = i32;
    type Dword = u32;
    type Hdc = *mut c_void;
    type Hbitmap = *mut c_void;

    #[repr(C)]
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

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: extern "system" fn(Hwnd, isize) -> Bool, param: isize) -> Bool;
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

    pub fn click(x: i32, y: i32, button: ClickButton, double: bool) -> Result<(), String> {
        if unsafe { SetCursorPos(x, y) } == 0 {
            return Err("screen: refused: the pointer could not be placed".to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
        let (down, up) = match button {
            ClickButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            ClickButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        };
        unsafe { mouse_event(down, 0, 0, 0, 0) };
        unsafe { mouse_event(up, 0, 0, 0, 0) };
        if double {
            std::thread::sleep(std::time::Duration::from_millis(60));
            unsafe { mouse_event(down, 0, 0, 0, 0) };
            unsafe { mouse_event(up, 0, 0, 0, 0) };
        }
        Ok(())
    }

    pub fn scroll(x: i32, y: i32, delta: i32) -> Result<(), String> {
        unsafe { SetCursorPos(x, y) };
        std::thread::sleep(std::time::Duration::from_millis(60));
        unsafe { mouse_event(MOUSEEVENTF_WHEEL, 0, 0, delta as Dword, 0) };
        Ok(())
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

    const NAMED_KEYS: [(&str, u16); 22] = [
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
        ("f1", 0x70),
        ("f2", 0x71),
        ("f5", 0x74),
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
            key_event(*modifier, false);
        }
        key_event(code, false);
        key_event(code, true);
        for modifier in modifier_codes.iter().rev() {
            key_event(*modifier, true);
        }
        Ok(())
    }

    fn key_event(code: u16, up: bool) {
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
        unsafe { SendInput(1, &input as *const Input as *mut Input, size_of::<Input>() as Int) };
    }

    /// Reads one rectangular region into 32-bit BGRA, top-down.
    fn grab(x: i32, y: i32, width: i32, height: i32) -> Result<Captured, String> {
        if width <= 0 || height <= 0 {
            return Err(format!(
                "screen: refused: the region is {width}x{height}; nothing to capture"
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
        Ok(Captured {
            bytes: png::encode_rgb(&pixels, width as u32, height as u32),
            width: width as u32,
            height: height as u32,
        })
    }

    pub fn capture_desktop() -> Result<Captured, String> {
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        grab(0, 0, width, height)
    }

    pub fn capture_window(handle: u64) -> Result<Captured, String> {
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
}

#[cfg(not(windows))]
mod native {
    //! Every other platform refuses by name. A path that was never qualified is
    //! reported as unqualified, never as an empty success -- the CU0 safe
    //! failure the sibling PRDs require.

    use super::{Captured, ClickButton, WindowInfo};
    use std::path::PathBuf;

    pub const QUALIFIED: bool = false;
    pub const PLATFORM: &str = std::env::consts::OS;

    pub fn windows() -> Result<Vec<WindowInfo>, String> {
        Err(unqualified())
    }

    pub fn find(_title: &str) -> Result<WindowInfo, String> {
        Err(unqualified())
    }

    pub fn focus(_handle: u64) -> Result<(), String> {
        Err(unqualified())
    }

    pub fn move_window(_h: u64, _x: i32, _y: i32, _w: i32, _h2: i32) -> Result<(), String> {
        Err(unqualified())
    }

    pub fn click(_x: i32, _y: i32, _b: ClickButton, _d: bool) -> Result<(), String> {
        Err(unqualified())
    }

    pub fn scroll(_x: i32, _y: i32, _delta: i32) -> Result<(), String> {
        Err(unqualified())
    }

    pub fn type_text(_text: &str) -> Result<(), String> {
        Err(unqualified())
    }

    pub fn press_key(_combo: &str) -> Result<(), String> {
        Err(unqualified())
    }

    pub fn capture_desktop() -> Result<Captured, String> {
        Err(unqualified())
    }

    pub fn capture_window(_handle: u64) -> Result<Captured, String> {
        Err(unqualified())
    }

    pub fn now_millis() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or(0)
    }

    #[allow(dead_code)]
    fn _type_check(_: PathBuf) {}

    fn unqualified() -> String {
        format!(
            "screen: refused: computer use is not qualified on {} (v1 is Windows only); a \
             refusal is the honest answer, an empty observation would not be",
            PLATFORM
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let error = tool.observe(None, None).unwrap_err();
        assert!(error.contains("--allow-screen"), "{error}");
        assert!(error.contains("refused"), "{error}");
    }

    #[test]
    fn screen_type_bound_is_enforced_before_any_input() {
        let tool = ScreenTool::new(&root("type"), true);
        let text = "x".repeat(SCREEN_TOOL_MAX_TYPE_CHARS + 1);
        // Reaches the bound check before the platform is consulted, so the
        // refusal is the same on every host.
        if native::QUALIFIED {
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
            .observe(None, Some(CaptureTarget::Desktop))
            .expect("observe a real desktop");
        assert!(observed.starts_with("platform=windows"), "{observed}");
        assert!(observed.contains("windows="), "{observed}");
        assert!(observed.contains("capture: .harness/capture/"), "{observed}");

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
