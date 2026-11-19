//! `odd devcert`: a local certificate authority, a certificate for localhost,
//! and the trust of the host in that authority.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "devcert";
const CA_NAME: &str = "odd devcert CA";
const MAC_KEYCHAIN: &str = "/Library/Keychains/System.keychain";
const DEBIAN_ANCHOR: &str = "/usr/local/share/ca-certificates/odd-devcert.crt";
const FEDORA_ANCHOR: &str = "/etc/pki/ca-trust/source/anchors/odd-devcert.pem";
const GIT_OPENSSL: &str = r"C:\Program Files\Git\usr\bin\openssl.exe";

/// `odd devcert`.
pub struct Devcert;

impl Oddity for Devcert {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Make a trusted HTTPS certificate for localhost, from a local certificate authority."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("names")
                .action(ArgAction::Append)
                .help("More names for the certificate, such as myapp.test"),
        )
    }
    fn needs_admin(&self, _m: &ArgMatches, ctx: &Ctx<'_>) -> bool {
        ctx.host.os != Os::Windows
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let names: Vec<String> = m
            .get_many::<String>("names")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let openssl = openssl(ctx)?;
        let dir = dir(ctx);
        ctx.make_dir(&dir)?;
        let p = |f: &str| dir.join(f).to_string_lossy().into_owned();
        let (ca_key, ca_pem) = (p("ca.key"), p("ca.pem"));
        if !(dir.join("ca.pem").exists() && dir.join("ca.key").exists()) {
            let subj = format!("/CN={CA_NAME}");
            run_ok(
                ctx,
                &openssl,
                &[
                    "req",
                    "-x509",
                    "-new",
                    "-newkey",
                    "ec",
                    "-pkeyopt",
                    "ec_paramgen_curve:prime256v1",
                    "-nodes",
                    "-keyout",
                    &ca_key,
                    "-out",
                    &ca_pem,
                    "-days",
                    "3650",
                    "-subj",
                    &subj,
                    "-addext",
                    "basicConstraints=critical,CA:TRUE",
                    "-addext",
                    "keyUsage=critical,keyCertSign,cRLSign",
                ],
            )?;
            ctx.say(&format!("The authority {CA_NAME} is in {}.", dir.display()));
        }
        let mut san = vec![
            "DNS:localhost".to_owned(),
            "IP:127.0.0.1".into(),
            "IP:::1".into(),
        ];
        for n in &names {
            let kind = if n.parse::<std::net::IpAddr>().is_ok() {
                "IP"
            } else {
                "DNS"
            };
            san.push(format!("{kind}:{n}"));
        }
        let ext = format!(
            "subjectAltName={}\nextendedKeyUsage=serverAuth\nkeyUsage=critical,digitalSignature\n",
            san.join(",")
        );
        ctx.write_file(&dir.join("ext.cnf"), &ext)?;
        let (key, csr, pem, cnf) = (
            p("localhost.key"),
            p("localhost.csr"),
            p("localhost.pem"),
            p("ext.cnf"),
        );
        run_ok(
            ctx,
            &openssl,
            &[
                "req",
                "-new",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:prime256v1",
                "-nodes",
                "-keyout",
                &key,
                "-out",
                &csr,
                "-subj",
                "/CN=localhost",
            ],
        )?;
        run_ok(
            ctx,
            &openssl,
            &[
                "x509",
                "-req",
                "-in",
                &csr,
                "-CA",
                &ca_pem,
                "-CAkey",
                &ca_key,
                "-CAcreateserial",
                "-out",
                &pem,
                "-days",
                "825",
                "-extfile",
                &cnf,
            ],
        )?;
        // Under sudo, openssl wrote the files as root, and a key that only
        // root can read is of no use to the server of the person.
        if let Some(user) = ctx.env.get("SUDO_USER").cloned()
            && ctx.host.os != Os::Windows
        {
            let d = dir.to_string_lossy().into_owned();
            run_ok(ctx, "chown", &["-R", &user, &d])?;
        }
        trust(ctx, &ca_pem)?;
        ctx.say("Use these files in your server:");
        ctx.say(&format!("  certificate: {pem}"));
        ctx.say(&format!("  key:         {key}"));
        ctx.say("Firefox has its own store of authorities. Import ca.pem there, or turn on security.enterprise_roots.enabled.");
        Ok(Exit::Done)
    }
}

/// The folder of the files of odd devcert.
pub fn dir(ctx: &Ctx<'_>) -> PathBuf {
    match ctx.host.os {
        Os::Windows => ctx
            .env
            .get("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.home.join("AppData").join("Local"))
            .join("odd")
            .join("devcert"),
        _ => ctx
            .home
            .join(".local")
            .join("share")
            .join("odd")
            .join("devcert"),
    }
}

fn openssl(ctx: &Ctx<'_>) -> Result<String> {
    if ctx.runner.on_path("openssl") {
        return Ok("openssl".into());
    }
    let git = ctx.sys_path(GIT_OPENSSL);
    if ctx.host.os == Os::Windows && git.exists() {
        return Ok(git.to_string_lossy().into_owned());
    }
    bail!(
        "this computer has no openssl. Install openssl (Git for Windows has one), then run odd devcert again"
    );
}

fn run_ok(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    let out = ctx.change(program, args)?;
    if !out.success() {
        bail!(
            "{program} {} failed: {}",
            args.first().unwrap_or(&""),
            out.stderr.trim()
        );
    }
    Ok(())
}

fn undo_run(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    let step = Undo::Run {
        program: program.into(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
    };
    state::record(ctx, NAME, step)
}

fn trust(ctx: &mut Ctx<'_>, ca_pem: &str) -> Result<()> {
    match ctx.host.os {
        Os::MacOs => {
            undo_run(
                ctx,
                "security",
                &["delete-certificate", "-c", CA_NAME, MAC_KEYCHAIN],
            )?;
            run_ok(
                ctx,
                "security",
                &[
                    "add-trusted-cert",
                    "-d",
                    "-r",
                    "trustRoot",
                    "-k",
                    MAC_KEYCHAIN,
                    ca_pem,
                ],
            )?;
        }
        Os::Windows => {
            undo_run(ctx, "certutil", &["-user", "-delstore", "Root", CA_NAME])?;
            ctx.say("Windows asks you to trust the authority. Select Yes.");
            run_ok(ctx, "certutil", &["-user", "-addstore", "Root", ca_pem])?;
        }
        Os::Linux => {
            let (anchor, update) = if ctx.runner.on_path("update-ca-certificates") {
                (DEBIAN_ANCHOR, "update-ca-certificates")
            } else if ctx.runner.on_path("update-ca-trust") {
                (FEDORA_ANCHOR, "update-ca-trust")
            } else {
                bail!(
                    "odd does not know how this Linux trusts an authority. Add {ca_pem} to its store by hand"
                );
            };
            let path = ctx.sys_path(anchor);
            let text = if ctx.dry_run {
                String::new()
            } else {
                std::fs::read_to_string(ca_pem)?
            };
            // Undo runs newest first: it removes the file, then updates.
            undo_run(ctx, update, &[])?;
            let before = std::fs::read_to_string(&path).ok();
            state::record(
                ctx,
                NAME,
                Undo::RestoreFile {
                    path: path.clone(),
                    before,
                    after_sha256: state::sha256_hex(text.as_bytes()),
                },
            )?;
            ctx.write_file(&path, &text)?;
            run_ok(ctx, update, &[])?;
        }
    }
    ctx.say(&format!("This computer trusts {CA_NAME}."));
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
        let argv = ["odd", "devcert"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Devcert])
    }

    fn host(os: Os) -> TestCtx {
        let t = TestCtx::new(os);
        t.runner.with_path(&["openssl", "update-ca-certificates"]);
        for p in [
            "openssl",
            "security",
            "update-ca-certificates",
            "update-ca-trust",
            "certutil",
        ] {
            t.runner.answer(p, &[], Output::ok(""));
        }
        t
    }

    /// The fake openssl writes no file, so a test that needs ca.pem makes it.
    fn with_ca(t: &mut TestCtx) {
        let dir = super::dir(&t.ctx());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ca.pem"), "-----BEGIN CERTIFICATE-----\n").unwrap();
        std::fs::write(dir.join("ca.key"), "key").unwrap();
    }

    fn reqs(t: &TestCtx) -> Vec<Vec<String>> {
        t.runner
            .calls()
            .into_iter()
            .filter(|c| c[0] == "openssl")
            .collect()
    }

    #[test]
    fn no_openssl_fails() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &[]), Exit::Failed);
        assert!(t.output().contains("openssl"), "{}", t.output());
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn makes_the_ca_once() {
        let mut t = host(Os::Linux);
        odd(&mut t, &[]);
        assert!(reqs(&t).iter().any(|c| c.contains(&"-x509".to_owned())));
        let dir = super::dir(&t.ctx());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ca.pem"), "ca").unwrap();
        std::fs::write(dir.join("ca.key"), "key").unwrap();
        let mut u = host(Os::Linux);
        u.dir = t.dir;
        odd(&mut u, &[]);
        assert!(!reqs(&u).iter().any(|c| c.contains(&"-x509".to_owned())));
    }

    #[test]
    fn the_names_go_in_the_san() {
        let mut t = host(Os::MacOs);
        odd(&mut t, &["myapp.test"]);
        let ext = std::fs::read_to_string(super::dir(&t.ctx()).join("ext.cnf")).unwrap();
        assert!(
            ext.contains("subjectAltName=DNS:localhost,IP:127.0.0.1,IP:::1,DNS:myapp.test"),
            "{ext}"
        );
        assert!(ext.contains("extendedKeyUsage=serverAuth"), "{ext}");
    }

    #[test]
    fn macos_trusts_in_the_system_keychain() {
        let mut t = host(Os::MacOs);
        odd(&mut t, &[]);
        let call = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "security")
            .unwrap();
        assert_eq!(
            call[1..6],
            ["add-trusted-cert", "-d", "-r", "trustRoot", "-k"]
        );
    }

    #[test]
    fn linux_debian_and_fedora_trust() {
        let mut t = host(Os::Linux);
        with_ca(&mut t);
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(
            t.sys("/usr/local/share/ca-certificates/odd-devcert.crt")
                .exists()
        );
        assert!(
            t.runner
                .calls()
                .iter()
                .any(|c| c[0] == "update-ca-certificates")
        );

        let mut f = TestCtx::new(Os::Linux);
        f.runner.with_path(&["openssl", "update-ca-trust"]);
        f.runner.answer("openssl", &[], Output::ok(""));
        f.runner.answer("update-ca-trust", &[], Output::ok(""));
        with_ca(&mut f);
        assert_eq!(odd(&mut f, &[]), Exit::Done);
        assert!(
            f.sys("/etc/pki/ca-trust/source/anchors/odd-devcert.pem")
                .exists()
        );
    }

    #[test]
    fn windows_trusts_in_the_user_store() {
        let mut t = host(Os::Windows);
        t.is_admin = false;
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        let call = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "certutil")
            .unwrap();
        assert_eq!(call[1..4], ["-user", "-addstore", "Root"]);
    }

    #[test]
    fn under_sudo_the_files_go_to_the_person() {
        let mut t = host(Os::Linux);
        with_ca(&mut t);
        t.env.insert("SUDO_USER".into(), "you".into());
        t.runner.answer("chown", &[], Output::ok(""));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        let dir = super::dir(&t.ctx()).to_string_lossy().into_owned();
        assert!(
            t.runner
                .calls()
                .contains(&vec!["chown".into(), "-R".into(), "you".into(), dir])
        );
    }

    #[test]
    fn undo_removes_the_trust() {
        let mut t = host(Os::Linux);
        with_ca(&mut t);
        odd(&mut t, &[]);
        assert_eq!(odd(&mut t, &["--undo"]), Exit::Done);
        assert!(
            !t.sys("/usr/local/share/ca-certificates/odd-devcert.crt")
                .exists()
        );
        let n = t
            .runner
            .calls()
            .iter()
            .filter(|c| c[0] == "update-ca-certificates")
            .count();
        assert_eq!(n, 2);
    }
}
