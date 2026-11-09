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
pub mod router;
pub mod secret;
pub mod server;
pub mod transport;

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
/// Loads the config, builds the router, binds the loopback listener and serves
/// until `SIGINT`. A refusal at any step exits non-zero with a named code, so a
/// gate never starts on a config it only half-understands.
pub fn run(config_path: &std::path::Path, allow_non_loopback: bool) -> Result<(), String> {
    let loaded = config::load(config_path, allow_non_loopback)?;
    println!(
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
    let command_line = config::CommandLine::from_process();
    let bearer = match &loaded.config.listen.auth {
        None => None,
        Some(secret) => Some(
            command_line
                .read_secret(secret, false, None)?
                .expose()
                .to_owned(),
        ),
    };
    let evidence = evidence::Evidence::new(loaded.config.events.ring, loaded.config.events.mirror.clone())?;    let router = std::sync::Arc::new(router::Router {
        config: loaded,
        runtime: router::Runtime::default(),
        evidence: std::sync::Mutex::new(evidence),
        bearer,
        command_line,
    });
    let server = config::bind(std::sync::Arc::clone(&router), 4, allow_non_loopback)?;
    println!("{}: listening on http://{}", PROGRAM, server.local);
    wait_for_interrupt();
    server.stop();
    println!("{}: stopped", PROGRAM);
    Ok(())
}

/// Blocks until the process is interrupted (Ctrl+C) or asked to terminate.
fn wait_for_interrupt() {
    #[cfg(unix)]
    {
        use std::io::Read as _;
        // A SIGINT on a terminal closes stdin's read; either way this returns.
        let mut sink = std::io::empty();
        let _ = std::io::stdin().read(&mut std::io::sink());
        let _ = sink.read(&mut []);
    }
    #[cfg(not(unix))]
    {
        // Windows has no signal handler without a crate; a bounded poll keeps
        // the binary honest without one, and the parent's kill ends it.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
