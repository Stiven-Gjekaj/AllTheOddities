//! `odd tweaks`: the developer settings of a fresh computer.

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "tweaks";

/// One value of the host that a tweak sets.
enum Setting {
    /// A DWORD of the registry. `on` is as `reg query` prints it, such as `0x1`.
    Reg {
        key: &'static str,
        name: &'static str,
        on: &'static str,
    },
    /// A value of `defaults`. `kind` is `bool` or `int`. `on` is as
    /// `defaults read` prints it; a bool prints 1 or 0.
    Defaults {
        domain: &'static str,
        key: &'static str,
        kind: &'static str,
        on: &'static str,
    },
    /// A kernel value, kept in /etc/sysctl.d/90-odd.conf.
    Sysctl { key: &'static str, on: &'static str },
}

struct Tweak {
    os: Os,
    name: &'static str,
    about: &'static str,
    admin: bool,
    settings: &'static [Setting],
}

const ADV: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SYSCTL_FILE: &str = "/etc/sysctl.d/90-odd.conf";

const TWEAKS: &[Tweak] = &[
    Tweak {
        os: Os::Windows,
        name: "file-extensions",
        about: "Show the extension of each file in Explorer",
        admin: false,
        settings: &[Setting::Reg {
            key: ADV,
            name: "HideFileExt",
            on: "0x0",
        }],
    },
    Tweak {
        os: Os::Windows,
        name: "hidden-files",
        about: "Show hidden files in Explorer",
        admin: false,
        settings: &[Setting::Reg {
            key: ADV,
            name: "Hidden",
            on: "0x1",
        }],
    },
    Tweak {
        os: Os::Windows,
        name: "long-paths",
        about: "Allow paths longer than 260 characters",
        admin: true,
        settings: &[Setting::Reg {
            key: r"HKLM\SYSTEM\CurrentControlSet\Control\FileSystem",
            name: "LongPathsEnabled",
            on: "0x1",
        }],
    },
    Tweak {
        os: Os::Windows,
        name: "developer-mode",
        about: "Turn on Developer Mode, for symbolic links without an administrator",
        admin: true,
        settings: &[Setting::Reg {
            key: r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock",
            name: "AllowDevelopmentWithoutDevLicense",
            on: "0x1",
        }],
    },
    Tweak {
        os: Os::MacOs,
        name: "hidden-files",
        about: "Show hidden files in Finder",
        admin: false,
        settings: &[Setting::Defaults {
            domain: "com.apple.finder",
            key: "AppleShowAllFiles",
            kind: "bool",
            on: "1",
        }],
    },
    Tweak {
        os: Os::MacOs,
        name: "key-repeat",
        about: "Repeat a held key fast, after a short delay",
        admin: false,
        settings: &[
            Setting::Defaults {
                domain: "NSGlobalDomain",
                key: "KeyRepeat",
                kind: "int",
                on: "2",
            },
            Setting::Defaults {
                domain: "NSGlobalDomain",
                key: "InitialKeyRepeat",
                kind: "int",
                on: "15",
            },
        ],
    },
    Tweak {
        os: Os::Linux,
        name: "inotify-watches",
        about: "Let file watchers, such as those of editors, watch more files",
        admin: true,
        settings: &[Setting::Sysctl {
            key: "fs.inotify.max_user_watches",
            on: "524288",
        }],
    },
];

/// `odd tweaks`.
pub struct Tweaks;

impl Oddity for Tweaks {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Apply the developer settings of a fresh computer, such as hidden files and long paths."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(Command::new("list").about("List the tweaks of this host and their state"))
            .subcommand(
                Command::new("apply")
                    .about("Turn on the tweaks that are off")
                    .arg(
                        Arg::new("only")
                            .long("only")
                            .action(ArgAction::Append)
                            .help("Apply only this tweak. Give it again for more"),
                    )
                    .arg(
                        Arg::new("user-only")
                            .long("user-only")
                            .action(ArgAction::SetTrue)
                            .help("Skip the tweaks that need an administrator"),
                    ),
            )
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match m.subcommand() {
            Some(("apply", a)) => {
                let only: Vec<String> = a
                    .get_many::<String>("only")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                apply(ctx, &only, a.get_flag("user-only"))
            }
            _ => {
                list(ctx)?;
                Ok(Exit::Done)
            }
        }
    }
}

fn of_host(os: Os) -> impl Iterator<Item = &'static Tweak> {
    TWEAKS.iter().filter(move |t| t.os == os)
}

/// The value of a setting now, as the host prints it. None when not set.
fn read(ctx: &Ctx<'_>, s: &Setting) -> Result<Option<String>> {
    Ok(match s {
        Setting::Reg { key, name, .. } => crate::winreg::read(ctx, key, name)?.map(|(_, d)| d),
        Setting::Defaults { domain, key, .. } => {
            let out = ctx.query("defaults", &["read", domain, key])?;
            out.success().then(|| out.stdout.trim().to_owned())
        }
        Setting::Sysctl { key, .. } => {
            let out = ctx.query("sysctl", &["-n", key])?;
            out.success().then(|| out.stdout.trim().to_owned())
        }
    })
}

fn on_value(s: &Setting) -> &'static str {
    match s {
        Setting::Reg { on, .. } | Setting::Defaults { on, .. } | Setting::Sysctl { on, .. } => on,
    }
}

fn is_on(ctx: &Ctx<'_>, t: &Tweak) -> Result<bool> {
    for s in t.settings {
        if read(ctx, s)?.as_deref() != Some(on_value(s)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn list(ctx: &mut Ctx<'_>) -> Result<()> {
    for t in of_host(ctx.host.os) {
        let state = if is_on(ctx, t)? { "on" } else { "off" };
        let admin = if t.admin { "  (administrator)" } else { "" };
        ctx.say(&format!("{:18} {:4} {}{admin}", t.name, state, t.about));
    }
    Ok(())
}

/// A bool of `defaults` as `defaults write -bool` takes it.
fn defaults_word(kind: &str, v: &str) -> String {
    match (kind, v) {
        ("bool", "1") => "true".into(),
        ("bool", "0") => "false".into(),
        _ => v.into(),
    }
}

fn set(ctx: &mut Ctx<'_>, s: &Setting) -> Result<()> {
    let before = read(ctx, s)?;
    if before.as_deref() == Some(on_value(s)) {
        return Ok(());
    }
    match s {
        Setting::Reg { key, name, on } => {
            let before = crate::winreg::read(ctx, key, name)?;
            state::record(ctx, NAME, crate::winreg::undo_step(key, name, before))?;
            crate::winreg::write(ctx, key, name, "REG_DWORD", on)?;
        }
        Setting::Defaults {
            domain,
            key,
            kind,
            on,
        } => {
            let undo: Vec<String> = match &before {
                Some(v) => vec![
                    "write".into(),
                    (*domain).into(),
                    (*key).into(),
                    format!("-{kind}"),
                    defaults_word(kind, v),
                ],
                None => vec!["delete".into(), (*domain).into(), (*key).into()],
            };
            state::record(
                ctx,
                NAME,
                Undo::Run {
                    program: "defaults".into(),
                    args: undo,
                },
            )?;
            let word = defaults_word(kind, on);
            let out = ctx.change(
                "defaults",
                &["write", domain, key, &format!("-{kind}"), &word],
            )?;
            if !out.success() {
                bail!(
                    "defaults write {domain} {key} failed: {}",
                    out.stderr.trim()
                );
            }
        }
        Setting::Sysctl { key, on } => {
            let path = ctx.sys_path(SYSCTL_FILE);
            let old = std::fs::read_to_string(&path).ok();
            let text =
                format!("# odd tweaks. odd tweaks --undo removes this file.\n{key} = {on}\n");
            state::record(
                ctx,
                NAME,
                Undo::Run {
                    program: "sysctl".into(),
                    args: vec!["--system".into()],
                },
            )?;
            state::record(
                ctx,
                NAME,
                Undo::RestoreFile {
                    path: path.clone(),
                    before: old,
                    after_sha256: state::sha256_hex(text.as_bytes()),
                },
            )?;
            ctx.write_file(&path, &text)?;
            ctx.change("sysctl", &["--system"])?;
        }
    }
    Ok(())
}

fn apply(ctx: &mut Ctx<'_>, only: &[String], user_only: bool) -> Result<Exit> {
    let os = ctx.host.os;
    for name in only {
        if !of_host(os).any(|t| t.name == name) {
            let names: Vec<&str> = of_host(os).map(|t| t.name).collect();
            ctx.say(&format!(
                "{name} is not a tweak of {os}. The tweaks are: {}.",
                names.join(", ")
            ));
            return Ok(Exit::Usage);
        }
    }
    let mut todo = Vec::new();
    for t in of_host(os) {
        if !only.is_empty() && !only.iter().any(|n| n == t.name) {
            continue;
        }
        if t.admin && user_only {
            continue;
        }
        if !is_on(ctx, t)? {
            todo.push(t);
        }
    }
    if !ctx.is_admin && !ctx.dry_run && todo.iter().any(|t| t.admin) {
        let names: Vec<&str> = todo.iter().filter(|t| t.admin).map(|t| t.name).collect();
        ctx.say(&format!(
            "{} need an administrator. odd changed nothing.",
            names.join(", ")
        ));
        ctx.say("Run again as an administrator, or add --user-only to skip them.");
        return Ok(Exit::NeedsAdmin);
    }
    if todo.is_empty() {
        ctx.say("Each tweak is on already.");
        return Ok(Exit::Done);
    }
    for t in &todo {
        for s in t.settings {
            set(ctx, s)?;
        }
        ctx.say(&format!("{} is on.", t.name));
    }
    if os == Os::MacOs {
        if todo.iter().any(|t| t.name == "hidden-files") {
            let _ = ctx.change("killall", &["Finder"]);
        }
        if todo.iter().any(|t| t.name == "key-repeat") {
            ctx.say("Key repeat changes at the next login.");
        }
    }
    Ok(Exit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "tweaks"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Tweaks])
    }

    const ADV: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";

    fn reg_value(t: &TestCtx, key: &str, name: &str, data: Option<&str>) {
        let out = match data {
            Some(d) => Output::ok(&format!("\r\n{key}\r\n    {name}    REG_DWORD    {d}\r\n")),
            None => Output::fail(1, "ERROR: not found"),
        };
        t.runner.answer("reg", &["query", key, "/v", name], out);
    }

    /// Windows with extensions hidden, hidden files shown, and the two admin
    /// tweaks off (not set).
    fn windows() -> TestCtx {
        let t = TestCtx::new(Os::Windows);
        reg_value(&t, ADV, "HideFileExt", Some("0x1"));
        reg_value(&t, ADV, "Hidden", Some("0x1"));
        reg_value(
            &t,
            r"HKLM\SYSTEM\CurrentControlSet\Control\FileSystem",
            "LongPathsEnabled",
            None,
        );
        reg_value(
            &t,
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock",
            "AllowDevelopmentWithoutDevLicense",
            None,
        );
        t.runner.answer("reg", &["add"], Output::ok(""));
        t.runner.answer("reg", &["delete"], Output::ok(""));
        t
    }

    fn adds(t: &TestCtx) -> Vec<String> {
        t.runner
            .calls()
            .into_iter()
            .filter(|c| c.get(1).is_some_and(|a| a == "add"))
            .map(|c| c[4].clone())
            .collect()
    }

    #[test]
    fn list_shows_the_state_of_each_tweak() {
        let mut t = windows();
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        let out = t.output();
        assert!(
            out.lines()
                .any(|l| l.contains("file-extensions") && l.contains("off")),
            "{out}"
        );
        assert!(
            out.lines()
                .any(|l| l.contains("hidden-files") && l.contains("on")),
            "{out}"
        );
    }

    #[test]
    fn apply_sets_only_the_tweaks_that_are_off() {
        let mut t = windows();
        assert_eq!(odd(&mut t, &["apply"]), Exit::Done);
        assert_eq!(
            adds(&t),
            [
                "HideFileExt",
                "LongPathsEnabled",
                "AllowDevelopmentWithoutDevLicense"
            ]
        );
    }

    #[test]
    fn admin_tweak_without_admin_exits_3() {
        let mut t = windows();
        t.is_admin = false;
        assert_eq!(odd(&mut t, &["apply"]), Exit::NeedsAdmin);
        assert!(adds(&t).is_empty());
    }

    #[test]
    fn user_only_skips_admin_tweaks() {
        let mut t = windows();
        t.is_admin = false;
        assert_eq!(odd(&mut t, &["apply", "--user-only"]), Exit::Done);
        assert_eq!(adds(&t), ["HideFileExt"]);
    }

    #[test]
    fn only_selects_tweaks() {
        let mut t = windows();
        assert_eq!(odd(&mut t, &["apply", "--only", "long-paths"]), Exit::Done);
        assert_eq!(adds(&t), ["LongPathsEnabled"]);
    }

    #[test]
    fn an_unknown_tweak_exits_2() {
        let mut t = windows();
        assert_eq!(odd(&mut t, &["apply", "--only", "nope"]), Exit::Usage);
    }

    #[test]
    fn windows_undo_deletes_a_value_that_did_not_exist() {
        let mut t = windows();
        odd(&mut t, &["apply", "--only", "long-paths"]);
        odd(&mut t, &["--undo"]);
        let del = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c.get(1).is_some_and(|a| a == "delete"))
            .unwrap();
        assert_eq!(del[4], "LongPathsEnabled");
    }

    fn macos() -> TestCtx {
        let t = TestCtx::new(Os::MacOs);
        t.runner.answer(
            "defaults",
            &["read", "com.apple.finder", "AppleShowAllFiles"],
            Output::ok("0\n"),
        );
        t.runner.answer(
            "defaults",
            &["read", "NSGlobalDomain", "KeyRepeat"],
            Output::ok("6\n"),
        );
        t.runner.answer(
            "defaults",
            &["read", "NSGlobalDomain", "InitialKeyRepeat"],
            Output::fail(1, "does not exist"),
        );
        t.runner.answer("defaults", &["write"], Output::ok(""));
        t.runner.answer("defaults", &["delete"], Output::ok(""));
        t.runner.answer("killall", &[], Output::ok(""));
        t
    }

    #[test]
    fn macos_undo_puts_back_the_old_value() {
        let mut t = macos();
        odd(&mut t, &["apply"]);
        odd(&mut t, &["--undo"]);
        let calls = t.runner.calls();
        assert!(
            calls.contains(
                &[
                    "defaults",
                    "write",
                    "NSGlobalDomain",
                    "KeyRepeat",
                    "-int",
                    "6"
                ]
                .map(String::from)
                .to_vec()
            ),
            "{calls:?}"
        );
        assert!(
            calls.contains(
                &[
                    "defaults",
                    "write",
                    "com.apple.finder",
                    "AppleShowAllFiles",
                    "-bool",
                    "false"
                ]
                .map(String::from)
                .to_vec()
            ),
            "{calls:?}"
        );
        assert!(
            calls.contains(
                &["defaults", "delete", "NSGlobalDomain", "InitialKeyRepeat"]
                    .map(String::from)
                    .to_vec()
            ),
            "{calls:?}"
        );
    }

    #[test]
    fn linux_writes_the_sysctl_file() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("sysctl", &["-n"], Output::ok("8192\n"));
        t.runner.answer("sysctl", &["--system"], Output::ok(""));
        assert_eq!(odd(&mut t, &["apply"]), Exit::Done);
        let text = std::fs::read_to_string(t.sys("/etc/sysctl.d/90-odd.conf")).unwrap();
        assert!(
            text.contains("fs.inotify.max_user_watches = 524288"),
            "{text}"
        );
        odd(&mut t, &["--undo"]);
        assert!(!t.sys("/etc/sysctl.d/90-odd.conf").exists());
    }
}
