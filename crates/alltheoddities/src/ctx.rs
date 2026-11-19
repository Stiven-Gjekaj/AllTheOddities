//! The context of a run: the only way that an oddity reaches the host.
//!
//! An oddity runs a host command, writes a file and asks a question only
//! through a [`Ctx`]. So `--dry-run` works in one place, and a test gives a
//! fake host.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::ask::Ask;
use crate::host::Host;
use crate::net::Http;
use crate::run::{Output, Runner};

/// The host, the person and the switches of one run.
pub struct Ctx<'a> {
    pub host: Host,
    pub home: PathBuf,
    pub state_dir: PathBuf,
    pub env: HashMap<String, String>,
    /// A directory that stands for `/` or `C:\`, for the tests. None in a
    /// real run.
    pub root: Option<PathBuf>,
    pub is_admin: bool,
    pub runner: &'a dyn Runner,
    pub ask: &'a dyn Ask,
    pub http: &'a dyn Http,
    pub dry_run: bool,
    pub out: &'a mut dyn Write,
}

impl Ctx<'_> {
    /// The path of a file of the system, such as `/etc/ssh/sshd_config`. A
    /// test puts it under its temporary directory.
    pub fn sys_path(&self, path: &str) -> PathBuf {
        match &self.root {
            Some(root) => root.join(path.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(path),
        }
    }

    /// Runs a command that only reads. It runs in dry-run too.
    pub fn query(&self, program: &str, args: &[&str]) -> Result<Output> {
        self.runner.run(program, args)
    }

    /// Runs a command that changes the host. In dry-run it prints the
    /// command, and gives a success with no text.
    pub fn change(&mut self, program: &str, args: &[&str]) -> Result<Output> {
        if self.dry_run {
            let line = std::iter::once(program)
                .chain(args.iter().copied())
                .collect::<Vec<_>>();
            self.say(&format!("would run: {}", line.join(" ")));
            return Ok(Output::ok(""));
        }
        self.runner.run(program, args)
    }

    /// Runs a command that changes the host on the terminal of the person,
    /// so it can ask its own questions. In dry-run it prints the command.
    pub fn interactive(&mut self, program: &str, args: &[&str]) -> Result<i32> {
        if self.dry_run {
            let line = std::iter::once(program)
                .chain(args.iter().copied())
                .collect::<Vec<_>>();
            self.say(&format!("would run: {}", line.join(" ")));
            return Ok(0);
        }
        self.runner.interactive(program, args)
    }

    /// Makes a directory and its parents. In dry-run it prints the path.
    pub fn make_dir(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            return Ok(());
        }
        if self.dry_run {
            self.say(&format!("would make: {}", path.display()));
            return Ok(());
        }
        std::fs::create_dir_all(path).with_context(|| format!("could not make {}", path.display()))
    }

    /// Writes a whole file, and makes its directories. In dry-run it prints
    /// the path.
    pub fn write_file(&mut self, path: &Path, text: &str) -> Result<()> {
        if self.dry_run {
            self.say(&format!("would write: {}", path.display()));
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("could not make {}", dir.display()))?;
        }
        std::fs::write(path, text).with_context(|| format!("could not write {}", path.display()))
    }

    /// Copies the bytes of a file, and makes the folders of `to`. In dry-run
    /// it prints the two paths.
    pub fn copy_file(&mut self, from: &Path, to: &Path) -> Result<()> {
        if self.dry_run {
            self.say(&format!(
                "would copy: {} to {}",
                from.display(),
                to.display()
            ));
            return Ok(());
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("could not make {}", dir.display()))?;
        }
        std::fs::copy(from, to).with_context(|| format!("could not copy to {}", to.display()))?;
        Ok(())
    }

    /// Deletes a folder and each thing in it. A link goes as a link: its
    /// target stays. In dry-run it prints the path.
    pub fn remove_dir(&mut self, path: &Path) -> Result<()> {
        if self.dry_run {
            self.say(&format!("would delete: {}", path.display()));
            return Ok(());
        }
        // remove_dir_all deletes a link, and does not follow it.
        std::fs::remove_dir_all(path)
            .with_context(|| format!("could not delete {}", path.display()))
    }

    /// Deletes a file. In dry-run it prints the path.
    pub fn remove_file(&mut self, path: &Path) -> Result<()> {
        if self.dry_run {
            self.say(&format!("would delete: {}", path.display()));
            return Ok(());
        }
        std::fs::remove_file(path).with_context(|| format!("could not delete {}", path.display()))
    }

    /// Prints one line for the person.
    pub fn say(&mut self, line: &str) {
        let _ = writeln!(self.out, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use crate::host::Os;
    use crate::testing::TestCtx;

    #[test]
    fn dry_run_prints_a_change_and_does_not_run_it() {
        let mut t = TestCtx::new(Os::Linux);
        t.dry_run = true;
        let out = t.ctx().change("systemctl", &["start", "ssh"]).unwrap();
        assert_eq!(out.status, 0);
        assert!(t.runner.calls().is_empty());
        assert!(
            t.output().contains("would run: systemctl start ssh"),
            "{}",
            t.output()
        );
    }

    #[test]
    fn a_query_runs_in_dry_run() {
        let mut t = TestCtx::new(Os::Linux);
        t.dry_run = true;
        t.runner.answer(
            "systemctl",
            &["is-active"],
            crate::run::Output::ok("active\n"),
        );
        let out = t.ctx().query("systemctl", &["is-active", "ssh"]).unwrap();
        assert_eq!(out.stdout, "active\n");
        assert_eq!(
            t.runner.calls(),
            vec![vec!["systemctl", "is-active", "ssh"]]
        );
    }

    #[test]
    fn dry_run_writes_no_file() {
        let mut t = TestCtx::new(Os::Linux);
        t.dry_run = true;
        let path = t.home().join("a/b.txt");
        t.ctx().write_file(&path, "x").unwrap();
        assert!(!path.exists());
        assert!(t.output().contains("would write:"));
    }

    #[test]
    fn a_change_runs_when_not_dry_run() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner
            .answer("systemctl", &["start"], crate::run::Output::ok(""));
        t.ctx().change("systemctl", &["start", "ssh"]).unwrap();
        assert_eq!(t.runner.calls().len(), 1);
    }

    #[test]
    fn copy_file_copies_bytes() {
        let mut t = TestCtx::new(Os::Linux);
        let from = t.dir.path().join("a.bin");
        std::fs::write(&from, [0u8, 255, 7]).unwrap();
        let to = t.home().join("x/y/a.bin");
        t.ctx().copy_file(&from, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), [0u8, 255, 7]);
    }

    #[cfg(unix)]
    #[test]
    fn remove_dir_keeps_the_target_of_a_link() {
        let mut t = TestCtx::new(Os::Linux);
        let target = t.dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("keep.txt"), "x").unwrap();
        let cache = t.dir.path().join("cache");
        std::fs::create_dir_all(cache.join("sub")).unwrap();
        std::fs::write(cache.join("sub/f"), "y").unwrap();
        std::os::unix::fs::symlink(&target, cache.join("link")).unwrap();
        t.ctx().remove_dir(&cache).unwrap();
        assert!(!cache.exists());
        assert!(target.join("keep.txt").exists());
    }

    #[test]
    fn dry_run_copies_and_removes_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        t.dry_run = true;
        let from = t.dir.path().join("a");
        std::fs::write(&from, "x").unwrap();
        let dir = t.dir.path().join("d");
        std::fs::create_dir_all(&dir).unwrap();
        let to = t.home().join("b");
        t.ctx().copy_file(&from, &to).unwrap();
        t.ctx().remove_dir(&dir).unwrap();
        assert!(!to.exists() && dir.exists());
    }

    #[test]
    fn write_file_makes_the_parent_directories() {
        let mut t = TestCtx::new(Os::Linux);
        let path = t.home().join("a/b.txt");
        t.ctx().write_file(&path, "x").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "x");
    }
}
