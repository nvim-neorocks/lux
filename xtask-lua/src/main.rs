use std::env;

use xtask_lua::{dist, DynError};

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{e}");
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);

    match task.as_deref() {
        Some("dist") => dist(true, None)?,
        Some("dist-debug") => dist(false, None)?,
        _ => print_help(),
    }

    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

dist          builds the lua libraries for a given lua version (must specify via features)
dist-debug    like dist, but builds debug artifacts
"
    )
}
