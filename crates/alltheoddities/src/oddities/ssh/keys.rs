//! The keys that may log in to this computer: the public keys of a GitHub user.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::ctx::Ctx;
use crate::host::Os;
use crate::state::{self, Undo};

use super::NAME;

/// The file of the keys that may log in. On Windows, the SSH server reads
/// the keys of a member of Administrators from one shared file.
pub fn authorized_keys_path(ctx: &Ctx<'_>) -> PathBuf {
    if ctx.host.os == Os::Windows && ctx.is_admin {
        ctx.sys_path(r"C:\ProgramData\ssh\administrators_authorized_keys")
    } else {
        ctx.home.join(".ssh").join("authorized_keys")
    }
}

/// Adds each public key of a GitHub user that the file does not have.
/// Gives the number of keys in the file after the step.
pub fn add_from_github(ctx: &mut Ctx<'_>, user: &str) -> Result<usize> {
    let url = format!("https://github.com/{user}.keys");
    let keys = ctx
        .http
        .get(&url)
        .with_context(|| format!("could not read the keys of {user}"))?;
    let path = authorized_keys_path(ctx);
    let before = std::fs::read_to_string(&path).ok();
    let mut text = before.clone().unwrap_or_default();
    let has = |text: &str, key: &str| text.lines().any(|l| l.trim() == key);
    let mut added = 0;
    for key in keys.lines().map(str::trim).filter(|k| !k.is_empty()) {
        if !has(&text, key) {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(key);
            text.push('\n');
            added += 1;
        }
    }
    let count = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .count();
    if added == 0 {
        if count == 0 {
            ctx.say(&format!("GitHub has no public key for {user}."));
        }
        return Ok(count);
    }
    let step = Undo::RestoreFile {
        path: path.clone(),
        before,
        after_sha256: state::sha256_hex(text.as_bytes()),
    };
    state::record(ctx, NAME, step)?;
    if let Some(dir) = path.parent() {
        ctx.make_dir(dir)?;
    }
    ctx.write_file(&path, &text)?;
    protect(ctx, &path)?;
    ctx.say(&format!(
        "{added} key(s) of {user} may log in. The file has {count} key(s)."
    ));
    Ok(count)
}

/// Sets the rights that the SSH server asks for.
fn protect(ctx: &mut Ctx<'_>, path: &Path) -> Result<()> {
    let p = path.to_string_lossy().into_owned();
    if ctx.host.os == Os::Windows {
        if ctx.is_admin {
            // The SIDs of Administrators and SYSTEM. A name such as
            // "Administrators" changes with the language of Windows.
            let out = ctx.change(
                "icacls",
                &[
                    &p,
                    "/inheritance:r",
                    "/grant",
                    "*S-1-5-32-544:F",
                    "/grant",
                    "*S-1-5-18:F",
                ],
            )?;
            if !out.success() {
                bail!("icacls failed: {}", out.stderr.trim());
            }
        }
        return Ok(());
    }
    ctx.change("chmod", &["600", &p])?;
    if let Some(user) = ctx.env.get("SUDO_USER").cloned() {
        let dir = path
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        ctx.change("chown", &[&user, &dir])?;
        ctx.change("chown", &[&user, &p])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::run::Output;
    use crate::testing::TestCtx;

    const KEYS: &str = "ssh-ed25519 AAAA1\nssh-ed25519 AAAA2\n";

    fn host(os: Os) -> TestCtx {
        let t = TestCtx::new(os);
        t.http.page("https://github.com/you.keys", KEYS);
        t.runner.answer("chmod", &[], Output::ok(""));
        t.runner.answer("icacls", &[], Output::ok(""));
        t
    }

    #[test]
    fn adds_only_new_keys() {
        let mut t = host(Os::Linux);
        let file = t.home().join(".ssh/authorized_keys");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "ssh-ed25519 AAAA1\n").unwrap();
        add_from_github(&mut t.ctx(), "you").unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(text, "ssh-ed25519 AAAA1\nssh-ed25519 AAAA2\n");
    }

    #[test]
    fn gives_the_count_after_the_step() {
        let mut t = host(Os::Linux);
        assert_eq!(add_from_github(&mut t.ctx(), "you").unwrap(), 2);
        assert_eq!(add_from_github(&mut t.ctx(), "you").unwrap(), 2);
    }

    #[test]
    fn a_user_with_no_keys_adds_nothing() {
        let mut t = host(Os::Linux);
        t.http.page("https://github.com/nobody.keys", "");
        assert_eq!(add_from_github(&mut t.ctx(), "nobody").unwrap(), 0);
        assert!(!t.home().join(".ssh/authorized_keys").exists());
    }

    #[test]
    fn no_network_is_an_error() {
        let mut t = host(Os::Linux);
        *t.http.fail.borrow_mut() = true;
        assert!(add_from_github(&mut t.ctx(), "you").is_err());
    }

    #[test]
    fn windows_admin_uses_the_programdata_file() {
        let mut t = host(Os::Windows);
        assert_eq!(
            authorized_keys_path(&t.ctx()),
            t.sys(r"C:\ProgramData\ssh\administrators_authorized_keys")
        );
        t.is_admin = false;
        assert_eq!(
            authorized_keys_path(&t.ctx()),
            t.home().join(".ssh").join("authorized_keys")
        );
    }

    #[test]
    fn windows_acl_uses_sids_not_names() {
        let mut t = host(Os::Windows);
        add_from_github(&mut t.ctx(), "you").unwrap();
        let call = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "icacls")
            .unwrap();
        assert!(call.contains(&"*S-1-5-32-544:F".to_owned()), "{call:?}");
        assert!(call.contains(&"*S-1-5-18:F".to_owned()), "{call:?}");
        assert!(
            !call.iter().any(|a| a.contains("Administrators")),
            "{call:?}"
        );
    }

    #[test]
    fn under_sudo_the_file_goes_to_the_person() {
        let mut t = host(Os::Linux);
        t.env.insert("SUDO_USER".into(), "you".into());
        t.runner.answer("chown", &[], Output::ok(""));
        add_from_github(&mut t.ctx(), "you").unwrap();
        assert!(
            t.runner
                .calls()
                .iter()
                .any(|c| c[0] == "chown" && c[1] == "you")
        );
    }

    #[test]
    fn undo_puts_back_the_file() {
        let mut t = host(Os::Linux);
        add_from_github(&mut t.ctx(), "you").unwrap();
        let mut none = |_: &str, _: &str, _: &mut Ctx<'_>| Ok(());
        crate::state::undo(&mut t.ctx(), "ssh", &mut none).unwrap();
        assert!(!t.home().join(".ssh/authorized_keys").exists());
    }
}
