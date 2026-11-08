//! 0.0.18 W1 — a11y text write diagnostic probe.
//!
//! A **diagnostic**, not a shipped capability: no source change, no
//! capability row, no ABI change. It verifies `IAccessible::put_accValue`
//! (vtable slot 27, next to the already-verified accLocation 22 and
//! accDoDefaultAction 25) against a real Windows desktop before any
//! production routing, pairing every write with a value readback through
//! the same MSAA surface — the exact evidence `plan-0.0.18` needs to gate W2.
//!
//!   cargo run --example cu_probe_write -- --hwnd <decimal> [--name <substr>] [--value <text>]
//!
//! List-only mode (no `--name`): walks the tree and prints every editable
//! candidate (textbox role or non-readonly focusable). With `--name` it
//! writes `--value` (default "cu-0018") to the unique matching node and
//! prints the before/after readback. Non-Windows prints a named verdict.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    probe::run()
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("step=platform reason=non-windows; put_accValue is a Windows MSAA probe");
    println!("VERDICT=PROBE_NOT_APPLICABLE");
    Ok(())
}

#[cfg(windows)]
mod probe {
    use minicon_harness::cu::CuHost;
    use minicon_harness::screen::ScreenTool;
    use serde_json::{json, Value};
    use std::ffi::c_void;

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    const IID_IACCESSIBLE: Guid = Guid {
        data1: 0x6187_36E0,
        data2: 0x3C3D,
        data3: 0x11CF,
        data4: [0x81, 0x0C, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    };

    const OBJID_CLIENT: u32 = 0xFFFFFFFC;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const VT_I4: u16 = 0x3;
    const VT_DISPATCH: u16 = 0x9;
    const STATE_READONLY: i32 = 0x40;
    const STATE_PROTECTED: i32 = 0x10;

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
        fn empty() -> Self {
            Self { vt: 0, r1: 0, r2: 0, r3: 0, data: [0; 16] }
        }
        fn i4(value: i32) -> Self {
            let mut v = Self::empty();
            v.vt = VT_I4;
            v.data[..4].copy_from_slice(&value.to_le_bytes());
            v
        }
        fn i4_value(&self) -> i32 {
            if self.vt == VT_I4 {
                i32::from_le_bytes(self.data[..4].try_into().unwrap_or([0; 4]))
            } else {
                0
            }
        }
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
        fn CoUninitialize();
        
    }

    #[link(name = "oleaut32")]
    extern "system" {
        fn SysAllocStringLen(source: *const u16, length: u32) -> *mut u16;
        fn SysFreeString(bstr: *mut u16);
    }

    #[link(name = "oleacc")]
    extern "system" {
        fn AccessibleObjectFromWindow(
            window: *mut c_void,
            id: u32,
            iid: *const Guid,
            object: *mut *mut c_void,
        ) -> i32;
        fn AccessibleChildren(
            object: *mut c_void,
            start: i32,
            count: i32,
            out: *mut Variant,
            obtained: *mut i32,
        ) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumChildWindows(
            parent: *mut c_void,
            callback: extern "system" fn(*mut c_void, isize) -> i32,
            param: isize,
        ) -> i32;
        fn GetClassNameW(window: *mut c_void, class: *mut u16, max: i32) -> i32;
    }

    extern "system" fn collect_child(window: *mut c_void, param: isize) -> i32 {
        unsafe {
            let found = &mut *(param as *mut Vec<*mut c_void>);
            let mut class = [0u16; 128];
            let length = GetClassNameW(window, class.as_mut_ptr(), class.len() as i32);
            let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
            if class.contains("Chrome_RenderWidgetHostHWND") {
                found.push(window);
            }
        }
        1
    }

    unsafe fn slot(object: *mut c_void, index: usize) -> usize {
        let table = *(object as *const *const usize);
        *table.add(index)
    }

    unsafe fn bstr_call(
        object: *mut c_void,
        index: usize,
        child: Variant,
        label: &str,
    ) -> String {
        type F = unsafe extern "system" fn(*mut c_void, Variant, *mut *mut u16) -> i32;
        let function: F = std::mem::transmute(slot(object, index));
        let mut bstr: *mut u16 = std::ptr::null_mut();
        let hr = function(object, child, &mut bstr);
        if hr != 0 || bstr.is_null() {
            println!("step={label} hr={hr:#010x}");
            return String::new();
        }
        let bytes = *((bstr as *const u8).offset(-4) as *const u32) as usize;
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(bstr, (bytes / 2).min(256)));
        SysFreeString(bstr);
        text
    }

    unsafe fn i4_variant_call(object: *mut c_void, index: usize, child: Variant, label: &str) -> i32 {
        type F = unsafe extern "system" fn(*mut c_void, Variant, *mut Variant) -> i32;
        let function: F = std::mem::transmute(slot(object, index));
        let mut value = Variant::empty();
        let hr = function(object, child, &mut value);
        println!("step={label} hr={hr:#010x}");
        if hr != 0 { -1 } else { value.i4_value() }
    }

    /// `IAccessible::put_accValue` (vtable slot 27).
    unsafe fn put_acc_value(object: *mut c_void, child: Variant, text: &str) -> i32 {
        type F = unsafe extern "system" fn(*mut c_void, Variant, *mut u16) -> i32;
        let function: F = std::mem::transmute(slot(object, 27));
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bstr = SysAllocStringLen(wide.as_ptr(), (wide.len() - 1) as u32);
        if bstr.is_null() {
            return -2147024882; // E_OUTOFMEMORY stand-in
        }
        let hr = function(object, child, bstr);
        SysFreeString(bstr);
        hr
    }

    #[allow(dead_code)]
    unsafe fn release(object: *mut c_void) {
        type F = unsafe extern "system" fn(*mut c_void) -> u32;
        let function: F = std::mem::transmute(slot(object, 2));
        function(object);
    }

    #[allow(dead_code)]
    struct Candidate2;

    fn live_hwnd(requested: Option<u64>) -> Result<(u64, String), String> {
        let root = std::env::temp_dir().join("mh-write-probe-root");
        std::fs::create_dir_all(&root).map_err(|e| format!("probe root: {e}"))?;
        let host = CuHost::new(ScreenTool::new(&root, true));
        let observed: Value = host.invoke_json(&json!({ "op": "observe" })).evidence;
        let windows = observed["windows"]
            .as_array()
            .ok_or_else(|| "observe published no windows[]".to_owned())?;
        if let Some(handle) = requested {
            return Ok((handle, "<--hwnd override>".to_owned()));
        }
        let chosen = windows
            .iter()
            .find(|w| w["focused"].as_bool().unwrap_or(false))
            .or_else(|| windows.first())
            .ok_or_else(|| "observe published an empty windows[]".to_owned())?;
        Ok((
            chosen["hwnd"].as_u64().ok_or("window row has no integer hwnd")?,
            chosen["title"].as_str().unwrap_or("").to_owned(),
        ))
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let mut requested = None;
        let mut name_filter: Option<String> = None;
        let mut value_text = "cu-0018".to_owned();
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--hwnd" => requested = Some(args.next().ok_or("--hwnd needs a value")?.parse::<u64>()?),
                "--name" => name_filter = Some(args.next().ok_or("--name needs a substring")?),
                "--value" => value_text = args.next().ok_or("--value needs text")?,
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

        unsafe {
            let init = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
            println!("step=CoInitializeEx hr={init:#010x} ok={}", init >= 0);
            if init < 0 {
                println!("VERDICT=PROBE_COM_INIT_FAILED");
                return Ok(());
            }

            let mut children: Vec<*mut c_void> = Vec::new();
            EnumChildWindows(
                handle as usize as *mut c_void,
                collect_child,
                &mut children as *mut _ as isize,
            );
            if children.is_empty() {
                children.push(handle as usize as *mut c_void);
            }

            #[allow(dead_code)]
            #[allow(dead_code)]
            struct Candidate {
                object: *mut c_void,
                child: Variant,
                name: String,
                role_name: String,
                state: i32,
                value: String,
            }
            let mut candidates: Vec<Candidate> = Vec::new();
            let mut visited = 0usize;

            unsafe fn collect(
                object: *mut c_void,
                child: Variant,
                depth: usize,
                filter: &str,
                candidates: &mut Vec<Candidate>,
                visited: &mut usize,
            ) {
                const MAX_ROWS: usize = 192;
                const MAX_DEPTH: usize = 10;
                if *visited >= MAX_ROWS || depth > MAX_DEPTH {
                    return;
                }
                *visited += 1;
                let name = bstr_call(object, 10, child, "get_accName");
                let role = i4_variant_call(object, 13, child, "get_accRole");
                let role_name = minicon_harness::screen::a11y_role_name(role);
                let state = i4_variant_call(object, 14, child, "get_accState");
                let editable = role_name == "textbox"
                    || (role_name == "unknown" && (state & STATE_READONLY) == 0 && state != 0);
                let matches = !name.trim().is_empty()
                    && (filter.is_empty() || name.contains(filter))
                    && editable;
                if matches {
                    let value = bstr_call(object, 11, child, "get_accValue");
                    candidates.push(Candidate {
                        object,
                        child,
                        name,
                        role_name: role_name.to_owned(),
                        state,
                        value,
                    });
                }
                type CF = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
                let child_count: CF = std::mem::transmute(slot(object, 8));
                let mut count = 0i32;
                let _ = child_count(object, &mut count);
                let count = count.min(32);
                if count <= 0 {
                    return;
                }
                let mut buffer = vec![Variant::empty(); count as usize];
                let mut obtained = 0i32;
                if AccessibleChildren(object, 0, count, buffer.as_mut_ptr(), &mut obtained) != 0 {
                    return;
                }
                for item in &buffer[..obtained.max(0) as usize] {
                    let (target, sub): (*mut c_void, Variant) = if item.vt == VT_DISPATCH {
                        let pointer = usize::from_le_bytes(
                            item.data[..8].try_into().unwrap_or([0; 8]),
                        ) as *mut c_void;
                        if pointer.is_null() { continue; }
                        (pointer, Variant::i4(0))
                    } else {
                        (object, *item)
                    };
                    collect(target, sub, depth + 1, filter, candidates, visited);
                }
            }

            for child_window in children {
                let mut object: *mut c_void = std::ptr::null_mut();
                let hr = AccessibleObjectFromWindow(
                    child_window,
                    OBJID_CLIENT,
                    &IID_IACCESSIBLE,
                    &mut object,
                );
                println!("step=AccessibleObjectFromWindow hr={hr:#010x}");
                if hr != 0 || object.is_null() {
                    continue;
                }
                collect(
                    object,
                    Variant::i4(0),
                    0,
                    name_filter.as_deref().unwrap_or(""),
                    &mut candidates,
                    &mut visited,
                );
            }
            println!(
                "step=walk visited={visited} editable_candidates={}",
                candidates.len()
            );

            if candidates.is_empty() {
                println!("VERDICT=PROBE_NO_EDITABLE_NODE");
                CoUninitialize();
                return Ok(());
            }
            if name_filter.is_none() {
                for node in &candidates {
                    println!(
                        "  candidate name={:?} role={} state={:#010x} value={:?}",
                        node.name, node.role_name, node.state, node.value
                    );
                }
                println!("VERDICT=PROBE_OK_LIST");
                CoUninitialize();
                return Ok(());
            }
            if candidates.len() > 1 {
                println!("step=match reason=ambiguous matches={}", candidates.len());
                println!("VERDICT=PROBE_AMBIGUOUS");
                CoUninitialize();
                return Ok(());
            }

            let node = &candidates[0];
            if node.state & STATE_READONLY != 0 {
                println!("step=veto reason=readonly state={:#010x}", node.state);
                println!("VERDICT=PROBE_READONLY_REFUSED");
                CoUninitialize();
                return Ok(());
            }
            if node.state & STATE_PROTECTED != 0 {
                println!("step=veto reason=protected state={:#010x}", node.state);
                println!("VERDICT=PROBE_PROTECTED_REFUSED");
                CoUninitialize();
                return Ok(());
            }
            println!(
                "step=match unique name={:?} role={} value_before={:?}",
                node.name, node.role_name, node.value
            );
            let hr = put_acc_value(node.object, node.child, &value_text);
            println!("step=put_accValue slot=27 hr={hr:#010x} ok={}", hr == 0);
            let after = bstr_call(node.object, 11, node.child, "get_accValue.after");
            println!(
                "step=readback value {:?} -> {:?} changed={}",
                node.value,
                after,
                node.value != after
            );
            CoUninitialize();
            if hr != 0 {
                println!("VERDICT=PROBE_WRITE_HRESULT_FAILED");
            } else if after == value_text {
                println!("VERDICT=PROBE_WRITE_CONFIRMED");
            } else if node.value != after {
                println!("VERDICT=PROBE_WRITE_CHANGED_UNEXPECTEDLY");
            } else {
                println!("VERDICT=PROBE_WRITE_SILENT_NOOP");
            }
        }
        Ok(())
    }
}
