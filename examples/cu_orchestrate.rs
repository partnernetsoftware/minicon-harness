//! Wire-level driver for self-hosted orchestration.
//!
//! This file contains NO GUI code. It feeds newline-delimited JSON requests to
//! `CuHost::invoke_json`, so every observation, pointer event, keystroke and
//! receipt is produced by minicon-harness's own `screen` backend. A controller
//! writes the request file; the harness's CU contract does the desktop work.
//!
//! Usage:
//!   cargo run --example cu_orchestrate -- --root <dir> --requests <jsonl> --out <jsonl>
//!
//! Blank lines and lines starting with `#` are skipped. Each output line is a
//! record with the request, verdict, capability, generation, evidence and the
//! human-readable text the host would have returned to a model.

use minicon_harness::{cu::CuHost, screen::ScreenTool};
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut root = PathBuf::from(".");
    let mut requests: Option<String> = None;
    let mut out: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root needs a path")?),
            "--requests" => requests = Some(args.next().ok_or("--requests needs a path")?),
            "--out" => out = Some(args.next().ok_or("--out needs a path")?),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let requests = requests.ok_or("--requests is required")?;
    let host = CuHost::new(ScreenTool::new(&root, true));
    let body = fs::read_to_string(&requests).map_err(|error| error.to_string())?;
    let mut records = Vec::new();
    for (index, line) in body.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let request: Value =
            serde_json::from_str(line).map_err(|error| format!("line {}: {error}", index + 1))?;
        let result = host.invoke_json(&request);
        let record = json!({
            "step": index + 1,
            "request": request,
            "verdict": format!("{:?}", result.verdict),
            "capability": result.capability,
            "generation": result.generation,
            "facts_as_of_ms": result.facts_as_of_ms,
            "evidence": result.evidence,
            "text": result.text,
        });
        let rendered = serde_json::to_string(&record).map_err(|error| error.to_string())?;
        println!("{rendered}");
        records.push(rendered);
    }
    if let Some(path) = out {
        fs::write(path, format!("{}\n", records.join("\n"))).map_err(|error| error.to_string())?;
    }
    Ok(())
}
