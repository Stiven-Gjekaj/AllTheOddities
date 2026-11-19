//! The command line of AllTheOddities.
//!
//! The binary is one line that calls [`run`]. Everything else is here, so a
//! test reaches each part of it.

pub mod ask;
pub mod ctx;
pub mod exit;
pub mod host;
pub mod net;
pub mod run;
#[cfg(test)]
pub mod testing;
pub mod version;

/// Run the command that the person typed, and give the exit code.
pub fn run() -> i32 {
    let cmd = clap::Command::new("odd")
        .about("Niche fixes in one command, for Windows, macOS and Linux.")
        .version(version::LONG);
    let _ = cmd.get_matches();
    0
}
