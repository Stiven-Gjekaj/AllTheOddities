//! `odd carry`: send settings files to another computer of the tailnet.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "carry";
const TAR: &str = "odd-carry.tar";

/// `odd carry`.
pub struct Carry;

impl Oddity for Carry {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Send settings files to another computer of your tailnet, and receive them there."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("send")
                .about("Send files of your home directory to a computer of the tailnet")
                .arg(Arg::new("computer").required(true).help("The tailnet name of the other computer"))
                .arg(Arg::new("files").action(ArgAction::Append).help("The files. The default is your settings files")),
        )
        .subcommand(
            Command::new("receive")
                .about("Wait for the files, unpack them, and list them")
                .arg(
                    Arg::new("apply")
                        .long("apply")
                        .action(ArgAction::SetTrue)
                        .help("Copy the files into your home directory. odd carry --undo puts back the old ones"),
                ),
        )
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match m.subcommand() {
            Some(("send", a)) => {
                let computer = a.get_one::<String>("computer").cloned().unwrap_or_default();
                let files: Vec<String> = a
                    .get_many::<String>("files")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                send(ctx, &computer, &files)?;
                Ok(Exit::Done)
            }
            Some(("receive", a)) => {
                receive(ctx, a.get_flag("apply"))?;
                Ok(Exit::Done)
            }
            _ => {
                ctx.say("Give a command: odd carry send <computer> [files], or odd carry receive.");
                Ok(Exit::Usage)
            }
        }
    }
}

fn tailscale(ctx: &Ctx<'_>) -> Result<String> {
    match crate::oddities::ssh::tailscale::cli(ctx) {
        Some(c) => Ok(c),
        None => bail!(
            "this computer has no Tailscale. Run odd ssh --tailscale first, on both computers"
        ),
    }
}

/// The settings files of the home directory that exist. No private key is
/// in the list.
fn default_files(home: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut add = |rel: &str| {
        if home.join(rel).is_file() {
            out.push(rel.to_owned());
        }
    };
    add(".ssh/config");
    add(".gitconfig");
    let mut extra: Vec<String> = std::fs::read_dir(home)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(".gitconfig-"))
                .collect()
        })
        .unwrap_or_default();
    extra.sort();
    for n in &extra {
        add(n);
    }
    for f in [".profile", ".zprofile", ".bashrc", ".zshrc"] {
        add(f);
    }
    out
}

/// A file as a path relative to the home directory, with forward slashes.
fn relative(home: &Path, file: &str) -> Result<String> {
    let full = std::fs::canonicalize(file).with_context(|| format!("{file} does not exist"))?;
    let home = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_owned());
    let rel = full.strip_prefix(&home).map_err(|_| {
        anyhow::anyhow!("{file} is not in your home directory, so odd carry does not send it")
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn send(ctx: &mut Ctx<'_>, computer: &str, files: &[String]) -> Result<()> {
    let rels = if files.is_empty() {
        default_files(&ctx.home)
    } else {
        files
            .iter()
            .map(|f| relative(&ctx.home, f))
            .collect::<Result<Vec<_>>>()?
    };
    if rels.is_empty() {
        bail!("odd found no settings file to send");
    }
    let cli = tailscale(ctx)?;
    let out_dir = ctx.state_dir.join("carry-out");
    ctx.make_dir(&out_dir)?;
    let tar = out_dir.join(TAR);
    let tar_s = tar.to_string_lossy().into_owned();
    let home = ctx.home.to_string_lossy().into_owned();
    let mut args = vec!["-cf", &tar_s, "-C", &home];
    args.extend(rels.iter().map(String::as_str));
    let out = ctx.change("tar", &args)?;
    if !out.success() {
        bail!("tar failed: {}", out.stderr.trim());
    }
    let target = format!("{computer}:");
    let out = ctx.change(&cli, &["file", "cp", &tar_s, &target]);
    let _ = std::fs::remove_file(&tar);
    let out = out?;
    if !out.success() {
        bail!(
            "Taildrop could not send to {computer}: {}",
            out.stderr.trim()
        );
    }
    ctx.say(&format!("Sent to {computer}:"));
    for r in &rels {
        ctx.say(&format!("  {r}"));
    }
    ctx.say("On the other computer, run: odd carry receive");
    Ok(())
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_owned());
        }
    }
}

fn receive(ctx: &mut Ctx<'_>, apply: bool) -> Result<()> {
    let cli = tailscale(ctx)?;
    let staging = ctx.state_dir.join("carry");
    let files_dir = staging.join("files");
    ctx.make_dir(&files_dir)?;
    let staging_s = staging.to_string_lossy().into_owned();
    ctx.say("Waiting for the files. Run odd carry send on the other computer.");
    if ctx.interactive(
        &cli,
        &["file", "get", "--wait", "--conflict=rename", &staging_s],
    )? != 0
    {
        bail!("Taildrop could not receive the files");
    }
    let tars: Vec<PathBuf> = std::fs::read_dir(&staging)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().is_some_and(|n| {
                        n.to_string_lossy().starts_with("odd-carry")
                            && n.to_string_lossy().ends_with(".tar")
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let files_s = files_dir.to_string_lossy().into_owned();
    for tar in tars {
        let t = tar.to_string_lossy().into_owned();
        let out = ctx.change("tar", &["-xf", &t, "-C", &files_s])?;
        if !out.success() {
            bail!("tar could not unpack {t}: {}", out.stderr.trim());
        }
        let _ = std::fs::remove_file(&tar);
    }
    let mut rels = Vec::new();
    walk(&files_dir, &files_dir, &mut rels);
    rels.sort();
    if rels.is_empty() {
        ctx.say("No file came.");
        return Ok(());
    }
    ctx.say(&format!("The files are in {}:", files_dir.display()));
    for r in &rels {
        ctx.say(&format!("  {}", r.to_string_lossy().replace('\\', "/")));
    }
    if !apply {
        ctx.say("Run odd carry receive --apply to copy them into your home directory.");
        return Ok(());
    }
    for r in &rels {
        let from = files_dir.join(r);
        let to = ctx.home.join(r);
        let Ok(text) = std::fs::read_to_string(&from) else {
            ctx.say(&format!(
                "{} is not text, so odd does not copy it.",
                r.display()
            ));
            continue;
        };
        let before = std::fs::read_to_string(&to).ok();
        if before.as_deref() == Some(text.as_str()) {
            continue;
        }
        let step = Undo::RestoreFile {
            path: to.clone(),
            before,
            after_sha256: state::sha256_hex(text.as_bytes()),
        };
        state::record(ctx, NAME, step)?;
        ctx.write_file(&to, &text)?;
    }
    ctx.say("The files are in your home directory. odd carry --undo puts back the old ones.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "carry"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Carry])
    }

    fn host() -> TestCtx {
        let t = TestCtx::new(Os::Linux);
        t.runner.with_path(&["tailscale"]);
        t.runner.answer("tailscale", &[], Output::ok(""));
        t.runner.answer("tar", &[], Output::ok(""));
        for f in [
            ".gitconfig",
            ".gitconfig-work",
            ".zshrc",
            ".ssh/config",
            ".ssh/id_ed25519",
            "notes.txt",
        ] {
            let p = t.home().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, f).unwrap();
        }
        t
    }

    fn tar_call(t: &TestCtx) -> Vec<String> {
        t.runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "tar")
            .unwrap()
    }

    #[test]
    fn send_takes_the_default_files_that_exist() {
        let mut t = host();
        assert_eq!(odd(&mut t, &["send", "laptop"]), Exit::Done);
        let call = tar_call(&t);
        let files = &call[call.iter().position(|a| a == "-C").unwrap() + 2..];
        assert_eq!(
            files,
            [".ssh/config", ".gitconfig", ".gitconfig-work", ".zshrc"]
        );
    }

    #[test]
    fn send_never_takes_a_private_key_by_default() {
        let mut t = host();
        odd(&mut t, &["send", "laptop"]);
        assert!(!tar_call(&t).iter().any(|a| a.contains("id_ed25519")));
    }

    #[test]
    fn a_named_file_goes_when_the_person_names_it() {
        let mut t = host();
        let key = t
            .home()
            .join(".ssh/id_ed25519")
            .to_string_lossy()
            .into_owned();
        odd(&mut t, &["send", "laptop", &key]);
        let call = tar_call(&t);
        assert_eq!(call.last().unwrap(), ".ssh/id_ed25519");
    }

    #[test]
    fn a_file_outside_home_is_refused() {
        let mut t = host();
        let outside = t.dir.path().join("x.txt");
        std::fs::write(&outside, "x").unwrap();
        assert_eq!(
            odd(&mut t, &["send", "laptop", &outside.to_string_lossy()]),
            Exit::Failed
        );
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn send_runs_tar_then_taildrop() {
        let mut t = host();
        odd(&mut t, &["send", "laptop"]);
        let calls = t.runner.calls();
        assert_eq!(calls[0][0], "tar");
        assert_eq!(calls[1][..3], ["tailscale", "file", "cp"]);
        assert_eq!(calls[1][4], "laptop:");
        assert!(calls[1][3].ends_with("odd-carry.tar"));
    }

    fn received(t: &TestCtx) {
        let staging = t.state_dir().join("carry");
        std::fs::create_dir_all(staging.join("files/.ssh")).unwrap();
        std::fs::write(staging.join("odd-carry.tar"), "tar").unwrap();
        std::fs::write(staging.join("files/.ssh/config"), "Host new\n").unwrap();
        std::fs::write(staging.join("files/.gitconfig"), "[user]\n").unwrap();
    }

    #[test]
    fn receive_lists_the_files() {
        let mut t = host();
        received(&t);
        assert_eq!(odd(&mut t, &["receive"]), Exit::Done);
        let out = t.output();
        assert!(
            out.contains(".ssh/config") && out.contains(".gitconfig"),
            "{out}"
        );
        assert!(out.contains("--apply"), "{out}");
        assert_eq!(
            std::fs::read_to_string(t.home().join(".gitconfig")).unwrap(),
            ".gitconfig"
        );
        assert!(
            t.runner
                .calls()
                .iter()
                .any(|c| c[0] == "tar" && c[1] == "-xf")
        );
    }

    #[test]
    fn apply_copies_and_undo_puts_back() {
        let mut t = host();
        received(&t);
        assert_eq!(odd(&mut t, &["receive", "--apply"]), Exit::Done);
        assert_eq!(
            std::fs::read_to_string(t.home().join(".ssh/config")).unwrap(),
            "Host new\n"
        );
        assert_eq!(odd(&mut t, &["--undo"]), Exit::Done);
        assert_eq!(
            std::fs::read_to_string(t.home().join(".ssh/config")).unwrap(),
            ".ssh/config"
        );
    }
}
