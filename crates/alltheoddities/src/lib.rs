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
    // odd update on Windows leaves the old binary beside the new one.
    if cfg!(windows)
        && let Ok(exe) = std::env::current_exe()
    {
        let _ = std::fs::remove_file(exe.with_file_name("odd.old.exe"));
    }
    let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let flag = |f: &str| argv.iter().any(|a| a == f);
    let env: std::collections::HashMap<String, String> = std::env::vars().collect();
    let home = env
        .get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let host = host::Host::current();
    let home = sudo_home(&env, host.os).unwrap_or(home);
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
        root: None,
        is_admin: host::is_admin(),
        http: &net::HostHttp,
        runner: &run::HostRunner,
        ask: ask.as_ref(),
        dry_run: false,
        out: &mut out,
    };
    oddity::dispatch(argv, &mut ctx, oddity::ODDITIES) as i32
}

/// Under sudo, the home directory of the person who ran sudo, not of root.
fn sudo_home(
    env: &std::collections::HashMap<String, String>,
    os: host::Os,
) -> Option<std::path::PathBuf> {
    let user = env.get("SUDO_USER")?;
    if os == host::Os::MacOs {
        return Some(std::path::PathBuf::from("/Users").join(user));
    }
    let out = std::process::Command::new("getent")
        .args(["passwd", user])
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&out.stdout).into_owned();
    line.trim()
        .split(':')
        .nth(5)
        .filter(|h| !h.is_empty())
        .map(std::path::PathBuf::from)
}
