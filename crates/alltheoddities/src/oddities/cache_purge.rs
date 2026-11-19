//! `odd cache-purge`: list the caches of developer tools, and delete the
//! ones that the person selects.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

/// `odd cache-purge`.
pub struct CachePurge;

impl Oddity for CachePurge {
    fn name(&self) -> &'static str {
        "cache-purge"
    }
    fn summary(&self) -> &'static str {
        "List the caches of developer tools with their size, and delete the ones that you select."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("list")
                .long("list")
                .action(ArgAction::SetTrue)
                .help("Only list the caches"),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .action(ArgAction::SetTrue)
                .help("Delete each cache, with no question"),
        )
        .arg(
            Arg::new("only")
                .long("only")
                .action(ArgAction::Append)
                .help("Delete this cache. Give it again for more"),
        )
    }
    fn can_undo(&self) -> bool {
        false
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let caches = existing(ctx);
        if caches.is_empty() {
            ctx.say("odd found no cache of a developer tool.");
            return Ok(Exit::Done);
        }
        let only: Vec<String> = m
            .get_many::<String>("only")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let mut freed = 0;
        for (name, path) in &caches {
            let (bytes, files) = size(path);
            let line = format!(
                "{name:9} {:>9}  {files:>7} files  {}",
                human(bytes),
                path.display()
            );
            if m.get_flag("list") {
                ctx.say(&line);
                continue;
            }
            let take = if m.get_flag("all") {
                true
            } else if !only.is_empty() {
                only.contains(&(*name).to_owned())
            } else {
                ctx.ask.yes_no(&format!("Delete {line}?"), false)
            };
            if take {
                ctx.remove_dir(path)?;
                ctx.say(&format!("Deleted {name}: {}", human(bytes)));
                freed += bytes;
            }
        }
        if !m.get_flag("list") {
            ctx.say(&format!(
                "Freed {}. Each tool fills its cache again when it needs it.",
                human(freed)
            ));
        }
        Ok(Exit::Done)
    }
}

/// The caches of this host, by name, and the ones that exist.
fn existing(ctx: &Ctx<'_>) -> Vec<(&'static str, PathBuf)> {
    let h = |rel: &str| ctx.home.join(rel);
    let local = ctx
        .env
        .get("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.home.join("AppData").join("Local"));
    let l = |rel: &str| local.join(rel);
    let list: Vec<(&'static str, PathBuf)> = match ctx.host.os {
        Os::MacOs => vec![
            ("npm", h(".npm/_cacache")),
            ("pnpm", h("Library/pnpm/store")),
            ("yarn", h("Library/Caches/Yarn")),
            ("pip", h("Library/Caches/pip")),
            ("cargo", h(".cargo/registry/cache")),
            ("go", h("Library/Caches/go-build")),
            ("gradle", h(".gradle/caches")),
            ("xcode", h("Library/Developer/Xcode/DerivedData")),
            ("homebrew", h("Library/Caches/Homebrew")),
        ],
        Os::Linux => vec![
            ("npm", h(".npm/_cacache")),
            ("pnpm", h(".local/share/pnpm/store")),
            ("yarn", h(".cache/yarn")),
            ("pip", h(".cache/pip")),
            ("cargo", h(".cargo/registry/cache")),
            ("go", h(".cache/go-build")),
            ("gradle", h(".gradle/caches")),
        ],
        Os::Windows => vec![
            ("npm", l("npm-cache/_cacache")),
            ("pnpm", l("pnpm/store")),
            ("yarn", l("Yarn/Cache")),
            ("pip", l("pip/Cache")),
            ("cargo", h(".cargo/registry/cache")),
            ("go", l("go-build")),
            ("gradle", h(".gradle/caches")),
        ],
    };
    list.into_iter().filter(|(_, p)| p.is_dir()).collect()
}

/// The bytes and the number of files under a folder. A link counts as
/// nothing, and `odd` does not follow it.
pub fn size(dir: &Path) -> (u64, u64) {
    let mut total = (0, 0);
    let Ok(rd) = std::fs::read_dir(dir) else {
        return total;
    };
    for e in rd.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        if meta.is_dir() {
            let (b, f) = size(&e.path());
            total.0 += b;
            total.1 += f;
        } else if meta.is_file() {
            total.0 += meta.len();
            total.1 += 1;
        }
    }
    total
}

/// A size for a person, in steps of 1024.
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64 / 1024.0;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    format!("{v:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "cache-purge"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&CachePurge])
    }

    fn fill(t: &TestCtx, rel: &str, bytes: usize) -> std::path::PathBuf {
        let dir = t.home().join(rel);
        std::fs::create_dir_all(dir.join("x")).unwrap();
        std::fs::write(dir.join("x/blob"), vec![1u8; bytes]).unwrap();
        dir
    }

    #[test]
    fn lists_only_caches_that_exist() {
        let mut t = TestCtx::new(Os::Linux);
        fill(&t, ".npm/_cacache", 10);
        fill(&t, ".cache/pip", 20);
        assert_eq!(odd(&mut t, &["--list"]), Exit::Done);
        let out = t.output();
        assert!(out.contains("npm") && out.contains("pip"), "{out}");
        assert!(!out.contains("gradle"), "{out}");
    }

    #[cfg(unix)]
    #[test]
    fn size_does_not_follow_links() {
        let t = TestCtx::new(Os::Linux);
        let dir = fill(&t, ".cache/pip", 100);
        let big = t.dir.path().join("big");
        std::fs::create_dir_all(&big).unwrap();
        std::fs::write(big.join("f"), vec![0u8; 5000]).unwrap();
        std::os::unix::fs::symlink(&big, dir.join("link")).unwrap();
        assert_eq!(size(&dir), (100, 1));
    }

    #[test]
    fn all_deletes_each_cache() {
        let mut t = TestCtx::new(Os::Linux);
        let a = fill(&t, ".npm/_cacache", 10);
        let b = fill(&t, ".gradle/caches", 10);
        assert_eq!(odd(&mut t, &["--all"]), Exit::Done);
        assert!(!a.exists() && !b.exists());
        assert!(t.output().contains("Freed"), "{}", t.output());
    }

    #[test]
    fn only_selects() {
        let mut t = TestCtx::new(Os::Linux);
        let a = fill(&t, ".npm/_cacache", 10);
        let b = fill(&t, ".gradle/caches", 10);
        odd(&mut t, &["--only", "gradle"]);
        assert!(a.exists() && !b.exists());
    }

    #[test]
    fn a_no_keeps_the_cache() {
        let mut t = TestCtx::new(Os::Linux);
        let a = fill(&t, ".npm/_cacache", 10);
        let b = fill(&t, ".cache/pip", 10);
        t.ask.push("n");
        t.ask.push("y");
        odd(&mut t, &[]);
        assert!(a.exists() && !b.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_a_cache_keeps_its_target() {
        let mut t = TestCtx::new(Os::Linux);
        let dir = fill(&t, ".cache/pip", 10);
        let keep = t.dir.path().join("keep");
        std::fs::create_dir_all(&keep).unwrap();
        std::fs::write(keep.join("f"), "x").unwrap();
        std::os::unix::fs::symlink(&keep, dir.join("link")).unwrap();
        odd(&mut t, &["--all"]);
        assert!(!dir.exists());
        assert!(keep.join("f").exists());
    }

    #[test]
    fn windows_uses_localappdata() {
        let mut t = TestCtx::new(Os::Windows);
        let local = t.dir.path().join("local");
        t.env
            .insert("LOCALAPPDATA".into(), local.to_string_lossy().into_owned());
        std::fs::create_dir_all(local.join("pip/Cache")).unwrap();
        std::fs::write(local.join("pip/Cache/f"), "x").unwrap();
        odd(&mut t, &["--list"]);
        assert!(t.output().contains("pip"), "{}", t.output());
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KB");
        assert_eq!(human(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
