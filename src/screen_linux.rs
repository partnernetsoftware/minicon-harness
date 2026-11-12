//! Linux computer-use backend: X11 + XTest, resolved entirely with dlopen.
//!
//! This file is the body of `screen::native` on `target_os = "linux"`. It has
//! exactly the same exported function set as the Windows, macOS and fallback
//! backends; `scripts/cu-cfg-check.py` type-checks every configuration so a
//! signature drift cannot hide behind a `cfg`.
//!
//! No `#[link(name = "X11")]`: the six-cell release pack is cross-compiled on
//! one macOS runner with zig, which has no X11 import library. Symbols come
//! from `libX11.so.6` / `libXtst.so.6` at first use. A missing library, a
//! missing `$DISPLAY` or a pure Wayland session is a **named refusal**.

use super::{
    capture_scale, downsample, png, ButtonDispatch, Captured, ClickButton, ControlInfo,
    DisplayInfo, Modifier, PlatformCapabilities, ScrollDispatch, WindowInfo,
    SCREEN_TOOL_MAX_DRAG_STEPS, SCREEN_TOOL_MAX_REGION_PIXELS,
    SCREEN_TOOL_POINTER_TOLERANCE_PX,
};
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::OnceLock;

const X11: &str = "libX11.so.6";
const XTST: &str = "libXtst.so.6";
/// 0.0.10: RandR is queried read-only through the same dlopen helper. No
/// `#[link(name = "Xrandr")]` is introduced, and no CRTC/mode is changed.
const XRANDR: &str = "libXrandr.so.2";
/// The X11 100% baseline; every other scale is a ratio against this DPI.
const X_BASELINE_DPI: f64 = 96.0;
const ZPIXMAP: c_int = 2;
const ANY_PROPERTY_TYPE: u64 = 0;
const ICONIC_STATE: u64 = 3;
const REVERT_TO_PARENT: c_int = 2;
const CURRENT_TIME: u64 = 0;

pub const PLATFORM: &str = "linux";

/// What this cell actually implements. `controls` and `act:control` stay named
/// refusals: X11 has no stable child-widget tree without AT-SPI.
pub const CAPABILITIES: PlatformCapabilities = PlatformCapabilities {
    observe: true,
    controls: false,
    capture: true,
    act_focus: true,
    act_move: true,
    act_click: true,
    act_hover: true,
    act_drag: true,
    act_type: true,
    act_key: true,
    act_scroll: true,
    act_control: false,
};

type FnXOpenDisplay = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type FnXDefaultRootWindow = unsafe extern "C" fn(*mut c_void) -> u64;
type FnXDefaultScreen = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnXDisplayWidth = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type FnXDisplayHeight = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type FnXInternAtom = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> u64;
type FnXGetWindowProperty = unsafe extern "C" fn(
    *mut c_void,
    u64,
    u64,
    i64,
    i64,
    c_int,
    u64,
    *mut u64,
    *mut c_int,
    *mut u64,
    *mut u64,
    *mut *mut u8,
) -> c_int;
type FnXFree = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnXQueryTree = unsafe extern "C" fn(
    *mut c_void,
    u64,
    *mut u64,
    *mut u64,
    *mut *mut u64,
    *mut c_uint,
) -> c_int;
type FnXGetGeometry = unsafe extern "C" fn(
    *mut c_void,
    u64,
    *mut u64,
    *mut c_int,
    *mut c_int,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
) -> c_int;
type FnXTranslateCoordinates = unsafe extern "C" fn(
    *mut c_void,
    u64,
    u64,
    c_int,
    c_int,
    *mut c_int,
    *mut c_int,
    *mut u64,
) -> c_int;
type FnXQueryPointer = unsafe extern "C" fn(
    *mut c_void,
    u64,
    *mut u64,
    *mut u64,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_uint,
) -> c_int;
type FnXWarpPointer = unsafe extern "C" fn(
    *mut c_void,
    u64,
    u64,
    c_int,
    c_int,
    c_uint,
    c_uint,
    c_int,
    c_int,
) -> c_int;
type FnXGetInputFocus = unsafe extern "C" fn(*mut c_void, *mut u64, *mut c_int) -> c_int;
type FnXSetInputFocus =
    unsafe extern "C" fn(*mut c_void, u64, c_int, u64) -> c_int;
type FnXRaiseWindow = unsafe extern "C" fn(*mut c_void, u64) -> c_int;
type FnXMoveResizeWindow =
    unsafe extern "C" fn(*mut c_void, u64, c_int, c_int, c_uint, c_uint) -> c_int;
type FnXFlush = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnXSync = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type FnXSetErrorHandler = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnXGetImage = unsafe extern "C" fn(
    *mut c_void,
    u64,
    c_int,
    c_int,
    c_uint,
    c_uint,
    u64,
    c_int,
) -> *mut c_void;
type FnXGetPixel = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> u64;
type FnXStringToKeysym = unsafe extern "C" fn(*const c_char) -> u64;
type FnXKeysymToKeycode = unsafe extern "C" fn(*mut c_void, u64) -> u8;
type FnXTestFakeButtonEvent =
    unsafe extern "C" fn(*mut c_void, c_uint, c_int, u64) -> c_int;
type FnXTestFakeKeyEvent =
    unsafe extern "C" fn(*mut c_void, c_uint, c_int, u64) -> c_int;
/// 0.0.10 read-only resource queries. `XResourceManagerString` carries the
/// `Xft.dpi` scaling value; `XRRGetScreenResources` is resolved to check that
/// RandR is present. Neither changes a display mode or a DPI setting.
type FnXResourceManagerString = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type FnXRRGetScreenResources = unsafe extern "C" fn(*mut c_void, u64) -> *mut c_void;

/// An X error would otherwise terminate the process through Xlib's default
/// handler (BadWindow is normal when a window closes mid-observation).
unsafe extern "C" fn ignore_x_error(_: *mut c_void, _: *mut c_void) -> c_int {
    0
}

static DISPLAY: OnceLock<Result<usize, String>> = OnceLock::new();

fn display() -> Result<*mut c_void, String> {
    let slot = DISPLAY.get_or_init(|| {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let shown = std::env::var("DISPLAY").unwrap_or_default();
        let open: FnXOpenDisplay = match unsafe { super::dynlib::sym(X11, "XOpenDisplay") } {
            Ok(symbol) => symbol,
            Err(error) => return Err(error),
        };
        let pointer = unsafe { open(std::ptr::null()) };
        if pointer.is_null() {
            return Err(format!(
                "cu: unsupported: no X11 display is reachable (DISPLAY={shown:?}, \
                 XDG_SESSION_TYPE={session:?}); a pure Wayland session without XWayland has no \
                 computer-use backend. No effect was performed."
            ));
        }
        let set_handler: FnXSetErrorHandler =
            match unsafe { super::dynlib::sym(X11, "XSetErrorHandler") } {
                Ok(symbol) => symbol,
                Err(error) => return Err(error),
            };
        unsafe {
            set_handler(ignore_x_error as *mut c_void);
        }
        Ok(pointer as usize)
    });
    match slot {
        Ok(handle) => Ok(*handle as *mut c_void),
        Err(error) => Err(error.clone()),
    }
}

fn sym<T: Copy>(library: &'static str, name: &str) -> Result<T, String> {
    unsafe { super::dynlib::sym::<T>(library, name) }
}

fn root_window(d: *mut c_void) -> Result<u64, String> {
    let f: FnXDefaultRootWindow = sym(X11, "XDefaultRootWindow")?;
    Ok(unsafe { f(d) })
}

fn atom(d: *mut c_void, name: &str) -> Result<u64, String> {
    let f: FnXInternAtom = sym(X11, "XInternAtom")?;
    let c = CString::new(name).map_err(|_| format!("cu: unsupported: bad atom {name:?}"))?;
    Ok(unsafe { f(d, c.as_ptr(), 0) })
}

/// One X property, copied out of Xlib before `XFree`. `format` 32 items are
/// `long`-sized (8 bytes on LP64), not 4 bytes.
fn property(d: *mut c_void, window: u64, name: &str) -> Option<(c_int, Vec<u8>)> {
    let get: FnXGetWindowProperty = sym(X11, "XGetWindowProperty").ok()?;
    let free: FnXFree = sym(X11, "XFree").ok()?;
    let atom_id = atom(d, name).ok()?;
    if atom_id == 0 {
        return None;
    }
    let mut actual_type: u64 = 0;
    let mut format: c_int = 0;
    let mut items: u64 = 0;
    let mut remaining: u64 = 0;
    let mut data: *mut u8 = std::ptr::null_mut();
    let status = unsafe {
        get(
            d,
            window,
            atom_id,
            0,
            0x7fff_ffff,
            0,
            ANY_PROPERTY_TYPE,
            &mut actual_type,
            &mut format,
            &mut items,
            &mut remaining,
            &mut data,
        )
    };
    if status != 0 || data.is_null() || items == 0 {
        if !data.is_null() {
            unsafe { free(data as *mut c_void) };
        }
        return None;
    }
    let width = match format {
        8 => 1usize,
        16 => 2,
        32 => std::mem::size_of::<u64>(),
        _ => 0,
    };
    if width == 0 {
        unsafe { free(data as *mut c_void) };
        return None;
    }
    let length = (items as usize).saturating_mul(width);
    let bytes = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
    unsafe { free(data as *mut c_void) };
    Some((format, bytes))
}

fn text_property(d: *mut c_void, window: u64, name: &str) -> Option<String> {
    let (format, bytes) = property(d, window, name)?;
    if format != 8 {
        return None;
    }
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    let text = String::from_utf8_lossy(&bytes[..end]).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn number_property(d: *mut c_void, window: u64, name: &str) -> Option<u64> {
    let (format, bytes) = property(d, window, name)?;
    if format != 32 || bytes.len() < std::mem::size_of::<u64>() {
        return None;
    }
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[..std::mem::size_of::<u64>()]);
    Some(u64::from_ne_bytes(raw))
}

fn window_title(d: *mut c_void, window: u64) -> String {
    text_property(d, window, "_NET_WM_NAME")
        .or_else(|| text_property(d, window, "WM_NAME"))
        .unwrap_or_default()
}

fn window_class(d: *mut c_void, window: u64) -> String {
    let (format, bytes) = match property(d, window, "WM_CLASS") {
        Some(value) => value,
        None => return "unknown".to_owned(),
    };
    if format != 8 {
        return "unknown".to_owned();
    }
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    let class = String::from_utf8_lossy(&bytes[..end]).trim().to_owned();
    if class.is_empty() {
        "unknown".to_owned()
    } else {
        class
    }
}

fn active_window(d: *mut c_void, root: u64) -> Option<u64> {
    number_property(d, root, "_NET_ACTIVE_WINDOW")
}

fn input_focus(d: *mut c_void) -> Option<u64> {
    let f: FnXGetInputFocus = sym(X11, "XGetInputFocus").ok()?;
    let mut focus: u64 = 0;
    let mut revert: c_int = 0;
    if unsafe { f(d, &mut focus, &mut revert) } == 0 {
        return None;
    }
    // 1 is PointerRoot: no window owns the keyboard.
    (focus > 1).then_some(focus)
}

fn state_atoms(d: *mut c_void, window: u64) -> Vec<u64> {
    let (format, bytes) = match property(d, window, "_NET_WM_STATE") {
        Some(value) => value,
        None => return Vec::new(),
    };
    if format != 32 {
        return Vec::new();
    }
    let width = std::mem::size_of::<u64>();
    bytes
        .chunks(width)
        .filter(|chunk| chunk.len() == width)
        .map(|chunk| {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(chunk);
            u64::from_ne_bytes(raw)
        })
        .collect()
}

fn geometry(d: *mut c_void, window: u64, root: u64) -> Option<(i32, i32, i32, i32)> {
    let geo: FnXGetGeometry = sym(X11, "XGetGeometry").ok()?;
    let mut owner: u64 = 0;
    let mut x: c_int = 0;
    let mut y: c_int = 0;
    let mut width: c_uint = 0;
    let mut height: c_uint = 0;
    let mut border: c_uint = 0;
    let mut depth: c_uint = 0;
    if unsafe {
        geo(
            d,
            window,
            &mut owner,
            &mut x,
            &mut y,
            &mut width,
            &mut height,
            &mut border,
            &mut depth,
        )
    } == 0
    {
        return None;
    }
    let translate: FnXTranslateCoordinates = sym(X11, "XTranslateCoordinates").ok()?;
    let mut abs_x: c_int = 0;
    let mut abs_y: c_int = 0;
    let mut child: u64 = 0;
    if unsafe {
        translate(
            d,
            window,
            root,
            0,
            0,
            &mut abs_x,
            &mut abs_y,
            &mut child,
        )
    } == 0
    {
        abs_x = x;
        abs_y = y;
    }
    Some((abs_x, abs_y, width as i32, height as i32))
}

fn tree(d: *mut c_void, window: u64, out: &mut Vec<u64>, depth: u32) {
    if depth > 3 || out.len() >= 256 {
        return;
    }
    let query: FnXQueryTree = match sym(X11, "XQueryTree") {
        Ok(f) => f,
        Err(_) => return,
    };
    let free: FnXFree = match sym(X11, "XFree") {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut root: u64 = 0;
    let mut parent: u64 = 0;
    let mut children: *mut u64 = std::ptr::null_mut();
    let mut count: c_uint = 0;
    if unsafe { query(d, window, &mut root, &mut parent, &mut children, &mut count) } == 0 {
        return;
    }
    if !children.is_null() {
        let slice = unsafe { std::slice::from_raw_parts(children, count as usize) }.to_vec();
        unsafe { free(children as *mut c_void) };
        for child in slice {
            out.push(child);
            tree(d, child, out, depth + 1);
        }
    }
}

fn client_windows(d: *mut c_void, root: u64) -> Vec<u64> {
    if let Some((format, bytes)) = property(d, root, "_NET_CLIENT_LIST") {
        if format == 32 {
            let width = std::mem::size_of::<u64>();
            let ids: Vec<u64> = bytes
                .chunks(width)
                .filter(|chunk| chunk.len() == width)
                .map(|chunk| {
                    let mut raw = [0u8; 8];
                    raw.copy_from_slice(chunk);
                    u64::from_ne_bytes(raw)
                })
                .filter(|id| *id > 1)
                .collect();
            if !ids.is_empty() {
                return ids;
            }
        }
    }
    // No EWMH window manager (a bare Xvfb session): walk the tree and keep
    // only named clients, so Xlib's internal windows never become "windows".
    let mut candidates = Vec::new();
    tree(d, root, &mut candidates, 0);
    candidates.retain(|id| !window_title(d, *id).is_empty());
    candidates
}

fn flush(d: *mut c_void) -> Result<(), String> {
    let flush: FnXFlush = sym(X11, "XFlush")?;
    let sync: FnXSync = sym(X11, "XSync")?;
    unsafe {
        flush(d);
        sync(d, 0);
    }
    Ok(())
}

pub fn virtual_screen() -> Result<(i32, i32, i32, i32), String> {
    let d = display()?;
    let screen: FnXDefaultScreen = sym(X11, "XDefaultScreen")?;
    let width: FnXDisplayWidth = sym(X11, "XDisplayWidth")?;
    let height: FnXDisplayHeight = sym(X11, "XDisplayHeight")?;
    let index = unsafe { screen(d) };
    let w = unsafe { width(d, index) };
    let h = unsafe { height(d, index) };
    if w <= 0 || h <= 0 {
        return Err("cu: unsupported: the X screen reports no geometry; no effect was performed".to_owned());
    }
    Ok((0, 0, w, h))
}

pub fn displays() -> Result<Vec<DisplayInfo>, String> {
    let (x, y, width, height) = virtual_screen()?;
    // One X screen is reported as one display. RandR/Xinerama geometry is a
    // non-goal; reporting a single real screen is honest, inventing monitors
    // would not be.
    Ok(vec![DisplayInfo {
        x,
        y,
        width,
        height,
        primary: true,
    }])
}

pub fn foreground_title() -> Option<String> {
    let d = display().ok()?;
    let root = root_window(d).ok()?;
    let window = active_window(d, root)
        .or_else(|| input_focus(d))
        .or_else(|| {
            windows()
                .ok()?
                .into_iter()
                .find(|window| window.focused)
                .map(|window| window.handle as u64)
        })?;
    let title = window_title(d, window);
    (!title.is_empty()).then_some(title)
}

/// The coarse session kind, derived from `XDG_SESSION_TYPE` and whether an X11
/// connection is actually reachable. Only the kind leaves this function: the
/// `$DISPLAY` value and any other environment value stay inside it.
fn session_kind(reachable: bool) -> super::SessionKind {
    if !reachable {
        return super::SessionKind::Headless;
    }
    let declared = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if declared == "wayland" {
        super::SessionKind::WaylandX11Compat
    } else {
        super::SessionKind::X11
    }
}

/// Why an X11 probe could not be answered, without publishing `$DISPLAY`.
fn no_display_reason() -> String {
    format!(
        "no X11 display is reachable (session={}); a pure Wayland or headless session has no \
         X11 backend in this cell",
        session_kind(false).tag()
    )
}

/// Read-only environment probe: `XOpenDisplay` is a connection, not an effect,
/// and every geometry call below it is a pure query.
pub fn environment() -> super::EnvironmentProbe {
    let reachable = display().is_ok();
    let desktop = if reachable { virtual_screen().ok() } else { None };
    let display_count = if reachable {
        displays().map(|list| list.len()).unwrap_or(0)
    } else {
        0
    };
    let foreground_present = if reachable {
        foreground_title().is_some()
    } else {
        false
    };
    super::environment_from(
        PLATFORM,
        session_kind(reachable),
        desktop,
        display_count,
        foreground_present,
    )
}

/// Read-only grant probes. X11 has no per-process grant to request: the only
/// precondition is a reachable display, and the only extra mechanism is XTest.
/// A missing library is `unknown` (a fact about the cell), not `denied`.
pub fn grants() -> Vec<super::GrantProbe> {
    let reachable = display().is_ok();
    let xtest = unsafe { super::dynlib::sym::<FnXTestFakeButtonEvent>(XTST, "XTestFakeButtonEvent") };
    let mut rows = Vec::new();
    for name in ["screen_capture", "window_enumeration", "x11_display"] {
        rows.push(if reachable {
            super::GrantProbe::granted(name)
        } else {
            super::GrantProbe::not_applicable(name, no_display_reason())
        });
    }
    rows.push(match (reachable, xtest) {
        (false, _) => super::GrantProbe::not_applicable("input_posting", no_display_reason()),
        (true, Ok(_)) => super::GrantProbe::granted("input_posting"),
        (true, Err(error)) => super::GrantProbe::unknown(
            "input_posting",
            format!("{XTST} does not export XTestFakeButtonEvent ({error})"),
        ),
    });
    rows
}

/// Parse the one scaling value an X resource database can give us. The raw
/// resource string never leaves this function: only a coarse ratio does.
fn xft_dpi(resource_text: &str) -> Option<f64> {
    for line in resource_text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            if key.trim().eq_ignore_ascii_case("Xft.dpi") {
                if let Ok(dpi) = value.trim().parse::<f64>() {
                    if dpi.is_finite() && dpi > 0.0 {
                        return Some(dpi);
                    }
                }
            }
        }
    }
    None
}

/// Read-only display scale (0.0.10). A pure Wayland / headless session has
/// no X11 connection, so both `Xft.dpi` and `XRRScreenResources` are absent;
/// that is a named `unknown`, never a plausible `1`.
fn linux_display_scale(d: *mut c_void) -> Result<super::DisplayScale, String> {
    let resource: FnXResourceManagerString = sym(X11, "XResourceManagerString").map_err(|_| {
        "linux: libX11.so.6 does not export XResourceManagerString, so Xft.dpi cannot be read"
            .to_owned()
    })?;
    let pointer = unsafe { resource(d) };
    let text = if pointer.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    };
    if let Some(dpi) = xft_dpi(&text) {
        let millis = (dpi * 1000.0 / X_BASELINE_DPI).round() as i64;
        return super::DisplayScale::from_millis(millis).ok_or_else(|| {
            "linux: Xft.dpi reports a scale outside the bounded set; no scale is guessed"
                .to_owned()
        });
    }
    // Xft.dpi is absent. RandR resources carry no logical/pixel ratio, so
    // even a readable XRRScreenResources cannot supply a scale. Name both
    // symbols so the reader can tell "not set" from "could not be probed".
    let randr = sym::<FnXRRGetScreenResources>(XRANDR, "XRRGetScreenResources");
    let note = if randr.is_ok() {
        "XRRGetScreenResources is available but carries no logical/pixel ratio"
    } else {
        "libXrandr.so.2 does not export XRRGetScreenResources"
    };
    Err(format!(
        "linux: XResourceManagerString exposes no Xft.dpi value and {note}; no scale can be \
         probed safely"
    ))
}

/// One row per display, in `displays()` order. All reads are queries: the X
/// resource database, a dlsym lookup and the existing screen geometry. No
/// display mode, DPI value or process setting is changed.
pub fn display_scales() -> Vec<super::DisplayScaleProbe> {
    let no_x11 = "linux: a pure Wayland or headless session has no reachable X11 display, so \
                  Xft.dpi (XResourceManagerString) and XRRScreenResources cannot be read";
    let unknown = |index: usize, rect: super::DisplayRect, reason: &str| {
        super::display_scale_row(
            index,
            rect,
            super::ScaleRectBasis::Physical,
            super::DisplayScale::Unknown,
            reason,
        )
    };
    let d = match display() {
        Ok(d) => d,
        Err(_) => return vec![unknown(0, (0, 0, 0, 0), no_x11)],
    };
    let scale = linux_display_scale(d);
    let list = displays().unwrap_or_default();
    if list.is_empty() {
        return vec![unknown(
            0,
            (0, 0, 0, 0),
            "linux: XDisplayWidth/XDisplayHeight reported no screen geometry; Xft.dpi cannot \
             be paired with a rect",
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
                    no_x11,
                ),
                Err(reason) => unknown(index, rect, reason),
            }
        })
        .collect()
}

pub fn windows() -> Result<Vec<WindowInfo>, String> {
    let d = display()?;
    let root = root_window(d)?;
    let active = active_window(d, root);
    let focused = input_focus(d);
    let hidden = atom(d, "_NET_WM_STATE_HIDDEN").ok();
    let max_v = atom(d, "_NET_WM_STATE_MAXIMIZED_VERT").ok();
    let max_h = atom(d, "_NET_WM_STATE_MAXIMIZED_HORZ").ok();
    let mut out = Vec::new();
    for id in client_windows(d, root) {
        let title = window_title(d, id);
        if title.is_empty() {
            continue;
        }
        let (x, y, width, height) = match geometry(d, id, root) {
            Some(value) => value,
            None => continue,
        };
        let states = state_atoms(d, id);
        let iconic = number_property(d, id, "WM_STATE") == Some(ICONIC_STATE);
        let minimized = iconic || hidden.map(|a| states.contains(&a)).unwrap_or(false);
        let maximized = max_v.map(|a| states.contains(&a)).unwrap_or(false)
            || max_h.map(|a| states.contains(&a)).unwrap_or(false);
        let is_focused = active == Some(id) || focused == Some(id);
        out.push(WindowInfo {
            handle: id as u64,
            title,
            class_name: window_class(d, id),
            pid: number_property(d, id, "_NET_WM_PID").unwrap_or(0) as u32,
            x,
            y,
            width,
            height,
            minimized,
            maximized,
            focused: is_focused,
        });
    }
    Ok(out)
}

pub fn window_by_handle(handle: u64) -> Option<WindowInfo> {
    windows().ok()?.into_iter().find(|window| window.handle == handle)
}

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
            "screen: refused: no window title contains {title:?}; observe first to see what is there"
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

pub fn handle_is_live(handle: u64, title: &str) -> Result<(), String> {
    let d = display()?;
    let root = root_window(d)?;
    let id = handle as u64;
    if geometry(d, id, root).is_none() || window_title(d, id) != title {
        return Err(format!(
            "screen: refused: the window {title:?} no longer exists; observe again before \
             operating on it"
        ));
    }
    Ok(())
}

pub fn a11y_controls(
    _handle: u64,
) -> Result<(Vec<crate::screen::A11yControl>, bool, Option<String>), String> {
    Err(
        "cu: unsupported: the structured a11y arm needs the platform's \
         accessibility API (AT-SPI on linux, AX on macos); no tree was \
         produced."
            .to_owned(),
    )
}

pub fn controls(_handle: u64) -> Result<(Vec<ControlInfo>, bool), String> {
    Err(
        "cu: unsupported: observe arm \"controls\" is not implemented by the linux backend; \
         X11 has no child-widget tree without AT-SPI. No control scan was produced."
            .to_owned(),
    )
}

fn check_point(x: i32, y: i32) -> Result<(), String> {
    let (vx, vy, vw, vh) = virtual_screen()?;
    if (x as i64) < vx as i64
        || (y as i64) < vy as i64
        || (x as i64) >= vx as i64 + vw as i64
        || (y as i64) >= vy as i64 + vh as i64
    {
        return Err(format!(
            "screen: refused: {x},{y} is outside the virtual desktop {vx},{vy} {vw}x{vh}"
        ));
    }
    Ok(())
}

fn query_pointer(d: *mut c_void, root: u64) -> Result<(i32, i32), String> {
    let f: FnXQueryPointer = sym(X11, "XQueryPointer")?;
    let mut child_root: u64 = 0;
    let mut child: u64 = 0;
    let mut root_x: c_int = 0;
    let mut root_y: c_int = 0;
    let mut win_x: c_int = 0;
    let mut win_y: c_int = 0;
    let mut mask: c_uint = 0;
    if unsafe {
        f(
            d,
            root,
            &mut child_root,
            &mut child,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        )
    } == 0
    {
        return Err("cu: unsupported: XQueryPointer could not read the pointer; no effect was performed".to_owned());
    }
    Ok((root_x, root_y))
}

fn pointer_readback(want_x: i32, want_y: i32) -> Result<(i32, i32), String> {
    let d = display()?;
    let root = root_window(d)?;
    let (x, y) = query_pointer(d, root)?;
    if (x - want_x).abs() > SCREEN_TOOL_POINTER_TOLERANCE_PX
        || (y - want_y).abs() > SCREEN_TOOL_POINTER_TOLERANCE_PX
    {
        return Err(format!(
            "screen: failed: the pointer is at {x},{y}, not {want_x},{want_y}; the requested \
             position was not reached. Nothing else was attempted"
        ));
    }
    Ok((x, y))
}

fn warp(d: *mut c_void, root: u64, x: i32, y: i32) -> Result<(), String> {
    let f: FnXWarpPointer = sym(X11, "XWarpPointer")?;
    // src_w = 0 means None: absolute warp into the root window.
    unsafe { f(d, 0, root, 0, 0, 0, 0, x, y) };
    flush(d)
}

pub fn place_pointer(x: i32, y: i32) -> Result<(i32, i32), String> {
    check_point(x, y)?;
    let d = display()?;
    let root = root_window(d)?;
    warp(d, root, x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    pointer_readback(x, y)
}

fn button_number(button: ClickButton) -> c_uint {
    match button {
        ClickButton::Left => 1,
        ClickButton::Middle => 2,
        ClickButton::Right => 3,
    }
}

fn button_event(d: *mut c_void, button: c_uint, press: bool) -> Result<(), String> {
    let f: FnXTestFakeButtonEvent = sym(XTST, "XTestFakeButtonEvent")?;
    let sent = unsafe { f(d, button, if press { 1 } else { 0 }, 0) };
    if sent == 0 {
        return Err(format!(
            "cu: unsupported: XTest could not post button {button}; no effect was performed"
        ));
    }
    Ok(())
}

pub fn click(
    x: i32, y: i32, button: ClickButton, double: bool,
) -> Result<(i32, i32, ButtonDispatch), String> {
    let position = place_pointer(x, y)?;
    let d = display()?;
    let number = button_number(button);
    let rounds = if double { 2 } else { 1 };
    let mut downs = 0;
    let mut ups = 0;
    for _ in 0..rounds {
        button_event(d, number, true)?;
        downs += 1;
        button_event(d, number, false)?;
        ups += 1;
        if double {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    }
    flush(d)?;
    Ok((position.0, position.1, ButtonDispatch {
        button: button.tag(), downs, ups, double, path_points: 0,
    }))
}

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
    let d = display()?;
    let root = root_window(d)?;
    let number = button_number(button);
    let mut downs = 0;
    let mut ups = 0;
    button_event(d, number, true)?;
    downs += 1;
    let mut from_x = x as i64;
    let mut from_y = y as i64;
    for &(to_x, to_y) in points {
        for step in 1..=STEPS {
            let px = from_x + (to_x as i64 - from_x) * step / STEPS;
            let py = from_y + (to_y as i64 - from_y) * step / STEPS;
            warp(d, root, px as i32, py as i32)?;
            std::thread::sleep(std::time::Duration::from_millis(12));
        }
        from_x = to_x as i64;
        from_y = to_y as i64;
    }
    button_event(d, number, false)?;
    ups += 1;
    flush(d)?;
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
    let d = display()?;
    // X11 buttons 4/5 are the vertical wheel, 6/7 the horizontal one. Native
    // button numbers never leak into the published evidence.
    for (delta, positive, negative) in [
        (delta_y, 4u32, 5u32),
        (delta_x, 7u32, 6u32),
    ] {
        if delta == 0 {
            continue;
        }
        let number = if delta >= 0 { positive } else { negative };
        let notches = (delta.unsigned_abs() / 120).clamp(1, 12);
        for _ in 0..notches {
            button_event(d, number, true)?;
            button_event(d, number, false)?;
        }
    }
    flush(d)?;
    Ok((position.0, position.1, ScrollDispatch { delta_x, delta_y }))
}

pub fn focus(handle: u64) -> Result<(), String> {
    let d = display()?;
    let raise: FnXRaiseWindow = sym(X11, "XRaiseWindow")?;
    let set: FnXSetInputFocus = sym(X11, "XSetInputFocus")?;
    let id = handle as u64;
    unsafe {
        raise(d, id);
        set(d, id, REVERT_TO_PARENT, CURRENT_TIME);
    }
    flush(d)?;
    if input_focus(d) != Some(id) {
        return Err(format!(
            "screen: failed: the X server did not give focus to window {handle}; the focus was \
             not read back"
        ));
    }
    Ok(())
}

pub fn move_window(handle: u64, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    if width <= 0 || height <= 0 {
        return Err(format!(
            "screen: refused: the requested window size {width}x{height} is not positive"
        ));
    }
    let d = display()?;
    let f: FnXMoveResizeWindow = sym(X11, "XMoveResizeWindow")?;
    unsafe { f(d, handle as u64, x, y, width as c_uint, height as c_uint) };
    flush(d)
}

/// Keysym name for one ASCII character. The boolean is the Shift modifier.
/// Non-ASCII needs an input method and is a named refusal, not a silent drop.
fn keysym_name(character: char) -> Option<(&'static str, bool)> {
    let table: &[(char, &str, bool)] = &[
        (' ', "space", false),
        ('\n', "Return", false),
        ('\t', "Tab", false),
        ('!', "exclam", true),
        ('"', "quotedbl", true),
        ('#', "numbersign", true),
        ('$', "dollar", true),
        ('%', "percent", true),
        ('&', "ampersand", true),
        ('\'', "apostrophe", false),
        ('(', "parenleft", true),
        (')', "parenright", true),
        ('*', "asterisk", true),
        ('+', "plus", true),
        (',', "comma", false),
        ('-', "minus", false),
        ('.', "period", false),
        ('/', "slash", false),
        (':', "colon", true),
        (';', "semicolon", false),
        ('<', "less", true),
        ('=', "equal", false),
        ('>', "greater", true),
        ('?', "question", true),
        ('@', "at", true),
        ('[', "bracketleft", false),
        ('\\', "backslash", false),
        (']', "bracketright", false),
        ('^', "asciicircum", true),
        ('_', "underscore", true),
        ('`', "grave", false),
        ('{', "braceleft", true),
        ('|', "bar", true),
        ('}', "braceright", true),
        ('~', "asciitilde", true),
    ];
    for (candidate, name, shift) in table {
        if *candidate == character {
            return Some((*name, *shift));
        }
    }
    if character.is_ascii_alphanumeric() {
        return Some(("", character.is_ascii_uppercase()));
    }
    None
}

fn keysym_for(d: *mut c_void, character: char) -> Result<(u64, bool), String> {
    let (name, shift) = keysym_name(character).ok_or_else(|| {
        format!(
            "cu: unsupported: character {character:?} has no X11 keysym without an input \
             method; the text was not delivered"
        )
    })?;
    let keysym = if name.is_empty() {
        character as u64
    } else {
        let f: FnXStringToKeysym = sym(X11, "XStringToKeysym")?;
        let c = CString::new(name).map_err(|_| "cu: unsupported: bad keysym name".to_owned())?;
        let value = unsafe { f(c.as_ptr()) };
        if value == 0 {
            return Err(format!(
                "cu: unsupported: X11 has no keysym {name:?}; the text was not delivered"
            ));
        }
        value
    };
    // Uppercase letters use their lowercase keysym plus Shift.
    let keysym = if name.is_empty() && character.is_ascii_uppercase() {
        keysym + 32
    } else {
        keysym
    };
    let _ = d;
    Ok((keysym, shift))
}

fn key_event(d: *mut c_void, code: c_uint, press: bool) -> Result<(), String> {
    let f: FnXTestFakeKeyEvent = sym(XTST, "XTestFakeKeyEvent")?;
    if unsafe { f(d, code, if press { 1 } else { 0 }, 0) } == 0 {
        return Err(format!(
            "cu: unsupported: XTest could not post keycode {code}; no effect was performed"
        ));
    }
    Ok(())
}

fn keycode(d: *mut c_void, keysym: u64, what: &str) -> Result<c_uint, String> {
    let f: FnXKeysymToKeycode = sym(X11, "XKeysymToKeycode")?;
    let code = unsafe { f(d, keysym) };
    if code == 0 {
        return Err(format!(
            "cu: unsupported: X11 has no keycode for {what}; no key was delivered"
        ));
    }
    Ok(code as c_uint)
}

/// X11 keysym names for the four held modifiers. XTest resolves keysyms to
/// keycodes at runtime; an unmapped one is a named `unsupported`, never a
/// guessed keycode (PRD 01.10).
const HELD_MODIFIER_KEYSYMS: [(Modifier, &str); 4] = [
    (Modifier::Ctrl, "Control_L"),
    (Modifier::Shift, "Shift_L"),
    (Modifier::Alt, "Alt_L"),
    (Modifier::Win, "Super_L"),
];

fn modifier_key_event(d: *mut c_void, name: &str, press: bool) -> Result<(), String> {
    let string_to_keysym: FnXStringToKeysym = sym(X11, "XStringToKeysym")?;
    let c_name = CString::new(name).unwrap();
    let keysym = unsafe { string_to_keysym(c_name.as_ptr()) };
    if keysym == 0 {
        return Err(format!(
            "cu: unsupported: X11 has no keysym for {name}; no modifier was held"
        ));
    }
    let code = keycode(d, keysym, name)?;
    key_event(d, code, press)
}

pub fn set_modifier(key: Modifier, down: bool) -> Result<(), String> {
    let d = display()?;
    let name = HELD_MODIFIER_KEYSYMS
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, name)| *name)
        .ok_or_else(|| format!("screen: refused: unknown modifier {key:?}"))?;
    modifier_key_event(d, name, down)?;
    flush(d)
}

pub fn release_all_modifiers() -> Result<(), String> {
    let d = display()?;
    let mut released = 0usize;
    let mut first_error = None;
    for (_, name) in HELD_MODIFIER_KEYSYMS {
        match modifier_key_event(d, name, false) {
            Ok(()) => released += 1,
            Err(error) => { first_error.get_or_insert(error); },
        };
    }
    flush(d)?;
    // An extra key-up for a modifier X11 never mapped is harmless. Only a
    // total failure is a refusal; a partial one still cleared the held set.
    if released == 0 {
        Err(first_error.unwrap_or_else(|| {
            "cu: unsupported: XTest released no modifiers".to_owned()
        }))
    } else {
        Ok(())
    }
}

pub fn type_text(text: &str) -> Result<(), String> {
    let d = display()?;
    let shift_keysym: FnXStringToKeysym = sym(X11, "XStringToKeysym")?;
    let shift_name = CString::new("Shift_L").unwrap();
    let shift_sym = unsafe { shift_keysym(shift_name.as_ptr()) };
    let shift_code = if shift_sym == 0 {
        None
    } else {
        Some(keycode(d, shift_sym, "Shift_L")?)
    };
    for character in text.chars() {
        let (keysym, shift) = keysym_for(d, character)?;
        let code = keycode(d, keysym, &format!("{character:?}"))?;
        if shift {
            let shift = shift_code.ok_or_else(|| {
                "cu: unsupported: X11 has no Shift keysym; the shifted character was not delivered"
                    .to_owned()
            })?;
            key_event(d, shift, true)?;
        }
        key_event(d, code, true)?;
        key_event(d, code, false)?;
        if shift {
            key_event(d, shift_code.unwrap(), false)?;
        }
    }
    flush(d)
}

const MODIFIERS: &[(&str, &str)] = &[
    ("ctrl", "Control_L"),
    ("control", "Control_L"),
    ("shift", "Shift_L"),
    ("alt", "Alt_L"),
    ("super", "Super_L"),
    ("meta", "Alt_L"),
];

const NAMED_KEYS: &[(&str, &str)] = &[
    ("enter", "Return"),
    ("return", "Return"),
    ("tab", "Tab"),
    ("escape", "Escape"),
    ("esc", "Escape"),
    ("backspace", "BackSpace"),
    ("delete", "Delete"),
    ("space", "space"),
    ("up", "Up"),
    ("down", "Down"),
    ("left", "Left"),
    ("right", "Right"),
    ("home", "Home"),
    ("end", "End"),
    ("f1", "F1"),
    ("f2", "F2"),
    ("f3", "F3"),
    ("f4", "F4"),
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
    let d = display()?;
    let string_to_keysym: FnXStringToKeysym = sym(X11, "XStringToKeysym")?;
    let mut modifier_codes = Vec::new();
    for modifier in modifiers {
        let name = MODIFIERS
            .iter()
            .find(|(alias, _)| alias == modifier)
            .map(|(_, name)| *name)
            .ok_or_else(|| format!("screen: refused: unknown modifier {modifier:?}"))?;
        let c = CString::new(name).unwrap();
        let keysym = unsafe { string_to_keysym(c.as_ptr()) };
        modifier_codes.push(keycode(d, keysym, name)?);
    }
    let key_name = NAMED_KEYS
        .iter()
        .find(|(alias, _)| alias == key)
        .map(|(_, name)| *name);
    let keysym = match key_name {
        Some(name) => {
            let c = CString::new(name).unwrap();
            unsafe { string_to_keysym(c.as_ptr()) }
        }
        None => {
            let mut chars = key.chars();
            let only = chars
                .next()
                .filter(|_| chars.next().is_none())
                .ok_or_else(|| format!("screen: refused: unknown key {key:?}"))?;
            let (keysym, _) = keysym_for(d, only)?;
            keysym
        }
    };
    if keysym == 0 {
        return Err(format!("screen: refused: unknown key {key:?}"));
    }
    let code = keycode(d, keysym, key)?;
    for modifier in &modifier_codes {
        key_event(d, *modifier, true)?;
    }
    key_event(d, code, true)?;
    key_event(d, code, false)?;
    for modifier in modifier_codes.iter().rev() {
        key_event(d, *modifier, false)?;
    }
    flush(d)
}

/// `XDestroyImage` is a C macro that dispatches through `XImage::f`, not an
/// exported symbol. Read the `data` pointer from the stable XImage prefix and
/// free the same two allocations the macro would free.
#[repr(C)]
struct XImageHead {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut c_void,
}

fn grab(x: i32, y: i32, width: i32, height: i32, scale: u32) -> Result<Captured, String> {
    if width <= 0 || height <= 0 {
        return Err(format!(
            "screen: refused: the region is {width}x{height}; nothing to capture"
        ));
    }
    if width > SCREEN_TOOL_MAX_REGION_PIXELS || height > SCREEN_TOOL_MAX_REGION_PIXELS {
        return Err(format!(
            "screen: refused: the region is {width}x{height}, over the \
             {SCREEN_TOOL_MAX_REGION_PIXELS} pixel bound on each side"
        ));
    }
    let (vx, vy, vw, vh) = virtual_screen()?;
    if x < vx || y < vy || x as i64 + width as i64 > vx as i64 + vw as i64
        || y as i64 + height as i64 > vy as i64 + vh as i64
    {
        return Err(format!(
            "screen: refused: region {x},{y} {width}x{height} is outside the X screen \
             {vx},{vy} {vw}x{vh}; XGetImage would fail with BadMatch"
        ));
    }
    let d = display()?;
    let root = root_window(d)?;
    let get_image: FnXGetImage = sym(X11, "XGetImage")?;
    let get_pixel: FnXGetPixel = sym(X11, "XGetPixel")?;
    let free: FnXFree = sym(X11, "XFree")?;
    let image = unsafe {
        get_image(
            d,
            root,
            x,
            y,
            width as c_uint,
            height as c_uint,
            u64::MAX,
            ZPIXMAP,
        )
    };
    if image.is_null() {
        return Err(format!(
            "cu: unsupported: XGetImage returned no pixels for {x},{y} {width}x{height}; \
             Screen Recording / a compositor grant may be missing. No image was produced."
        ));
    }
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    for row in 0..height {
        for column in 0..width {
            // TrueColor ZPixmap on little-endian LP64: memory bytes are BGRA,
            // so the unsigned long reads back as A<<24 | R<<16 | G<<8 | B.
            let value = unsafe { get_pixel(image, column, row) };
            let offset = (row as usize * width as usize + column as usize) * 4;
            pixels[offset] = (value & 0xff) as u8;
            pixels[offset + 1] = ((value >> 8) & 0xff) as u8;
            pixels[offset + 2] = ((value >> 16) & 0xff) as u8;
            pixels[offset + 3] = 0xff;
        }
    }
    let data = unsafe { (*(image as *const XImageHead)).data };
    unsafe {
        if !data.is_null() {
            free(data);
        }
        free(image);
    }
    let chosen = capture_scale(width as u32, height as u32, scale);
    let (pixels, out_width, out_height) =
        downsample(&pixels, width as u32, height as u32, chosen);
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
    let d = display()?;
    let root = root_window(d)?;
    let (x, y, width, height) = geometry(d, handle as u64, root).ok_or_else(|| {
        "screen: refused: the window has no readable rectangle".to_owned()
    })?;
    grab(x, y, width, height, scale)
}

pub fn now_millis() -> u128 {
    // The sequence counter in `capture()` disambiguates two captures in the
    // same millisecond.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}
