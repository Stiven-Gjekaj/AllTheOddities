//! The shape of an oddity, the list of them, and the run of one.

use std::ffi::OsString;

use clap::ArgMatches;
use serde::Serialize;

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;

/// What a release promises about an oddity. docs/releasing.md gives the rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    /// Flags and output can change in any release.
    Experimental,
    /// Flags and exit codes stay the same.
    Stable,
    /// It still runs, and it goes away in a later release.
    Deprecated,
}

impl Label {
    pub fn word(self) -> &'static str {
        match self {
            Label::Experimental => "experimental",
            Label::Stable => "stable",
            Label::Deprecated => "deprecated",
        }
    }
}

/// One oddity: one subcommand of `odd`.
pub trait Oddity: Sync {
    fn name(&self) -> &'static str;
    fn summary(&self) -> &'static str;
    fn hosts(&self) -> &'static [Os];
    fn label(&self) -> Label {
        Label::Experimental
    }
    /// The arguments of the oddity. The common flags come from cli.rs.
    fn args(&self, cmd: clap::Command) -> clap::Command {
        cmd
    }
    /// False for an oddity that changes nothing.
    fn can_undo(&self) -> bool {
        true
    }
    /// True when this run needs an administrator. `odd` checks it before
    /// `run`, so a run never stops halfway for lack of rights.
    fn needs_admin(&self, _m: &ArgMatches, _ctx: &Ctx<'_>) -> bool {
        false
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> anyhow::Result<Exit>;
    /// Reverts one `Undo::Call` step that this oddity recorded.
    fn undo_call(&self, action: &str, _arg: &str, _ctx: &mut Ctx<'_>) -> anyhow::Result<()> {
        anyhow::bail!("odd {} has no undo step named {action}", self.name())
    }
}

/// Each oddity of this build. To add one: add its file, and one line here.
pub static ODDITIES: &[&dyn Oddity] = &[
    &crate::oddities::path::Path,
    &crate::oddities::ssh::Ssh,
    &crate::oddities::git::Git,
    &crate::oddities::env::Env,
    &crate::oddities::tweaks::Tweaks,
    &crate::oddities::hostname::Hostname,
    &crate::oddities::hosts::Hosts,
    &crate::oddities::dns::Dns,
];

/// Parses `argv` and runs the command. Each message goes to `ctx.out`.
pub fn dispatch(argv: Vec<OsString>, ctx: &mut Ctx<'_>, oddities: &[&dyn Oddity]) -> Exit {
    let shown: Vec<String> = argv
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let m = match crate::cli::command(oddities).try_get_matches_from(argv) {
        Ok(m) => m,
        Err(e) => {
            ctx.say(e.render().to_string().trim_end());
            return if e.use_stderr() {
                Exit::Usage
            } else {
                Exit::Done
            };
        }
    };
    let Some((name, sub)) = m.subcommand() else {
        return Exit::Usage;
    };
    if name == "update" {
        ctx.dry_run |= sub.get_flag("dry-run");
        let result = std::env::current_exe()
            .map_err(anyhow::Error::from)
            .and_then(|exe| crate::oddities::update::run(ctx, &exe));
        return match result {
            Ok(exit) => exit,
            Err(e) => {
                ctx.say(&format!("odd update: {e:#}"));
                Exit::Failed
            }
        };
    }
    if name == "list" {
        crate::oddities::list::run(oddities, sub.get_flag("json"), ctx);
        return Exit::Done;
    }
    let Some(oddity) = oddities.iter().find(|o| o.name() == name) else {
        return Exit::Usage;
    };
    if sub.get_flag("dry-run") {
        ctx.dry_run = true;
    }
    if !oddity.hosts().contains(&ctx.host.os) {
        ctx.say(&format!("odd {name} does not run on {} yet.", ctx.host.os));
        return Exit::Usage;
    }
    if sub.get_flag("undo") {
        return run_undo(*oddity, ctx);
    }
    if !ctx.dry_run && !ctx.is_admin && oddity.needs_admin(sub, ctx) {
        let again = format!("odd {}", shown.join(" "));
        ctx.say(&format!(
            "odd {name} needs an administrator. It changed nothing."
        ));
        match ctx.host.os {
            Os::Windows => ctx.say(&format!(
                "Run again in a PowerShell that runs as administrator: {again}"
            )),
            _ => ctx.say(&format!("Run again with: sudo {again}")),
        }
        return Exit::NeedsAdmin;
    }
    match oddity.run(sub, ctx) {
        Ok(exit) => exit,
        Err(e) => {
            ctx.say(&format!("odd {name}: {e:#}"));
            Exit::Failed
        }
    }
}

fn run_undo(oddity: &dyn Oddity, ctx: &mut Ctx<'_>) -> Exit {
    let name = oddity.name();
    if !oddity.can_undo() {
        ctx.say(&format!(
            "odd {name} changes nothing, so it has nothing to undo."
        ));
        return Exit::Usage;
    }
    let mut call = |action: &str, arg: &str, ctx: &mut Ctx<'_>| oddity.undo_call(action, arg, ctx);
    let report = match crate::state::undo(ctx, name, &mut call) {
        Ok(r) => r,
        Err(e) => {
            ctx.say(&format!("odd {name}: {e:#}"));
            return Exit::Failed;
        }
    };
    if report.undone.is_empty() && report.skipped.is_empty() {
        ctx.say(&format!("odd {name} has no recorded change to undo."));
        return Exit::Done;
    }
    for line in &report.undone {
        ctx.say(&format!("Undone: {line}"));
    }
    for line in &report.skipped {
        ctx.say(&format!("Skipped: {line}"));
    }
    if report.skipped.is_empty() {
        Exit::Done
    } else {
        Exit::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::run::Output;
    use crate::testing::TestCtx;

    struct Probe {
        hosts: &'static [Os],
        admin: bool,
        undo: bool,
    }

    impl Oddity for Probe {
        fn name(&self) -> &'static str {
            "probe"
        }
        fn summary(&self) -> &'static str {
            "A test oddity."
        }
        fn hosts(&self) -> &'static [Os] {
            self.hosts
        }
        fn can_undo(&self) -> bool {
            self.undo
        }
        fn needs_admin(&self, _m: &clap::ArgMatches, _ctx: &Ctx<'_>) -> bool {
            self.admin
        }
        fn run(&self, _m: &clap::ArgMatches, ctx: &mut Ctx<'_>) -> anyhow::Result<Exit> {
            ctx.change("touch", &["x"])?;
            Ok(Exit::Done)
        }
    }

    const ALL: &[Os] = &[Os::MacOs, Os::Linux, Os::Windows];

    fn argv(args: &[&str]) -> Vec<std::ffi::OsString> {
        std::iter::once("odd")
            .chain(args.iter().copied())
            .map(Into::into)
            .collect()
    }

    #[test]
    fn an_oddity_on_another_host_exits_2() {
        let mut t = TestCtx::new(Os::Windows);
        let p = Probe {
            hosts: &[Os::Linux],
            admin: false,
            undo: true,
        };
        let exit = dispatch(argv(&["probe"]), &mut t.ctx(), &[&p]);
        assert_eq!(exit, Exit::Usage);
        assert!(
            t.output()
                .contains("odd probe does not run on Windows yet."),
            "{}",
            t.output()
        );
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn needs_admin_exits_3_and_runs_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        t.is_admin = false;
        let p = Probe {
            hosts: ALL,
            admin: true,
            undo: true,
        };
        let exit = dispatch(argv(&["probe", "--yes"]), &mut t.ctx(), &[&p]);
        assert_eq!(exit, Exit::NeedsAdmin);
        assert!(t.runner.calls().is_empty());
        assert!(
            t.output().contains("Run again with: sudo odd probe --yes"),
            "{}",
            t.output()
        );
    }

    #[test]
    fn needs_admin_on_windows_names_an_administrator_shell() {
        let mut t = TestCtx::new(Os::Windows);
        t.is_admin = false;
        let p = Probe {
            hosts: ALL,
            admin: true,
            undo: true,
        };
        dispatch(argv(&["probe"]), &mut t.ctx(), &[&p]);
        assert!(
            t.output().contains("as administrator: odd probe"),
            "{}",
            t.output()
        );
    }

    #[test]
    fn a_run_runs() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("touch", &[], Output::ok(""));
        let p = Probe {
            hosts: ALL,
            admin: false,
            undo: true,
        };
        assert_eq!(dispatch(argv(&["probe"]), &mut t.ctx(), &[&p]), Exit::Done);
        assert_eq!(t.runner.calls().len(), 1);
    }

    #[test]
    fn dry_run_flag_reaches_the_context() {
        let mut t = TestCtx::new(Os::Linux);
        let p = Probe {
            hosts: ALL,
            admin: false,
            undo: true,
        };
        dispatch(argv(&["probe", "--dry-run"]), &mut t.ctx(), &[&p]);
        assert!(t.runner.calls().is_empty());
        assert!(t.output().contains("would run: touch x"));
    }

    #[test]
    fn undo_calls_the_state() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("rm", &[], Output::ok(""));
        let step = crate::state::Undo::Run {
            program: "rm".into(),
            args: vec!["y".into()],
        };
        crate::state::record(&mut t.ctx(), "probe", step).unwrap();
        let p = Probe {
            hosts: ALL,
            admin: false,
            undo: true,
        };
        assert_eq!(
            dispatch(argv(&["probe", "--undo"]), &mut t.ctx(), &[&p]),
            Exit::Done
        );
        assert_eq!(t.runner.calls(), vec![vec!["rm", "y"]]);
    }

    #[test]
    fn undo_of_an_oddity_that_changes_nothing_exits_2() {
        let mut t = TestCtx::new(Os::Linux);
        let p = Probe {
            hosts: ALL,
            admin: false,
            undo: false,
        };
        assert_eq!(
            dispatch(argv(&["probe", "--undo"]), &mut t.ctx(), &[&p]),
            Exit::Usage
        );
        assert!(t.output().contains("nothing to undo"));
    }

    #[test]
    fn a_bad_option_exits_2() {
        let mut t = TestCtx::new(Os::Linux);
        let p = Probe {
            hosts: ALL,
            admin: false,
            undo: true,
        };
        assert_eq!(
            dispatch(argv(&["probe", "--nope"]), &mut t.ctx(), &[&p]),
            Exit::Usage
        );
    }

    #[test]
    fn update_is_a_command_of_odd() {
        let mut t = TestCtx::new(Os::Linux);
        // The fake network has no release, so the update fails, but it parses.
        assert_eq!(
            dispatch(argv(&["update", "--dry-run"]), &mut t.ctx(), &[]),
            Exit::Failed
        );
        assert!(t.output().starts_with("odd update:"), "{}", t.output());
    }

    #[test]
    fn list_json_has_each_oddity() {
        let mut t = TestCtx::new(Os::Linux);
        let p = Probe {
            hosts: &[Os::Linux],
            admin: false,
            undo: true,
        };
        assert_eq!(
            dispatch(argv(&["list", "--json"]), &mut t.ctx(), &[&p]),
            Exit::Done
        );
        let v: serde_json::Value = serde_json::from_str(&t.output()).unwrap();
        assert_eq!(v[0]["name"], "probe");
        assert_eq!(v[0]["label"], "experimental");
        assert_eq!(v[0]["hosts"][0], "linux");
        assert_eq!(v[0]["summary"], "A test oddity.");
    }

    #[test]
    fn list_shows_a_row_for_each_oddity() {
        let mut t = TestCtx::new(Os::Linux);
        let p = Probe {
            hosts: ALL,
            admin: false,
            undo: true,
        };
        dispatch(argv(&["list"]), &mut t.ctx(), &[&p]);
        let row = t
            .output()
            .lines()
            .find(|l| l.starts_with("probe"))
            .unwrap()
            .to_owned();
        assert!(
            row.contains("experimental") && row.contains("A test oddity."),
            "{row}"
        );
    }
}
