//! Run the commands of the host.

use std::io::ErrorKind;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// What a command gave back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// A command that succeeded and wrote `stdout`.
    pub fn ok(stdout: &str) -> Output {
        Output {
            status: 0,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        }
    }

    /// A command that failed with `status` and wrote `stderr`.
    pub fn fail(status: i32, stderr: &str) -> Output {
        Output {
            status,
            stdout: String::new(),
            stderr: stderr.to_owned(),
        }
    }

    pub fn success(&self) -> bool {
        self.status == 0
    }
}

/// Runs a program of the host. The tests use a fake.
pub trait Runner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output>;
    /// Runs a program on the terminal of the person, so it can ask its own
    /// questions, such as the passphrase of `ssh-keygen`. Gives the status.
    fn interactive(&self, program: &str, args: &[&str]) -> Result<i32>;
    /// True when a program with this name is in a directory of PATH.
    fn on_path(&self, program: &str) -> bool;
}

/// The runner of the real host.
pub struct HostRunner;

impl Runner for HostRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        let result = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output();
        let out = match result {
            Ok(out) => out,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                bail!("this host has no `{program}`")
            }
            Err(e) => return Err(e).with_context(|| format!("could not start `{program}`")),
        };
        Ok(Output {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn interactive(&self, program: &str, args: &[&str]) -> Result<i32> {
        let status = match Command::new(program).args(args).status() {
            Ok(s) => s,
            Err(e) if e.kind() == ErrorKind::NotFound => bail!("this host has no `{program}`"),
            Err(e) => return Err(e).with_context(|| format!("could not start `{program}`")),
        };
        Ok(status.code().unwrap_or(-1))
    }

    fn on_path(&self, program: &str) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        let suffixes: Vec<String> = if cfg!(windows) {
            std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_owned())
                .split(';')
                .map(str::to_owned)
                .chain(std::iter::once(String::new()))
                .collect()
        } else {
            vec![String::new()]
        };
        std::env::split_paths(&path).any(|dir| {
            suffixes
                .iter()
                .any(|s| dir.join(format!("{program}{s}")).is_file())
        })
    }
}
