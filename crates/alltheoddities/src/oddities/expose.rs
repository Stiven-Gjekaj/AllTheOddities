//! `odd expose`: share a local port on the tailnet, or in public, until Ctrl+C.

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

/// `odd expose`.
pub struct Expose;

impl Oddity for Expose {
    fn name(&self) -> &'static str {
        "expose"
    }
    fn summary(&self) -> &'static str {
        "Share a local port on your tailnet, or in public with --public, until Ctrl+C."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("port")
                .required(true)
                .value_parser(value_parser!(u16).range(1..)),
        )
        .arg(
            Arg::new("public")
                .long("public")
                .action(ArgAction::SetTrue)
                .help("Share it on the internet with Tailscale Funnel"),
        )
    }
    fn can_undo(&self) -> bool {
        false
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let port = m
            .get_one::<u16>("port")
            .copied()
            .unwrap_or_default()
            .to_string();
        let Some(cli) = crate::oddities::ssh::tailscale::cli(ctx) else {
            bail!("this computer has no Tailscale. Run odd ssh --tailscale first");
        };
        let mode = if m.get_flag("public") {
            ctx.say("Anybody on the internet with the URL can reach this port until Ctrl+C.");
            "funnel"
        } else {
            ctx.say("Each device of your tailnet can reach this port until Ctrl+C.");
            "serve"
        };
        // Tailscale prints the URL, and stops sharing on Ctrl+C.
        let status = ctx.interactive(&cli, &[mode, &port])?;
        if status != 0 {
            bail!("tailscale {mode} stopped with exit code {status}");
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
        let argv = ["odd", "expose"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Expose])
    }

    fn host() -> TestCtx {
        let t = TestCtx::new(Os::Linux);
        t.runner.with_path(&["tailscale"]);
        t.runner.answer("tailscale", &[], Output::ok(""));
        t
    }

    #[test]
    fn serve_runs_on_the_tailnet() {
        let mut t = host();
        assert_eq!(odd(&mut t, &["3000"]), Exit::Done);
        assert_eq!(t.runner.calls(), vec![vec!["tailscale", "serve", "3000"]]);
    }

    #[test]
    fn public_runs_funnel() {
        let mut t = host();
        odd(&mut t, &["3000", "--public"]);
        assert_eq!(t.runner.calls(), vec![vec!["tailscale", "funnel", "3000"]]);
        assert!(t.output().contains("internet"), "{}", t.output());
    }

    #[test]
    fn no_tailscale_fails() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &["3000"]), Exit::Failed);
        assert!(t.output().contains("odd ssh --tailscale"), "{}", t.output());
    }

    #[test]
    fn a_bad_port_exits_2() {
        let mut t = host();
        assert_eq!(odd(&mut t, &["0"]), Exit::Usage);
        assert!(t.runner.calls().is_empty());
    }
}
