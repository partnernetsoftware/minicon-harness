//! macOS computer-use backend: CoreGraphics / CoreFoundation, resolved
//! entirely with dlopen.
//!
//! This file is the body of `screen::native` on `target_os = "macos"`. It has
//! exactly the same exported function set as the Windows, Linux and fallback
//! backends; `scripts/cu-cfg-check.py` type-checks every configuration.
//!
//! No framework link: the six-cell release pack is cross-compiled on one macOS
//! runner, but the Darwin cells must still be buildable without an SDK link
//! step. Symbols come from the CoreGraphics / CoreFoundation binaries at first
//! use. A missing **OS grant** (Screen Recording, Accessibility/Input
//! Monitoring) is a *named permission refusal* -- never a black PNG and never
//! a silent no-op.

use super::{
    capture_scale, downsample, png, ButtonDispatch, Captured, ClickButton, ControlInfo,
    DisplayInfo, PlatformCapabilities, ScrollDispatch, WindowInfo,
    SCREEN_TOOL_MAX_DRAG_STEPS, SCREEN_TOOL_MAX_REGION_PIXELS,
    SCREEN_TOOL_POINTER_TOLERANCE_PX,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};

const CG: &str = "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics";
const CF: &str = "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation";

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64: isize = 4;
const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1;
const K_CG_NULL_WINDOW_ID: u32 = 0;
const K_CG_HID_EVENT_TAP: u32 = 0;

const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
const K_CG_EVENT_OTHER_MOUSE_DRAGGED: u32 = 27;

const K_CG_SCROLL_EVENT_UNIT_LINE: u32 = 1;
const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x0002_0000;
const K_CG_EVENT_FLAG_MASK_CONTROL: u64 = 0x0004_0000;
const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 0x0008_0000;
const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;

pub const PLATFORM: &str = "macos";

/// `act:focus` and `act:move` are deliberately not implemented: moving or
/// raising another process's window needs the Accessibility API and an AX
/// grant, not CoreGraphics. `controls`/`act:control` need an AX tree.
pub const CAPABILITIES: PlatformCapabilities = PlatformCapabilities {
    observe: true,
    controls: false,
    capture: true,
    act_focus: false,
    act_move: false,
    act_click: true,
    act_hover: true,
    act_drag: true,
    act_type: true,
    act_key: true,
    act_scroll: true,
    act_control: false,
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

type FnCGMainDisplayID = unsafe extern "C" fn() -> u32;
type FnCGGetActiveDisplayList =
    unsafe extern "C" fn(u32, *mut u32, *mut u32) -> c_int;
type FnCGDisplayBounds = unsafe extern "C" fn(u32) -> CGRect;
type FnCGWindowListCopyWindowInfo = unsafe extern "C" fn(u32, u32) -> *const c_void;
type FnCGRectMakeWithDictionaryRepresentation =
    unsafe extern "C" fn(*const c_void, *mut CGRect) -> u8;
type FnCGDisplayCreateImage = unsafe extern "C" fn(u32) -> *const c_void;
type FnCGImageGetWidth = unsafe extern "C" fn(*const c_void) -> usize;
type FnCGImageGetHeight = unsafe extern "C" fn(*const c_void) -> usize;
type FnCGImageGetBytesPerRow = unsafe extern "C" fn(*const c_void) -> usize;
type FnCGImageGetBitsPerPixel = unsafe extern "C" fn(*const c_void) -> usize;
type FnCGImageGetBitmapInfo = unsafe extern "C" fn(*const c_void) -> u32;
type FnCGImageGetDataProvider = unsafe extern "C" fn(*const c_void) -> *const c_void;
type FnCGDataProviderCopyData = unsafe extern "C" fn(*const c_void) -> *const c_void;
type FnCGEventCreate = unsafe extern "C" fn(*const c_void) -> *const c_void;
type FnCGEventGetLocation = unsafe extern "C" fn(*const c_void) -> CGPoint;
type FnCGWarpMouseCursorPosition = unsafe extern "C" fn(CGPoint) -> c_int;
type FnCGEventCreateMouseEvent =
    unsafe extern "C" fn(*const c_void, u32, CGPoint, u32) -> *const c_void;
type FnCGEventCreateKeyboardEvent =
    unsafe extern "C" fn(*const c_void, u16, u8) -> *const c_void;
type FnCGEventKeyboardSetUnicodeString =
    unsafe extern "C" fn(*const c_void, usize, *const u16);
type FnCGEventSetFlags = unsafe extern "C" fn(*const c_void, u64);
type FnCGEventPost = unsafe extern "C" fn(u32, *const c_void);
// The variadic CGEventCreateScrollWheelEvent is not ABI-safe to call through
// a fixed Rust fn pointer on arm64. ...2 is the non-variadic six-argument form.
type FnCGEventCreateScrollWheelEvent2 =
    unsafe extern "C" fn(*const c_void, u32, u32, i32, i32, i32) -> *const c_void;
type FnCFStringCreateWithCString =
    unsafe extern "C" fn(*const c_void, *const c_char, u32) -> *const c_void;
type FnCGPreflightScreenCaptureAccess = unsafe extern "C" fn() -> u8;
type FnCGPreflightPostEventAccess = unsafe extern "C" fn() -> u8;
type FnCFArrayGetCount = unsafe extern "C" fn(*const c_void) -> isize;
type FnCFArrayGetValueAtIndex =
    unsafe extern "C" fn(*const c_void, isize) -> *const c_void;
type FnCFDictionaryGetValue =
    unsafe extern "C" fn(*const c_void, *const c_void) -> *const c_void;
type FnCFStringGetCString =
    unsafe extern "C" fn(*const c_void, *mut c_char, isize, u32) -> u8;
type FnCFNumberGetValue = unsafe extern "C" fn(*const c_void, isize, *mut c_void) -> u8;
type FnCFDataGetBytePtr = unsafe extern "C" fn(*const c_void) -> *const u8;
type FnCFDataGetLength = unsafe extern "C" fn(*const c_void) -> isize;
type FnCFRelease = unsafe extern "C" fn(*const c_void);
type FnCFGetTypeID = unsafe extern "C" fn(*const c_void) -> usize;
type FnCFStringGetTypeID = unsafe extern "C" fn() -> usize;
type FnCFNumberGetTypeID = unsafe extern "C" fn() -> usize;

fn sym<T: Copy>(library: &'static str, name: &str) -> Result<T, String> {
    unsafe { super::dynlib::sym::<T>(library, name) }
}

/// Build a CFString dictionary key from its literal name.
///
/// `kCGWindowName` and friends are exported *data* symbols holding a
/// `CFStringRef`. Dereferencing a dlsym data symbol is fragile across macOS
/// SDK/dyld versions, so the backend creates an equal CFString instead; the
/// CG window dictionaries use `kCFTypeDictionaryKeyCallBacks` (CFEqual), so
/// the literal keys match the constants. A NULL allocator is the documented
/// default allocator.
fn cf_key(name: &str) -> Result<*const c_void, String> {
    let create: FnCFStringCreateWithCString = sym(CF, "CFStringCreateWithCString")?;
    let c = CString::new(name).map_err(|_| format!("cu: unsupported: bad key {name:?}"))?;
    let key = unsafe { create(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) };
    if key.is_null() {
        return Err(format!(
            "cu: unsupported: CoreFoundation could not create key {name:?}; no effect was performed"
        ));
    }
    Ok(key)
}

fn cf_type_is(value: *const c_void, expected: &str) -> bool {
    if value.is_null() {
        return false;
    }
    let get: FnCFGetTypeID = match sym(CF, "CFGetTypeID") {
        Ok(f) => f,
        Err(_) => return true,
    };
    let wanted: usize = match expected {
        "CFString" => match sym::<FnCFStringGetTypeID>(CF, "CFStringGetTypeID") {
            Ok(f) => unsafe { f() },
            Err(_) => return true,
        },
        "CFNumber" => match sym::<FnCFNumberGetTypeID>(CF, "CFNumberGetTypeID") {
            Ok(f) => unsafe { f() },
            Err(_) => return true,
        },
        _ => return false,
    };
    wanted != 0 && unsafe { get(value) } == wanted
}

fn cf_string(value: *const c_void) -> Option<String> {
    if !cf_type_is(value, "CFString") {
        return None;
    }
    let get: FnCFStringGetCString = sym(CF, "CFStringGetCString").ok()?;
    let mut buffer = [0 as c_char; 1024];
    let ok = unsafe {
        get(
            value,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if ok == 0 {
        return None;
    }
    let text = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .trim()
        .to_owned();
    (!text.is_empty()).then_some(text)
}

fn cf_number(value: *const c_void) -> Option<i64> {
    if !cf_type_is(value, "CFNumber") {
        return None;
    }
    let get: FnCFNumberGetValue = sym(CF, "CFNumberGetValue").ok()?;
    let mut out: i64 = 0;
    let ok = unsafe {
        get(
            value,
            K_CF_NUMBER_SINT64,
            &mut out as *mut i64 as *mut c_void,
        )
    };
    (ok != 0).then_some(out)
}

fn release(value: *const c_void) {
    if value.is_null() {
        return;
    }
    if let Ok(f) = sym::<FnCFRelease>(CF, "CFRelease") {
        unsafe { f(value) };
    }
}

fn dictionary_value(dictionary: *const c_void, key: *const c_void) -> *const c_void {
    if dictionary.is_null() || key.is_null() {
        return std::ptr::null();
    }
    let get: FnCFDictionaryGetValue = match sym(CF, "CFDictionaryGetValue") {
        Ok(f) => f,
        Err(_) => return std::ptr::null(),
    };
    unsafe { get(dictionary, key) }
}

fn permission_refusal(what: &str, grant: &str) -> String {
    format!(
        "cu: unsupported: permission:{grant} is not granted on this macOS session, so {what} \
         cannot be performed; the harness never prompts for permission and reports a named \
         boundary instead of a black image or a silent no-op"
    )
}

fn screen_grant() -> Result<(), String> {
    let preflight: FnCGPreflightScreenCaptureAccess =
        match sym(CG, "CGPreflightScreenCaptureAccess") {
            Ok(f) => f,
            // Older SDKs lack the preflight; CGDisplayCreateImage still reports
            // a null image when the grant is missing.
            Err(_) => return Ok(()),
        };
    if unsafe { preflight() } == 0 {
        return Err(permission_refusal("capture", "ScreenRecording"));
    }
    Ok(())
}

fn input_grant() -> Result<(), String> {
    let preflight: FnCGPreflightPostEventAccess = match sym(CG, "CGPreflightPostEventAccess") {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    if unsafe { preflight() } == 0 {
        return Err(permission_refusal("act", "InputMonitoring"));
    }
    Ok(())
}

fn display_ids() -> Result<Vec<u32>, String> {
    let list: FnCGGetActiveDisplayList = sym(CG, "CGGetActiveDisplayList")?;
    let mut ids = [0u32; 16];
    let mut count: u32 = 0;
    let status = unsafe { list(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
    if status != 0 || count == 0 {
        return Err(
            "cu: unsupported: CGGetActiveDisplayList reported no active display; no effect was \
             performed"
                .to_owned(),
        );
    }
    Ok(ids[..count as usize].to_vec())
}

pub fn virtual_screen() -> Result<(i32, i32, i32, i32), String> {
    let bounds: FnCGDisplayBounds = sym(CG, "CGDisplayBounds")?;
    let displays = display_ids()?;
    let mut left = i64::MAX;
    let mut top = i64::MAX;
    let mut right = i64::MIN;
    let mut bottom = i64::MIN;
    for id in displays {
        let rect = unsafe { bounds(id) };
        let x = rect.origin.x.floor() as i64;
        let y = rect.origin.y.floor() as i64;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + rect.size.width.ceil() as i64);
        bottom = bottom.max(y + rect.size.height.ceil() as i64);
    }
    if right <= left || bottom <= top {
        return Err(
            "cu: unsupported: the displays report no geometry; no effect was performed".to_owned(),
        );
    }
    Ok((left as i32, top as i32, (right - left) as i32, (bottom - top) as i32))
}

pub fn displays() -> Result<Vec<DisplayInfo>, String> {
    let bounds: FnCGDisplayBounds = sym(CG, "CGDisplayBounds")?;
    let main: FnCGMainDisplayID = sym(CG, "CGMainDisplayID")?;
    let main_id = unsafe { main() };
    Ok(display_ids()?
        .into_iter()
        .map(|id| {
            let rect = unsafe { bounds(id) };
            DisplayInfo {
                x: rect.origin.x.floor() as i32,
                y: rect.origin.y.floor() as i32,
                width: rect.size.width.ceil() as i32,
                height: rect.size.height.ceil() as i32,
                primary: id == main_id,
            }
        })
        .collect())
}

struct Entry {
    handle: u64,
    title: String,
    class_name: String,
    pid: u32,
    rect: CGRect,
    focused: bool,
}

fn window_entries() -> Result<Vec<Entry>, String> {
    let copy: FnCGWindowListCopyWindowInfo = sym(CG, "CGWindowListCopyWindowInfo")?;
    let array = unsafe {
        copy(
            K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY,
            K_CG_NULL_WINDOW_ID,
        )
    };
    if array.is_null() {
        return Err(
            "cu: unsupported: CGWindowListCopyWindowInfo returned no window list; no effect was \
             performed"
                .to_owned(),
        );
    }
    let key_number = cf_key("kCGWindowNumber").ok();
    let key_name = cf_key("kCGWindowName").ok();
    let key_owner = cf_key("kCGWindowOwnerName").ok();
    let key_pid = cf_key("kCGWindowOwnerPID").ok();
    let key_bounds = cf_key("kCGWindowBounds").ok();
    let key_layer = cf_key("kCGWindowLayer").ok();
    let count_f: FnCFArrayGetCount = sym(CF, "CFArrayGetCount")?;
    let at_f: FnCFArrayGetValueAtIndex = sym(CF, "CFArrayGetValueAtIndex")?;
    let dict_rect: FnCGRectMakeWithDictionaryRepresentation =
        sym(CG, "CGRectMakeWithDictionaryRepresentation")?;
    let count = unsafe { count_f(array) };
    let mut out = Vec::new();
    let mut frontmost_taken = false;
    for index in 0..count.max(0) {
        let dictionary = unsafe { at_f(array, index) };
        if dictionary.is_null() {
            continue;
        }
        let number = key_number
            .and_then(|key| cf_number(dictionary_value(dictionary, key)))
            .unwrap_or(0);
        if number <= 0 {
            continue;
        }
        let layer = key_layer
            .and_then(|key| cf_number(dictionary_value(dictionary, key)))
            .unwrap_or(-1);
        let owner = key_owner
            .and_then(|key| cf_string(dictionary_value(dictionary, key)))
            .unwrap_or_else(|| "unknown".to_owned());
        // Without Screen Recording, kCGWindowName is withheld for other apps.
        // The owner name is a real, readable title-ish label; using it keeps
        // `find` usable without inventing a window name.
        let title = key_name
            .and_then(|key| cf_string(dictionary_value(dictionary, key)))
            .unwrap_or_else(|| owner.clone());
        let pid = key_pid
            .and_then(|key| cf_number(dictionary_value(dictionary, key)))
            .unwrap_or(0) as u32;
        let mut rect = CGRect::default();
        if let Some(key) = key_bounds {
            let value = dictionary_value(dictionary, key);
            if !value.is_null() && unsafe { dict_rect(value, &mut rect) } == 0 {
                rect = CGRect::default();
            }
        }
        if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            continue;
        }
        // CGWindowListCopyWindowInfo is front-to-back. The first on-screen,
        // layer-0 window is the frontmost application window.
        let focused = layer == 0 && !frontmost_taken;
        if focused {
            frontmost_taken = true;
        }
        out.push(Entry {
            handle: number as u64,
            title,
            class_name: owner,
            pid,
            rect,
            focused,
        });
    }
    release(array);
    for key in [key_number, key_name, key_owner, key_pid, key_bounds, key_layer]
        .into_iter()
        .flatten()
    {
        release(key);
    }
    Ok(out)
}

pub fn windows() -> Result<Vec<WindowInfo>, String> {
    Ok(window_entries()?
        .into_iter()
        .map(|entry| WindowInfo {
            handle: entry.handle,
            title: entry.title,
            class_name: entry.class_name,
            pid: entry.pid,
            x: entry.rect.origin.x.floor() as i32,
            y: entry.rect.origin.y.floor() as i32,
            width: entry.rect.size.width.ceil() as i32,
            height: entry.rect.size.height.ceil() as i32,
            minimized: false,
            maximized: false,
            focused: entry.focused,
        })
        .collect())
}

pub fn foreground_title() -> Option<String> {
    window_entries()
        .ok()?
        .into_iter()
        .find(|entry| entry.focused)
        .map(|entry| entry.title)
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
    let exists = window_entries()?
        .into_iter()
        .any(|entry| entry.handle == handle && entry.title == title);
    if !exists {
        return Err(format!(
            "screen: refused: the window {title:?} no longer exists; observe again before \
             operating on it"
        ));
    }
    Ok(())
}

pub fn controls(_handle: u64) -> Result<(Vec<ControlInfo>, bool), String> {
    Err(
        "cu: unsupported: observe arm \"controls\" is not implemented by the macos backend; \
         an AX tree needs Accessibility and a separate design. No control scan was produced."
            .to_owned(),
    )
}

pub fn focus(_handle: u64) -> Result<(), String> {
    Err(
        "cu: unsupported: capability \"act:focus\" is not implemented by the macos backend; \
         raising another process's window needs the Accessibility API. No effect was performed."
            .to_owned(),
    )
}

pub fn move_window(
    _handle: u64,
    _x: i32,
    _y: i32,
    _width: i32,
    _height: i32,
) -> Result<(), String> {
    Err(
        "cu: unsupported: capability \"act:move\" is not implemented by the macos backend; \
         moving another process's window needs the Accessibility API. No effect was performed."
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

fn pointer_position() -> Result<(i32, i32), String> {
    let create: FnCGEventCreate = sym(CG, "CGEventCreate")?;
    let location: FnCGEventGetLocation = sym(CG, "CGEventGetLocation")?;
    let event = unsafe { create(std::ptr::null()) };
    if event.is_null() {
        return Err(
            "cu: unsupported: CGEventCreate could not read the pointer; no effect was performed"
                .to_owned(),
        );
    }
    let point = unsafe { location(event) };
    release(event);
    Ok((point.x.round() as i32, point.y.round() as i32))
}

fn pointer_readback(want_x: i32, want_y: i32) -> Result<(i32, i32), String> {
    let (x, y) = pointer_position()?;
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

fn warp(x: i32, y: i32) -> Result<(), String> {
    let f: FnCGWarpMouseCursorPosition = sym(CG, "CGWarpMouseCursorPosition")?;
    let status = unsafe {
        f(CGPoint {
            x: x as f64,
            y: y as f64,
        })
    };
    if status != 0 {
        return Err(format!(
            "cu: unsupported: CGWarpMouseCursorPosition failed with {status}; no effect was \
             performed"
        ));
    }
    Ok(())
}

pub fn place_pointer(x: i32, y: i32) -> Result<(i32, i32), String> {
    check_point(x, y)?;
    warp(x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    pointer_readback(x, y)
}

fn post_mouse(kind: u32, x: i32, y: i32, button: u32) -> Result<(), String> {
    input_grant()?;
    let create: FnCGEventCreateMouseEvent = sym(CG, "CGEventCreateMouseEvent")?;
    let post: FnCGEventPost = sym(CG, "CGEventPost")?;
    let event = unsafe {
        create(
            std::ptr::null(),
            kind,
            CGPoint {
                x: x as f64,
                y: y as f64,
            },
            button,
        )
    };
    if event.is_null() {
        return Err(format!(
            "cu: unsupported: CGEventCreateMouseEvent failed for type {kind}; no effect was \
             performed"
        ));
    }
    unsafe { post(K_CG_HID_EVENT_TAP, event) };
    release(event);
    Ok(())
}

fn mouse_pair(button: ClickButton) -> (u32, u32, u32) {
    match button {
        ClickButton::Left => (0, K_CG_EVENT_LEFT_MOUSE_DOWN, K_CG_EVENT_LEFT_MOUSE_UP),
        ClickButton::Right => (1, K_CG_EVENT_RIGHT_MOUSE_DOWN, K_CG_EVENT_RIGHT_MOUSE_UP),
        ClickButton::Middle => (
            2,
            K_CG_EVENT_OTHER_MOUSE_DOWN,
            K_CG_EVENT_OTHER_MOUSE_UP,
        ),
    }
}

pub fn click(
    x: i32, y: i32, button: ClickButton, double: bool,
) -> Result<(i32, i32, ButtonDispatch), String> {
    let position = place_pointer(x, y)?;
    let (number, down, up) = mouse_pair(button);
    let rounds = if double { 2 } else { 1 };
    let mut downs = 0;
    let mut ups = 0;
    for _ in 0..rounds {
        post_mouse(down, x, y, number)?;
        downs += 1;
        post_mouse(up, x, y, number)?;
        ups += 1;
        if double {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    }
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
    let (number, down, up) = mouse_pair(button);
    let dragged = match button {
        ClickButton::Left => K_CG_EVENT_LEFT_MOUSE_DRAGGED,
        ClickButton::Right => K_CG_EVENT_RIGHT_MOUSE_DRAGGED,
        ClickButton::Middle => K_CG_EVENT_OTHER_MOUSE_DRAGGED,
    };
    let mut downs = 0;
    let mut ups = 0;
    post_mouse(down, x, y, number)?;
    downs += 1;
    let mut from_x = x as i64;
    let mut from_y = y as i64;
    for &(to_x, to_y) in points {
        for step in 1..=STEPS {
            let px = from_x + (to_x as i64 - from_x) * step / STEPS;
            let py = from_y + (to_y as i64 - from_y) * step / STEPS;
            post_mouse(dragged, px as i32, py as i32, number)?;
            std::thread::sleep(std::time::Duration::from_millis(12));
        }
        from_x = to_x as i64;
        from_y = to_y as i64;
    }
    post_mouse(up, last_x, last_y, number)?;
    ups += 1;
    warp(last_x, last_y)?;
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
    input_grant()?;
    // ...2 is the non-variadic six-argument form; a fixed Rust fn pointer must
    // never call the C variadic CGEventCreateScrollWheelEvent on arm64.
    let create: FnCGEventCreateScrollWheelEvent2 = sym(CG, "CGEventCreateScrollWheelEvent2")?;
    let post: FnCGEventPost = sym(CG, "CGEventPost")?;
    let notches = |delta: i32| -> i32 {
        let value = (delta / 120).clamp(-12, 12);
        if value == 0 { delta.signum() } else { value }
    };
    let event = unsafe {
        create(
            std::ptr::null(),
            K_CG_SCROLL_EVENT_UNIT_LINE,
            2, // wheelCount: wheel1 = vertical, wheel2 = horizontal
            notches(delta_y),
            notches(delta_x),
            0,
        )
    };
    if event.is_null() {
        return Err(
            "cu: unsupported: CGEventCreateScrollWheelEvent2 failed; no effect was performed"
                .to_owned(),
        );
    }
    unsafe { post(K_CG_HID_EVENT_TAP, event) };
    release(event);
    Ok((position.0, position.1, ScrollDispatch { delta_x, delta_y }))
}

fn post_key(code: u16, down: bool, flags: u64, units: &[u16]) -> Result<(), String> {
    input_grant()?;
    let create: FnCGEventCreateKeyboardEvent = sym(CG, "CGEventCreateKeyboardEvent")?;
    let set_flags: FnCGEventSetFlags = sym(CG, "CGEventSetFlags")?;
    let set_unicode: FnCGEventKeyboardSetUnicodeString =
        sym(CG, "CGEventKeyboardSetUnicodeString")?;
    let post: FnCGEventPost = sym(CG, "CGEventPost")?;
    let event = unsafe {
        create(
            std::ptr::null(),
            code,
            if down { 1 } else { 0 },
        )
    };
    if event.is_null() {
        return Err(
            "cu: unsupported: CGEventCreateKeyboardEvent failed; no key was delivered".to_owned(),
        );
    }
    unsafe {
        set_flags(event, flags);
        if !units.is_empty() {
            set_unicode(event, units.len(), units.as_ptr());
        }
        post(K_CG_HID_EVENT_TAP, event);
    }
    release(event);
    Ok(())
}

pub fn type_text(text: &str) -> Result<(), String> {
    // CGEventKeyboardSetUnicodeString delivers any Unicode scalar without a
    // keyboard-layout keycode map. That is why `act:type` is not ASCII-only on
    // this backend.
    for character in text.chars() {
        let mut buffer = [0u16; 2];
        let units = character.encode_utf16(&mut buffer);
        post_key(0, true, 0, units)?;
        post_key(0, false, 0, units)?;
    }
    Ok(())
}

const KEYCODES: &[(&str, u16)] = &[
    ("a", 0), ("s", 1), ("d", 2), ("f", 3), ("h", 4), ("g", 5),
    ("z", 6), ("x", 7), ("c", 8), ("v", 9), ("b", 11), ("q", 12),
    ("w", 13), ("e", 14), ("r", 15), ("y", 16), ("t", 17), ("1", 18),
    ("2", 19), ("3", 20), ("4", 21), ("6", 22), ("5", 23), ("=", 24),
    ("9", 25), ("7", 26), ("-", 27), ("8", 28), ("0", 29), ("o", 31),
    ("u", 32), ("i", 34), ("p", 35), ("l", 37), ("j", 38), ("k", 40),
    ("n", 45), ("m", 46), ("return", 36), ("enter", 36), ("tab", 48),
    ("space", 49), ("backspace", 51), ("escape", 53), ("esc", 53),
    ("up", 126), ("down", 125), ("left", 123), ("right", 124),
    ("delete", 117), ("home", 115), ("end", 119),
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
    let mut flags = 0u64;
    for modifier in modifiers {
        flags |= match modifier.as_str() {
            "shift" => K_CG_EVENT_FLAG_MASK_SHIFT,
            "ctrl" | "control" => K_CG_EVENT_FLAG_MASK_CONTROL,
            "alt" | "option" => K_CG_EVENT_FLAG_MASK_ALTERNATE,
            "cmd" | "command" | "super" | "meta" => K_CG_EVENT_FLAG_MASK_COMMAND,
            other => return Err(format!("screen: refused: unknown modifier {other:?}")),
        };
    }
    let code = KEYCODES
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, code)| *code)
        .ok_or_else(|| format!("screen: refused: unknown key {key:?}"))?;
    post_key(code, true, flags, &[])?;
    post_key(code, false, flags, &[])?;
    Ok(())
}

/// The main display's pixels as 32-bit BGRA, top-down, with its origin.
fn full_screen() -> Result<(Vec<u8>, usize, usize, i32, i32), String> {
    screen_grant()?;
    let main: FnCGMainDisplayID = sym(CG, "CGMainDisplayID")?;
    let create: FnCGDisplayCreateImage = sym(CG, "CGDisplayCreateImage")?;
    let width_f: FnCGImageGetWidth = sym(CG, "CGImageGetWidth")?;
    let height_f: FnCGImageGetHeight = sym(CG, "CGImageGetHeight")?;
    let stride_f: FnCGImageGetBytesPerRow = sym(CG, "CGImageGetBytesPerRow")?;
    let bpp_f: FnCGImageGetBitsPerPixel = sym(CG, "CGImageGetBitsPerPixel")?;
    let info_f: FnCGImageGetBitmapInfo = sym(CG, "CGImageGetBitmapInfo")?;
    let provider_f: FnCGImageGetDataProvider = sym(CG, "CGImageGetDataProvider")?;
    let copy_data: FnCGDataProviderCopyData = sym(CG, "CGDataProviderCopyData")?;
    let byte_ptr: FnCFDataGetBytePtr = sym(CF, "CFDataGetBytePtr")?;
    let byte_len: FnCFDataGetLength = sym(CF, "CFDataGetLength")?;
    let bounds: FnCGDisplayBounds = sym(CG, "CGDisplayBounds")?;

    let id = unsafe { main() };
    let rect = unsafe { bounds(id) };
    let origin_x = rect.origin.x.floor() as i32;
    let origin_y = rect.origin.y.floor() as i32;
    let image = unsafe { create(id) };
    if image.is_null() {
        return Err(permission_refusal("capture", "ScreenRecording"));
    }
    let width = unsafe { width_f(image) };
    let height = unsafe { height_f(image) };
    let stride = unsafe { stride_f(image) };
    let bits = unsafe { bpp_f(image) };
    let info = unsafe { info_f(image) };
    let provider = unsafe { provider_f(image) };
    let data = if provider.is_null() {
        std::ptr::null()
    } else {
        unsafe { copy_data(provider) }
    };
    if bits != 32 || data.is_null() || width == 0 || height == 0 {
        release(data);
        release(image);
        return Err(format!(
            "cu: unsupported: the display image is {bits}bpp / {width}x{height}; the shared \
             store-mode encoder needs 32bpp. No image was produced."
        ));
    }
    let pointer = unsafe { byte_ptr(data) };
    let length = unsafe { byte_len(data) } as usize;
    let needed = stride.saturating_mul(height);
    if pointer.is_null() || length < needed {
        release(data);
        release(image);
        return Err(
            "cu: unsupported: CFData did not carry the whole display bitmap; no image was produced"
                .to_owned(),
        );
    }
    let raw = unsafe { std::slice::from_raw_parts(pointer, needed) };
    // kCGBitmapByteOrder32Little (0x2000) stores BGRA/XRGB little-endian;
    // 32Big (0x4000) stores ARGB. Never guess silently: an unknown order is a
    // named refusal rather than a colour-swapped "successful" screenshot.
    let order = info & 0x7000;
    let big_endian = order == 0x4000;
    if order != 0x2000 && !big_endian {
        release(data);
        release(image);
        return Err(format!(
            "cu: unsupported: CGImage bitmap byte order {order:#x} is not handled; no image was \
             produced"
        ));
    }
    let mut pixels = vec![0u8; width * height * 4];
    for row in 0..height {
        for column in 0..width {
            let source = row * stride + column * 4;
            let target = (row * width + column) * 4;
            let (blue, green, red) = if big_endian {
                (raw[source + 3], raw[source + 2], raw[source + 1])
            } else {
                (raw[source], raw[source + 1], raw[source + 2])
            };
            pixels[target] = blue;
            pixels[target + 1] = green;
            pixels[target + 2] = red;
            pixels[target + 3] = 0xff;
        }
    }
    release(data);
    release(image);
    Ok((pixels, width, height, origin_x, origin_y))
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
    let (pixels, screen_w, screen_h, origin_x, origin_y) = full_screen()?;
    let left = x - origin_x;
    let top = y - origin_y;
    if left < 0 || top < 0 || left + width > screen_w as i32 || top + height > screen_h as i32 {
        return Err(format!(
            "screen: refused: region {x},{y} {width}x{height} is outside the main display \
             {origin_x},{origin_y} {screen_w}x{screen_h}; capture covers the main display only"
        ));
    }
    let mut region = vec![0u8; (width as usize) * (height as usize) * 4];
    for row in 0..height as usize {
        let from = ((top as usize + row) * screen_w + left as usize) * 4;
        let to = row * width as usize * 4;
        region[to..to + width as usize * 4]
            .copy_from_slice(&pixels[from..from + width as usize * 4]);
    }
    let chosen = capture_scale(width as u32, height as u32, scale);
    let (region, out_width, out_height) =
        downsample(&region, width as u32, height as u32, chosen);
    Ok(Captured {
        bytes: png::encode_rgb(&region, out_width, out_height),
        width: out_width,
        height: out_height,
        scale: chosen,
    })
}

pub fn capture_desktop(scale: u32) -> Result<Captured, String> {
    let (pixels, width, height, origin_x, origin_y) = full_screen()?;
    let chosen = capture_scale(width as u32, height as u32, scale);
    let (pixels, out_width, out_height) =
        downsample(&pixels, width as u32, height as u32, chosen);
    let _ = (origin_x, origin_y);
    Ok(Captured {
        bytes: png::encode_rgb(&pixels, out_width, out_height),
        width: out_width,
        height: out_height,
        scale: chosen,
    })
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
    let entry = window_entries()?
        .into_iter()
        .find(|entry| entry.handle == handle)
        .ok_or_else(|| "screen: refused: the window has no readable rectangle".to_owned())?;
    grab(
        entry.rect.origin.x.floor() as i32,
        entry.rect.origin.y.floor() as i32,
        entry.rect.size.width.ceil() as i32,
        entry.rect.size.height.ceil() as i32,
        scale,
    )
}

pub fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}
