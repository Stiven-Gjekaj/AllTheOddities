//! `odd autostart`: start a command when the user logs in.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Arg, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "autostart";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// `odd autostart`.
pub struct Autostart;

impl Oddity for Autostart {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Start a command when you log in: a launch agent, a systemd user unit, or a Run value."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("add")
                .about(
                    "Start a command at each login: odd autostart add <name> -- <command> [args]",
                )
                .arg(Arg::new("name").required(true))
                .arg(
                    Arg::new("command")
                        .required(true)
                        .num_args(1..)
                        .trailing_var_arg(true)
                        .allow_hyphen_values(true),
                ),
        )
        .subcommand(
            Command::new("remove")
                .about("Stop a command that odd added")
                .arg(Arg::new("name").required(true)),
        )
        .subcommand(Command::new("list").about("List the commands that odd added"))
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match m.subcommand() {
            Some(("add", a)) => {
                let name = a.get_one::<String>("name").cloned().unwrap_or_default();
                if !valid(&name) {
                    ctx.say(&format!(
                        "{name} is not a name for odd autostart. Use letters, digits, - and _."
                    ));
                    return Ok(Exit::Usage);
                }
                let command: Vec<String> = a
                    .get_many::<String>("command")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                add(ctx, &name, &command)?;
                Ok(Exit::Done)
            }
            Some(("remove", a)) => {
                let name = a.get_one::<String>("name").cloned().unwrap_or_default();
                if !valid(&name) {
                    return Ok(Exit::Usage);
                }
                remove(ctx, &name)?;
                Ok(Exit::Done)
            }
            _ => {
                list(ctx)?;
                Ok(Exit::Done)
            }
        }
    }
}

fn valid(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn agent_path(ctx: &Ctx<'_>, name: &str) -> PathBuf {
    ctx.home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("com.odd.{name}.plist"))
}

fn unit_name(name: &str) -> String {
    format!("odd-{name}.service")
}

fn unit_path(ctx: &Ctx<'_>, name: &str) -> PathBuf {
    ctx.home
        .join(".config")
        .join("systemd")
        .join("user")
        .join(unit_name(name))
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The ExecStart of systemd: each argument in double quotes, with `\` and
/// `"` escaped, and `$` and `%`, which systemd expands, doubled.
pub fn systemd_line(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            let e = a
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "$$")
                .replace('%', "%%");
            format!("\"{e}\"")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A command line of Windows: an argument with a space or a quote goes in
/// double quotes, with each quote as `\"`.
pub fn windows_line(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() || a.contains([' ', '\t', '"']) {
                format!("\"{}\"", a.replace('"', "\\\""))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn ok(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    let out = ctx.change(program, args)?;
    if !out.success() {
        bail!("{program} failed: {}", out.stderr.trim());
    }
    Ok(())
}

fn undo_run(ctx: &mut Ctx<'_>, program: &str, args: &[&str]) -> Result<()> {
    state::record(
        ctx,
        NAME,
        Undo::Run {
            program: program.into(),
            args: args.iter().map(|s| (*s).to_owned()).collect(),
        },
    )
}

/// Writes a new file, and records that undo deletes it.
fn write_new(ctx: &mut Ctx<'_>, path: &std::path::Path, text: &str) -> Result<()> {
    let before = std::fs::read_to_string(path).ok();
    state::record(
        ctx,
        NAME,
        Undo::RestoreFile {
            path: path.to_owned(),
            before,
            after_sha256: state::sha256_hex(text.as_bytes()),
        },
    )?;
    ctx.write_file(path, text)
}

fn add(ctx: &mut Ctx<'_>, name: &str, command: &[String]) -> Result<()> {
    match ctx.host.os {
        Os::MacOs => {
            let path = agent_path(ctx, name);
            let args: String = command
                .iter()
                .map(|a| format!("    <string>{}</string>\n", xml(a)))
                .collect();
            let text = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>com.odd.{name}</string>\n  <key>ProgramArguments</key>\n  <array>\n{args}  </array>\n  <key>RunAtLoad</key>\n  <true/>\n</dict>\n</plist>\n"
            );
            // Undo runs newest first: unload, then delete the file.
            write_new(ctx, &path, &text)?;
            let p = path.to_string_lossy().into_owned();
            undo_run(ctx, "launchctl", &["unload", "-w", &p])?;
            ok(ctx, "launchctl", &["load", "-w", &p])?;
        }
        Os::Linux => {
            let path = unit_path(ctx, name);
            let text = format!(
                "[Unit]\nDescription=odd autostart {name}\n\n[Service]\nExecStart={}\n\n[Install]\nWantedBy=default.target\n",
                systemd_line(command)
            );
            let unit = unit_name(name);
            // Undo runs newest first: disable, delete the file, reload.
            undo_run(ctx, "systemctl", &["--user", "daemon-reload"])?;
            write_new(ctx, &path, &text)?;
            undo_run(ctx, "systemctl", &["--user", "disable", "--now", &unit])?;
            ok(ctx, "systemctl", &["--user", "daemon-reload"])?;
            ok(ctx, "systemctl", &["--user", "enable", "--now", &unit])?;
        }
        Os::Windows => {
            let value = format!("odd-{name}");
            let before = crate::winreg::read(ctx, RUN_KEY, &value)?;
            state::record(ctx, NAME, crate::winreg::undo_step(RUN_KEY, &value, before))?;
            crate::winreg::write(ctx, RUN_KEY, &value, "REG_SZ", &windows_line(command))?;
        }
    }
    ctx.say(&format!("{name} starts at each login."));
    Ok(())
}

fn remove(ctx: &mut Ctx<'_>, name: &str) -> Result<()> {
    match ctx.host.os {
        Os::MacOs => {
            let path = agent_path(ctx, name);
            let p = path.to_string_lossy().into_owned();
            let _ = ctx.change("launchctl", &["unload", "-w", &p]);
            if path.exists() {
                ctx.remove_file(&path)?;
            }
        }
        Os::Linux => {
            let unit = unit_name(name);
            let _ = ctx.change("systemctl", &["--user", "disable", "--now", &unit]);
            let path = unit_path(ctx, name);
            if path.exists() {
                ctx.remove_file(&path)?;
            }
            ok(ctx, "systemctl", &["--user", "daemon-reload"])?;
        }
        Os::Windows => {
            let value = format!("odd-{name}");
            let _ = ctx.change("reg", &["delete", RUN_KEY, "/v", &value, "/f"]);
        }
    }
    ctx.say(&format!("{name} no longer starts at login."));
    Ok(())
}

fn list(ctx: &mut Ctx<'_>) -> Result<()> {
    let names: Vec<String> = match ctx.host.os {
        Os::MacOs | Os::Linux => {
            let (dir, prefix, suffix) = if ctx.host.os == Os::MacOs {
                (ctx.home.join("Library/LaunchAgents"), "com.odd.", ".plist")
            } else {
                (ctx.home.join(".config/systemd/user"), "odd-", ".service")
            };
            let mut v: Vec<String> = std::fs::read_dir(dir)
                .map(|rd| {
                    rd.flatten()
                        .filter_map(|e| {
                            let n = e.file_name().to_string_lossy().into_owned();
                            n.strip_prefix(prefix)?
                                .strip_suffix(suffix)
                                .map(str::to_owned)
                        })
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            v
        }
        Os::Windows => {
            let out = ctx.query("reg", &["query", RUN_KEY])?;
            out.stdout
                .lines()
                .filter_map(|l| {
                    l.trim_start()
                        .strip_prefix("odd-")?
                        .split_whitespace()
                        .next()
                        .map(str::to_owned)
                })
                .collect()
        }
    };
    if names.is_empty() {
        ctx.say("odd autostart has added no command.");
    }
    for n in names {
        ctx.say(&n);
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
        let argv = ["odd", "autostart"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Autostart])
    }

    fn host(os: Os) -> TestCtx {
        let t = TestCtx::new(os);
        for p in ["launchctl", "systemctl", "reg"] {
            t.runner.answer(p, &[], Output::ok(""));
        }
        t.runner
            .answer("reg", &["query"], Output::fail(1, "not found"));
        t
    }

    const ARGS: [&str; 6] = [
        "add",
        "sync",
        "--",
        "/opt/my tools/sync",
        "--dir",
        "a \"b\" $HOME 50%",
    ];

    #[test]
    fn macos_writes_a_launch_agent() {
        let mut t = host(Os::MacOs);
        assert_eq!(odd(&mut t, &ARGS), Exit::Done);
        let path = agent_path(&t.ctx(), "sync");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("<string>/opt/my tools/sync</string>"),
            "{text}"
        );
        assert!(
            text.contains("<string>a &quot;b&quot; $HOME 50%</string>"),
            "{text}"
        );
        assert!(text.contains("<key>RunAtLoad</key>"), "{text}");
        assert!(
            t.runner.calls().contains(
                &["launchctl", "load", "-w", &path.to_string_lossy()]
                    .map(String::from)
                    .to_vec()
            )
        );
    }

    #[test]
    fn linux_writes_a_user_unit() {
        let mut t = host(Os::Linux);
        assert_eq!(odd(&mut t, &ARGS), Exit::Done);
        let text = std::fs::read_to_string(t.home().join(".config/systemd/user/odd-sync.service"))
            .unwrap();
        assert!(
            text.contains(r#"ExecStart="/opt/my tools/sync" "--dir" "a \"b\" $$HOME 50%%""#),
            "{text}"
        );
        assert!(text.contains("WantedBy=default.target"), "{text}");
        let calls = t.runner.calls();
        assert!(
            calls.contains(
                &["systemctl", "--user", "enable", "--now", "odd-sync.service"]
                    .map(String::from)
                    .to_vec()
            )
        );
    }

    #[test]
    fn windows_writes_the_run_value() {
        let mut t = host(Os::Windows);
        assert_eq!(odd(&mut t, &ARGS), Exit::Done);
        let add = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[1] == "add")
            .unwrap();
        assert_eq!(add[4], "odd-sync");
        assert_eq!(add[8], r#""/opt/my tools/sync" --dir "a \"b\" $HOME 50%""#);
    }

    #[test]
    fn a_command_with_spaces_stays_whole() {
        assert_eq!(
            windows_line(&["C:\\Program Files\\x.exe".into(), "-v".into()]),
            r#""C:\Program Files\x.exe" -v"#
        );
        assert_eq!(systemd_line(&["a b".into()]), r#""a b""#);
    }

    #[test]
    fn a_bad_name_exits_2() {
        let mut t = host(Os::Linux);
        assert_eq!(odd(&mut t, &["add", "my sync", "--", "x"]), Exit::Usage);
        assert_eq!(odd(&mut t, &["add", "../x", "--", "x"]), Exit::Usage);
        assert!(t.runner.calls().is_empty());
    }

    #[test]
    fn remove_undoes_add() {
        let mut t = host(Os::Linux);
        odd(&mut t, &ARGS);
        assert_eq!(odd(&mut t, &["remove", "sync"]), Exit::Done);
        assert!(
            !t.home()
                .join(".config/systemd/user/odd-sync.service")
                .exists()
        );
        assert!(
            t.runner.calls().contains(
                &[
                    "systemctl",
                    "--user",
                    "disable",
                    "--now",
                    "odd-sync.service"
                ]
                .map(String::from)
                .to_vec()
            )
        );
    }

    #[test]
    fn undo_undoes_add() {
        let mut t = host(Os::MacOs);
        odd(&mut t, &ARGS);
        assert_eq!(odd(&mut t, &["--undo"]), Exit::Done);
        assert!(
            !t.home()
                .join("Library/LaunchAgents/com.odd.sync.plist")
                .exists()
        );
        assert_eq!(t.runner.calls().last().unwrap()[1], "unload");
    }

    #[test]
    fn list_shows_the_names() {
        let mut t = host(Os::Linux);
        odd(&mut t, &ARGS);
        t.out.clear();
        odd(&mut t, &["list"]);
        assert_eq!(t.output().trim(), "sync");
    }
}
