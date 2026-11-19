//! `odd firewall`: open or close a port in the firewall of the host.

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "firewall";

/// `odd firewall`.
pub struct Firewall;

fn port_args(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(
            Arg::new("port")
                .required(true)
                .value_parser(value_parser!(u16).range(1..)),
        )
        .arg(
            Arg::new("udp")
                .long("udp")
                .action(ArgAction::SetTrue)
                .help("UDP in place of TCP"),
        )
}

impl Oddity for Firewall {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Open or close a port in the firewall of the host."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(port_args("open", "Let connections in on a port"))
            .subcommand(port_args("close", "Stop connections on a port"))
            .subcommand(Command::new("list").about("List the ports that odd opened"))
    }
    fn needs_admin(&self, m: &ArgMatches, _ctx: &Ctx<'_>) -> bool {
        matches!(m.subcommand_name(), Some("open" | "close"))
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let (open, a) = match m.subcommand() {
            Some(("open", a)) => (true, a),
            Some(("close", a)) => (false, a),
            _ => {
                list(ctx)?;
                return Ok(Exit::Done);
            }
        };
        let port = a.get_one::<u16>("port").copied().unwrap_or_default();
        let proto = if a.get_flag("udp") { "udp" } else { "tcp" };
        let Some(kind) = kind(ctx)? else {
            ctx.say("No firewall runs, so the port is open already. odd changed nothing.");
            return Ok(Exit::Done);
        };
        let (on, off) = commands(kind, port, proto);
        let (todo, undo) = if open { (on, off) } else { (off, on) };
        if open {
            for (program, args) in undo.into_iter().rev() {
                state::record(ctx, NAME, Undo::Run { program, args })?;
            }
        }
        for (program, args) in todo {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = ctx.change(&program, &refs)?;
            if !out.success() {
                bail!(
                    "{program} failed: {}{}",
                    out.stderr.trim(),
                    out.stdout.trim()
                );
            }
        }
        let word = if open { "open" } else { "closed" };
        ctx.say(&format!("Port {port}/{proto} is {word}."));
        Ok(Exit::Done)
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Netsh,
    Ufw,
    Firewalld,
}

/// The firewall that runs. None on a Linux with no firewall.
fn kind(ctx: &Ctx<'_>) -> Result<Option<Kind>> {
    if ctx.host.os == Os::Windows {
        return Ok(Some(Kind::Netsh));
    }
    if ctx.runner.on_path("ufw")
        && ctx
            .query("ufw", &["status"])?
            .stdout
            .contains("Status: active")
    {
        return Ok(Some(Kind::Ufw));
    }
    if ctx.runner.on_path("firewall-cmd")
        && ctx.query("firewall-cmd", &["--state"])?.stdout.trim() == "running"
    {
        return Ok(Some(Kind::Firewalld));
    }
    Ok(None)
}

type Steps = Vec<(String, Vec<String>)>;

/// The commands that open, and the commands that close, a port.
fn commands(kind: Kind, port: u16, proto: &str) -> (Steps, Steps) {
    let v = |p: &str, a: &[&str]| {
        (
            p.to_owned(),
            a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        )
    };
    let spec = format!("{port}/{proto}");
    match kind {
        Kind::Netsh => {
            let name = format!("name=odd-{port}-{proto}");
            let protocol = format!("protocol={}", proto.to_uppercase());
            let local = format!("localport={port}");
            (
                vec![v(
                    "netsh",
                    &[
                        "advfirewall",
                        "firewall",
                        "add",
                        "rule",
                        &name,
                        "dir=in",
                        "action=allow",
                        &protocol,
                        &local,
                    ],
                )],
                vec![v(
                    "netsh",
                    &["advfirewall", "firewall", "delete", "rule", &name],
                )],
            )
        }
        Kind::Ufw => (
            vec![v("ufw", &["allow", &spec])],
            vec![v("ufw", &["delete", "allow", &spec])],
        ),
        Kind::Firewalld => (
            vec![
                v(
                    "firewall-cmd",
                    &["--permanent", &format!("--add-port={spec}")],
                ),
                v("firewall-cmd", &["--reload"]),
            ],
            vec![
                v(
                    "firewall-cmd",
                    &["--permanent", &format!("--remove-port={spec}")],
                ),
                v("firewall-cmd", &["--reload"]),
            ],
        ),
    }
}

fn list(ctx: &mut Ctx<'_>) -> Result<()> {
    let ports: Vec<String> = state::steps(ctx, NAME)?
        .into_iter()
        .filter_map(|s| match s {
            Undo::Run { args, .. } => args.iter().find_map(|a| {
                a.strip_prefix("name=odd-")
                    .map(|r| r.replacen('-', "/", 1))
                    .or_else(|| a.strip_prefix("--remove-port=").map(str::to_owned))
                    .or_else(|| {
                        (args.first().map(String::as_str) == Some("delete") && a.contains('/'))
                            .then(|| a.clone())
                    })
            }),
            _ => None,
        })
        .collect();
    if ports.is_empty() {
        ctx.say("odd firewall has opened no port.");
    }
    for p in ports {
        ctx.say(&p);
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
        let argv = ["odd", "firewall"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Firewall])
    }

    fn ran(t: &TestCtx, words: &[&str]) -> bool {
        t.runner
            .calls()
            .iter()
            .any(|c| c.len() >= words.len() && c.iter().zip(words).all(|(a, b)| a == b))
    }

    #[test]
    fn a_bad_port_exits_2() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &["open", "0"]), Exit::Usage);
        assert_eq!(odd(&mut t, &["open", "70000"]), Exit::Usage);
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn windows_adds_a_named_rule() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer("netsh", &[], Output::ok("Ok."));
        assert_eq!(odd(&mut t, &["open", "3000"]), Exit::Done);
        assert!(ran(
            &t,
            &[
                "netsh",
                "advfirewall",
                "firewall",
                "add",
                "rule",
                "name=odd-3000-tcp",
                "dir=in",
                "action=allow",
                "protocol=TCP",
                "localport=3000"
            ]
        ));
    }

    fn linux(ufw: bool, firewalld: bool) -> TestCtx {
        let t = TestCtx::new(Os::Linux);
        if ufw {
            t.runner.with_path(&["ufw"]);
        }
        if firewalld {
            t.runner.with_path(&["firewall-cmd"]);
        }
        t.runner
            .answer("ufw", &["status"], Output::ok("Status: active\n"));
        t.runner.answer("ufw", &["allow"], Output::ok(""));
        t.runner.answer("ufw", &["delete"], Output::ok(""));
        t.runner
            .answer("firewall-cmd", &[], Output::ok("running\n"));
        t
    }

    #[test]
    fn linux_uses_ufw_when_it_is_active() {
        let mut t = linux(true, true);
        odd(&mut t, &["open", "8080", "--udp"]);
        assert!(ran(&t, &["ufw", "allow", "8080/udp"]));
        assert!(!ran(&t, &["firewall-cmd", "--permanent"]));
    }

    #[test]
    fn linux_uses_firewalld_else() {
        let mut t = linux(false, true);
        odd(&mut t, &["open", "8080"]);
        assert!(ran(
            &t,
            &["firewall-cmd", "--permanent", "--add-port=8080/tcp"]
        ));
        assert!(ran(&t, &["firewall-cmd", "--reload"]));
    }

    #[test]
    fn linux_with_no_firewall_changes_nothing() {
        let mut t = linux(false, false);
        assert_eq!(odd(&mut t, &["open", "8080"]), Exit::Done);
        assert!(t.output().contains("No firewall runs"), "{}", t.output());
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn undo_closes_the_port() {
        let mut t = linux(true, false);
        odd(&mut t, &["open", "8080"]);
        odd(&mut t, &["--undo"]);
        assert!(ran(&t, &["ufw", "delete", "allow", "8080/tcp"]));
    }

    #[test]
    fn close_removes_the_rule() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer("netsh", &[], Output::ok("Ok."));
        odd(&mut t, &["close", "3000"]);
        assert!(ran(
            &t,
            &[
                "netsh",
                "advfirewall",
                "firewall",
                "delete",
                "rule",
                "name=odd-3000-tcp"
            ]
        ));
    }

    #[test]
    fn list_shows_the_open_ports() {
        for (os, ufw) in [(Os::Windows, false), (Os::Linux, true), (Os::Linux, false)] {
            let mut t = if os == Os::Windows {
                TestCtx::new(os)
            } else {
                linux(ufw, !ufw)
            };
            t.runner.answer("netsh", &[], Output::ok("Ok."));
            odd(&mut t, &["open", "3000"]);
            t.out.clear();
            odd(&mut t, &["list"]);
            assert_eq!(t.output().trim(), "3000/tcp", "{os:?} ufw={ufw}");
        }
    }

    #[test]
    fn macos_is_not_a_host() {
        let mut t = TestCtx::new(Os::MacOs);
        assert_eq!(odd(&mut t, &["open", "80"]), Exit::Usage);
    }
}
