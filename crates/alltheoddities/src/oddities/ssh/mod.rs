//! `odd ssh`: make SSH ready, out from this computer and in to it.
//!
//! Three parts, in this order: out (connect from this computer to GitHub),
//! in (let other computers connect to this one), and Tailscale (reach this
//! computer from anywhere). With no part flag, `odd ssh` asks about each.

pub mod out;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

pub const NAME: &str = "ssh";

/// `odd ssh`.
pub struct Ssh;

fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::SetTrue)
        .help(help)
}

fn value(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).value_name("VALUE").help(help)
}

/// The parts that this run does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Parts {
    pub out: bool,
    pub inbound: bool,
    pub tailscale: bool,
}

impl Parts {
    fn from_flags(m: &ArgMatches) -> Option<Parts> {
        let p = Parts {
            out: m.get_flag("out"),
            inbound: m.get_flag("in"),
            tailscale: m.get_flag("tailscale"),
        };
        (p != Parts::default()).then_some(p)
    }
}

/// The path of the key: `--key`, or `~/.ssh/id_ed25519`.
pub fn key_path(m: &ArgMatches, ctx: &Ctx<'_>) -> PathBuf {
    m.get_one::<String>("key")
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.home.join(".ssh").join("id_ed25519"))
}

impl Oddity for Ssh {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Make SSH ready: connect out to GitHub, let others connect in, and join Tailscale."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(flag(
            "out",
            "Make a key and connect from this computer to GitHub",
        ))
        .arg(flag(
            "in",
            "Let other computers connect to this one (needs an administrator)",
        ))
        .arg(flag(
            "tailscale",
            "Join Tailscale with an ephemeral auth key",
        ))
        .arg(flag(
            "tailscale-ssh",
            "Let Tailscale SSH serve the connections (macOS, Linux)",
        ))
        .arg(flag(
            "keep-passwords",
            "Keep password login on, with the keys",
        ))
        .arg(flag("no-passphrase", "Make the key with no passphrase"))
        .arg(value(
            "comment",
            "The comment of the key. The default is user@host",
        ))
        .arg(value(
            "key",
            "The path of the key. The default is ~/.ssh/id_ed25519",
        ))
        .arg(value(
            "github-user",
            "The GitHub user whose public keys may log in",
        ))
    }
    fn needs_admin(&self, m: &ArgMatches, _ctx: &Ctx<'_>) -> bool {
        m.get_flag("in") || m.get_flag("tailscale")
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let parts = match Parts::from_flags(m) {
            Some(p) => p,
            None => Parts {
                out: ctx.ask.yes_no(
                    "Make SSH ready to connect from this computer to GitHub?",
                    true,
                ),
                inbound: ctx.ask.yes_no(
                    "Let other computers connect to this one with SSH? It needs an administrator.",
                    false,
                ),
                tailscale: ctx.ask.yes_no(
                    "Join Tailscale, so you can reach this computer from anywhere?",
                    false,
                ),
            },
        };
        if (parts.inbound || parts.tailscale) && !ctx.is_admin && !ctx.dry_run {
            ctx.say("This part of odd ssh needs an administrator. It changed nothing.");
            let mut again = vec!["odd ssh"];
            if parts.out {
                again.push("--out");
            }
            if parts.inbound {
                again.push("--in");
            }
            if parts.tailscale {
                again.push("--tailscale");
            }
            let again = again.join(" ");
            match ctx.host.os {
                Os::Windows => ctx.say(&format!(
                    "Run again in a PowerShell that runs as administrator: {again}"
                )),
                _ => ctx.say(&format!("Run again with: sudo {again}")),
            }
            return Ok(Exit::NeedsAdmin);
        }
        let mut exit = Exit::Done;
        if parts.out {
            exit = out::run(m, ctx)?;
        }
        Ok(exit)
    }
}
