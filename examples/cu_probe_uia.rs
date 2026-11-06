//! 0.0.15 W1 — UIA diagnostic probe.
//!
//! A read-only **diagnostic**, not a shipped capability. It verifies the
//! `IUIAutomation` COM surface recorded in `PRD_01_17` (W0 pass 4) against a
//! real Windows desktop and prints every raw HRESULT before interpreting any
//! value. It changes no source file, no capability row, no ABI and no matrix.
//!
//!   cargo run --example cu_probe_uia -- [--hwnd <decimal>]
//!
//! Without `--hwnd` it asks the repo's own `CuHost::invoke_json({"op":"observe"})`
//! for a live window handle (focused first), so the probe uses the same handle
//! identity that `act:focus` and `observe.a11y` publish. On a non-Windows cell
//! it prints a named reason and exits successfully: it is a Windows COM probe.
//!
//! Verdicts are printed, never asserted: `PROBE_OK` means every slot returned
//! `S_OK` and a name plus a `RECT` were read; anything else names the failing
//! slot and HRESULT. A wrong vtable slot can still crash the process — that is
//! exactly why this probe exists before any production routing.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    probe::run()
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("step=platform reason=non-windows; UIA is a Windows-only COM probe");
    println!("VERDICT=PROBE_NOT_APPLICABLE");
    Ok(())
}

#[cfg(windows)]
mod probe {
    use minicon_harness::cu::CuHost;
    use minicon_harness::screen::ScreenTool;
    use serde_json::{json, Value};
    use std::ffi::c_void;

    /// GUID layout matches the `Guid` in `src/screen.rs` (MSAA tree, 0.0.11).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Guid {
        a: u32,
        b: u16,
        c: u16,
        d: [u8; 8],
    }

    /// Verified from the installed Windows SDK 10.0.22000.0
    /// `um/UIAutomationClient.h` / `um/UIAutomationClient.idl` (W0 pass 4).
    const CLSID_CUIAUTOMATION: Guid = Guid {
        a: 0xff48_dba4,
        b: 0x60ef,
        c: 0x4201,
        d: [0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e],
    };
    const IID_IUIAUTOMATION: Guid = Guid {
        a: 0x30cb_e57d,
        b: 0xd9d0,
        c: 0x452a,
        d: [0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee],
    };
    const CLSCTX_ALL: u32 = 23;
    const COINIT_APARTMENTTHREADED: u32 = 2;
    const TREE_SCOPE_SUBTREE: i32 = 0x1 | 0x2 | 0x4;

    // `ole32` is already linked by `src/screen.rs` for `CoInitializeEx`; this
    // probe adds no crate and no new platform library.
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(
            rclsid: *const Guid,
            outer: *mut c_void,
            context: u32,
            iid: *const Guid,
            out: *mut *mut c_void,
        ) -> i32;
    }

    /// Win32 `RECT`: left / top / right / bottom — **not** `[x, y, w, h]`.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    unsafe fn slot(object: *mut c_void, index: usize) -> usize {
        let table = *(object as *const *const usize);
        *table.add(index)
    }

    unsafe fn release(object: *mut c_void) {
        if object.is_null() {
            return;
        }
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        let function: Release = std::mem::transmute(slot(object, 2));
        function(object);
    }

    fn hresult(hr: i32) -> String {
        format!("0x{:08x}", hr as u32)
    }

    /// `S_OK` is the only success. `S_FALSE` is not a substitute for a read,
    /// and a negative HRESULT is a failure with the failing slot named.
    fn report(step: &str, slot_index: usize, hr: i32) -> bool {
        println!(
            "step={step} slot={slot_index} hr={} ok={}",
            hresult(hr),
            hr == 0
        );
        hr == 0
    }

    fn fail(step: &str, slot_index: usize, hr: i32) {
        println!(
            "VERDICT=PROBE_FAILED failed_step={step} failed_slot={slot_index} hr={}",
            hresult(hr)
        );
    }

    fn live_hwnd(requested: Option<u64>) -> Result<(u64, String), String> {
        let root = std::env::temp_dir().join("mh-uia-probe-root");
        std::fs::create_dir_all(&root).map_err(|e| format!("probe root: {e}"))?;
        let host = CuHost::new(ScreenTool::new(&root, true));
        let observed: Value = host.invoke_json(&json!({ "op": "observe" })).evidence;
        let windows = observed["windows"]
            .as_array()
            .ok_or_else(|| "observe published no windows[]".to_owned())?;
        for (index, window) in windows.iter().enumerate() {
            println!(
                "step=window index={index} hwnd={} focused={} title={:?}",
                window["hwnd"].as_u64().unwrap_or(0),
                window["focused"].as_bool().unwrap_or(false),
                window["title"].as_str().unwrap_or("")
            );
        }
        if let Some(handle) = requested {
            return Ok((handle, "<--hwnd override>".to_owned()));
        }
        let chosen = windows
            .iter()
            .find(|w| w["focused"].as_bool().unwrap_or(false))
            .or_else(|| windows.first())
            .ok_or_else(|| "observe published an empty windows[]".to_owned())?;
        let handle = chosen["hwnd"]
            .as_u64()
            .ok_or_else(|| "window row has no integer hwnd".to_owned())?;
        let title = chosen["title"].as_str().unwrap_or("").to_owned();
        println!(
            "step=observe windows={} hwnd={handle} title={title:?}",
            windows.len()
        );
        Ok((handle, title))
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let mut requested = None;
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--hwnd" => {
                    let value = args.next().ok_or("--hwnd needs a decimal value")?;
                    requested = Some(value.parse::<u64>()?);
                }
                other => return Err(format!("unknown argument {other}").into()),
            }
        }

        let (handle, title) = match live_hwnd(requested) {
            Ok(found) => found,
            Err(reason) => {
                println!("step=observe reason={reason}");
                println!("VERDICT=PROBE_NO_WINDOW");
                return Ok(());
            }
        };
        println!("step=target hwnd={handle} title={title:?}");

        // Every call below prints its raw HRESULT before its out-value is read.
        // The probe is deliberately a flat sequence: a wrong slot must be
        // attributable to one named step, not swallowed by a helper.
        unsafe {
            let init = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
            println!("step=CoInitializeEx hr={} ok={}", hresult(init), init >= 0);
            if init < 0 {
                fail("CoInitializeEx", 0, init);
                return Ok(());
            }

            let mut automation: *mut c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_CUIAUTOMATION,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IUIAUTOMATION,
                &mut automation,
            );
            if !report("CoCreateInstance", 0, hr) || automation.is_null() {
                fail("CoCreateInstance", 0, hr);
                CoUninitialize();
                return Ok(());
            }
            println!("step=CoCreateInstance automation_ptr_nonnull=true");

            // Control read: the UIA desktop ROOT element must have a real,
            // non-zero bounding rectangle (the virtual desktop). If slot 43
            // were wrong, or the RECT layout were misread, this control would
            // fail too. A working root RECT alongside empty per-window RECTs
            // proves the provider (not the vtable) is what withheld bounds.
            // IUIAutomation slot 5: GetRootElement(&root)
            type GetRootElement = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
            type GetRect = unsafe extern "system" fn(*mut c_void, *mut Rect) -> i32;
            let mut root: *mut c_void = std::ptr::null_mut();
            let f5: GetRootElement = std::mem::transmute(slot(automation, 5));
            let mut control_hr = f5(automation, &mut root);
            if !report("IUIAutomation::GetRootElement(control)", 5, control_hr) || root.is_null() {
                fail("IUIAutomation::GetRootElement(control)", 5, control_hr);
                release(automation);
                CoUninitialize();
                return Ok(());
            }
            let mut root_rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let fr43: GetRect = std::mem::transmute(slot(root, 43));
            control_hr = fr43(root, &mut root_rect);
            let root_width = root_rect.right - root_rect.left;
            let root_height = root_rect.bottom - root_rect.top;
            println!(
                "step=control_root_rect slot=43 hr={} RECT={},{},{},{} => [x,y,w,h]=[{},{},{},{}]",
                hresult(control_hr),
                root_rect.left,
                root_rect.top,
                root_rect.right,
                root_rect.bottom,
                root_rect.left,
                root_rect.top,
                root_width,
                root_height
            );
            let root_rect_ok = control_hr == 0 && root_width > 0 && root_height > 0;
            println!("step=control_root_rect usable={root_rect_ok}");
            release(root);

            // IUIAutomation slot 6: ElementFromHandle(UIA_HWND, &element)
            type ElementFromHandle =
                unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32;
            let mut element: *mut c_void = std::ptr::null_mut();
            let f6: ElementFromHandle = std::mem::transmute(slot(automation, 6));
            let mut hr = f6(automation, handle as usize as *mut c_void, &mut element);
            if !report("IUIAutomation::ElementFromHandle", 6, hr) || element.is_null() {
                fail("IUIAutomation::ElementFromHandle", 6, hr);
                release(automation);
                CoUninitialize();
                return Ok(());
            }

            // IUIAutomation slot 21: CreateTrueCondition(&condition)
            type CreateTrueCondition =
                unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
            let mut condition: *mut c_void = std::ptr::null_mut();
            let f21: CreateTrueCondition = std::mem::transmute(slot(automation, 21));
            hr = f21(automation, &mut condition);
            if !report("IUIAutomation::CreateTrueCondition", 21, hr) || condition.is_null() {
                fail("IUIAutomation::CreateTrueCondition", 21, hr);
                release(element);
                release(automation);
                CoUninitialize();
                return Ok(());
            }

            // IUIAutomationElement slot 6: FindAll(TreeScope_Subtree, condition, &array)
            type FindAll =
                unsafe extern "system" fn(*mut c_void, i32, *mut c_void, *mut *mut c_void) -> i32;
            let mut array: *mut c_void = std::ptr::null_mut();
            let fe6: FindAll = std::mem::transmute(slot(element, 6));
            hr = fe6(element, TREE_SCOPE_SUBTREE, condition, &mut array);
            if !report("IUIAutomationElement::FindAll", 6, hr) || array.is_null() {
                fail("IUIAutomationElement::FindAll", 6, hr);
                release(condition);
                release(element);
                release(automation);
                CoUninitialize();
                return Ok(());
            }

            // IUIAutomationElementArray slot 3: get_Length(&count)
            type GetLength = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
            let mut count: i32 = -1;
            let fa3: GetLength = std::mem::transmute(slot(array, 3));
            hr = fa3(array, &mut count);
            if !report("IUIAutomationElementArray::get_Length", 3, hr) || count < 0 {
                fail("IUIAutomationElementArray::get_Length", 3, hr);
                release(array);
                release(condition);
                release(element);
                release(automation);
                CoUninitialize();
                return Ok(());
            }
            println!("step=FindAll length={count}");

            // A bounded walk, not just the root: the root of a Chromium window
            // commonly reports an empty rectangle with S_OK, which proves
            // nothing about the RECT -> [x,y,w,h] conversion. The probe reads
            // several elements so a zero box can be told apart from a real one.
            let limit = (count as usize).min(16);
            let mut named = 0usize;
            let mut nonzero = 0usize;
            let mut invalid = 0usize;
            let mut non_pane = 0usize;
            // UIA_PaneControlTypeId = 50033. A tree of nothing but Pane nodes
            // has no addressable control to click or type into.
            const UIA_PANE: i32 = 50033;

            // IUIAutomationElementArray slot 4: GetElement(index, &child)
            type GetElement = unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> i32;
            // IUIAutomationElement slot 21: get_CurrentControlType(&CONTROLTYPEID)
            type GetControlType = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
            // IUIAutomationElement slot 23: get_CurrentName(&BSTR)
            type GetName = unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> i32;
            // get_CurrentBoundingRectangle's `GetRect` alias is declared with
            // the desktop-root control read above and reused here.

            let fa4: GetElement = std::mem::transmute(slot(array, 4));
            for index in 0..limit {
                let mut child: *mut c_void = std::ptr::null_mut();
                hr = fa4(array, index as i32, &mut child);
                if !report(&format!("GetElement[{index}]"), 4, hr) || child.is_null() {
                    fail("IUIAutomationElementArray::GetElement", 4, hr);
                    release(array);
                    release(condition);
                    release(element);
                    release(automation);
                    CoUninitialize();
                    return Ok(());
                }

                let fe21: GetControlType = std::mem::transmute(slot(child, 21));
                let mut control_type: i32 = -1;
                hr = fe21(child, &mut control_type);
                if !report(&format!("get_CurrentControlType[{index}]"), 21, hr) {
                    fail("IUIAutomationElement::get_CurrentControlType", 21, hr);
                    release(child);
                    release(array);
                    release(condition);
                    release(element);
                    release(automation);
                    CoUninitialize();
                    return Ok(());
                }

                let fe23: GetName = std::mem::transmute(slot(child, 23));
                let mut bstr: *mut u16 = std::ptr::null_mut();
                hr = fe23(child, &mut bstr);
                if !report(&format!("get_CurrentName[{index}]"), 23, hr) {
                    fail("IUIAutomationElement::get_CurrentName", 23, hr);
                    release(child);
                    release(array);
                    release(condition);
                    release(element);
                    release(automation);
                    CoUninitialize();
                    return Ok(());
                }
                // A BSTR carries its byte length in the 4 bytes before the
                // data. W1 is a short-lived diagnostic and deliberately does
                // not call SysFreeString (that would need a second platform
                // library); W2's production route must, and that is recorded
                // as a required adapter rule in plan-0.0.15.
                let name = if bstr.is_null() {
                    String::new()
                } else {
                    let bytes = *((bstr as *const u8).offset(-4) as *const u32) as usize;
                    String::from_utf16_lossy(std::slice::from_raw_parts(bstr, (bytes / 2).min(256)))
                };
                if !name.is_empty() {
                    named += 1;
                }
                if control_type != UIA_PANE {
                    non_pane += 1;
                }

                let fe43: GetRect = std::mem::transmute(slot(child, 43));
                let mut rect = Rect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                hr = fe43(child, &mut rect);
                if !report(&format!("get_CurrentBoundingRectangle[{index}]"), 43, hr) {
                    fail("IUIAutomationElement::get_CurrentBoundingRectangle", 43, hr);
                    release(child);
                    release(array);
                    release(condition);
                    release(element);
                    release(automation);
                    CoUninitialize();
                    return Ok(());
                }
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                // The 0.0.14 schema requires non-negative width/height; a
                // negative value is the signature of an unconverted RECT.
                if width < 0 || height < 0 {
                    invalid += 1;
                }
                if width > 0 && height > 0 {
                    nonzero += 1;
                }
                println!(
                    "step=element index={index} control_type={control_type} name={name:?} \
                     RECT={},{},{},{} => [x,y,w,h]=[{},{},{},{}]",
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                    rect.left,
                    rect.top,
                    width,
                    height
                );
                release(child);
            }

            release(array);
            release(condition);
            release(element);
            release(automation);
            CoUninitialize();
            println!(
                "step=summary elements={count} examined={limit} named={named} \
                 non_pane_controls={non_pane} nonzero_rects={nonzero} invalid_rects={invalid}"
            );
            if invalid > 0 {
                println!("VERDICT=PROBE_FAILED reason=negative-width-or-height");
            } else if nonzero == 0 {
                println!(
                    "VERDICT=PROBE_PARTIAL reason=S_OK-but-all-rectangles-empty; \
                     COM surface works, provider exposed no bounds for this handle"
                );
            } else {
                println!("VERDICT=PROBE_OK elements={count} nonzero_rects={nonzero}");
            }
        }
        Ok(())
    }
}
