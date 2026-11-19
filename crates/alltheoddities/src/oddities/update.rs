//! `odd update`: replace this binary with the binary of the latest release.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::state::sha256_hex;

/// The newest release, from the API of GitHub.
pub const LATEST: &str =
    "https://api.github.com/repos/Stiven-Gjekaj/AllTheOddities/releases/latest";
const DOWNLOAD: &str = "https://github.com/Stiven-Gjekaj/AllTheOddities/releases/download";
/// The target of this build, such as `aarch64-apple-darwin`.
const TARGET: &str = env!("ODD_TARGET");

/// The name of the file of this target in a release.
pub fn file_name(os: Os) -> String {
    let suffix = if os == Os::Windows { ".exe" } else { "" };
    format!("odd-{TARGET}{suffix}")
}

/// Replaces `binary` with the binary of the latest release, after it checks
/// the SHA-256 against `SHA256SUMS`.
pub fn run(ctx: &mut Ctx<'_>, binary: &Path) -> Result<Exit> {
    let latest: serde_json::Value = serde_json::from_str(&ctx.http.get(LATEST)?)
        .context("GitHub gave an answer that odd cannot read")?;
    let Some(tag) = latest["tag_name"].as_str() else {
        bail!("GitHub gave no release");
    };
    let current = format!("v{}", env!("CARGO_PKG_VERSION"));
    if tag == current {
        ctx.say(&format!("odd {} is the latest.", crate::version::LONG));
        return Ok(Exit::Done);
    }
    let file = file_name(ctx.host.os);
    let bytes = ctx.http.get_bytes(&format!("{DOWNLOAD}/{tag}/{file}"))?;
    let sums = ctx.http.get(&format!("{DOWNLOAD}/{tag}/SHA256SUMS"))?;
    let want = sums.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let (sum, name) = (parts.next()?, parts.next()?);
        (name == file || name.strip_prefix('*') == Some(file.as_str())).then(|| sum.to_lowercase())
    });
    if want.as_deref() != Some(sha256_hex(&bytes).as_str()) {
        bail!("the checksum of {file} of {tag} does not match SHA256SUMS. odd is unchanged");
    }
    if ctx.dry_run {
        ctx.say(&format!(
            "would replace {} with {file} of {tag}",
            binary.display()
        ));
        return Ok(Exit::Done);
    }
    let new = binary.with_file_name(format!("{file}.new"));
    std::fs::write(&new, &bytes).with_context(|| format!("could not write {}", new.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
    }
    if ctx.host.os == Os::Windows {
        // Windows cannot overwrite a running binary, but it can rename it.
        // The next start of odd deletes the old one.
        let old = binary.with_file_name("odd.old.exe");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(binary, &old).context("could not move the running odd aside")?;
    }
    std::fs::rename(&new, binary).context("could not put the new odd in place")?;
    ctx.say(&format!("odd is now {tag}."));
    Ok(Exit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::state::sha256_hex;
    use crate::testing::TestCtx;

    fn release(t: &TestCtx, tag: &str, bytes: &[u8], sum: &str) {
        t.http.page(LATEST, &format!(r#"{{"tag_name":"{tag}"}}"#));
        let base =
            format!("https://github.com/Stiven-Gjekaj/AllTheOddities/releases/download/{tag}");
        t.http
            .bytes(&format!("{base}/{}", file_name(t.os)), bytes.to_vec());
        t.http.page(
            &format!("{base}/SHA256SUMS"),
            &format!("{sum}  {}\n", file_name(t.os)),
        );
    }

    fn binary(t: &TestCtx) -> std::path::PathBuf {
        let p = t.dir.path().join("bin").join(if t.os == Os::Windows {
            "odd.exe"
        } else {
            "odd"
        });
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"old binary").unwrap();
        p
    }

    #[test]
    fn the_same_version_changes_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        t.http.page(
            LATEST,
            &format!(r#"{{"tag_name":"v{}"}}"#, env!("CARGO_PKG_VERSION")),
        );
        let bin = binary(&t);
        assert_eq!(run(&mut t.ctx(), &bin).unwrap(), Exit::Done);
        assert_eq!(std::fs::read(&bin).unwrap(), b"old binary");
        assert!(t.output().contains("is the latest"), "{}", t.output());
    }

    #[test]
    fn a_bad_checksum_changes_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        release(
            &t,
            "v2099.1.0",
            b"new binary",
            &sha256_hex(b"something else"),
        );
        let bin = binary(&t);
        assert!(run(&mut t.ctx(), &bin).is_err());
        assert_eq!(std::fs::read(&bin).unwrap(), b"old binary");
    }

    #[test]
    fn a_new_version_replaces_the_binary() {
        let mut t = TestCtx::new(Os::Linux);
        release(&t, "v2099.1.0", b"new binary", &sha256_hex(b"new binary"));
        let bin = binary(&t);
        assert_eq!(run(&mut t.ctx(), &bin).unwrap(), Exit::Done);
        assert_eq!(std::fs::read(&bin).unwrap(), b"new binary");
    }

    #[test]
    fn windows_moves_the_running_binary_aside() {
        let mut t = TestCtx::new(Os::Windows);
        release(&t, "v2099.1.0", b"new binary", &sha256_hex(b"new binary"));
        let bin = binary(&t);
        run(&mut t.ctx(), &bin).unwrap();
        assert_eq!(std::fs::read(&bin).unwrap(), b"new binary");
        assert_eq!(
            std::fs::read(bin.with_file_name("odd.old.exe")).unwrap(),
            b"old binary"
        );
    }

    #[test]
    fn dry_run_changes_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        t.dry_run = true;
        release(&t, "v2099.1.0", b"new binary", &sha256_hex(b"new binary"));
        let bin = binary(&t);
        run(&mut t.ctx(), &bin).unwrap();
        assert_eq!(std::fs::read(&bin).unwrap(), b"old binary");
    }
}
