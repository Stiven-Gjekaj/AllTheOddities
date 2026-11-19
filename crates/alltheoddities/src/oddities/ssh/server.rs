//! The SSH server of `odd ssh --in`: turn it on, by host.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::ctx::Ctx;
use crate::host::Os;
use crate::state::{self, Undo};

use super::NAME;

/// The PowerShell line that gives the state of the OpenSSH server feature.
pub const CAPABILITY_STATE: &str = "(Get-WindowsCapability -Online -Name 'OpenSSH.Server*').State";
const CAPABILITY_ADD: &str = "Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0";
const FIREWALL_RULE: &str = "OpenSSH-Server-In-TCP";

/// The config file of the SSH server.
pub fn config_path(ctx: &Ctx<'_>) -> PathBuf {
    match ctx.host.os {
        Os::Windows => ctx.sys_path(r"C:\ProgramData\ssh\sshd_config"),
        _ => ctx.sys_path("/etc/ssh/sshd_config"),
    }
}

fn run_ok(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    let out = ctx.change(program, args)?;
    if !out.success() {
        bail!("{program} {} failed: {}", args.join(" "), out.stderr.trim());
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

/// Turns on the SSH server. Records an undo step for each part that `odd`
/// turned on, and none for a part that was on.
pub fn enable(ctx: &mut Ctx<'_>) -> Result<()> {
    match ctx.host.os {
        Os::MacOs => macos(ctx),
        Os::Linux => linux(ctx),
        Os::Windows => windows(ctx),
    }
}

const FDA_STEPS: &str = "macOS asks for Full Disk Access to change Remote Login from a terminal. \
Turn it on in System Settings > General > Sharing > Remote Login, then run odd ssh --in again.";

fn macos(ctx: &mut Ctx<'_>) -> Result<()> {
    let now = ctx.query("systemsetup", &["-getremotelogin"])?;
    let text = format!("{}{}", now.stdout, now.stderr);
    if text.contains("Full Disk Access") {
        bail!("{FDA_STEPS}");
    }
    if now.success() && text.contains(": On") {
        ctx.say("Remote Login is on already.");
        return Ok(());
    }
    undo_run(ctx, "systemsetup", &["-f", "-setremotelogin", "off"])?;
    let out = ctx.change("systemsetup", &["-setremotelogin", "on"])?;
    if format!("{}{}", out.stdout, out.stderr).contains("Full Disk Access") {
        bail!("{FDA_STEPS}");
    }
    if !out.success() {
        bail!("systemsetup failed: {}", out.stderr.trim());
    }
    ctx.say("Remote Login is on.");
    Ok(())
}

/// The command that installs the SSH server on a distribution.
fn install_command(distro: &str) -> Option<&'static [&'static str]> {
    let apt = &["apt-get", "install", "-y", "openssh-server"][..];
    let dnf = &["dnf", "install", "-y", "openssh-server"][..];
    let pacman = &["pacman", "-S", "--noconfirm", "openssh"][..];
    let zypper = &["zypper", "install", "-y", "openssh"][..];
    match distro {
        "debian" | "ubuntu" | "linuxmint" | "pop" | "raspbian" | "kali" | "elementary"
        | "zorin" | "neon" => Some(apt),
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "ol" | "amzn" => Some(dnf),
        "arch" | "manjaro" | "endeavouros" | "garuda" | "cachyos" => Some(pacman),
        d if d.starts_with("opensuse") || d == "sles" => Some(zypper),
        _ => None,
    }
}

/// The name of the systemd unit of the server: `ssh` on Debian and its
/// family, `sshd` elsewhere. None when neither exists.
fn unit(ctx: &Ctx<'_>) -> Option<&'static str> {
    ["ssh", "sshd"].into_iter().find(|u| {
        ctx.query("systemctl", &["cat", u])
            .is_ok_and(|o| o.success())
    })
}

fn linux(ctx: &mut Ctx<'_>) -> Result<()> {
    let unit = match unit(ctx) {
        Some(u) => u,
        None => {
            let distro = ctx.host.distro.clone().unwrap_or_default();
            let Some(cmd) = install_command(&distro) else {
                bail!(
                    "odd does not know the package manager of {distro}. Install the openssh \
                     server with it, then run odd ssh --in again."
                );
            };
            run_ok(ctx, cmd[0], &cmd[1..])?;
            if ctx.dry_run {
                "ssh"
            } else {
                unit(ctx).unwrap_or("sshd")
            }
        }
    };
    let active = ctx
        .query("systemctl", &["is-active", unit])
        .is_ok_and(|o| o.success());
    if active {
        ctx.say(&format!("The SSH server ({unit}) runs already."));
        return Ok(());
    }
    undo_run(ctx, "systemctl", &["disable", "--now", unit])?;
    run_ok(ctx, "systemctl", &["enable", "--now", unit])?;
    ctx.say(&format!(
        "The SSH server ({unit}) runs, and starts with the computer."
    ));
    Ok(())
}

fn windows(ctx: &mut Ctx<'_>) -> Result<()> {
    let state = ctx.query("powershell", &["-NoProfile", "-Command", CAPABILITY_STATE])?;
    if !state.stdout.contains("Installed") {
        ctx.say("Windows adds the OpenSSH server. This can take a few minutes.");
        run_ok(
            ctx,
            "powershell",
            &["-NoProfile", "-Command", CAPABILITY_ADD],
        )?;
    }
    let running = ctx
        .query("sc", &["query", "sshd"])
        .is_ok_and(|o| o.stdout.contains("RUNNING"));
    if running {
        ctx.say("The SSH server runs already.");
    } else {
        // Undo runs newest first: it stops the service, then sets it back to
        // start by hand, as Windows installs it.
        undo_run(ctx, "sc", &["config", "sshd", "start=", "demand"])?;
        undo_run(ctx, "sc", &["stop", "sshd"])?;
        run_ok(ctx, "sc", &["config", "sshd", "start=", "auto"])?;
        run_ok(ctx, "sc", &["start", "sshd"])?;
        ctx.say("The SSH server runs, and starts with Windows.");
    }
    let rule = format!("name={FIREWALL_RULE}");
    let has_rule = ctx
        .query("netsh", &["advfirewall", "firewall", "show", "rule", &rule])
        .is_ok_and(|o| o.success());
    if !has_rule {
        undo_run(
            ctx,
            "netsh",
            &["advfirewall", "firewall", "delete", "rule", &rule],
        )?;
        run_ok(
            ctx,
            "netsh",
            &[
                "advfirewall",
                "firewall",
                "add",
                "rule",
                &rule,
                "dir=in",
                "action=allow",
                "protocol=TCP",
                "localport=22",
            ],
        )?;
        ctx.say("The firewall lets SSH in on port 22.");
    }
    Ok(())
}

/// Restarts the SSH server, so it reads its config again.
pub fn restart(ctx: &mut Ctx<'_>) -> Result<()> {
    match ctx.host.os {
        Os::MacOs => run_ok(
            ctx,
            "launchctl",
            &["kickstart", "-k", "system/com.openssh.sshd"],
        ),
        Os::Linux => {
            let unit = unit(ctx).unwrap_or("sshd");
            run_ok(ctx, "systemctl", &["restart", unit])
        }
        Os::Windows => run_ok(
            ctx,
            "powershell",
            &["-NoProfile", "-Command", "Restart-Service sshd"],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::run::Output;
    use crate::state::Undo;
    use crate::testing::TestCtx;

    fn ran(t: &TestCtx, words: &[&str]) -> bool {
        t.runner
            .calls()
            .iter()
            .any(|c| c.len() >= words.len() && c.iter().zip(words).all(|(a, b)| a == b))
    }

    fn undo_steps(t: &mut TestCtx) -> Vec<Undo> {
        crate::state::steps(&t.ctx(), "ssh").unwrap()
    }

    #[test]
    fn macos_turns_on_remote_login() {
        let mut t = TestCtx::new(Os::MacOs);
        t.runner.answer(
            "systemsetup",
            &["-getremotelogin"],
            Output::ok("Remote Login: Off\n"),
        );
        t.runner
            .answer("systemsetup", &["-setremotelogin"], Output::ok(""));
        enable(&mut t.ctx()).unwrap();
        assert!(ran(&t, &["systemsetup", "-setremotelogin", "on"]));
        assert_eq!(undo_steps(&mut t).len(), 1);
    }

    #[test]
    fn macos_full_disk_access_gives_the_settings_steps() {
        let mut t = TestCtx::new(Os::MacOs);
        t.runner.answer(
            "systemsetup",
            &[],
            Output::fail(
                1,
                "Turning Remote Login on or off requires Full Disk Access privileges.",
            ),
        );
        let err = enable(&mut t.ctx()).unwrap_err().to_string();
        assert!(
            err.contains("System Settings > General > Sharing > Remote Login"),
            "{err}"
        );
    }

    fn linux(distro: &str) -> TestCtx {
        let mut t = TestCtx::new(Os::Linux);
        t.distro = Some(distro.into());
        t.runner.answer(
            "systemctl",
            &["cat", "ssh"],
            Output::fail(1, "No files found"),
        );
        t.runner.answer(
            "systemctl",
            &["cat", "sshd"],
            Output::fail(1, "No files found"),
        );
        t.runner
            .answer("systemctl", &["is-active"], Output::fail(3, "inactive"));
        t.runner.answer("systemctl", &["enable"], Output::ok(""));
        for p in ["apt-get", "dnf", "pacman", "zypper"] {
            t.runner.answer(p, &[], Output::ok(""));
        }
        t
    }

    #[test]
    fn linux_picks_the_package_manager() {
        for (distro, words) in [
            (
                "ubuntu",
                &["apt-get", "install", "-y", "openssh-server"][..],
            ),
            ("debian", &["apt-get", "install", "-y", "openssh-server"]),
            ("fedora", &["dnf", "install", "-y", "openssh-server"]),
            ("arch", &["pacman", "-S", "--noconfirm", "openssh"]),
            (
                "opensuse-tumbleweed",
                &["zypper", "install", "-y", "openssh"],
            ),
        ] {
            let mut t = linux(distro);
            let _ = enable(&mut t.ctx());
            assert!(ran(&t, words), "{distro}: {:?}", t.runner.calls());
        }
    }

    #[test]
    fn linux_with_an_unknown_distro_stops() {
        let mut t = linux("plan9");
        let err = enable(&mut t.ctx()).unwrap_err().to_string();
        assert!(err.contains("openssh"), "{err}");
    }

    #[test]
    fn linux_uses_sshd_when_ssh_is_not_a_unit() {
        let mut t = linux("fedora");
        t.runner
            .answer("systemctl", &["cat", "sshd"], Output::ok("[Unit]\n"));
        enable(&mut t.ctx()).unwrap();
        assert!(ran(&t, &["systemctl", "enable", "--now", "sshd"]));
        assert!(!ran(&t, &["dnf"]), "the server is there, so no install");
    }

    #[test]
    fn a_server_that_runs_already_records_no_undo() {
        let mut t = linux("ubuntu");
        t.runner
            .answer("systemctl", &["cat", "ssh"], Output::ok("[Unit]\n"));
        t.runner
            .answer("systemctl", &["is-active"], Output::ok("active\n"));
        enable(&mut t.ctx()).unwrap();
        assert!(!ran(&t, &["systemctl", "enable"]));
        assert!(undo_steps(&mut t).is_empty());
    }

    fn windows() -> TestCtx {
        let t = TestCtx::new(Os::Windows);
        t.runner
            .answer("powershell", &[], Output::ok("Installed\r\n"));
        t.runner.answer(
            "sc",
            &["query", "sshd"],
            Output::ok("        STATE              : 1  STOPPED\r\n"),
        );
        t.runner.answer("sc", &["config"], Output::ok(""));
        t.runner.answer("sc", &["start"], Output::ok(""));
        t.runner.answer(
            "netsh",
            &["advfirewall", "firewall", "show"],
            Output::fail(1, "No rules match"),
        );
        t.runner.answer(
            "netsh",
            &["advfirewall", "firewall", "add"],
            Output::ok("Ok."),
        );
        t
    }

    #[test]
    fn windows_starts_the_service_and_adds_the_firewall_rule() {
        let mut t = windows();
        enable(&mut t.ctx()).unwrap();
        assert!(ran(&t, &["sc", "config", "sshd", "start=", "auto"]));
        assert!(ran(&t, &["sc", "start", "sshd"]));
        assert!(ran(
            &t,
            &["netsh", "advfirewall", "firewall", "add", "rule"]
        ));
    }

    #[test]
    fn windows_adds_the_firewall_rule_only_when_missing() {
        let mut t = windows();
        t.runner.answer(
            "netsh",
            &["advfirewall", "firewall", "show"],
            Output::ok("Rule Name: x\r\n"),
        );
        enable(&mut t.ctx()).unwrap();
        assert!(!ran(&t, &["netsh", "advfirewall", "firewall", "add"]));
    }

    #[test]
    fn windows_installs_the_server_when_it_is_not_present() {
        let mut t = windows();
        t.runner.answer(
            "powershell",
            &["-NoProfile", "-Command", CAPABILITY_STATE],
            Output::ok("NotPresent\r\n"),
        );
        enable(&mut t.ctx()).unwrap();
        assert!(
            t.runner
                .calls()
                .iter()
                .any(|c| c.iter().any(|a| a.contains("Add-WindowsCapability")))
        );
    }
}
