//! Thin `main`: parse flags, read the key, run one bounded task.

fn main() {
    if let Err(error) = minicon_harness::main_entry() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
