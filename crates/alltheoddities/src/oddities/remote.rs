//! `odd remote`: turn on remote desktop.

use anyhow::{Result, bail};
use clap::{ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "remote";
const EDITION_KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const TS_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server";
/// The name of the rule, not its display name, which changes with the
/// language of Windows.
const RDP_RULE: &str = "RemoteDesktop-UserMode-In-TCP";
const MAC_PLIST: &str = "/System/Library/LaunchDaemons/com.apple.screensharing.plist";

/// `odd remote`.
pub struct Remote;

impl Oddity for Remote {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Turn on remote desktop: Screen Sharing, Remote Desktop, or GNOME Remote Desktop."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd
    }
    fn needs_admin(&self, _m: &ArgMatches, ctx: &Ctx<'_>) -> bool {
        ctx.host.os != Os::Linux
    }
    fn run(&self, _m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match ctx.host.os {
            Os::MacOs => {
                let undo = Undo::Run {
                    program: "launchctl".into(),
                    args: vec!["unload".into(), "-w".into(), MAC_PLIST.into()],
                };
                state::record(ctx, NAME, undo)?;
                ok(ctx, "launchctl", &["load", "-w", MAC_PLIST])?;
                ctx.say("Screen Sharing is on. Connect with vnc://<this computer>.");
                ctx.say("If macOS refuses, turn it on in System Settings > General > Sharing > Screen Sharing.");
            }
            Os::Windows => {
                let edition = crate::winreg::read(ctx, EDITION_KEY, "EditionID")?
                    .map(|(_, d)| d)
                    .unwrap_or_default();
                if edition.starts_with("Core") {
                    ctx.say("Windows Home has no Remote Desktop server. odd changed nothing.");
                    return Ok(Exit::Usage);
                }
                let before = crate::winreg::read(ctx, TS_KEY, "fDenyTSConnections")?;
                if before.as_ref().map(|(_, d)| d.as_str()) != Some("0x0") {
                    state::record(
                        ctx,
                        NAME,
                        crate::winreg::undo_step(TS_KEY, "fDenyTSConnections", before),
                    )?;
                    crate::winreg::write(ctx, TS_KEY, "fDenyTSConnections", "REG_DWORD", "0x0")?;
                }
                let undo = Undo::Run {
                    program: "powershell".into(),
                    args: vec![
                        "-NoProfile".into(),
                        "-Command".into(),
                        format!("Disable-NetFirewallRule -Name {RDP_RULE}"),
                    ],
                };
                state::record(ctx, NAME, undo)?;
                ok(
                    ctx,
                    "powershell",
                    &[
                        "-NoProfile",
                        "-Command",
                        &format!("Enable-NetFirewallRule -Name {RDP_RULE}"),
                    ],
                )?;
                ctx.say(
                    "Remote Desktop is on. Connect with the Remote Desktop app to this computer.",
                );
            }
            Os::Linux => {
                if !ctx.runner.on_path("grdctl") {
                    bail!("this Linux has no grdctl, so odd cannot turn on GNOME Remote Desktop");
                }
                let undo = Undo::Run {
                    program: "grdctl".into(),
                    args: vec!["rdp".into(), "disable".into()],
                };
                state::record(ctx, NAME, undo)?;
                ok(ctx, "grdctl", &["rdp", "enable"])?;
                ctx.say("GNOME Remote Desktop is on.");
                ctx.say("Set its user name and password in Settings > System > Remote Desktop. grdctl takes them only on the command line, where other programs can read them.");
            }
        }
        Ok(Exit::Done)
    }
}

fn ok(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    let out = ctx.change(program, args)?;
    if !out.success() {
        bail!("{program} failed: {}", out.stderr.trim());
    }
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
        let argv = ["odd", "remote"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Remote])
    }

    fn windows(edition: &str) -> TestCtx {
        let t = TestCtx::new(Os::Windows);
        t.runner.answer(
            "reg",
            &["query", EDITION_KEY],
            Output::ok(&format!(
                "\r\nX\r\n    EditionID    REG_SZ    {edition}\r\n"
            )),
        );
        t.runner.answer(
            "reg",
            &["query", TS_KEY],
            Output::ok("\r\nX\r\n    fDenyTSConnections    REG_DWORD    0x1\r\n"),
        );
        t.runner.answer("reg", &["add"], Output::ok(""));
        t.runner.answer("powershell", &[], Output::ok(""));
        t
    }

    #[test]
    fn windows_home_exits_2() {
        let mut t = windows("Core");
        assert_eq!(odd(&mut t, &[]), Exit::Usage);
        assert!(t.output().contains("Home"), "{}", t.output());
        assert!(!t.runner.calls().iter().any(|c| c[1] == "add"));
    }

    #[test]
    fn windows_turns_on_rdp_and_the_rule() {
        let mut t = windows("Professional");
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        let calls = t.runner.calls();
        let add = calls.iter().find(|c| c[1] == "add").unwrap();
        assert_eq!(add[4], "fDenyTSConnections");
        assert_eq!(add[8], "0x0");
        assert!(
            calls.iter().any(|c| c
                .iter()
                .any(|a| a.contains("Enable-NetFirewallRule -Name RemoteDesktop-UserMode-In-TCP")))
        );
    }

    #[test]
    fn macos_loads_screen_sharing() {
        let mut t = TestCtx::new(Os::MacOs);
        t.runner.answer("launchctl", &[], Output::ok(""));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert_eq!(t.runner.calls()[0], ["launchctl", "load", "-w", MAC_PLIST]);
    }

    #[test]
    fn linux_without_grdctl_fails() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &[]), Exit::Failed);
        assert!(t.output().contains("GNOME"), "{}", t.output());
    }

    #[test]
    fn linux_enables_rdp_and_says_to_set_the_credentials() {
        let mut t = TestCtx::new(Os::Linux);
        t.is_admin = false;
        t.runner.with_path(&["grdctl"]);
        t.runner.answer("grdctl", &[], Output::ok(""));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert_eq!(t.runner.calls()[0], ["grdctl", "rdp", "enable"]);
        assert!(t.output().contains("Settings"), "{}", t.output());
    }

    #[test]
    fn undo_turns_it_off() {
        let mut t = windows("Professional");
        odd(&mut t, &[]);
        odd(&mut t, &["--undo"]);
        let calls = t.runner.calls();
        assert!(
            calls
                .iter()
                .any(|c| c.iter().any(|a| a.contains("Disable-NetFirewallRule")))
        );
        assert_eq!(
            calls.iter().filter(|c| c[1] == "add").last().unwrap()[8],
            "0x1"
        );
    }
}
