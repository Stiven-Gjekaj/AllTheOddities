//! `odd env`: set an environment variable of the user for good.

use anyhow::{Result, bail};
use clap::{Arg, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "env";
const ENV_KEY: &str = r"HKCU\Environment";

/// `odd env`.
pub struct Env;

impl Oddity for Env {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Set an environment variable of the user for good."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("set")
                .about("Set a variable for each new shell or window")
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("value").required(true).allow_hyphen_values(true)),
        )
        .subcommand(Command::new("list").about("List the variables that odd set"))
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match m.subcommand() {
            Some(("set", a)) => {
                let name = a.get_one::<String>("name").cloned().unwrap_or_default();
                let value = a.get_one::<String>("value").cloned().unwrap_or_default();
                if !valid_name(&name) {
                    ctx.say(&format!(
                        "{name} is not a name for a variable. Use letters, digits and _, and not a digit first."
                    ));
                    return Ok(Exit::Usage);
                }
                set(ctx, &name, &value)?;
                Ok(Exit::Done)
            }
            Some(("list", _)) => {
                list(ctx)?;
                Ok(Exit::Done)
            }
            _ => {
                ctx.say("Give a command: odd env set <NAME> <value>, or odd env list.");
                Ok(Exit::Usage)
            }
        }
    }
}

/// `[A-Za-z_][A-Za-z0-9_]*`.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A value in single quotes for sh. A `'` becomes `'\''`.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn set(ctx: &mut Ctx<'_>, name: &str, value: &str) -> Result<()> {
    if ctx.host.os == Os::Windows {
        let before = crate::winreg::read(ctx, ENV_KEY, name)?;
        if before.as_ref().is_some_and(|(_, d)| d == value) {
            ctx.say(&format!("{name} has this value already."));
            return Ok(());
        }
        state::record(ctx, NAME, crate::winreg::undo_step(ENV_KEY, name, before))?;
        crate::winreg::write(ctx, ENV_KEY, name, "REG_SZ", value)?;
        if !ctx.dry_run {
            crate::winreg::broadcast_change();
        }
        ctx.say(&format!("{name} is set for each new window."));
        return Ok(());
    }
    let shell = ctx.env.get("SHELL").cloned().unwrap_or_default();
    if shell.ends_with("fish") {
        let q = crate::profile::fish_quote(value);
        let undo = Undo::Run {
            program: "fish".into(),
            args: vec!["-c".into(), format!("set -Ue {name}")],
        };
        state::record(ctx, NAME, undo)?;
        let out = ctx.change("fish", &["-c", &format!("set -Ux {name} {q}")])?;
        if !out.success() {
            bail!("fish could not set {name}: {}", out.stderr.trim());
        }
        ctx.say(&format!("{name} is set for each new fish shell."));
        return Ok(());
    }
    let line = format!("export {name}={}", sh_quote(value));
    for file in crate::profile::profiles(ctx, &shell) {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let prefix = format!("export {name}=");
        let mut lines: Vec<String> = crate::block::body(&text, NAME, "#")
            .map(|b| {
                b.lines()
                    .filter(|l| !l.starts_with(&prefix))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        lines.push(line.clone());
        state::upsert_block(ctx, NAME, &file, NAME, &lines.join("\n"), &|_: &str| false)?;
    }
    ctx.say(&format!("{name} is set for each new shell."));
    Ok(())
}

fn list(ctx: &mut Ctx<'_>) -> Result<()> {
    let names: Vec<String> = if ctx.host.os == Os::Windows {
        state::steps(ctx, NAME)?
            .into_iter()
            .filter_map(|s| match s {
                Undo::Run { args, .. } => args.get(3).cloned(),
                _ => None,
            })
            .collect()
    } else {
        let text = std::fs::read_to_string(ctx.home.join(".profile")).unwrap_or_default();
        crate::block::body(&text, NAME, "#")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                l.strip_prefix("export ")?
                    .split_once('=')
                    .map(|(n, _)| n.to_owned())
            })
            .collect()
    };
    if names.is_empty() {
        ctx.say("odd env has set no variable.");
    }
    let mut seen = Vec::new();
    for n in names {
        if !seen.contains(&n) {
            ctx.say(&n);
            seen.push(n);
        }
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
        let argv = ["odd", "env"].iter().chain(args).map(Into::into).collect();
        dispatch(argv, &mut t.ctx(), &[&Env])
    }

    fn unix() -> TestCtx {
        let mut t = TestCtx::new(Os::Linux);
        t.env.insert("SHELL".into(), "/bin/bash".into());
        t
    }

    fn profile(t: &TestCtx) -> String {
        std::fs::read_to_string(t.home().join(".profile")).unwrap()
    }

    #[test]
    fn set_writes_an_export_line() {
        let mut t = unix();
        assert_eq!(odd(&mut t, &["set", "EDITOR", "nvim"]), Exit::Done);
        assert!(
            profile(&t).contains("export EDITOR='nvim'"),
            "{}",
            profile(&t)
        );
    }

    #[test]
    fn a_second_set_replaces_the_line() {
        let mut t = unix();
        odd(&mut t, &["set", "EDITOR", "nvim"]);
        odd(&mut t, &["set", "EDITOR", "hx"]);
        let text = profile(&t);
        assert!(
            text.contains("export EDITOR='hx'") && !text.contains("nvim"),
            "{text}"
        );
    }

    #[test]
    fn a_quote_in_the_value_survives() {
        let mut t = unix();
        odd(&mut t, &["set", "GREETING", "it's $HOME \"x\""]);
        assert!(
            profile(&t).contains(r#"export GREETING='it'\''s $HOME "x"'"#),
            "{}",
            profile(&t)
        );
    }

    #[test]
    fn a_bad_name_exits_2() {
        let mut t = unix();
        assert_eq!(odd(&mut t, &["set", "1BAD", "x"]), Exit::Usage);
        assert_eq!(odd(&mut t, &["set", "A-B", "x"]), Exit::Usage);
        assert!(!t.home().join(".profile").exists());
    }

    fn windows(before: Option<&str>) -> TestCtx {
        let t = TestCtx::new(Os::Windows);
        match before {
            Some(v) => t.runner.answer(
                "reg",
                &["query"],
                Output::ok(&format!(
                    "\r\nHKEY_CURRENT_USER\\Environment\r\n    EDITOR    REG_SZ    {v}\r\n"
                )),
            ),
            None => t
                .runner
                .answer("reg", &["query"], Output::fail(1, "ERROR: not found")),
        }
        t.runner.answer("reg", &["add"], Output::ok(""));
        t.runner.answer("reg", &["delete"], Output::ok(""));
        t
    }

    fn calls(t: &TestCtx, verb: &str) -> Vec<Vec<String>> {
        t.runner
            .calls()
            .into_iter()
            .filter(|c| c[0] == "reg" && c[1] == verb)
            .collect()
    }

    #[test]
    fn windows_sets_the_user_value() {
        let mut t = windows(None);
        odd(&mut t, &["set", "EDITOR", "code -w"]);
        let add = &calls(&t, "add")[0];
        assert_eq!(
            add[2..],
            [
                "HKCU\\Environment",
                "/v",
                "EDITOR",
                "/t",
                "REG_SZ",
                "/d",
                "code -w",
                "/f"
            ]
        );
    }

    #[test]
    fn windows_undo_deletes_a_new_value() {
        let mut t = windows(None);
        odd(&mut t, &["set", "EDITOR", "nvim"]);
        odd(&mut t, &["--undo"]);
        assert_eq!(calls(&t, "delete").len(), 1);
    }

    #[test]
    fn windows_undo_puts_back_the_old_value() {
        let mut t = windows(Some("vim"));
        odd(&mut t, &["set", "EDITOR", "nvim"]);
        odd(&mut t, &["--undo"]);
        let adds = calls(&t, "add");
        assert_eq!(adds.last().unwrap()[8], "vim");
        assert!(calls(&t, "delete").is_empty());
    }

    #[test]
    fn list_shows_the_names() {
        let mut t = unix();
        odd(&mut t, &["set", "EDITOR", "nvim"]);
        t.out.clear();
        odd(&mut t, &["list"]);
        assert!(t.output().contains("EDITOR"), "{}", t.output());
    }
}
