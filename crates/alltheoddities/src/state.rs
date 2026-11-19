//! The record of the changes that an oddity made, so `--undo` can revert them.
//!
//! Each oddity has one file in the state directory. It holds no secret.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ctx::Ctx;

/// One step that reverts one change.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Undo {
    /// Removes the managed block `name` from a file.
    RemoveBlock {
        path: PathBuf,
        name: String,
        comment: String,
    },
    /// Puts back the text of a file, or deletes it when `before` is none.
    /// It skips a file that changed again after `odd` wrote it.
    RestoreFile {
        path: PathBuf,
        before: Option<String>,
        after_sha256: String,
    },
    /// Runs a host command.
    Run { program: String, args: Vec<String> },
    /// Calls `Oddity::undo_call` of the oddity that recorded it.
    Call { action: String, arg: String },
}

impl Undo {
    fn describe(&self) -> String {
        match self {
            Undo::RemoveBlock { path, name, .. } => {
                format!("remove the block {name} from {}", path.display())
            }
            Undo::RestoreFile { path, .. } => format!("put back {}", path.display()),
            Undo::Run { program, args } => format!("run {program} {}", args.join(" ")),
            Undo::Call { action, arg } => format!("{action} {arg}"),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    steps: Vec<Undo>,
}

/// What `undo` did, and what it skipped, with the reason.
#[derive(Debug, Default)]
pub struct UndoReport {
    pub undone: Vec<String>,
    pub skipped: Vec<String>,
}

/// The SHA-256 of some bytes, as lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn path_of(ctx: &Ctx<'_>, oddity: &str) -> PathBuf {
    ctx.state_dir.join(format!("{oddity}.json"))
}

/// The recorded steps of an oddity, oldest first.
pub fn steps(ctx: &Ctx<'_>, oddity: &str) -> Result<Vec<Undo>> {
    Ok(load(&path_of(ctx, oddity))?.steps)
}

fn load(path: &Path) -> Result<File> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("the state file {} is damaged", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(File::default()),
        Err(e) => Err(e).with_context(|| format!("could not read {}", path.display())),
    }
}

fn save(path: &Path, file: &File) -> Result<()> {
    if file.steps.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(file)?)
        .with_context(|| format!("could not write {}", path.display()))
}

/// Records a step before the change that it reverts. A step that is there
/// already is not added again. A second change to a file keeps the first
/// text of the file, and takes the new checksum. In dry-run it records
/// nothing.
pub fn record(ctx: &mut Ctx<'_>, oddity: &str, step: Undo) -> Result<()> {
    if ctx.dry_run {
        return Ok(());
    }
    let path = path_of(ctx, oddity);
    let mut file = load(&path)?;
    if file.steps.contains(&step) {
        return Ok(());
    }
    if let Undo::RestoreFile {
        path: p,
        after_sha256: new_sum,
        ..
    } = &step
    {
        for old in &mut file.steps {
            if let Undo::RestoreFile {
                path: q,
                after_sha256,
                ..
            } = old
                && q == p
            {
                *after_sha256 = new_sum.clone();
                return save(&path, &file);
            }
        }
    }
    file.steps.push(step);
    save(&path, &file)
}

/// Puts `body` in the managed block `name` of a file, and records how to
/// revert it: a file that `odd` makes goes away on undo, and in a file of the
/// person undo removes only the block. Gives true when the file changed.
pub fn upsert_block(
    ctx: &mut Ctx<'_>,
    oddity: &str,
    path: &Path,
    name: &str,
    body: &str,
    before: &dyn Fn(&str) -> bool,
) -> Result<bool> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let new = crate::block::upsert_before(&text, name, body, "#", before);
    if new == text {
        return Ok(false);
    }
    let step = if path.exists() {
        Undo::RemoveBlock {
            path: path.to_owned(),
            name: name.into(),
            comment: "#".into(),
        }
    } else {
        Undo::RestoreFile {
            path: path.to_owned(),
            before: None,
            after_sha256: sha256_hex(new.as_bytes()),
        }
    };
    record(ctx, oddity, step)?;
    ctx.write_file(path, &new)?;
    Ok(true)
}

/// Reverts the recorded steps of an oddity, newest first. `call` handles a
/// [`Undo::Call`] step. The steps that it skips stay in the file.
pub fn undo(
    ctx: &mut Ctx<'_>,
    oddity: &str,
    call: &mut dyn FnMut(&str, &str, &mut Ctx<'_>) -> Result<()>,
) -> Result<UndoReport> {
    let path = path_of(ctx, oddity);
    let file = load(&path)?;
    let mut report = UndoReport::default();
    let mut kept = Vec::new();
    for step in file.steps.iter().rev() {
        match undo_one(ctx, step, call) {
            Ok(()) => report.undone.push(step.describe()),
            Err(why) => {
                report.skipped.push(format!("{}: {why:#}", step.describe()));
                kept.push(step.clone());
            }
        }
    }
    if !ctx.dry_run {
        kept.reverse();
        save(&path, &File { steps: kept })?;
    }
    Ok(report)
}

fn undo_one(
    ctx: &mut Ctx<'_>,
    step: &Undo,
    call: &mut dyn FnMut(&str, &str, &mut Ctx<'_>) -> Result<()>,
) -> Result<()> {
    match step {
        Undo::RemoveBlock {
            path,
            name,
            comment,
        } => {
            let Ok(text) = std::fs::read_to_string(path) else {
                return Ok(());
            };
            if crate::block::contains(&text, name, comment) {
                ctx.write_file(path, &crate::block::remove(&text, name, comment))?;
            }
            Ok(())
        }
        Undo::RestoreFile {
            path,
            before,
            after_sha256,
        } => {
            let now = std::fs::read(path).context("the file is gone")?;
            if sha256_hex(&now) != *after_sha256 {
                anyhow::bail!("the file changed after odd wrote it, so odd keeps it");
            }
            match before {
                Some(text) => ctx.write_file(path, text),
                None => ctx.remove_file(path),
            }
        }
        Undo::Run { program, args } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = ctx.change(program, &args)?;
            if !out.success() {
                anyhow::bail!("it failed: {}", out.stderr.trim());
            }
            Ok(())
        }
        Undo::Call { action, arg } => call(action, arg, ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::run::Output;
    use crate::testing::TestCtx;

    fn run(program: &str, arg: &str) -> Undo {
        Undo::Run {
            program: program.into(),
            args: vec![arg.into()],
        }
    }

    fn no_call() -> impl FnMut(&str, &str, &mut Ctx<'_>) -> anyhow::Result<()> {
        |_: &str, _: &str, _: &mut Ctx<'_>| panic!("no call step in this test")
    }

    #[test]
    fn the_same_step_twice_is_recorded_once() {
        let mut t = TestCtx::new(Os::Linux);
        record(&mut t.ctx(), "x", run("a", "1")).unwrap();
        record(&mut t.ctx(), "x", run("a", "1")).unwrap();
        assert_eq!(steps(&t.ctx(), "x").unwrap().len(), 1);
    }

    #[test]
    fn undo_runs_newest_first() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("a", &[], Output::ok(""));
        record(&mut t.ctx(), "x", run("a", "1")).unwrap();
        record(&mut t.ctx(), "x", run("a", "2")).unwrap();
        let report = undo(&mut t.ctx(), "x", &mut no_call()).unwrap();
        assert_eq!(t.runner.calls(), vec![vec!["a", "2"], vec!["a", "1"]]);
        assert_eq!(report.undone.len(), 2);
        assert!(!t.state_dir().join("x.json").exists());
    }

    #[test]
    fn undo_skips_a_file_that_changed_again() {
        let mut t = TestCtx::new(Os::Linux);
        let path = t.home().join("f");
        t.ctx().write_file(&path, "odd wrote this").unwrap();
        let step = Undo::RestoreFile {
            path: path.clone(),
            before: Some("old".into()),
            after_sha256: sha256_hex(b"odd wrote this"),
        };
        record(&mut t.ctx(), "x", step).unwrap();
        std::fs::write(&path, "the person changed it").unwrap();
        let report = undo(&mut t.ctx(), "x", &mut no_call()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "the person changed it"
        );
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(
            steps(&t.ctx(), "x").unwrap().len(),
            1,
            "the skipped step stays"
        );
    }

    #[test]
    fn undo_puts_back_a_file_that_did_not_exist() {
        let mut t = TestCtx::new(Os::Linux);
        let path = t.home().join("f");
        t.ctx().write_file(&path, "new").unwrap();
        let step = Undo::RestoreFile {
            path: path.clone(),
            before: None,
            after_sha256: sha256_hex(b"new"),
        };
        record(&mut t.ctx(), "x", step).unwrap();
        undo(&mut t.ctx(), "x", &mut no_call()).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn a_second_change_to_a_file_keeps_the_first_before() {
        let mut t = TestCtx::new(Os::Linux);
        let path = t.home().join("f");
        let first = Undo::RestoreFile {
            path: path.clone(),
            before: Some("original".into()),
            after_sha256: sha256_hex(b"one"),
        };
        let second = Undo::RestoreFile {
            path: path.clone(),
            before: Some("one".into()),
            after_sha256: sha256_hex(b"two"),
        };
        record(&mut t.ctx(), "x", first).unwrap();
        record(&mut t.ctx(), "x", second).unwrap();
        t.ctx().write_file(&path, "two").unwrap();
        undo(&mut t.ctx(), "x", &mut no_call()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    }

    #[test]
    fn undo_removes_a_block() {
        let mut t = TestCtx::new(Os::Linux);
        let path = t.home().join("profile");
        t.ctx()
            .write_file(&path, &crate::block::upsert("a\n", "path", "x", "#"))
            .unwrap();
        let step = Undo::RemoveBlock {
            path: path.clone(),
            name: "path".into(),
            comment: "#".into(),
        };
        record(&mut t.ctx(), "x", step).unwrap();
        undo(&mut t.ctx(), "x", &mut no_call()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\n");
    }

    #[test]
    fn undo_passes_a_call_step_to_the_oddity() {
        let mut t = TestCtx::new(Os::Linux);
        let step = Undo::Call {
            action: "remove-entry".into(),
            arg: "/x".into(),
        };
        record(&mut t.ctx(), "x", step).unwrap();
        let mut seen = Vec::new();
        let mut call = |a: &str, b: &str, _: &mut Ctx<'_>| {
            seen.push(format!("{a} {b}"));
            Ok(())
        };
        undo(&mut t.ctx(), "x", &mut call).unwrap();
        assert_eq!(seen, vec!["remove-entry /x"]);
    }

    #[test]
    fn dry_run_records_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        t.dry_run = true;
        record(&mut t.ctx(), "x", run("a", "1")).unwrap();
        assert!(!t.state_dir().join("x.json").exists());
    }
}
