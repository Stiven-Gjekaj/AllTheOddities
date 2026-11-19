//! `odd path`: add a directory to the PATH of the user for good.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Arg, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "path";

/// `odd path`.
pub struct Path;

impl Oddity for Path {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Add a directory to the PATH of the user for good, and list the PATH."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("add")
                .about("Add a directory to the PATH of the user")
                .arg(
                    Arg::new("dir")
                        .required(true)
                        .help("The full path of the directory"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List each entry of PATH, and mark repeats and missing ones"),
        )
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        match m.subcommand() {
            Some(("add", a)) => {
                let dir = a
                    .get_one::<String>("dir")
                    .map(String::as_str)
                    .unwrap_or_default();
                add(dir, ctx)?;
                Ok(Exit::Done)
            }
            Some(("list", _)) => {
                list(ctx);
                Ok(Exit::Done)
            }
            _ => {
                ctx.say("Give a command: odd path add <dir>, or odd path list.");
                Ok(Exit::Usage)
            }
        }
    }
    fn undo_call(&self, action: &str, arg: &str, ctx: &mut Ctx<'_>) -> Result<()> {
        match action {
            "remove-entry" => windows_remove(arg, ctx),
            _ => bail!("odd path has no undo step named {action}"),
        }
    }
}

/// True when two PATH entries name the same directory: a trailing slash
/// does not count, and on Windows the letter case and the kind of slash do
/// not count.
pub fn same_entry(a: &str, b: &str, os: Os) -> bool {
    fn norm(s: &str, os: Os) -> String {
        let s = if os == Os::Windows {
            s.replace('/', "\\").to_lowercase()
        } else {
            s.to_owned()
        };
        let trimmed = s.trim_end_matches(['/', '\\']);
        if trimmed.is_empty() && !s.is_empty() {
            s[..1].to_owned()
        } else {
            trimmed.to_owned()
        }
    }
    norm(a, os) == norm(b, os)
}

fn separator(os: Os) -> char {
    if os == Os::Windows { ';' } else { ':' }
}

fn entries(ctx: &Ctx<'_>) -> Vec<String> {
    let path = ctx.env.get("PATH").map(String::as_str).unwrap_or("");
    path.split(separator(ctx.host.os))
        .filter(|e| !e.is_empty())
        .map(str::to_owned)
        .collect()
}

fn add(dir: &str, ctx: &mut Ctx<'_>) -> Result<()> {
    let os = ctx.host.os;
    let absolute = std::path::Path::new(dir).is_absolute()
        || (os == Os::Windows && dir.len() > 2 && dir.as_bytes()[1] == b':');
    if !absolute {
        bail!("give the full path of the directory, not {dir}");
    }
    if entries(ctx).iter().any(|e| same_entry(e, dir, os)) {
        ctx.say(&format!("{dir} is already on PATH. odd changes nothing."));
        return Ok(());
    }
    if os == Os::Windows {
        return windows_add(dir, ctx);
    }
    let shell = ctx.env.get("SHELL").cloned().unwrap_or_default();
    if shell.ends_with("fish") {
        return fish_add(dir, ctx);
    }
    for file in profiles(ctx, &shell) {
        add_to_profile(&file, dir, ctx)?;
    }
    ctx.say(&format!("{dir} is on PATH in each new shell."));
    Ok(())
}

/// The files that a login shell reads. bash reads ~/.bash_profile in place
/// of ~/.profile when it exists, and zsh reads ~/.zprofile.
fn profiles(ctx: &Ctx<'_>, shell: &str) -> Vec<PathBuf> {
    let mut files = vec![ctx.home.join(".profile")];
    if shell.ends_with("zsh") {
        files.push(ctx.home.join(".zprofile"));
    }
    let bash_profile = ctx.home.join(".bash_profile");
    if shell.ends_with("bash") && bash_profile.exists() {
        files.push(bash_profile);
    }
    files
}

fn add_to_profile(file: &std::path::Path, dir: &str, ctx: &mut Ctx<'_>) -> Result<()> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let mut lines: Vec<String> = crate::block::body(&text, NAME, "#")
        .map(|b| b.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    let line = format!(r#"export PATH="{dir}:$PATH""#);
    let has = |l: &String| {
        l.strip_prefix(r#"export PATH=""#)
            .and_then(|rest| rest.strip_suffix(r#":$PATH""#))
            .is_some_and(|d| same_entry(d, dir, Os::Linux))
    };
    if lines.iter().any(has) {
        return Ok(());
    }
    lines.push(line);
    let new = crate::block::upsert(&text, NAME, &lines.join("\n"), "#");
    // A file that odd makes goes away on undo. In a file of the person, undo
    // removes only the block.
    let step = if file.exists() {
        Undo::RemoveBlock {
            path: file.to_owned(),
            name: NAME.into(),
            comment: "#".into(),
        }
    } else {
        Undo::RestoreFile {
            path: file.to_owned(),
            before: None,
            after_sha256: state::sha256_hex(new.as_bytes()),
        }
    };
    state::record(ctx, NAME, step)?;
    ctx.write_file(file, &new)
}

/// A string in single quotes for fish, which takes `\'` and `\\` there.
fn fish_quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn fish_add(dir: &str, ctx: &mut Ctx<'_>) -> Result<()> {
    let q = fish_quote(dir);
    let undo = format!("set -U fish_user_paths (string match -v -- {q} $fish_user_paths)");
    state::record(
        ctx,
        NAME,
        Undo::Run {
            program: "fish".into(),
            args: vec!["-c".into(), undo],
        },
    )?;
    let out = ctx.change("fish", &["-c", &format!("fish_add_path -U -- {q}")])?;
    if !out.success() {
        bail!("fish_add_path failed: {}", out.stderr.trim());
    }
    ctx.say(&format!("{dir} is on PATH in each new fish shell."));
    Ok(())
}

const ENV_KEY: &str = r"HKCU\Environment";

/// The user Path of Windows, as the registry keeps it, with %VARIABLES%.
fn windows_read(ctx: &Ctx<'_>) -> Result<String> {
    let out = ctx.query("reg", &["query", ENV_KEY, "/v", "Path"])?;
    if !out.success() {
        return Ok(String::new());
    }
    Ok(out
        .stdout
        .lines()
        .find_map(|l| {
            let l = l.trim_start();
            let rest = l.strip_prefix("Path")?.trim_start();
            let rest = rest
                .strip_prefix("REG_EXPAND_SZ")
                .or_else(|| rest.strip_prefix("REG_SZ"))?;
            Some(rest.trim().to_owned())
        })
        .unwrap_or_default())
}

fn windows_write(value: &str, ctx: &mut Ctx<'_>) -> Result<()> {
    let out = ctx.change(
        "reg",
        &[
            "add",
            ENV_KEY,
            "/v",
            "Path",
            "/t",
            "REG_EXPAND_SZ",
            "/d",
            value,
            "/f",
        ],
    )?;
    if !out.success() {
        bail!("reg add failed: {}", out.stderr.trim());
    }
    if !ctx.dry_run {
        broadcast_change();
    }
    Ok(())
}

fn windows_add(dir: &str, ctx: &mut Ctx<'_>) -> Result<()> {
    let now = windows_read(ctx)?;
    if now.split(';').any(|e| same_entry(e, dir, Os::Windows)) {
        ctx.say(&format!(
            "{dir} is in the user Path already. Open a new window to see it."
        ));
        return Ok(());
    }
    state::record(
        ctx,
        NAME,
        Undo::Call {
            action: "remove-entry".into(),
            arg: dir.into(),
        },
    )?;
    let value = if now.is_empty() {
        dir.to_owned()
    } else {
        format!("{now};{dir}")
    };
    windows_write(&value, ctx)?;
    ctx.say(&format!(
        "{dir} is in the user Path. Open a new window to use it."
    ));
    Ok(())
}

fn windows_remove(dir: &str, ctx: &mut Ctx<'_>) -> Result<()> {
    let now = windows_read(ctx)?;
    let kept: Vec<&str> = now
        .split(';')
        .filter(|e| !same_entry(e, dir, Os::Windows))
        .collect();
    windows_write(&kept.join(";"), ctx)
}

/// Tells open programs, such as Explorer, that the environment changed.
#[cfg(windows)]
#[allow(unsafe_code)]
fn broadcast_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };
    let what: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result = 0;
    // SAFETY: `what` is a NUL-terminated UTF-16 string that lives for the
    // call, and `result` is a valid place for the answer.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            what.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

#[cfg(not(windows))]
fn broadcast_change() {}

fn list(ctx: &mut Ctx<'_>) {
    let os = ctx.host.os;
    let all = entries(ctx);
    for (i, entry) in all.iter().enumerate() {
        let mut marks = Vec::new();
        if !std::path::Path::new(entry).is_dir() {
            marks.push("missing");
        }
        if all[..i].iter().any(|e| same_entry(e, entry, os)) {
            marks.push("repeat");
        }
        if marks.is_empty() {
            ctx.say(entry);
        } else {
            ctx.say(&format!("{entry}  ({})", marks.join(", ")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exit::Exit;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd"].iter().chain(args).map(Into::into).collect();
        dispatch(argv, &mut t.ctx(), &[&Path])
    }

    fn unix(shell: &str) -> TestCtx {
        let mut t = TestCtx::new(Os::Linux);
        t.env.insert("PATH".into(), "/usr/bin:/bin".into());
        t.env.insert("SHELL".into(), shell.into());
        t
    }

    #[test]
    fn same_entry_ignores_a_trailing_slash() {
        assert!(same_entry("/a/b/", "/a/b", Os::Linux));
        assert!(!same_entry("/", "", Os::Linux));
    }

    #[test]
    fn same_entry_ignores_case_on_windows_only() {
        assert!(same_entry(r"C:\Tools", r"c:\tools\", Os::Windows));
        assert!(same_entry(r"C:\Tools", "C:/Tools", Os::Windows));
        assert!(!same_entry("/Tools", "/tools", Os::Linux));
    }

    #[test]
    fn same_entry_keeps_spaces() {
        assert!(!same_entry("/a b", "/a  b", Os::Linux));
        assert!(same_entry("/a b/", "/a b", Os::Linux));
    }

    #[test]
    fn add_writes_the_profile() {
        let mut t = unix("/bin/bash");
        assert_eq!(odd(&mut t, &["path", "add", "/opt/x"]), Exit::Done);
        let text = std::fs::read_to_string(t.home().join(".profile")).unwrap();
        assert!(text.contains(r#"export PATH="/opt/x:$PATH""#), "{text}");
    }

    #[test]
    fn add_twice_writes_one_line() {
        let mut t = unix("/bin/zsh");
        odd(&mut t, &["path", "add", "/opt/x"]);
        odd(&mut t, &["path", "add", "/opt/x/"]);
        let text = std::fs::read_to_string(t.home().join(".zprofile")).unwrap();
        assert_eq!(text.matches("/opt/x").count(), 1, "{text}");
    }

    #[test]
    fn a_second_dir_joins_the_same_block() {
        let mut t = unix("/bin/bash");
        odd(&mut t, &["path", "add", "/opt/x"]);
        odd(&mut t, &["path", "add", "/opt/y"]);
        let text = std::fs::read_to_string(t.home().join(".profile")).unwrap();
        assert_eq!(text.matches(">>> odd path >>>").count(), 1);
        assert!(text.contains("/opt/x") && text.contains("/opt/y"), "{text}");
    }

    #[test]
    fn add_of_a_dir_already_on_path_changes_nothing() {
        let mut t = unix("/bin/bash");
        t.env.insert("PATH".into(), "/usr/bin:/opt/x/".into());
        assert_eq!(odd(&mut t, &["path", "add", "/opt/x"]), Exit::Done);
        assert!(!t.home().join(".profile").exists());
        assert!(t.output().contains("already"), "{}", t.output());
    }

    #[test]
    fn a_relative_dir_is_refused() {
        let mut t = unix("/bin/bash");
        assert_eq!(odd(&mut t, &["path", "add", "opt/x"]), Exit::Failed);
    }

    #[test]
    fn bash_with_a_bash_profile_writes_there_too() {
        let mut t = unix("/bin/bash");
        std::fs::create_dir_all(t.home()).unwrap();
        std::fs::write(t.home().join(".bash_profile"), "# mine\n").unwrap();
        odd(&mut t, &["path", "add", "/opt/x"]);
        let text = std::fs::read_to_string(t.home().join(".bash_profile")).unwrap();
        assert!(
            text.starts_with("# mine\n") && text.contains("/opt/x"),
            "{text}"
        );
    }

    #[test]
    fn undo_removes_the_block() {
        let mut t = unix("/bin/zsh");
        std::fs::create_dir_all(t.home()).unwrap();
        std::fs::write(t.home().join(".zprofile"), "# mine\n").unwrap();
        odd(&mut t, &["path", "add", "/opt/x"]);
        assert_eq!(odd(&mut t, &["path", "--undo"]), Exit::Done);
        assert_eq!(
            std::fs::read_to_string(t.home().join(".zprofile")).unwrap(),
            "# mine\n"
        );
        assert!(!t.home().join(".profile").exists());
    }

    #[test]
    fn fish_uses_fish_add_path_and_records_its_undo() {
        let mut t = unix("/usr/bin/fish");
        t.runner.answer("fish", &[], Output::ok(""));
        odd(&mut t, &["path", "add", "/opt/x"]);
        let calls = t.runner.calls();
        assert!(
            calls[0].iter().any(|a| a.contains("fish_add_path")),
            "{calls:?}"
        );
        assert!(calls[0][2].contains("'/opt/x'"), "{calls:?}");
        let steps = crate::state::steps(&t.ctx(), "path").unwrap();
        assert!(
            steps
                .iter()
                .any(|s| matches!(s, crate::state::Undo::Run { program, .. } if program == "fish"))
        );
    }

    const REG_OUT: &str = "\r\nHKEY_CURRENT_USER\\Environment\r\n    Path    REG_EXPAND_SZ    %USERPROFILE%\\bin;C:\\Tools\r\n\r\n";

    fn windows() -> TestCtx {
        let mut t = TestCtx::new(Os::Windows);
        t.env.insert("PATH".into(), r"C:\Windows;C:\Tools".into());
        t.runner.answer("reg", &["query"], Output::ok(REG_OUT));
        t.runner.answer("reg", &["add"], Output::ok(""));
        t
    }

    #[test]
    fn windows_appends_to_the_user_path_and_keeps_variables() {
        let mut t = windows();
        assert_eq!(odd(&mut t, &["path", "add", r"C:\Odd"]), Exit::Done);
        let add = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[1] == "add")
            .unwrap();
        let value = &add[add.iter().position(|a| a == "/d").unwrap() + 1];
        assert_eq!(value, r"%USERPROFILE%\bin;C:\Tools;C:\Odd");
        assert!(add.contains(&"REG_EXPAND_SZ".to_owned()));
    }

    #[test]
    fn windows_undo_removes_only_that_entry() {
        let mut t = windows();
        odd(&mut t, &["path", "add", r"C:\Odd"]);
        let mut u = windows();
        u.runner.answer(
            "reg",
            &["query"],
            Output::ok(&REG_OUT.replace(r"C:\Tools", r"C:\Tools;c:\odd\")),
        );
        // The state of the first run moves to the second test context.
        std::fs::create_dir_all(u.state_dir()).unwrap();
        std::fs::copy(
            t.state_dir().join("path.json"),
            u.state_dir().join("path.json"),
        )
        .unwrap();
        assert_eq!(odd(&mut u, &["path", "--undo"]), Exit::Done);
        let add = u
            .runner
            .calls()
            .into_iter()
            .find(|c| c[1] == "add")
            .unwrap();
        let value = &add[add.iter().position(|a| a == "/d").unwrap() + 1];
        assert_eq!(value, r"%USERPROFILE%\bin;C:\Tools");
    }

    #[test]
    fn list_marks_repeats_and_missing_dirs() {
        let mut t = unix("/bin/bash");
        let real = t.dir.path().to_string_lossy().into_owned();
        t.env
            .insert("PATH".into(), format!("{real}:/no/such/dir:{real}/"));
        assert_eq!(odd(&mut t, &["path", "list"]), Exit::Done);
        let out = t.output();
        assert!(
            out.lines()
                .any(|l| l.contains("/no/such/dir") && l.contains("missing")),
            "{out}"
        );
        assert!(out.lines().any(|l| l.contains("repeat")), "{out}");
    }
}
