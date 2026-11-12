//! cu_aff_probe.rs — 用 CuHost 的真实 computer-use 面（observe/act 分离、
//! readback 验证）观测 aff-shell（algo_2026 v0.0.12）窗口。
//!
//! 用法：cargo run --offline --example cu_aff_probe
//! 纯 observe，零 act——只证明 CU 通道可用（业主 2026-11-12 指令：
//! 操控/验收走 minicon-harness computer-use + CDP，不走截屏）。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        use minicon_harness::cu::CuHost;
        use serde_json::json;

        let host = minicon_harness::cu::CuHost::new(
            minicon_harness::screen::ScreenTool::new(std::path::Path::new("."), true),
        );
        let obs = host.invoke_json(&json!({ "op": "observe" })).evidence;
        let s = serde_json::to_string_pretty(&obs)?;
        // 找 algo_2026 窗口
        if let Some(wins) = obs["windows"].as_array() {
            let aff = wins
                .iter()
                .filter(|w| {
                    w["title"]
                        .as_str()
                        .map(|t| t.contains("algo_2026"))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            println!("== algo_2026 窗口 ==");
            for w in &aff {
                println!(
                    "hwnd={} title={:?} rect={}",
                    w["hwnd"],
                    w["title"].as_str().unwrap_or(""),
                    w.get("rect").cloned().unwrap_or(serde_json::json!("-"))
                );
            }
            if let Some(w) = aff.first() {
                let hwnd = w["hwnd"].as_u64().unwrap();
                let scan = host.invoke_json(&json!({ "op": "observe", "a11y": hwnd })).evidence;
                let n = scan["a11y_controls"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0);
                println!("== a11y 扫描：{n} 个控件 ==");
                // 打印前几个含 TDX/会话字样的控件（若 WebView2 有暴露）
                if let Some(rows) = scan["a11y_controls"].as_array() {
                    let hit: Vec<_> = rows
                        .iter()
                        .filter(|c| {
                            c["name"]
                                .as_str()
                                .map(|n| {
                                    n.contains("TDX") || n.contains("会话") || n.contains("守护")
                                })
                                .unwrap_or(false)
                        })
                        .take(8)
                        .collect();
                    for c in &hit {
                        println!("控件: {:?}", c["name"].as_str().unwrap_or(""));
                    }
                    if hit.is_empty() {
                        println!("（WebView2 DOM 未暴露给 a11y——预期内，界面操控走 CDP）");
                    }
                }
            }
        } else {
            println!("observe 无 windows 数组，输出前 2000 字：\n{}", &s[..s.len().min(2000)]);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        println!("non-windows; CU 不可用");
        Ok(())
    }
}
