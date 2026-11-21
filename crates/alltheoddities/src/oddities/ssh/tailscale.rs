//! The Tailscale part of `odd ssh`: join the tailnet with an ephemeral key.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::ArgMatches;

use crate::ctx::Ctx;
use crate::host::Os;
use crate::state::{self, Undo};

use super::NAME;

/// The CLI inside the app of the macOS package. The package does not put
/// it on PATH.
pub const MAC_CLI: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";
const WINDOWS_CLI: &str = r"C:\Program Files\Tailscale\tailscale.exe";
const MAC_PKG: &str = "https://pkgs.tailscale.com/stable/Tailscale-latest-macos.pkg";
const KEYS_PAGE: &str = "https://login.tailscale.com/admin/settings/keys";
const STATIC_URL: &str = "https://pkgs.tailscale.com/stable/tailscale_latest_";
/// The value of `ID` in `/etc/os-release` for each Linux that the install
/// script of Tailscale knows. The script stops on each other value.
const SCRIPT_DISTROS: &[&str] = &[
    "ubuntu",
    "pop",
    "neon",
    "tuxedo",
    "debian",
    "linuxmint",
    "elementary",
    "industrial-os",
    "parrot",
    "mendel",
    "galliumos",
    "pureos",
    "kaisen",
    "raspbian",
    "kali",
    "pika",
    "sparky",
    "centos",
    "ol",
    "rhel",
    "miraclelinux",
    "fedora",
    "rocky",
    "almalinux",
    "nobara",
    "openmandriva",
    "sangoma",
    "risios",
    "cloudlinux",
    "alinux",
    "fedora-asahi-remix",
    "ultramarine",
    "amzn",
    "xenenterprise",
    "opensuse-leap",
    "sles",
    "opensuse-tumbleweed",
    "opensuse-slowroll",
    "sle-micro-rancher",
    "arch",
    "archarm",
    "endeavouros",
    "blendos",
    "garuda",
    "archcraft",
    "cachyos",
    "omarchy",
    "manjaro",
    "manjaro-arm",
    "biglinux",
    "alpine",
    "postmarketos",
    "bazzite",
    "void",
    "gentoo",
    "photon",
    "zorin",
    "steamos",
];

/// The Tailscale CLI of this host, if it has one. odd expose and odd carry
/// use it too.
pub fn cli(ctx: &Ctx<'_>) -> Option<String> {
    if ctx.runner.on_path("tailscale") {
        return Some("tailscale".into());
    }
    let path = match ctx.host.os {
        Os::MacOs => ctx.sys_path(MAC_CLI),
        Os::Windows => ctx.sys_path(WINDOWS_CLI),
        Os::Linux => ctx.state_dir.join("tailscale").join("tailscale"),
    };
    path.exists().then(|| path.to_string_lossy().into_owned())
}

/// The name of the processor in the names of the static files of
/// Tailscale.
fn static_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        "x86" => Some("386"),
        "arm" => Some("arm"),
        _ => None,
    }
}

/// Puts the static binaries of Tailscale in the state directory of odd.
/// Gives the path of the CLI.
fn install_static(ctx: &mut Ctx<'_>, arch: &str) -> Result<String> {
    let Some(name) = static_arch(arch) else {
        bail!("Tailscale has no static binaries for the processor {arch}");
    };
    let url = format!("{STATIC_URL}{name}.tgz");
    let dir = ctx.state_dir.join("tailscale");
    let tgz = ctx.state_dir.join(format!("tailscale_latest_{name}.tgz"));
    if ctx.dry_run {
        ctx.say(&format!("would download: {url}"));
    } else {
        let bytes = ctx.http.get_bytes(&url)?;
        ctx.make_dir(&dir)?;
        std::fs::write(&tgz, bytes)?;
    }
    let t = tgz.to_string_lossy().into_owned();
    let d = dir.to_string_lossy().into_owned();
    // The archive has one directory, such as tailscale_1.2.3_amd64. The
    // binaries go directly into dir.
    let out = ctx.change("tar", &["-xzf", &t, "-C", &d, "--strip-components=1"]);
    let _ = std::fs::remove_file(&tgz);
    if !out?.success() {
        bail!("tar could not unpack {url}");
    }
    Ok(dir.join("tailscale").to_string_lossy().into_owned())
}

/// Installs Tailscale. Gives the path of the CLI when the place of the CLI
/// is not a usual place.
fn install(ctx: &mut Ctx<'_>) -> Result<Option<String>> {
    match ctx.host.os {
        Os::Linux => {
            let arch = std::env::consts::ARCH;
            let known = ctx
                .host
                .distro
                .as_deref()
                .is_none_or(|d| SCRIPT_DISTROS.contains(&d));
            if !known {
                ctx.say("The install script of Tailscale does not know this Linux. odd uses the static binaries.");
                return install_static(ctx, arch).map(Some);
            }
            let out = ctx.change(
                "sh",
                &["-c", "curl -fsSL https://tailscale.com/install.sh | sh"],
            )?;
            if !out.success() {
                ctx.say("The install script of Tailscale failed. odd uses the static binaries.");
                return install_static(ctx, arch).map(Some);
            }
        }
        Os::MacOs => {
            let pkg = ctx.state_dir.join("Tailscale-latest-macos.pkg");
            if ctx.dry_run {
                ctx.say(&format!("would download: {MAC_PKG}"));
            } else {
                let bytes = ctx.http.get_bytes(MAC_PKG)?;
                ctx.make_dir(&ctx.state_dir.clone())?;
                std::fs::write(&pkg, bytes)?;
            }
            let p = pkg.to_string_lossy().into_owned();
            let out = ctx.change("installer", &["-pkg", &p, "-target", "/"]);
            let _ = std::fs::remove_file(&pkg);
            if !out?.success() {
                bail!("installer could not install Tailscale");
            }
            ctx.change("open", &["-a", "Tailscale"])?;
            ctx.say("Tailscale opens. Allow its VPN configuration when macOS asks.");
        }
        Os::Windows => {
            if !ctx.runner.on_path("winget") {
                bail!(
                    "this Windows has no winget. Install Tailscale from \
                     https://tailscale.com/download/windows, then run odd ssh --tailscale again"
                );
            }
            let out = ctx.change(
                "winget",
                &[
                    "install",
                    "--id",
                    "Tailscale.Tailscale",
                    "-e",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ],
            )?;
            if !out.success() {
                bail!("winget could not install Tailscale: {}", out.stdout.trim());
            }
        }
    }
    Ok(None)
}

/// A directory with the key in it, that only the user can read. It goes
/// away when this value goes away, also after an error.
struct KeyFile {
    dir: PathBuf,
    file: PathBuf,
}

impl KeyFile {
    fn new(ctx: &mut Ctx<'_>, key: &str) -> Result<KeyFile> {
        let dir = ctx.state_dir.join(format!("key-{}", std::process::id()));
        let file = dir.join("authkey");
        let _ = std::fs::remove_dir_all(&dir);
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&dir)?;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&file)?;
            f.write_all(key.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&file, key)?;
        }
        let guard = KeyFile { dir, file };
        if ctx.host.os == Os::Windows {
            let user = ctx.env.get("USERNAME").cloned().unwrap_or_default();
            let f = guard.file.to_string_lossy().into_owned();
            ctx.change(
                "icacls",
                &[&f, "/inheritance:r", "/grant:r", &format!("{user}:F")],
            )?;
        }
        Ok(guard)
    }
}

impl Drop for KeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Joins the tailnet, and prints how to connect.
pub fn join(m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<()> {
    let cli = match cli(ctx) {
        Some(c) => c,
        None => {
            if !ctx
                .ask
                .yes_no("Tailscale is not installed. Install it now?", true)
            {
                ctx.say("odd skips Tailscale.");
                return Ok(());
            }
            let installed = install(ctx)?;
            match installed.or_else(|| cli(ctx)) {
                Some(c) => c,
                None if ctx.dry_run => "tailscale".into(),
                None => bail!(
                    "Tailscale is installed, but odd cannot find its CLI. Open a new window and run odd ssh --tailscale again"
                ),
            }
        }
    };
    let key = match ctx.env.get("TS_AUTHKEY").filter(|k| !k.is_empty()) {
        Some(k) => Some(k.clone()),
        None => ctx.ask.secret(&format!(
            "Paste an ephemeral, single-use auth key from {KEYS_PAGE}:"
        )),
    };
    let Some(key) = key else {
        ctx.say("No auth key: odd skips Tailscale.");
        return Ok(());
    };

    let mut args = vec!["up".to_owned()];
    if m.get_flag("tailscale-ssh") {
        if ctx.host.os == Os::Windows {
            ctx.say("Tailscale SSH has no server on Windows. The SSH server of Windows serves the connections.");
        } else {
            args.push("--ssh".into());
        }
    }
    state::record(
        ctx,
        NAME,
        Undo::Run {
            program: cli.clone(),
            args: vec!["logout".into()],
        },
    )?;
    if ctx.dry_run {
        ctx.say(&format!(
            "would run: {cli} up --auth-key=file:<a file that only you can read>"
        ));
        return Ok(());
    }
    let guard = KeyFile::new(ctx, &key)?;
    drop(key);
    args.insert(
        1,
        format!("--auth-key=file:{}", guard.file.to_string_lossy()),
    );
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = ctx.change(&cli, &refs)?;
    drop(guard);
    if !out.success() {
        bail!("tailscale up failed: {}", out.stderr.trim());
    }

    let status = ctx.query(&cli, &["status", "--json"])?;
    let v: serde_json::Value = serde_json::from_str(&status.stdout).unwrap_or_default();
    let name = v["Self"]["DNSName"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_owned();
    let ip = v["Self"]["TailscaleIPs"][0]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let user = ["SUDO_USER", "USER", "USERNAME"]
        .iter()
        .find_map(|k| ctx.env.get(*k).cloned())
        .unwrap_or_else(|| "you".into());
    ctx.say(&format!("This computer is on your tailnet: {name} ({ip})."));
    ctx.say(&format!("Connect from another computer: ssh {user}@{name}"));
    ctx.say("The key is ephemeral, so the computer leaves the tailnet soon after it goes offline.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::exit::Exit;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    const KEY: &str = "tskey-auth-SECRET123";
    const STATUS: &str =
        r#"{"Self":{"DNSName":"box.tail1234.ts.net.","TailscaleIPs":["100.64.0.7","fd7a::7"]}}"#;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "ssh", "--tailscale"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&super::super::Ssh])
    }

    fn host(os: Os) -> TestCtx {
        let mut t = TestCtx::new(os);
        t.runner.with_path(&["tailscale"]);
        t.runner.answer("tailscale", &["up"], Output::ok(""));
        t.runner
            .answer("tailscale", &["status"], Output::ok(STATUS));
        t.runner.answer("tailscale", &["logout"], Output::ok(""));
        t.runner.answer("icacls", &[], Output::ok(""));
        t.env.insert("USER".into(), "you".into());
        t
    }

    fn up_call(t: &TestCtx) -> Vec<String> {
        t.runner
            .calls()
            .into_iter()
            .find(|c| c.get(1).is_some_and(|a| a == "up"))
            .unwrap()
    }

    #[test]
    fn the_key_is_never_an_argument() {
        let mut t = host(Os::Linux);
        t.ask.push(KEY);
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        for call in t.runner.calls() {
            assert!(!call.iter().any(|a| a.contains("SECRET123")), "{call:?}");
        }
        assert!(
            up_call(&t)
                .iter()
                .any(|a| a.starts_with("--auth-key=file:"))
        );
        assert!(!t.output().contains("SECRET123"));
    }

    #[test]
    fn it_prints_how_to_connect() {
        let mut t = host(Os::Linux);
        t.ask.push(KEY);
        odd(&mut t, &[]);
        let out = t.output();
        assert!(
            out.contains("box.tail1234.ts.net") && out.contains("100.64.0.7"),
            "{out}"
        );
        assert!(out.contains("ssh you@box.tail1234.ts.net"), "{out}");
    }

    #[test]
    fn the_key_file_is_gone_after_an_error() {
        let mut t = host(Os::Linux);
        t.ask.push(KEY);
        t.runner
            .answer("tailscale", &["up"], Output::fail(1, "invalid key"));
        assert_eq!(odd(&mut t, &[]), Exit::Failed);
        let file = up_call(&t)
            .iter()
            .find_map(|a| a.strip_prefix("--auth-key=file:").map(str::to_owned))
            .unwrap();
        let path = std::path::Path::new(&file);
        assert!(!path.exists() && !path.parent().unwrap().exists(), "{file}");
    }

    #[test]
    fn ts_authkey_takes_the_place_of_the_question() {
        let mut t = host(Os::Linux);
        t.env.insert("TS_AUTHKEY".into(), KEY.into());
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.ask.0.borrow().is_empty());
    }

    #[test]
    fn no_key_skips_tailscale() {
        let mut t = host(Os::Linux);
        t.ask.push("");
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(
            !t.runner
                .calls()
                .iter()
                .any(|c| c.get(1).is_some_and(|a| a == "up"))
        );
    }

    #[test]
    fn tailscale_ssh_adds_the_flag_on_linux() {
        let mut t = host(Os::Linux);
        t.ask.push(KEY);
        odd(&mut t, &["--tailscale-ssh"]);
        assert!(up_call(&t).contains(&"--ssh".to_owned()));
    }

    #[test]
    fn tailscale_ssh_is_not_offered_on_windows() {
        let mut t = host(Os::Windows);
        t.ask.push(KEY);
        odd(&mut t, &["--tailscale-ssh"]);
        assert!(!up_call(&t).contains(&"--ssh".to_owned()));
        assert!(t.output().contains("Windows"), "{}", t.output());
    }

    #[test]
    fn macos_finds_the_cli_in_the_app() {
        let mut t = TestCtx::new(Os::MacOs);
        let cli = t.sys(super::MAC_CLI);
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, "").unwrap();
        let cli = cli.to_string_lossy().into_owned();
        t.runner.answer(&cli, &["up"], Output::ok(""));
        t.runner.answer(&cli, &["status"], Output::ok(STATUS));
        t.ask.push(KEY);
        assert_eq!(odd(&mut t, &[]), Exit::Done);
    }

    #[test]
    fn a_missing_tailscale_is_installed_after_a_yes() {
        let mut t = TestCtx::new(Os::Linux);
        t.ask.push("y");
        t.ask.push("");
        t.runner.answer("sh", &["-c"], Output::ok(""));
        odd(&mut t, &[]);
        let sh = t.runner.calls().into_iter().find(|c| c[0] == "sh").unwrap();
        assert!(sh[2].contains("https://tailscale.com/install.sh"), "{sh:?}");
    }

    /// A Linux with no Tailscale, that serves each static archive.
    fn bare_linux(distro: &str) -> TestCtx {
        let mut t = TestCtx::new(Os::Linux);
        t.distro = Some(distro.into());
        for arch in ["amd64", "arm64", "386", "arm"] {
            let url = format!("{}{arch}.tgz", super::STATIC_URL);
            t.http.bytes(&url, vec![0x1f, 0x8b]);
        }
        t.runner.answer("tar", &[], Output::ok(""));
        t.env.insert("USER".into(), "you".into());
        t
    }

    fn tar_call(t: &TestCtx) -> Vec<String> {
        t.runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "tar")
            .expect("no tar call")
    }

    #[test]
    fn an_unknown_distro_takes_the_static_binaries() {
        let mut t = bare_linux("sysrescue");
        let dir = t.state_dir().join("tailscale");
        let cli = dir.join("tailscale").to_string_lossy().into_owned();
        t.runner.answer(&cli, &["up"], Output::ok(""));
        t.runner.answer(&cli, &["status"], Output::ok(STATUS));
        t.ask.push("y");
        t.ask.push(KEY);
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(!t.runner.calls().iter().any(|c| c[0] == "sh"));
        let tar = tar_call(&t);
        assert!(tar.contains(&dir.to_string_lossy().into_owned()), "{tar:?}");
        assert!(tar.contains(&"--strip-components=1".to_owned()), "{tar:?}");
        assert_eq!(up_call(&t)[0], cli);
        let tgz = std::path::Path::new(&tar[2]);
        assert!(!tgz.exists(), "{tgz:?}");
    }

    #[test]
    fn a_failed_install_script_takes_the_static_binaries() {
        let mut t = bare_linux("ubuntu");
        t.runner
            .answer("sh", &["-c"], Output::fail(1, "no repository"));
        t.ask.push("y");
        t.ask.push("");
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.runner.calls().iter().any(|c| c[0] == "sh"));
        tar_call(&t);
    }

    #[test]
    fn no_network_stops_the_static_install() {
        let mut t = bare_linux("sysrescue");
        *t.http.fail.borrow_mut() = true;
        t.ask.push("y");
        assert_eq!(odd(&mut t, &[]), Exit::Failed);
        assert!(!t.runner.calls().iter().any(|c| c[0] == "tar"));
    }

    #[test]
    fn the_static_archive_has_the_name_of_the_processor() {
        assert_eq!(super::static_arch("x86_64"), Some("amd64"));
        assert_eq!(super::static_arch("aarch64"), Some("arm64"));
        assert_eq!(super::static_arch("riscv64"), None);
    }

    #[test]
    fn undo_logs_out() {
        let mut t = host(Os::Linux);
        t.ask.push(KEY);
        odd(&mut t, &[]);
        let argv = ["odd", "ssh", "--undo"].iter().map(Into::into).collect();
        assert_eq!(
            dispatch(argv, &mut t.ctx(), &[&super::super::Ssh]),
            Exit::Done
        );
        assert!(
            t.runner
                .calls()
                .iter()
                .any(|c| c[0] == "tailscale" && c[1] == "logout")
        );
    }
}
