//! BridgeWine CLI (`bw`). Faza 0: samo kostur komandi.

use std::env;
use std::process::ExitCode;

const USAGE: &str = "\
bw - BridgeWine

Upotreba:
  bw doctor                 proveri sistem (macOS, Rosetta 2, GPTK)
  bw bottle create <igra>   napravi Wine prefiks za igru
  bw steam install <igra>   instaliraj Windows Steam u bottle
  bw run <igra>             pokreni igru po receptu
  bw report <igra>          napravi izvestaj o kompatibilnosti
  bw --version
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") => {
            println!("bw {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(cmd @ ("doctor" | "bottle" | "steam" | "run" | "report")) => {
            eprintln!("bw {cmd}: jos nije implementirano (faza 1)");
            ExitCode::from(2)
        }
        Some(other) => {
            eprintln!("nepoznata komanda: {other}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
