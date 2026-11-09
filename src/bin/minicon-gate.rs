//! `minicon-gate` -- the M-line LLM gate binary.
//!
//! A thin main. Everything below the argument parse lives in `minicon_harness::gate`,
//! so the contract has one implementation and the binary stays a banner plus a
//! listener.

fn main() {
    let mut config: Option<String> = None;
    let mut allow_non_loopback = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => match args.next() {
                Some(value) => config = Some(value),
                None => fail("--config requires a path"),
            },
            "--allow-non-loopback" => allow_non_loopback = true,
            "--version" | "-V" => {
                print!("{}", minicon_harness::gate::VERSION_BANNER);
                return;
            }
            "--help" | "-h" => {
                print!("{}", minicon_harness::gate::USAGE);
                return;
            }
            other => fail(&format!(
                "unrecognised argument {other:?}; run with --help"
            )),
        }
    }
    let Some(path) = config else {
        fail("--config PATH is required in 0.0.20")
    };
    if let Err(error) = minicon_harness::gate::run(std::path::Path::new(&path), allow_non_loopback) {
        fail(&error);
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{}", minicon_harness::gate::config::program_message(message));
    std::process::exit(2);
}
