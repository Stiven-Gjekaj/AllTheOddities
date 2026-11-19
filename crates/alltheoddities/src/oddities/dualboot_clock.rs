//! `odd dualboot-clock`: stop the clock from jumping between Windows and Linux
//! on one computer.

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "dualboot-clock";
const TZ_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\TimeZoneInformation";

/// `odd dualboot-clock`.
pub struct DualbootClock;

impl Oddity for DualbootClock {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Stop the clock from jumping between Windows and Linux on one computer."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("linux-local")
                .long("linux-local")
                .action(ArgAction::SetTrue)
                .help("Make Linux keep local time, in place of a change on Windows"),
        )
    }
    fn needs_admin(&self, m: &ArgMatches, ctx: &Ctx<'_>) -> bool {
        ctx.host.os == Os::Windows || m.get_flag("linux-local")
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match ctx.host.os {
            Os::Windows => {
                // Windows keeps local time in the hardware clock, and Linux
                // keeps UTC. This makes Windows keep UTC too.
                let before = crate::winreg::read(ctx, TZ_KEY, "RealTimeIsUniversal")?;
                if before.as_ref().map(|(_, d)| d.as_str()) == Some("0x1") {
                    ctx.say("Windows keeps UTC in the hardware clock already.");
                    return Ok(Exit::Done);
                }
                state::record(
                    ctx,
                    NAME,
                    crate::winreg::undo_step(TZ_KEY, "RealTimeIsUniversal", before),
                )?;
                crate::winreg::write(ctx, TZ_KEY, "RealTimeIsUniversal", "REG_DWORD", "0x1")?;
                ctx.say(
                    "Windows keeps UTC in the hardware clock from the next restart, as Linux does.",
                );
            }
            _ => {
                if !m.get_flag("linux-local") {
                    ctx.say("Run odd dualboot-clock on Windows. One side is enough, and Windows is the better one.");
                    ctx.say(
                        "Or run odd dualboot-clock --linux-local here, so Linux keeps local time.",
                    );
                    return Ok(Exit::Done);
                }
                let undo = Undo::Run {
                    program: "timedatectl".into(),
                    args: ["set-local-rtc", "0", "--adjust-system-clock"]
                        .map(String::from)
                        .to_vec(),
                };
                state::record(ctx, NAME, undo)?;
                let out = ctx.change(
                    "timedatectl",
                    &["set-local-rtc", "1", "--adjust-system-clock"],
                )?;
                if !out.success() {
                    bail!("timedatectl failed: {}", out.stderr.trim());
                }
                ctx.say("Linux keeps local time in the hardware clock, as Windows does.");
            }
        }
        Ok(Exit::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "dualboot-clock"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&DualbootClock])
    }

    #[test]
    fn windows_sets_universal_time() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner
            .answer("reg", &["query"], Output::fail(1, "not found"));
        t.runner.answer("reg", &["add"], Output::ok(""));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        let add = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[1] == "add")
            .unwrap();
        assert_eq!(
            add[2..],
            [
                TZ_KEY,
                "/v",
                "RealTimeIsUniversal",
                "/t",
                "REG_DWORD",
                "/d",
                "0x1",
                "/f"
            ]
        );
    }

    #[test]
    fn linux_without_the_flag_changes_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.runner.calls().is_empty());
        assert!(t.output().contains("--linux-local"), "{}", t.output());
    }

    #[test]
    fn linux_local_sets_the_rtc() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("timedatectl", &[], Output::ok(""));
        odd(&mut t, &["--linux-local"]);
        assert_eq!(
            t.runner.calls()[0],
            ["timedatectl", "set-local-rtc", "1", "--adjust-system-clock"]
        );
    }

    #[test]
    fn undo_puts_it_back() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("timedatectl", &[], Output::ok(""));
        odd(&mut t, &["--linux-local"]);
        odd(&mut t, &["--undo"]);
        assert_eq!(
            t.runner.calls().last().unwrap()[..3],
            ["timedatectl", "set-local-rtc", "0"]
        );
    }

    #[test]
    fn macos_is_not_a_host() {
        let mut t = TestCtx::new(Os::MacOs);
        assert_eq!(odd(&mut t, &[]), Exit::Usage);
    }
}
