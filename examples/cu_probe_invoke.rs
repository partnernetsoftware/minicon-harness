//! 0.0.17 W1 — native default action diagnostic probe.
//!
//! A **diagnostic**, not a shipped capability: it changes no source file, no
//! capability row, no ABI and no matrix. It verifies `IAccessible::
//! accDoDefaultAction` (vtable slot 25) against a real Windows desktop before
//! any production routing, and pairs every invoke with a before/after
//! state/value readback through the same MSAA surface — the exact evidence
//! `plan-0.0.17` needs to gate W2.
//!
//!   cargo run --example cu_probe_invoke -- [--hwnd <decimal>] [--name <substr>]
//!
//! Without `--hwnd` it uses the repo's own `CuHost::invoke_json` observe to
//! pick a live window handle (focused first). Without `--name` it lists
//! candidate nodes and performs **no invoke** (list-only mode); with `--name`
//! it invokes the unique matching node and prints the readback delta. On a
//! non-Windows cell it prints a named reason and exits successfully.
//!
//! Verdicts are printed, never asserted: `PROBE_OK_LIST` (list-only),
//! `PROBE_INVOKE_CHANGED` (invoke performed and a published field changed),
//! `PROBE_INVOKE_UNCHANGED` (invoke returned success but nothing changed —
//! the W2 downgrade evidence), or a named failure with the raw HRESULT.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    probe::run()
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("step=platform reason=non-windows; accDoDefaultAction is a Windows MSAA probe");
    println!("VERDICT=PROBE_NOT_APPLICABLE");
    Ok(())
}

#[cfg(windows)]
mod probe {
    use minicon_harness::cu::CuHost;
    use minicon_harness::screen::ScreenTool;
    use serde_json::{json, Value};
    use std::ffi::c_void;

    // ---- MSAA FFI (mirrors src/screen.rs; a probe must not depend on
    // private items, and a wrong slot must be attributable here). ----

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

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Variant {
        vt: u16,
        r1: u16,
        r2: u16,
        r3: u16,
        data: [u8; 16],
    }

    #[allow(dead_code)]
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
        String::from_utf16_lossy(std::slice::from_raw_parts(bstr, (bytes / 2).min(256)))
    }

    unsafe fn i4_variant_call(
        object: *mut c_void,
        index: usize,
        child: Variant,
        label: &str,
    ) -> i32 {
        type F = unsafe extern "system" fn(*mut c_void, Variant, *mut Variant) -> i32;
        let function: F = std::mem::transmute(slot(object, index));
        let mut value = Variant::empty();
        let hr = function(object, child, &mut value);
        println!("step={label} hr={hr:#010x}");
        if hr != 0 {
            -1
        } else {
            value.i4_value()
        }
    }

    /// The probe's subject: `IAccessible::accDoDefaultAction`, vtable slot 25.
    unsafe fn do_default_action(object: *mut c_void, child: Variant) -> i32 {
        type F = unsafe extern "system" fn(*mut c_void, Variant) -> i32;
        let function: F = std::mem::transmute(slot(object, 25));
        function(object, child)
    }

    unsafe fn release(object: *mut c_void) {
        type F = unsafe extern "system" fn(*mut c_void) -> u32;
        let function: F = std::mem::transmute(slot(object, 2));
        function(object);
    }

    fn live_hwnd(requested: Option<u64>) -> Result<(u64, String), String> {
        let root = std::env::temp_dir().join("mh-invoke-probe-root");
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
        Ok((
            chosen["hwnd"].as_u64().ok_or("window row has no integer hwnd")?,
            chosen["title"].as_str().unwrap_or("").to_owned(),
        ))
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let mut requested = None;
        let mut name_filter: Option<String> = None;
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--hwnd" => {
                    let value = args.next().ok_or("--hwnd needs a decimal value")?;
                    requested = Some(value.parse::<u64>()?);
                }
                "--name" => {
                    let value = args.next().ok_or("--name needs a substring")?;
                    name_filter = Some(value);
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

            // Find matching named nodes (name substring, list-only first).
            #[allow(dead_code)]
            struct NodeRef {
                object: *mut c_void,
                child: Variant,
                name: String,
                role: i32,
                state: i32,
                value: String,
            }
            let mut matches: Vec<NodeRef> = Vec::new();
            let mut visited = 0usize;
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
                // Bounded read-only recursion to find named nodes; the only
                // live side effect in this probe is the single filtered
                // invoke at the end. get_accChildCount is vtable slot 8;
                // AccessibleChildren is an oleacc free function.
                unsafe fn collect(
                    object: *mut c_void,
                    child: Variant,
                    depth: usize,
                    filter: &str,
                    matches: &mut Vec<NodeRef>,
                    visited: &mut usize,
                ) {
                    const MAX_ROWS: usize = 128;
                    const MAX_DEPTH: usize = 8;
                    if *visited >= MAX_ROWS || depth > MAX_DEPTH {
                        return;
                    }
                    *visited += 1;
                    let name = bstr_call(object, 10, child, "get_accName");
                    if !name.trim().is_empty()
                        && (filter.is_empty() || name.contains(filter))
                    {
                        let role = i4_variant_call(object, 13, child, "get_accRole");
                        matches.push(NodeRef {
                            object,
                            child,
                            name,
                            role,
                            state: 0,
                            value: String::new(),
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
                        let (target, sub): (*mut c_void, Variant) = if item.vt == 9 {
                            let pointer = usize::from_le_bytes(
                                item.data[..8].try_into().unwrap_or([0; 8]),
                            ) as *mut c_void;
                            if pointer.is_null() {
                                continue;
                            }
                            (pointer, Variant::i4(0))
                        } else {
                            (object, *item)
                        };
                        collect(target, sub, depth + 1, filter, matches, visited);
                    }
                }
                collect(
                    object,
                    Variant::i4(0),
                    0,
                    name_filter.as_deref().unwrap_or(""),
                    &mut matches,
                    &mut visited,
                );
            }
            println!("step=walk visited={visited} named_matches={}", matches.len());

            if matches.is_empty() {
                println!("step=match reason=no-named-node{}", if name_filter.is_some() { " matching filter" } else { "" });
                println!("VERDICT=PROBE_NO_CANDIDATE");
                CoUninitialize();
                return Ok(());
            }
            if matches.len() > 1 {
                println!("step=match reason=ambiguous matches={}", matches.len());
                for node in &matches {
                    println!("  candidate name={:?} role={}", node.name, node.role);
                }
                println!("VERDICT=PROBE_AMBIGUOUS");
                CoUninitialize();
                return Ok(());
            }

            let node = &mut matches[0];
            println!(
                "step=match unique name={:?} role={} child_id={}",
                node.name, node.role, node.child.i4_value()
            );
            if name_filter.is_none() {
                println!("step=invoke skipped reason=list-only");
                println!("VERDICT=PROBE_OK_LIST");
                release(node.object);
                CoUninitialize();
                return Ok(());
            }

            // Read state/value before, invoke, read after. Every HRESULT is
            // printed; a "successful" invoke with no delta is a verdict, not
            // an error.
            let before_state = i4_variant_call(node.object, 14, node.child, "get_accState.before");
            let before_value = bstr_call(node.object, 11, node.child, "get_accValue.before");
            let hr = do_default_action(node.object, node.child);
            println!("step=accDoDefaultAction slot=25 hr={hr:#010x} ok={}", hr == 0);
            let after_state = i4_variant_call(node.object, 14, node.child, "get_accState.after");
            let after_value = bstr_call(node.object, 11, node.child, "get_accValue.after");
            println!(
                "step=readback state {before_state:#010x} -> {after_state:#010x} changed={}",
                before_state != after_state
            );
            println!(
                "step=readback value {:?} -> {:?} changed={}",
                before_value,
                after_value,
                before_value != after_value
            );
            release(node.object);
            CoUninitialize();

            if hr != 0 {
                println!("VERDICT=PROBE_INVOKE_HRESULT_FAILED");
            } else if before_state != after_state || before_value != after_value {
                println!("VERDICT=PROBE_INVOKE_CHANGED");
            } else {
                println!("VERDICT=PROBE_INVOKE_UNCHANGED");
            }
        }
        Ok(())
    }
}
