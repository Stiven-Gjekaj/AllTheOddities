//! `odd hosts`: add and remove entries of the hosts file, in a block of odd.

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

const NAME: &str = "hosts";
const UNIX_FILE: &str = "/etc/hosts";
const WINDOWS_FILE: &str = r"C:\Windows\System32\drivers\etc\hosts";

/// `odd hosts`.
pub struct Hosts;

impl Oddity for Hosts {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Add and remove entries of the hosts file, in a block that odd manages."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("add")
                .about("Make one or more names point to an address")
                .arg(Arg::new("ip").required(true))
                .arg(Arg::new("names").required(true).action(ArgAction::Append)),
        )
        .subcommand(
            Command::new("remove")
                .about("Remove a name that odd added")
                .arg(Arg::new("name").required(true)),
        )
        .subcommand(Command::new("list").about("List the entries that odd added"))
    }
    fn needs_admin(&self, m: &ArgMatches, _ctx: &Ctx<'_>) -> bool {
        matches!(m.subcommand_name(), Some("add" | "remove"))
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let file = ctx.sys_path(if ctx.host.os == Os::Windows {
            WINDOWS_FILE
        } else {
            UNIX_FILE
        });
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let mut lines: Vec<(String, Vec<String>)> = crate::block::body(&text, NAME, "#")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let mut words = l.split_whitespace();
                let ip = words.next()?.to_owned();
                Some((ip, words.map(str::to_owned).collect()))
            })
            .collect();
        match m.subcommand() {
            Some(("add", a)) => {
                let ip = a.get_one::<String>("ip").cloned().unwrap_or_default();
                let names: Vec<String> = a
                    .get_many::<String>("names")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                if ip.parse::<std::net::IpAddr>().is_err() {
                    ctx.say(&format!("{ip} is not an IP address."));
                    return Ok(Exit::Usage);
                }
                if let Some(bad) = names.iter().find(|n| !valid_host(n)) {
                    ctx.say(&format!("{bad} is not a host name."));
                    return Ok(Exit::Usage);
                }
                for (_, ns) in &mut lines {
                    ns.retain(|n| !names.contains(n));
                }
                lines.retain(|(_, ns)| !ns.is_empty());
                lines.push((ip.clone(), names.clone()));
                save(ctx, &file, &lines)?;
                ctx.say(&format!("{} point(s) to {ip}.", names.join(", ")));
            }
            Some(("remove", a)) => {
                let name = a.get_one::<String>("name").cloned().unwrap_or_default();
                if !lines.iter().any(|(_, ns)| ns.contains(&name)) {
                    ctx.say(&format!("odd did not add {name}."));
                    return Ok(Exit::Done);
                }
                for (_, ns) in &mut lines {
                    ns.retain(|n| *n != name);
                }
                lines.retain(|(_, ns)| !ns.is_empty());
                save(ctx, &file, &lines)?;
                ctx.say(&format!("{name} is gone from the hosts file."));
            }
            _ => {
                if lines.is_empty() {
                    ctx.say("odd hosts has added no entry.");
                }
                for (ip, ns) in &lines {
                    ctx.say(&format!("{ip} {}", ns.join(" ")));
                }
            }
        }
        Ok(Exit::Done)
    }
}

/// Labels of letters, digits and hyphens, with dots between them.
pub fn valid_host(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

fn save(ctx: &mut Ctx<'_>, file: &std::path::Path, lines: &[(String, Vec<String>)]) -> Result<()> {
    let body: Vec<String> = lines
        .iter()
        .map(|(ip, ns)| format!("{ip} {}", ns.join(" ")))
        .collect();
    crate::state::upsert_block(ctx, NAME, file, NAME, &body.join("\n"), &|_: &str| false)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "hosts"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&Hosts])
    }

    fn host(os: Os) -> (TestCtx, std::path::PathBuf) {
        let t = TestCtx::new(os);
        let path = t.sys(if os == Os::Windows {
            WINDOWS_FILE
        } else {
            UNIX_FILE
        });
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "127.0.0.1 localhost\n").unwrap();
        (t, path)
    }

    fn block(path: &std::path::Path) -> String {
        crate::block::body(&std::fs::read_to_string(path).unwrap(), NAME, "#").unwrap_or_default()
    }

    #[test]
    fn add_writes_the_block() {
        let (mut t, path) = host(Os::Linux);
        assert_eq!(
            odd(&mut t, &["add", "10.0.0.5", "nas", "nas.lan"]),
            Exit::Done
        );
        assert_eq!(block(&path), "10.0.0.5 nas nas.lan");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("127.0.0.1 localhost\n")
        );
    }

    #[test]
    fn windows_uses_its_hosts_file() {
        let (mut t, path) = host(Os::Windows);
        odd(&mut t, &["add", "10.0.0.5", "nas"]);
        assert_eq!(block(&path), "10.0.0.5 nas");
    }

    #[test]
    fn add_of_a_name_again_replaces_its_line() {
        let (mut t, path) = host(Os::Linux);
        odd(&mut t, &["add", "10.0.0.5", "nas"]);
        odd(&mut t, &["add", "10.0.0.6", "nas"]);
        assert_eq!(block(&path), "10.0.0.6 nas");
    }

    #[test]
    fn remove_takes_the_name_out() {
        let (mut t, path) = host(Os::Linux);
        odd(&mut t, &["add", "10.0.0.5", "nas", "files"]);
        odd(&mut t, &["remove", "nas"]);
        assert_eq!(block(&path), "10.0.0.5 files");
        odd(&mut t, &["remove", "files"]);
        assert_eq!(block(&path), "");
    }

    #[test]
    fn a_bad_ip_exits_2() {
        let (mut t, path) = host(Os::Linux);
        assert_eq!(odd(&mut t, &["add", "10.0.0.999", "nas"]), Exit::Usage);
        assert_eq!(odd(&mut t, &["add", "10.0.0.5", "bad name"]), Exit::Usage);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "127.0.0.1 localhost\n"
        );
    }

    #[test]
    fn ipv6_works() {
        let (mut t, path) = host(Os::Linux);
        assert_eq!(odd(&mut t, &["add", "fd00::5", "nas"]), Exit::Done);
        assert_eq!(block(&path), "fd00::5 nas");
    }

    #[test]
    fn undo_keeps_the_lines_of_the_person() {
        let (mut t, path) = host(Os::Linux);
        odd(&mut t, &["add", "10.0.0.5", "nas"]);
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("192.168.1.9 printer\n");
        std::fs::write(&path, &text).unwrap();
        assert_eq!(odd(&mut t, &["--undo"]), Exit::Done);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            after.contains("192.168.1.9 printer") && !after.contains("nas"),
            "{after}"
        );
    }

    #[test]
    fn list_needs_no_admin() {
        let (mut t, _) = host(Os::Linux);
        odd(&mut t, &["add", "10.0.0.5", "nas"]);
        t.is_admin = false;
        t.out.clear();
        assert_eq!(odd(&mut t, &["list"]), Exit::Done);
        assert!(t.output().contains("10.0.0.5 nas"));
    }
}
