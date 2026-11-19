//! The "out" part of `odd ssh`: connect from this computer to GitHub.

use std::path::Path;

use anyhow::{Result, bail};
use clap::ArgMatches;

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::state::{self, Undo};

use super::NAME;

/// Does the out part. Gives `Exit::Failed` when GitHub does not accept the key.
pub fn run(m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
    let key = super::key_path(m, ctx);
    let os = ctx.host.os;
    let ssh_dir = key
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ctx.home.join(".ssh"));

    // 1. The directory. SSH refuses keys in a directory that others can read.
    ctx.make_dir(&ssh_dir)?;
    if os != Os::Windows {
        chmod(ctx, "700", &ssh_dir)?;
    }

    // 2. The key. A key that exists is kept: a deleted key cannot come back.
    if key.exists() {
        ctx.say(&format!("Key exists: {}. It is kept.", key.display()));
    } else {
        let comment = match m.get_one::<String>("comment") {
            Some(c) => c.clone(),
            None => default_comment(ctx),
        };
        let key_s = key.to_string_lossy().into_owned();
        let mut args = vec!["-q", "-t", "ed25519", "-C", &comment, "-f", &key_s];
        if m.get_flag("no-passphrase") {
            args.extend(["-N", ""]);
        }
        if ctx.interactive("ssh-keygen", &args)? != 0 {
            bail!("ssh-keygen failed");
        }
        ctx.say(&format!("Key made: {}", key.display()));
    }

    // 3. The config block.
    let config = ctx.home.join(".ssh").join("config");
    let mut body = vec![
        "Host github.com".to_owned(),
        "    HostName github.com".to_owned(),
        "    User git".to_owned(),
        format!(
            "    IdentityFile {}",
            key.to_string_lossy().replace('\\', "/")
        ),
        "    IdentitiesOnly yes".to_owned(),
        "    AddKeysToAgent yes".to_owned(),
    ];
    if os == Os::MacOs {
        body.push("    UseKeychain yes".to_owned());
    }
    if state::upsert_block(
        ctx,
        NAME,
        &config,
        "github",
        &body.join("\n"),
        &|_: &str| false,
    )? {
        ctx.say(&format!("The GitHub block is in {}.", config.display()));
    }
    if os != Os::Windows {
        chmod(ctx, "600", &config)?;
    }

    // 4. The agent.
    agent(ctx, &key)?;

    // 5. Git on Windows.
    if os == Os::Windows {
        git_ssh(ctx)?;
    }

    // 6. GitHub.
    send_key(ctx, &key)?;

    // 7. The test. GitHub ends the session with exit code 1 when the key is good.
    if ctx.dry_run {
        ctx.say("would run: ssh -T git@github.com");
        return Ok(Exit::Done);
    }
    let out = ctx.change(
        "ssh",
        &[
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "BatchMode=yes",
            "-T",
            "git@github.com",
        ],
    )?;
    let text = format!("{}{}", out.stdout, out.stderr);
    if text.contains("successfully authenticated") {
        ctx.say("GitHub accepts the key.");
        Ok(Exit::Done)
    } else {
        ctx.say("GitHub does not accept the key yet:");
        ctx.say(text.trim());
        Ok(Exit::Failed)
    }
}

fn chmod(ctx: &mut Ctx<'_>, mode: &str, path: &Path) -> Result<()> {
    let out = ctx.change("chmod", &[mode, &path.to_string_lossy()])?;
    if !out.success() {
        bail!(
            "chmod {mode} {} failed: {}",
            path.display(),
            out.stderr.trim()
        );
    }
    Ok(())
}

fn default_comment(ctx: &Ctx<'_>) -> String {
    let user = ctx
        .env
        .get("USER")
        .or_else(|| ctx.env.get("USERNAME"))
        .cloned()
        .unwrap_or_default();
    let host = ctx
        .query("hostname", &[])
        .map(|o| o.stdout.trim().split('.').next().unwrap_or("").to_owned())
        .unwrap_or_default();
    format!("{user}@{host}")
}

fn agent(ctx: &mut Ctx<'_>, key: &Path) -> Result<()> {
    let key_s = key.to_string_lossy().into_owned();
    match ctx.host.os {
        Os::MacOs => {
            if ctx.interactive("ssh-add", &["--apple-use-keychain", &key_s])? != 0 {
                ctx.say("The agent did not take the key.");
            }
        }
        Os::Linux => {
            if ctx.env.contains_key("SSH_AUTH_SOCK") {
                if ctx.interactive("ssh-add", &[&key_s])? != 0 {
                    ctx.say("The agent did not take the key.");
                }
            } else {
                ctx.say("No SSH agent runs. Start one with: eval \"$(ssh-agent -s)\"");
            }
        }
        Os::Windows => {
            let running = |ctx: &Ctx<'_>| {
                ctx.query("sc", &["query", "ssh-agent"])
                    .is_ok_and(|o| o.stdout.contains("RUNNING"))
            };
            if !running(ctx) {
                let disabled = ctx
                    .query("sc", &["qc", "ssh-agent"])
                    .is_ok_and(|o| o.stdout.contains("DISABLED"));
                if ctx.is_admin {
                    ctx.change("sc", &["config", "ssh-agent", "start=", "auto"])?;
                    ctx.change("sc", &["start", "ssh-agent"])?;
                    ctx.say("The ssh-agent service runs, and starts with Windows.");
                } else if !disabled {
                    ctx.change("sc", &["start", "ssh-agent"])?;
                } else {
                    ctx.say("The ssh-agent service is off. Run odd ssh --out one time as an administrator to turn it on.");
                    return Ok(());
                }
            }
            if ctx.interactive("ssh-add", &[&key_s])? != 0 {
                ctx.say("The agent did not take the key.");
            }
        }
    }
    Ok(())
}

/// Git for Windows has its own ssh, which does not use the agent of Windows.
/// So Git asks for the passphrase each time. The ssh of Windows fixes it.
fn git_ssh(ctx: &mut Ctx<'_>) -> Result<()> {
    if !ctx.runner.on_path("git") {
        return Ok(());
    }
    let now = ctx.query("git", &["config", "--global", "core.sshCommand"])?;
    if now.success() && !now.stdout.trim().is_empty() {
        ctx.say(&format!(
            "Git uses this ssh already: {}. It is kept.",
            now.stdout.trim()
        ));
        return Ok(());
    }
    let ssh = ctx
        .query("where", &["ssh"])
        .ok()
        .and_then(|o| o.stdout.lines().next().map(str::trim).map(str::to_owned))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| r"C:\Windows\System32\OpenSSH\ssh.exe".to_owned())
        .replace('\\', "/");
    let undo = Undo::Run {
        program: "git".into(),
        args: vec![
            "config".into(),
            "--global".into(),
            "--unset".into(),
            "core.sshCommand".into(),
        ],
    };
    state::record(ctx, NAME, undo)?;
    ctx.change("git", &["config", "--global", "core.sshCommand", &ssh])?;
    ctx.say(&format!("Git uses {ssh} now."));
    Ok(())
}

fn send_key(ctx: &mut Ctx<'_>, key: &Path) -> Result<()> {
    let public = format!("{}.pub", key.to_string_lossy());
    let signed_in = ctx.runner.on_path("gh")
        && ctx
            .query("gh", &["auth", "status"])
            .is_ok_and(|o| o.success());
    if signed_in {
        let title = ctx
            .query("hostname", &[])
            .map(|o| o.stdout.trim().split('.').next().unwrap_or("").to_owned())
            .unwrap_or_default();
        let out = ctx.change("gh", &["ssh-key", "add", &public, "--title", &title])?;
        if out.success() {
            ctx.say(&format!("The key is on GitHub with the title \"{title}\"."));
        } else {
            ctx.say("gh did not add the key. GitHub can have it already.");
        }
    } else {
        ctx.say("gh is not signed in. Add this key at https://github.com/settings/ssh/new:");
        let text = std::fs::read_to_string(&public).unwrap_or_default();
        ctx.say(text.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::exit::Exit;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    const OK_TEXT: &str =
        "Hi you! You've successfully authenticated, but GitHub does not provide shell access.";

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "ssh", "--out"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&super::super::Ssh])
    }

    /// A host where each command works, gh is not signed in, and GitHub
    /// accepts the key.
    fn host(os: Os) -> TestCtx {
        let mut t = TestCtx::new(os);
        for p in ["chmod", "ssh-keygen", "ssh-add", "hostname"] {
            t.runner.answer(p, &[], Output::ok("box\n"));
        }
        t.runner.answer(
            "ssh",
            &[],
            Output {
                status: 1,
                stdout: String::new(),
                stderr: OK_TEXT.into(),
            },
        );
        t.runner.answer(
            "sc",
            &["query"],
            Output::ok("        STATE              : 4  RUNNING\n"),
        );
        t.runner.answer(
            "where",
            &[],
            Output::ok("C:\\Windows\\System32\\OpenSSH\\ssh.exe\r\n"),
        );
        t.runner.answer(
            "git",
            &["config", "--global", "core.sshCommand"],
            Output::fail(1, ""),
        );
        t.runner
            .answer("git", &["config", "--global", "--unset"], Output::ok(""));
        t.env.insert("USER".into(), "you".into());
        t
    }

    fn keygen(t: &TestCtx) -> Option<Vec<String>> {
        t.runner.calls().into_iter().find(|c| c[0] == "ssh-keygen")
    }

    fn config(t: &TestCtx) -> String {
        std::fs::read_to_string(t.home().join(".ssh/config")).unwrap()
    }

    #[test]
    fn out_makes_the_key_with_the_comment() {
        let mut t = host(Os::Linux);
        assert_eq!(
            odd(&mut t, &["--comment", "test@test", "--no-passphrase"]),
            Exit::Done
        );
        let call = keygen(&t).unwrap();
        assert!(
            call.windows(2).any(|w| w == ["-C", "test@test"]),
            "{call:?}"
        );
        assert!(call.windows(2).any(|w| w == ["-N", ""]), "{call:?}");
    }

    #[test]
    fn without_no_passphrase_ssh_keygen_asks_for_it() {
        let mut t = host(Os::Linux);
        odd(&mut t, &[]);
        assert!(!keygen(&t).unwrap().contains(&"-N".to_owned()));
    }

    #[test]
    fn out_keeps_an_existing_key() {
        let mut t = host(Os::Linux);
        std::fs::create_dir_all(t.home().join(".ssh")).unwrap();
        std::fs::write(t.home().join(".ssh/id_ed25519"), "key").unwrap();
        odd(&mut t, &[]);
        assert!(keygen(&t).is_none());
    }

    #[test]
    fn out_keeps_the_old_config_entry() {
        let mut t = host(Os::Linux);
        std::fs::create_dir_all(t.home().join(".ssh")).unwrap();
        std::fs::write(
            t.home().join(".ssh/config"),
            "Host old\n    HostName old.example",
        )
        .unwrap();
        odd(&mut t, &[]);
        let text = config(&t);
        assert!(
            text.contains("Host old\n    HostName old.example\n"),
            "{text}"
        );
        assert!(text.contains("Host github.com"), "{text}");
    }

    #[test]
    fn out_twice_gives_one_block() {
        let mut t = host(Os::Linux);
        odd(&mut t, &[]);
        odd(&mut t, &[]);
        assert_eq!(config(&t).matches("Host github.com").count(), 1);
    }

    #[test]
    fn use_keychain_on_macos_only() {
        let mut mac = host(Os::MacOs);
        odd(&mut mac, &[]);
        assert!(config(&mac).contains("UseKeychain yes"));
        let mut linux = host(Os::Linux);
        odd(&mut linux, &[]);
        assert!(!config(&linux).contains("UseKeychain"));
    }

    #[test]
    fn the_key_path_has_forward_slashes() {
        let mut t = host(Os::Windows);
        t.runner.with_path(&["git"]);
        t.runner.answer(
            "git",
            &["config", "--global", "core.sshCommand", "C:"],
            Output::ok(""),
        );
        odd(&mut t, &["--key", r"C:\Users\you\.ssh\id_ed25519"]);
        assert!(
            config(&t).contains("IdentityFile C:/Users/you/.ssh/id_ed25519"),
            "{}",
            config(&t)
        );
    }

    #[test]
    fn windows_sets_git_ssh_only_when_unset() {
        let mut t = host(Os::Windows);
        t.runner.with_path(&["git"]);
        t.runner.answer(
            "git",
            &["config", "--global", "core.sshCommand", "C:"],
            Output::ok(""),
        );
        odd(&mut t, &[]);
        let set = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c.len() == 5 && c[0] == "git");
        assert_eq!(set.unwrap()[4], "C:/Windows/System32/OpenSSH/ssh.exe");

        let mut u = host(Os::Windows);
        u.runner.with_path(&["git"]);
        u.runner.answer(
            "git",
            &["config", "--global", "core.sshCommand"],
            Output::ok("plink\n"),
        );
        odd(&mut u, &[]);
        assert!(
            !u.runner
                .calls()
                .iter()
                .any(|c| c.len() == 5 && c[0] == "git")
        );
    }

    #[test]
    fn a_disabled_agent_without_admin_says_how_to_turn_it_on() {
        let mut t = host(Os::Windows);
        t.is_admin = false;
        t.runner.answer(
            "sc",
            &["query"],
            Output::ok("        STATE              : 1  STOPPED\n"),
        );
        t.runner.answer(
            "sc",
            &["qc"],
            Output::ok("        START_TYPE         : 4   DISABLED\n"),
        );
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.output().contains("administrator"), "{}", t.output());
        assert!(!t.runner.calls().iter().any(|c| c[0] == "ssh-add"));
    }

    #[test]
    fn gh_signed_in_adds_the_key() {
        let mut t = host(Os::Linux);
        t.runner.with_path(&["gh"]);
        t.runner.answer("gh", &["auth"], Output::ok(""));
        t.runner.answer("gh", &["ssh-key"], Output::ok(""));
        odd(&mut t, &[]);
        assert!(
            t.runner
                .calls()
                .iter()
                .any(|c| c[0] == "gh" && c[1] == "ssh-key")
        );
    }

    #[test]
    fn without_gh_it_prints_the_link() {
        let mut t = host(Os::Linux);
        odd(&mut t, &["--no-passphrase"]);
        assert!(
            t.output().contains("https://github.com/settings/ssh/new"),
            "{}",
            t.output()
        );
    }

    #[test]
    fn the_test_reads_success_from_exit_1() {
        let mut t = host(Os::Linux);
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(
            t.output().contains("GitHub accepts the key."),
            "{}",
            t.output()
        );
    }

    #[test]
    fn a_refused_key_fails() {
        let mut t = host(Os::Linux);
        t.runner.answer(
            "ssh",
            &[],
            Output::fail(255, "git@github.com: Permission denied (publickey)."),
        );
        assert_eq!(odd(&mut t, &[]), Exit::Failed);
        assert!(t.output().contains("Permission denied"), "{}", t.output());
    }

    #[test]
    fn undo_removes_the_block_and_keeps_the_key() {
        let mut t = host(Os::Linux);
        odd(&mut t, &["--no-passphrase"]);
        std::fs::write(t.home().join(".ssh/id_ed25519"), "key").unwrap();
        let argv = ["odd", "ssh", "--undo"].iter().map(Into::into).collect();
        assert_eq!(
            dispatch(argv, &mut t.ctx(), &[&super::super::Ssh]),
            Exit::Done
        );
        assert!(!t.home().join(".ssh/config").exists());
        assert!(t.home().join(".ssh/id_ed25519").exists());
    }
}
