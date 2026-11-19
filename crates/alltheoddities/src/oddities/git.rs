//! `odd git`: a commit name and email for each folder, and signed commits.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "git";

/// `odd git`.
pub struct Git;

impl Oddity for Git {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Set the commit name and email for each folder, and sign commits with the SSH key."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("add")
                .about("Use a name and an email for each repository under a folder")
                .arg(
                    Arg::new("folder")
                        .required(true)
                        .help("The folder of the repositories"),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .required(true)
                        .help("The commit name"),
                )
                .arg(
                    Arg::new("email")
                        .long("email")
                        .required(true)
                        .help("The commit email"),
                )
                .arg(
                    Arg::new("sign")
                        .long("sign")
                        .action(ArgAction::SetTrue)
                        .help("Sign commits and tags with the SSH key of odd ssh"),
                ),
        )
        .subcommand(Command::new("list").about("List each folder with its name and email"))
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match m.subcommand() {
            Some(("add", a)) => {
                let get = |k: &str| a.get_one::<String>(k).cloned().unwrap_or_default();
                add(
                    ctx,
                    &get("folder"),
                    &get("name"),
                    &get("email"),
                    a.get_flag("sign"),
                )?;
                Ok(Exit::Done)
            }
            Some(("list", _)) => {
                list(ctx);
                Ok(Exit::Done)
            }
            _ => {
                ctx.say("Give a command: odd git add <folder> --name <name> --email <email>, or odd git list.");
                Ok(Exit::Usage)
            }
        }
    }
}

/// The folder as git matches it: full, with its links resolved, with
/// forward slashes, and with a final `/`, so it matches each repository
/// under it.
pub fn full_folder(folder: &str, os: Os) -> Result<String> {
    let path = std::fs::canonicalize(folder)
        .with_context(|| format!("the folder {folder} does not exist"))?;
    let mut s = path.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_owned();
    }
    if os == Os::Windows {
        s = s.replace('\\', "/");
    }
    if !s.ends_with('/') {
        s.push('/');
    }
    Ok(s)
}

/// A value in double quotes, as git reads it.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn slashes(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Writes a whole file and records how to put back the text before it.
fn write_recorded(ctx: &mut Ctx<'_>, path: &std::path::Path, text: &str) -> Result<()> {
    let before = std::fs::read_to_string(path).ok();
    if before.as_deref() == Some(text) {
        return Ok(());
    }
    let step = Undo::RestoreFile {
        path: path.to_owned(),
        before,
        after_sha256: state::sha256_hex(text.as_bytes()),
    };
    state::record(ctx, NAME, step)?;
    ctx.write_file(path, text)
}

fn add(ctx: &mut Ctx<'_>, folder: &str, name: &str, email: &str, sign: bool) -> Result<()> {
    let os = ctx.host.os;
    let folder = full_folder(folder, os)?;
    let last = folder
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("folder");
    let safe: String = last
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let profile: PathBuf = ctx.home.join(format!(".gitconfig-{safe}"));

    let mut text = format!(
        "[user]\n\tname = {}\n\temail = {}\n",
        quote(name),
        quote(email)
    );
    if sign {
        let public = ctx.home.join(".ssh").join("id_ed25519.pub");
        let Ok(key) = std::fs::read_to_string(&public) else {
            ctx.say("--sign needs the key of odd ssh. Run odd ssh --out first.");
            bail!("no public key at {}", public.display());
        };
        let signers = ctx.home.join(".ssh").join("allowed_signers");
        text.push_str(&format!(
            "\tsigningkey = {}\n[gpg]\n\tformat = ssh\n[gpg \"ssh\"]\n\tallowedSignersFile = {}\n[commit]\n\tgpgsign = true\n[tag]\n\tgpgsign = true\n",
            quote(&slashes(&public)),
            quote(&slashes(&signers)),
        ));
        let line = format!("{email} namespaces=\"git\" {}", key.trim());
        let old = std::fs::read_to_string(&signers).unwrap_or_default();
        if !old.lines().any(|l| l.trim() == line) {
            let mut new = old.clone();
            if !new.is_empty() && !new.ends_with('\n') {
                new.push('\n');
            }
            new.push_str(&line);
            new.push('\n');
            write_recorded(ctx, &signers, &new)?;
        }
    }
    write_recorded(ctx, &profile, &text)?;

    // Linux has case-sensitive paths. macOS and Windows do not.
    let kind = if os == Os::Linux {
        "gitdir"
    } else {
        "gitdir/i"
    };
    let config = ctx.home.join(".gitconfig");
    let old = std::fs::read_to_string(&config).unwrap_or_default();
    let mut entries: Vec<(String, String)> = includes(&old)
        .into_iter()
        .filter(|(f, _)| !f.ends_with(&format!(":{folder}")))
        .collect();
    entries.push((format!("{kind}:{folder}"), slashes(&profile)));
    let body: Vec<String> = entries
        .iter()
        .map(|(f, p)| format!("[includeIf \"{f}\"]\n\tpath = {}", quote(p)))
        .collect();
    state::upsert_block(ctx, NAME, &config, NAME, &body.join("\n"), &|_: &str| false)?;
    ctx.say(&format!(
        "Each repository under {folder} commits as {name} <{email}>."
    ));
    Ok(())
}

/// The `gitdir` conditions and the profile files in the block of odd.
fn includes(config: &str) -> Vec<(String, String)> {
    let Some(body) = crate::block::body(config, NAME, "#") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cond = None;
    for line in body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[includeIf \"") {
            cond = rest.strip_suffix("\"]").map(str::to_owned);
        } else if let Some(path) = line.strip_prefix("path = ")
            && let Some(c) = cond.take()
        {
            out.push((c, path.trim_matches('"').replace("\\\\", "\\")));
        }
    }
    out
}

fn value(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix(&format!("{key} = "))
                .map(|v| v.trim_matches('"').to_owned())
        })
        .unwrap_or_default()
}

fn list(ctx: &mut Ctx<'_>) {
    let config = std::fs::read_to_string(ctx.home.join(".gitconfig")).unwrap_or_default();
    let entries = includes(&config);
    if entries.is_empty() {
        ctx.say("odd git has no folder yet.");
    }
    for (cond, path) in entries {
        let folder = cond
            .split_once(':')
            .map(|(_, f)| f.to_owned())
            .unwrap_or(cond);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        ctx.say(&format!(
            "{folder}  {} <{}>",
            value(&text, "name"),
            value(&text, "email")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "git"].iter().chain(args).map(Into::into).collect();
        dispatch(argv, &mut t.ctx(), &[&Git])
    }

    fn folder(t: &TestCtx, name: &str) -> String {
        let p = t.dir.path().join(name);
        std::fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn read(t: &TestCtx, name: &str) -> String {
        std::fs::read_to_string(t.home().join(name)).unwrap()
    }

    #[test]
    fn add_writes_the_profile_and_the_include() {
        let mut t = TestCtx::new(Os::MacOs);
        let work = folder(&t, "work");
        assert_eq!(
            odd(
                &mut t,
                &["add", &work, "--name", "Ann", "--email", "ann@work.example"]
            ),
            Exit::Done
        );
        let profile = read(&t, ".gitconfig-work");
        assert!(
            profile.contains("name = \"Ann\"") && profile.contains("email = \"ann@work.example\""),
            "{profile}"
        );
        let config = read(&t, ".gitconfig");
        let canon = full_folder(&work, Os::MacOs).unwrap();
        assert!(
            config.contains(&format!("[includeIf \"gitdir/i:{canon}\"]")),
            "{config}"
        );
        assert!(canon.ends_with('/'));
    }

    #[test]
    fn a_second_folder_joins_the_block() {
        let mut t = TestCtx::new(Os::MacOs);
        let (a, b) = (folder(&t, "a"), folder(&t, "b"));
        odd(&mut t, &["add", &a, "--name", "A", "--email", "a@x"]);
        odd(&mut t, &["add", &b, "--name", "B", "--email", "b@x"]);
        let config = read(&t, ".gitconfig");
        assert_eq!(config.matches(">>> odd git >>>").count(), 1);
        assert_eq!(config.matches("[includeIf").count(), 2, "{config}");
    }

    #[test]
    fn the_same_folder_twice_writes_one_include() {
        let mut t = TestCtx::new(Os::MacOs);
        let a = folder(&t, "a");
        odd(&mut t, &["add", &a, "--name", "A", "--email", "a@x"]);
        odd(&mut t, &["add", &a, "--name", "A2", "--email", "a2@x"]);
        assert_eq!(read(&t, ".gitconfig").matches("[includeIf").count(), 1);
        assert!(read(&t, ".gitconfig-a").contains("A2"));
    }

    #[test]
    fn a_value_with_a_quote_is_escaped() {
        let mut t = TestCtx::new(Os::Linux);
        let a = folder(&t, "a");
        odd(
            &mut t,
            &["add", &a, "--name", r#"Ann "the" O\Neil"#, "--email", "a@x"],
        );
        assert!(read(&t, ".gitconfig-a").contains(r#"name = "Ann \"the\" O\\Neil""#));
    }

    #[test]
    fn a_missing_folder_fails() {
        let mut t = TestCtx::new(Os::Linux);
        let missing = t.dir.path().join("nope").to_string_lossy().into_owned();
        assert_eq!(
            odd(&mut t, &["add", &missing, "--name", "A", "--email", "a@x"]),
            Exit::Failed
        );
    }

    #[test]
    fn sign_needs_a_public_key() {
        let mut t = TestCtx::new(Os::Linux);
        let a = folder(&t, "a");
        assert_eq!(
            odd(
                &mut t,
                &["add", &a, "--name", "A", "--email", "a@x", "--sign"]
            ),
            Exit::Failed
        );
        assert!(t.output().contains("odd ssh --out"), "{}", t.output());
    }

    #[test]
    fn sign_adds_the_allowed_signer() {
        let mut t = TestCtx::new(Os::Linux);
        let a = folder(&t, "a");
        std::fs::create_dir_all(t.home().join(".ssh")).unwrap();
        std::fs::write(
            t.home().join(".ssh/id_ed25519.pub"),
            "ssh-ed25519 AAAA me\n",
        )
        .unwrap();
        odd(
            &mut t,
            &["add", &a, "--name", "A", "--email", "a@x", "--sign"],
        );
        let profile = read(&t, ".gitconfig-a");
        assert!(
            profile.contains("format = ssh") && profile.contains("gpgsign = true"),
            "{profile}"
        );
        assert_eq!(
            read(&t, ".ssh/allowed_signers"),
            "a@x namespaces=\"git\" ssh-ed25519 AAAA me\n"
        );
    }

    #[test]
    fn linux_uses_gitdir_and_others_gitdir_i() {
        let mut t = TestCtx::new(Os::Linux);
        let a = folder(&t, "a");
        odd(&mut t, &["add", &a, "--name", "A", "--email", "a@x"]);
        assert!(read(&t, ".gitconfig").contains("\"gitdir:"));
    }

    #[test]
    fn list_shows_each_folder() {
        let mut t = TestCtx::new(Os::Linux);
        let a = folder(&t, "a");
        odd(&mut t, &["add", &a, "--name", "Ann", "--email", "a@x"]);
        t.out.clear();
        odd(&mut t, &["list"]);
        assert!(t.output().contains("Ann <a@x>"), "{}", t.output());
    }

    #[test]
    fn undo_removes_the_block_and_the_file() {
        let mut t = TestCtx::new(Os::Linux);
        let a = folder(&t, "a");
        odd(&mut t, &["add", &a, "--name", "A", "--email", "a@x"]);
        assert_eq!(odd(&mut t, &["--undo"]), Exit::Done);
        assert!(!t.home().join(".gitconfig").exists());
        assert!(!t.home().join(".gitconfig-a").exists());
    }
}
