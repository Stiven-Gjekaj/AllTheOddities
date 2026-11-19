//! `odd apps`: install a list of apps with the package manager of the host.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{ArgMatches, Command};
use serde::Deserialize;

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

/// The package of an app for each manager. None: that manager has no package.
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Ids {
    brew: Option<String>,
    cask: Option<String>,
    winget: Option<String>,
    apt: Option<String>,
    dnf: Option<String>,
    pacman: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct AppsFile {
    #[serde(default)]
    apps: Vec<String>,
    #[serde(default)]
    custom: HashMap<String, Ids>,
}

/// name, brew, cask, winget, apt, dnf, pacman. An empty text: no package.
const CATALOG: &[[&str; 7]] = &[
    ["git", "git", "", "Git.Git", "git", "git", "git"],
    ["gh", "gh", "", "GitHub.cli", "gh", "gh", "github-cli"],
    [
        "vscode",
        "",
        "visual-studio-code",
        "Microsoft.VisualStudioCode",
        "",
        "",
        "code",
    ],
    [
        "firefox",
        "",
        "firefox",
        "Mozilla.Firefox",
        "firefox",
        "firefox",
        "firefox",
    ],
    ["chrome", "", "google-chrome", "Google.Chrome", "", "", ""],
    [
        "nodejs",
        "node",
        "",
        "OpenJS.NodeJS.LTS",
        "nodejs",
        "nodejs",
        "nodejs",
    ],
    [
        "python",
        "python",
        "",
        "Python.Python.3.13",
        "python3",
        "python3",
        "python",
    ],
    [
        "rustup",
        "rustup",
        "",
        "Rustlang.Rustup",
        "",
        "rustup",
        "rustup",
    ],
    [
        "ripgrep",
        "ripgrep",
        "",
        "BurntSushi.ripgrep.MSVC",
        "ripgrep",
        "ripgrep",
        "ripgrep",
    ],
    ["fd", "fd", "", "sharkdp.fd", "fd-find", "fd-find", "fd"],
    ["jq", "jq", "", "jqlang.jq", "jq", "jq", "jq"],
    [
        "neovim",
        "neovim",
        "",
        "Neovim.Neovim",
        "neovim",
        "neovim",
        "neovim",
    ],
    ["tmux", "tmux", "", "", "tmux", "tmux", "tmux"],
    ["vlc", "", "vlc", "VideoLAN.VLC", "vlc", "", "vlc"],
    ["obsidian", "", "obsidian", "Obsidian.Obsidian", "", "", ""],
];

fn catalog(name: &str) -> Option<Ids> {
    let row = CATALOG.iter().find(|r| r[0] == name)?;
    let o = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    Some(Ids {
        brew: o(row[1]),
        cask: o(row[2]),
        winget: o(row[3]),
        apt: o(row[4]),
        dnf: o(row[5]),
        pacman: o(row[6]),
    })
}

/// `odd apps`.
pub struct Apps;

impl Oddity for Apps {
    fn name(&self) -> &'static str {
        "apps"
    }
    fn summary(&self) -> &'static str {
        "Install the apps of one list with winget, Homebrew, apt, dnf or pacman."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("install").about("Install each app of the list that is not installed"),
        )
        .subcommand(Command::new("list").about("Show the list and the path of its file"))
        .subcommand(Command::new("catalog").about("List the apps that odd knows by name"))
        .subcommand(Command::new("init").about("Write a first list, if there is none"))
    }
    fn can_undo(&self) -> bool {
        false
    }
    fn needs_admin(&self, m: &ArgMatches, ctx: &Ctx<'_>) -> bool {
        ctx.host.os == Os::Linux && m.subcommand_name() == Some("install")
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let path = file_path(ctx);
        match m.subcommand() {
            Some(("init", _)) => {
                if path.exists() {
                    ctx.say(&format!("{} exists already. odd keeps it.", path.display()));
                } else {
                    ctx.write_file(&path, "# The apps that odd apps install installs. odd apps catalog lists the names.\napps = [\"git\", \"gh\"]\n\n# An app that the catalog does not know:\n# [custom.myapp]\n# brew = \"myapp\"\n# winget = \"Vendor.MyApp\"\n")?;
                    ctx.say(&format!(
                        "Edit {}, then run odd apps install.",
                        path.display()
                    ));
                }
                Ok(Exit::Done)
            }
            Some(("catalog", _)) => {
                for row in CATALOG {
                    ctx.say(row[0]);
                }
                Ok(Exit::Done)
            }
            Some(("install", _)) => install(ctx, &path),
            _ => {
                let file = read(&path)?;
                ctx.say(&format!("{}:", path.display()));
                for a in &file.apps {
                    ctx.say(&format!("  {a}"));
                }
                Ok(Exit::Done)
            }
        }
    }
}

fn file_path(ctx: &Ctx<'_>) -> PathBuf {
    match ctx.host.os {
        Os::Windows => ctx
            .env
            .get("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.home.join("AppData").join("Roaming"))
            .join("odd")
            .join("apps.toml"),
        _ => ctx.home.join(".config").join("odd").join("apps.toml"),
    }
}

fn read(path: &std::path::Path) -> Result<AppsFile> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("there is no {}. Run odd apps init first", path.display()))?;
    toml::from_str(&text).with_context(|| format!("{} is not a correct list", path.display()))
}

#[derive(Clone, Copy, PartialEq)]
enum Manager {
    Brew,
    Apt,
    Dnf,
    Pacman,
    Winget,
}

fn manager(ctx: &Ctx<'_>) -> Result<Manager> {
    Ok(match ctx.host.os {
        Os::MacOs => Manager::Brew,
        Os::Windows => Manager::Winget,
        Os::Linux => match ctx.host.distro.as_deref().unwrap_or("") {
            "debian" | "ubuntu" | "linuxmint" | "pop" | "raspbian" | "kali" | "elementary"
            | "zorin" | "neon" => Manager::Apt,
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "ol" | "amzn" => Manager::Dnf,
            "arch" | "manjaro" | "endeavouros" | "garuda" | "cachyos" => Manager::Pacman,
            d => bail!("odd apps does not know the package manager of {d}"),
        },
    })
}

/// The path of brew, or None. Homebrew puts it on PATH in a new shell only.
fn brew(ctx: &Ctx<'_>) -> Option<String> {
    if ctx.runner.on_path("brew") {
        return Some("brew".into());
    }
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .iter()
        .find(|p| ctx.sys_path(p).exists())
        .map(|p| ctx.sys_path(p).to_string_lossy().into_owned())
}

/// The command that installs, and the command that checks, an app. None
/// when this manager has no package of it.
fn commands(m: Manager, brew: &str, ids: &Ids) -> Option<(Vec<String>, Vec<String>)> {
    let v = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    Some(match m {
        Manager::Brew => {
            if let Some(f) = &ids.brew {
                (
                    v(&[brew, "install", f]),
                    v(&[brew, "list", "--versions", f]),
                )
            } else {
                let c = ids.cask.as_ref()?;
                (
                    v(&[brew, "install", "--cask", c]),
                    v(&[brew, "list", "--cask", "--versions", c]),
                )
            }
        }
        Manager::Apt => {
            let p = ids.apt.as_ref()?;
            (v(&["apt-get", "install", "-y", p]), v(&["dpkg", "-s", p]))
        }
        Manager::Dnf => {
            let p = ids.dnf.as_ref()?;
            (v(&["dnf", "install", "-y", p]), v(&["rpm", "-q", p]))
        }
        Manager::Pacman => {
            let p = ids.pacman.as_ref()?;
            (
                v(&["pacman", "-S", "--noconfirm", "--needed", p]),
                v(&["pacman", "-Q", p]),
            )
        }
        Manager::Winget => {
            let p = ids.winget.as_ref()?;
            (
                v(&[
                    "winget",
                    "install",
                    "--id",
                    p,
                    "-e",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ]),
                v(&["winget", "list", "--id", p, "-e"]),
            )
        }
    })
}

fn install(ctx: &mut Ctx<'_>, path: &std::path::Path) -> Result<Exit> {
    let file = read(path)?;
    let m = manager(ctx)?;
    let mut brew_path = String::from("brew");
    if m == Manager::Brew {
        brew_path = match brew(ctx) {
            Some(b) => b,
            None => {
                if !ctx.ask.yes_no(
                    "Homebrew is not installed. Install it now with its official script?",
                    true,
                ) {
                    bail!("odd apps needs Homebrew on macOS");
                }
                let script = "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"";
                if ctx.interactive("/bin/bash", &["-c", script])? != 0 {
                    bail!("the install of Homebrew failed");
                }
                brew(ctx).unwrap_or_else(|| "/opt/homebrew/bin/brew".into())
            }
        };
    }
    let mut failed = Vec::new();
    let mut updated = false;
    for name in &file.apps {
        let Some(ids) = file.custom.get(name).cloned().or_else(|| catalog(name)) else {
            ctx.say(&format!(
                "odd does not know {name}. Add [custom.{name}] to {}.",
                path.display()
            ));
            continue;
        };
        let Some((install, check)) = commands(m, &brew_path, &ids) else {
            ctx.say(&format!(
                "{name} has no package for this computer. odd skips it."
            ));
            continue;
        };
        let check_refs: Vec<&str> = check.iter().skip(1).map(String::as_str).collect();
        if ctx.query(&check[0], &check_refs).is_ok_and(|o| o.success()) {
            ctx.say(&format!("{name} is installed already."));
            continue;
        }
        if m == Manager::Apt && !updated {
            ctx.change("apt-get", &["update"])?;
            updated = true;
        }
        ctx.say(&format!("Installing {name}"));
        let refs: Vec<&str> = install.iter().skip(1).map(String::as_str).collect();
        let status = ctx.interactive(&install[0], &refs)?;
        if status != 0 {
            ctx.say(&format!("{name} failed with exit code {status}."));
            failed.push(name.clone());
        }
    }
    if failed.is_empty() {
        Ok(Exit::Done)
    } else {
        ctx.say(&format!("These apps failed: {}.", failed.join(", ")));
        Ok(Exit::Failed)
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
        let argv = ["odd", "apps"].iter().chain(args).map(Into::into).collect();
        dispatch(argv, &mut t.ctx(), &[&Apps])
    }

    fn file(t: &TestCtx, text: &str) {
        let p = t.home().join(".config/odd/apps.toml");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn installs(t: &TestCtx) -> Vec<Vec<String>> {
        t.runner
            .calls()
            .into_iter()
            .filter(|c| c.iter().any(|a| a == "install"))
            .collect()
    }

    fn mac() -> TestCtx {
        let t = TestCtx::new(Os::MacOs);
        t.runner.with_path(&["brew"]);
        t.runner
            .answer("brew", &["list"], Output::fail(1, "not installed"));
        t.runner.answer("brew", &["install"], Output::ok(""));
        t
    }

    #[test]
    fn install_uses_the_manager_of_the_host() {
        let mut t = mac();
        file(&t, r#"apps = ["git", "jq"]"#);
        assert_eq!(odd(&mut t, &["install"]), Exit::Done);
        assert_eq!(
            installs(&t),
            vec![
                vec!["brew", "install", "git"],
                vec!["brew", "install", "jq"]
            ]
        );

        let mut w = TestCtx::new(Os::Windows);
        w.runner
            .answer("winget", &["list"], Output::fail(1, "No installed package"));
        w.runner.answer("winget", &["install"], Output::ok(""));
        let p = w.dir.path().join("roaming/odd/apps.toml");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, r#"apps = ["git"]"#).unwrap();
        w.env.insert(
            "APPDATA".into(),
            w.dir.path().join("roaming").to_string_lossy().into_owned(),
        );
        odd(&mut w, &["install"]);
        assert_eq!(
            installs(&w)[0][..4],
            ["winget", "install", "--id", "Git.Git"]
        );
    }

    #[test]
    fn cask_apps_use_cask() {
        let mut t = mac();
        file(&t, r#"apps = ["vscode"]"#);
        odd(&mut t, &["install"]);
        assert_eq!(
            installs(&t),
            vec![vec!["brew", "install", "--cask", "visual-studio-code"]]
        );
    }

    #[test]
    fn custom_wins_over_the_catalog() {
        let mut t = mac();
        file(
            &t,
            "apps = [\"git\", \"mytool\"]\n[custom.git]\nbrew = \"git-gui\"\n[custom.mytool]\nbrew = \"me/tap/mytool\"\n",
        );
        odd(&mut t, &["install"]);
        assert_eq!(
            installs(&t),
            vec![
                vec!["brew", "install", "git-gui"],
                vec!["brew", "install", "me/tap/mytool"]
            ]
        );
    }

    #[test]
    fn an_unknown_app_is_named_and_skipped() {
        let mut t = mac();
        file(&t, r#"apps = ["nope", "jq"]"#);
        assert_eq!(odd(&mut t, &["install"]), Exit::Done);
        assert!(t.output().contains("nope"), "{}", t.output());
        assert_eq!(installs(&t), vec![vec!["brew", "install", "jq"]]);
    }

    #[test]
    fn an_installed_app_is_skipped() {
        let mut t = mac();
        t.runner.answer(
            "brew",
            &["list", "--versions", "jq"],
            Output::ok("jq 1.7\n"),
        );
        file(&t, r#"apps = ["jq", "git"]"#);
        odd(&mut t, &["install"]);
        assert_eq!(installs(&t), vec![vec!["brew", "install", "git"]]);
    }

    #[test]
    fn linux_needs_admin() {
        let mut t = TestCtx::new(Os::Linux);
        t.is_admin = false;
        t.distro = Some("ubuntu".into());
        file(&t, r#"apps = ["git"]"#);
        assert_eq!(odd(&mut t, &["install"]), Exit::NeedsAdmin);
    }

    #[test]
    fn linux_updates_once_then_installs() {
        let mut t = TestCtx::new(Os::Linux);
        t.distro = Some("debian".into());
        t.runner
            .answer("dpkg", &[], Output::fail(1, "not installed"));
        t.runner.answer("apt-get", &[], Output::ok(""));
        file(&t, r#"apps = ["git", "fd"]"#);
        odd(&mut t, &["install"]);
        let apt: Vec<Vec<String>> = t
            .runner
            .calls()
            .into_iter()
            .filter(|c| c[0] == "apt-get")
            .collect();
        assert_eq!(
            apt,
            vec![
                vec!["apt-get", "update"],
                vec!["apt-get", "install", "-y", "git"],
                vec!["apt-get", "install", "-y", "fd-find"]
            ]
        );
    }

    #[test]
    fn init_writes_a_file_once() {
        let mut t = mac();
        assert_eq!(odd(&mut t, &["init"]), Exit::Done);
        let p = t.home().join(".config/odd/apps.toml");
        assert!(std::fs::read_to_string(&p).unwrap().contains("apps = ["));
        std::fs::write(&p, "apps = []\n").unwrap();
        odd(&mut t, &["init"]);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "apps = []\n");
    }

    #[test]
    fn no_file_says_to_run_init() {
        let mut t = mac();
        assert_eq!(odd(&mut t, &["install"]), Exit::Failed);
        assert!(t.output().contains("odd apps init"), "{}", t.output());
    }
}
