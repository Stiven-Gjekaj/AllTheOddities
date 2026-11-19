//! The command line of AllTheOddities.
//!
//! The binary is one line that calls [`run`]. Everything else is here, so a
//! test reaches each part of it.

pub mod ask;
pub mod block;
pub mod cli;
pub mod ctx;
pub mod exit;
pub mod host;
pub mod net;
pub mod oddities;
pub mod oddity;
pub mod run;
pub mod state;
#[cfg(test)]
pub mod testing;
pub mod version;

/// Run the command that the person typed, and give the exit code.
pub fn run() -> i32 {
    let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let flag = |f: &str| argv.iter().any(|a| a == f);
    let env: std::collections::HashMap<String, String> = std::env::vars().collect();
    let home = env
        .get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let host = host::Host::current();
    let state_dir = match host.os {
        host::Os::Windows => env
            .get("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Local"))
            .join("odd")
            .join("state"),
        _ => home.join(".local").join("state").join("odd"),
    };
    let ask: Box<dyn ask::Ask> = if flag("--yes") {
        Box::new(ask::Defaults)
    } else {
        ask::Terminal::open()
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut ctx = ctx::Ctx {
        host,
        home,
        state_dir,
        env,
        is_admin: host::is_admin(),
        runner: &run::HostRunner,
        ask: ask.as_ref(),
        http: &net::NoHttp,
        dry_run: false,
        out: &mut out,
    };
    oddity::dispatch(argv, &mut ctx, oddity::ODDITIES) as i32
}
