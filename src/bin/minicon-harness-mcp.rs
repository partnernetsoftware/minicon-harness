//! The H-line MCP adapter binary (0.0.19). stdio only; grants come from
//! argv and are the whole policy:
//!
//!   minicon-harness-mcp --root DIR [--allow-cmd NAME]... [--allow-screen]

fn main() {
    let mut root: Option<String> = None;
    let mut allow_cmd: Vec<String> = Vec::new();
    let mut allow_screen = false;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => {
                match args.next() {
                    Some(value) => root = Some(value),
                    None => {
                        eprintln!("--root needs a directory");
                        std::process::exit(2);
                    }
                }
            }
            "--allow-cmd" => match args.next() {
                Some(value) => allow_cmd.push(value),
                None => {
                    eprintln!("--allow-cmd needs a program name");
                    std::process::exit(2);
                }
            },
            "--allow-screen" => allow_screen = true,
            other => {
                eprintln!("unknown argument {other:?}; expected --root, --allow-cmd, --allow-screen");
                std::process::exit(2);
            }
        }
    }
    let root = match root {
        Some(root) => root,
        None => {
            eprintln!("--root is required; the mounting client supplies the task root");
            std::process::exit(2);
        }
    };
    minicon_harness::mcp::Adapter::new(root, allow_cmd, allow_screen).serve();
}
