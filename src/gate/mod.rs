//! The M-line LLM gate -- a loopback sidecar that pools the tuple
//! (endpoint, credential, egress) and routes OpenAI-compatible model calls.
//!
//! Contract: `prd/PRD_01_27_llm_gate.md`. Work plan: `plan/v0.0.20.md`.
//! The three planes are kept apart on purpose:
//!
//! ```text
//! definition  endpoints, credentials, egress, lanes, agents, workflows  (data, revisioned)
//! resolution  intent -> expanded, ordered candidates                    (pure, no I/O)
//! transport   one attempt: secret, tunnel, dispatch, classify           (bounded)
//! ```
//!
//! This module starts with the definition and resolution planes, which are
//! pure and testable without a socket.

pub mod config;
pub mod evidence;
pub mod map;
pub mod refusal;

/// Program name in every diagnostic line.
pub const PROGRAM: &str = "minicon-gate";

/// Banner printed by `--version`; the same shape the core's `VERSION_BANNER`
/// uses so a packaging check can assert it.
pub const VERSION_BANNER: &str = concat!("minicon-gate ", env!("CARGO_PKG_VERSION"), "\n");

/// Printed by `--help`; deliberately shorter than the contract.
pub const USAGE: &str = "\
usage: minicon-gate --config PATH [--allow-non-loopback]

  --config PATH          the gate config (required in 0.0.20)
  --allow-non-loopback   bind a non-loopback address (a new risk class; the
                         threat model in PRD_01_27 section Egress must hold)
  --version              print the banner
  --help                 print this text
";

/// One-run entry point used by the binary and by the loopback test tier.
///
/// 0.0.20 W2 lands the definition plane; the listener arrives with the
/// transport plane. Until then a valid config is loaded, validated and
/// reported, and the process exits without binding, so a half-built gate can
/// never accept a request it cannot route.
pub fn run(config_path: &std::path::Path, allow_non_loopback: bool) -> Result<(), String> {
    let loaded = config::load(config_path, allow_non_loopback)?;    println!(
        "{}: config {} loaded (revision {})",
        PROGRAM,
        loaded.path.display(),
        loaded.revision
    );
    println!(
        "{}: {} endpoints, {} credentials, {} egress, {} lanes, {} agents, {} workflows",
        PROGRAM,
        loaded.config.endpoints.len(),
        loaded.config.credentials.len(),
        loaded.config.egress.len(),
        loaded.config.lanes.len(),
        loaded.config.agents.len(),
        loaded.config.workflows.len(),
    );
    Ok(())
}
