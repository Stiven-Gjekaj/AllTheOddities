//! `odd hostname`: set the name of the computer in each place that the host keeps it.

use anyhow::{Result, bail};
use clap::{Arg, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "hostname";
const MAC_KEYS: [&str; 3] = ["ComputerName", "LocalHostName", "HostName"];

/// `odd hostname`.
pub struct Hostname;

impl Oddity for Hostname {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Set the name of the computer in each place that the host keeps it."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(Arg::new("new").help("The new name. With no name, odd prints the names"))
    }
    fn needs_admin(&self, m: &ArgMatches, _ctx: &Ctx<'_>) -> bool {
        m.get_one::<String>("new").is_some()
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let Some(name) = m.get_one::<String>("new").cloned() else {
            show(ctx)?;
            return Ok(Exit::Done);
        };
        if let Err(why) = check(&name, ctx.host.os) {
            ctx.say(&why);
            return Ok(Exit::Usage);
        }
        match ctx.host.os {
            Os::MacOs => macos(ctx, &name)?,
            Os::Linux => linux(ctx, &name)?,
            Os::Windows => windows(ctx, &name)?,
        }
        Ok(Exit::Done)
    }
}

/// A name that each host takes: 1 to 63 letters, digits and hyphens, with
/// no hyphen first or last, and at most 15 characters on Windows.
pub fn check(name: &str, os: Os) -> Result<(), String> {
    let ok_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if name.is_empty()
        || name.len() > 63
        || !ok_chars
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err(format!(
            "{name} is not a name for a computer. Use 1 to 63 letters, digits and hyphens, with no hyphen first or last."
        ));
    }
    if os == Os::Windows && name.len() > 15 {
        return Err(format!(
            "Windows takes at most 15 characters for the name of a computer. {name} has {}.",
            name.len()
        ));
    }
    Ok(())
}

fn ok(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    let out = ctx.change(program, args)?;
    if !out.success() {
        bail!("{program} failed: {}", out.stderr.trim());
    }
    Ok(())
}

fn first_line(ctx: &Ctx<'_>, program: &str, args: &[&str]) -> Option<String> {
    let out = ctx.query(program, args).ok()?;
    let line = out.stdout.lines().next()?.trim().to_owned();
    (out.success() && !line.is_empty()).then_some(line)
}

fn show(ctx: &mut Ctx<'_>) -> Result<()> {
    match ctx.host.os {
        Os::MacOs => {
            for key in MAC_KEYS {
                let v = first_line(ctx, "scutil", &["--get", key])
                    .unwrap_or_else(|| "(not set)".into());
                ctx.say(&format!("{key}: {v}"));
            }
        }
        Os::Linux => {
            let v = first_line(ctx, "hostnamectl", &["--static"]).unwrap_or_default();
            ctx.say(&v);
        }
        Os::Windows => {
            let v = first_line(ctx, "hostname", &[]).unwrap_or_default();
            ctx.say(&v);
        }
    }
    Ok(())
}

fn macos(ctx: &mut Ctx<'_>, name: &str) -> Result<()> {
    for key in MAC_KEYS {
        if let Some(old) = first_line(ctx, "scutil", &["--get", key]) {
            let undo = Undo::Run {
                program: "scutil".into(),
                args: vec!["--set".into(), key.into(), old],
            };
            state::record(ctx, NAME, undo)?;
        }
        ok(ctx, "scutil", &["--set", key, name])?;
    }
    ctx.say(&format!("The computer is {name}."));
    Ok(())
}

fn linux(ctx: &mut Ctx<'_>, name: &str) -> Result<()> {
    let old = first_line(ctx, "hostnamectl", &["--static"]);
    if let Some(old) = &old {
        let undo = Undo::Run {
            program: "hostnamectl".into(),
            args: vec!["set-hostname".into(), old.clone()],
        };
        state::record(ctx, NAME, undo)?;
    }
    ok(ctx, "hostnamectl", &["set-hostname", name])?;
    // Debian and its family name the computer in /etc/hosts too. An old
    // name there makes sudo wait for a lookup.
    let hosts = ctx.sys_path("/etc/hosts");
    if let (Some(old), Ok(text)) = (old, std::fs::read_to_string(&hosts)) {
        let new: String = text
            .split_inclusive('\n')
            .map(|line| {
                if !line.starts_with("127.0.1.1") {
                    return line.to_owned();
                }
                let end = &line[line.trim_end().len()..];
                let words: Vec<&str> = line
                    .split_whitespace()
                    .map(|w| if w == old { name } else { w })
                    .collect();
                format!("{}{end}", words.join(" "))
            })
            .collect();
        if new != text {
            let step = Undo::RestoreFile {
                path: hosts.clone(),
                before: Some(text),
                after_sha256: state::sha256_hex(new.as_bytes()),
            };
            state::record(ctx, NAME, step)?;
            ctx.write_file(&hosts, &new)?;
        }
    }
    ctx.say(&format!("The computer is {name}."));
    Ok(())
}

fn rename(name: &str) -> Vec<String> {
    vec![
        "-NoProfile".into(),
        "-Command".into(),
        format!("Rename-Computer -NewName '{name}' -Force"),
    ]
}

fn windows(ctx: &mut Ctx<'_>, name: &str) -> Result<()> {
    if let Some(old) = first_line(ctx, "hostname", &[]) {
        state::record(
            ctx,
            NAME,
            Undo::Run {
                program: "powershell".into(),
                args: rename(&old),
            },
        )?;
    }
    let args = rename(name);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    ok(ctx, "powershell", &refs)?;
    ctx.say(&format!("The computer is {name} after the next restart."));
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
        let argv = ["odd", "hostname"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Hostname])
    }

    #[test]
    fn a_name_with_a_space_exits_2() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &["my box"]), Exit::Usage);
        assert_eq!(odd(&mut t, &["-box"]), Exit::Usage);
        assert_eq!(odd(&mut t, &[&"a".repeat(64)]), Exit::Usage);
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn windows_refuses_16_characters() {
        let mut t = TestCtx::new(Os::Windows);
        assert_eq!(odd(&mut t, &["sixteen-letters1"]), Exit::Usage);
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn macos_sets_three_names() {
        let mut t = TestCtx::new(Os::MacOs);
        t.runner.answer("scutil", &["--get"], Output::ok("old\n"));
        t.runner.answer("scutil", &["--set"], Output::ok(""));
        assert_eq!(odd(&mut t, &["studio"]), Exit::Done);
        let sets: Vec<String> = t
            .runner
            .calls()
            .into_iter()
            .filter(|c| c[1] == "--set")
            .map(|c| c[2].clone())
            .collect();
        assert_eq!(sets, ["ComputerName", "LocalHostName", "HostName"]);
    }

    #[test]
    fn linux_updates_etc_hosts() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner
            .answer("hostnamectl", &["--static"], Output::ok("old\n"));
        t.runner
            .answer("hostnamectl", &["set-hostname"], Output::ok(""));
        let hosts = t.sys("/etc/hosts");
        std::fs::create_dir_all(hosts.parent().unwrap()).unwrap();
        std::fs::write(
            &hosts,
            "127.0.0.1 localhost\n127.0.1.1 old old.lan\n10.0.0.2 older\n",
        )
        .unwrap();
        assert_eq!(odd(&mut t, &["box"]), Exit::Done);
        assert_eq!(
            std::fs::read_to_string(&hosts).unwrap(),
            "127.0.0.1 localhost\n127.0.1.1 box old.lan\n10.0.0.2 older\n"
        );
    }

    #[test]
    fn undo_sets_the_old_name_back() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer("hostname", &[], Output::ok("OLD-PC\r\n"));
        t.runner.answer("powershell", &[], Output::ok(""));
        odd(&mut t, &["new-pc"]);
        odd(&mut t, &["--undo"]);
        let last = t.runner.calls().pop().unwrap();
        assert!(last.iter().any(|a| a.contains("'OLD-PC'")), "{last:?}");
    }

    #[test]
    fn no_name_prints_the_names() {
        let mut t = TestCtx::new(Os::Linux);
        t.is_admin = false;
        t.runner
            .answer("hostnamectl", &["--static"], Output::ok("box\n"));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.output().contains("box"));
    }
}
