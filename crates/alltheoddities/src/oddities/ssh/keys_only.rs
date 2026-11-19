//! Make the SSH server accept keys only, and refuse passwords.

use anyhow::{Result, bail};

use crate::ctx::Ctx;
use crate::host::Os;
use crate::state::{self, Undo};

use super::NAME;

/// The SSH server on macOS and Linux.
pub const SSHD_UNIX: &str = "/usr/sbin/sshd";
/// The SSH server of Windows.
pub const SSHD_WINDOWS: &str = r"C:\Windows\System32\OpenSSH\sshd.exe";

/// The two settings. `ChallengeResponseAuthentication` and not
/// `KbdInteractiveAuthentication`: Windows 10 has OpenSSH 8.1, which does not
/// know the new name, and each later version still takes the old one.
const BODY: &str = "PasswordAuthentication no\nChallengeResponseAuthentication no";
const BLOCK: &str = "keys-only";

/// Turns off password login when at least one key may log in. `keys` is
/// the number of keys in the file, or the reason that odd could not read
/// them. On a failed check, it puts back the config and gives an error.
pub fn apply(ctx: &mut Ctx<'_>, keys: Result<usize, String>) -> Result<()> {
    match keys {
        Err(why) => {
            ctx.say(&format!("Passwords stay on: {why}"));
            return Ok(());
        }
        Ok(0) => {
            ctx.say("Passwords stay on: no key may log in yet, so odd does not lock you out.");
            return Ok(());
        }
        Ok(_) => {}
    }
    let config = super::server::config_path(ctx);
    let Ok(text) = std::fs::read_to_string(&config) else {
        bail!(
            "{} is not there, so odd cannot turn off passwords",
            config.display()
        );
    };
    let includes_dir = text.lines().any(|l| {
        let l = l.trim();
        l.starts_with("Include") && l.contains("sshd_config.d")
    });
    let drop_in = ctx.sys_path("/etc/ssh/sshd_config.d/50-odd.conf");
    let (path, before, new) = if includes_dir && ctx.host.os != Os::Windows {
        let before = std::fs::read_to_string(&drop_in).ok();
        let body = format!("# odd ssh: keys only. odd ssh --undo removes this file.\n{BODY}\n");
        (drop_in, before, body)
    } else {
        let is_match = |l: &str| l.trim_start().starts_with("Match ");
        let new = crate::block::upsert_before(&text, BLOCK, BODY, "#", &is_match);
        (config.clone(), Some(text.clone()), new)
    };
    if before.as_deref() == Some(new.as_str()) {
        ctx.say("The SSH server accepts keys only already.");
        return Ok(());
    }
    ctx.write_file(&path, &new)?;
    if !ctx.dry_run {
        let sshd = if ctx.host.os == Os::Windows {
            SSHD_WINDOWS
        } else {
            SSHD_UNIX
        };
        let check = ctx.query(sshd, &["-t"]);
        let good = check.as_ref().is_ok_and(|o| o.success());
        if !good {
            match &before {
                Some(old) => ctx.write_file(&path, old)?,
                None => ctx.remove_file(&path)?,
            }
            let why = match check {
                Ok(o) => o.stderr.trim().to_owned(),
                Err(e) => format!("{e:#}"),
            };
            bail!("sshd refused the new config, so odd put back the old one: {why}");
        }
    }
    // Recorded after the check, so a config that odd put back leaves no step.
    let step = Undo::RestoreFile {
        path,
        before,
        after_sha256: state::sha256_hex(new.as_bytes()),
    };
    state::record(ctx, NAME, step)?;
    super::server::restart(ctx)?;
    ctx.say("The SSH server accepts keys only.");
    ctx.say("Test a key login from a second window before you close this one.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::run::Output;
    use crate::testing::TestCtx;

    const WITH_INCLUDE: &str = "Include /etc/ssh/sshd_config.d/*.conf\nPort 22\n";
    const WINDOWS: &str = "Port 22\r\nMatch Group administrators\r\n       AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys\r\n";

    fn host(os: Os, config: &str) -> TestCtx {
        let mut t = TestCtx::new(os);
        let path = super::super::server::config_path(&t.ctx());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, config).unwrap();
        t.runner.answer(SSHD_UNIX, &["-t"], Output::ok(""));
        t.runner.answer(SSHD_WINDOWS, &["-t"], Output::ok(""));
        t.runner.answer("systemctl", &[], Output::ok(""));
        t.runner.answer("powershell", &[], Output::ok(""));
        t
    }

    fn config(t: &mut TestCtx) -> String {
        std::fs::read_to_string(super::super::server::config_path(&t.ctx())).unwrap()
    }

    fn checked(t: &TestCtx) -> bool {
        t.runner
            .calls()
            .iter()
            .any(|c| c.get(1).is_some_and(|a| a == "-t"))
    }

    #[test]
    fn no_keys_keeps_passwords_on() {
        let mut t = host(Os::Linux, WITH_INCLUDE);
        apply(&mut t.ctx(), Ok(0)).unwrap();
        assert!(!t.sys("/etc/ssh/sshd_config.d/50-odd.conf").exists());
        assert!(!checked(&t));
        assert!(t.output().contains("Passwords stay on"), "{}", t.output());
    }

    #[test]
    fn a_key_fetch_error_keeps_passwords_on() {
        let mut t = host(Os::Linux, WITH_INCLUDE);
        apply(&mut t.ctx(), Err("no network".into())).unwrap();
        assert!(!checked(&t));
        assert!(
            t.output().contains("Passwords stay on: no network"),
            "{}",
            t.output()
        );
    }

    #[test]
    fn uses_the_drop_in_when_the_config_includes_it() {
        let mut t = host(Os::Linux, WITH_INCLUDE);
        apply(&mut t.ctx(), Ok(1)).unwrap();
        let drop_in = std::fs::read_to_string(t.sys("/etc/ssh/sshd_config.d/50-odd.conf")).unwrap();
        assert!(drop_in.contains("PasswordAuthentication no"), "{drop_in}");
        assert_eq!(config(&mut t), WITH_INCLUDE);
        assert!(t.output().contains("second window"), "{}", t.output());
    }

    #[test]
    fn a_commented_include_does_not_count() {
        let mut t = host(
            Os::Linux,
            "#Include /etc/ssh/sshd_config.d/*.conf\nPort 22\n",
        );
        apply(&mut t.ctx(), Ok(1)).unwrap();
        assert!(!t.sys("/etc/ssh/sshd_config.d/50-odd.conf").exists());
        assert!(config(&mut t).contains("PasswordAuthentication no"));
    }

    #[test]
    fn goes_before_match_when_there_is_no_include() {
        let mut t = host(Os::Windows, WINDOWS);
        apply(&mut t.ctx(), Ok(1)).unwrap();
        let text = config(&mut t);
        let block = text.find("PasswordAuthentication no").unwrap();
        assert!(block < text.find("Match Group").unwrap(), "{text}");
        assert!(
            !text.replace("\r\n", "").contains('\n'),
            "CRLF kept: {text:?}"
        );
    }

    #[test]
    fn a_failed_check_puts_back_the_config() {
        let mut t = host(Os::Windows, WINDOWS);
        t.runner.answer(
            SSHD_WINDOWS,
            &["-t"],
            Output::fail(255, "Bad configuration option"),
        );
        assert!(apply(&mut t.ctx(), Ok(1)).is_err());
        assert_eq!(config(&mut t), WINDOWS);
        assert!(
            !t.runner.calls().iter().any(|c| c[0] == "powershell"),
            "no restart"
        );
        assert!(
            crate::state::steps(&t.ctx(), "ssh").unwrap().is_empty(),
            "nothing to undo"
        );
    }

    #[test]
    fn undo_turns_passwords_back_on() {
        let mut t = host(Os::Linux, WITH_INCLUDE);
        apply(&mut t.ctx(), Ok(1)).unwrap();
        let mut none = |_: &str, _: &str, _: &mut crate::ctx::Ctx<'_>| Ok(());
        crate::state::undo(&mut t.ctx(), "ssh", &mut none).unwrap();
        assert!(!t.sys("/etc/ssh/sshd_config.d/50-odd.conf").exists());
    }
}
