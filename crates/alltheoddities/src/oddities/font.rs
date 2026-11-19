//! `odd font`: install a font for the user only, with no administrator.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::{Arg, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "font";
const FONTS_KEY: &str = r"HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts";
const NERD: &str = "https://github.com/ryanoasis/nerd-fonts/releases/latest/download";

/// `odd font`.
pub struct Font;

impl Oddity for Font {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Install a font for the user only: a file, a folder, an archive, or a Nerd Font by name."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("font")
                .help("A .ttf or .otf file, a folder, a .zip or .tar.xz, or a Nerd Font name such as JetBrainsMono"),
        )
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        // Not required for clap, so `odd font --undo` parses.
        let Some(arg) = m.get_one::<String>("font").cloned() else {
            ctx.say("Give a font: a file, a folder, an archive, or a Nerd Font name.");
            return Ok(Exit::Usage);
        };
        let work = ctx.state_dir.join("font-work");
        let result = install(ctx, &arg, &work);
        let _ = std::fs::remove_dir_all(&work);
        result.map(|()| Exit::Done)
    }
}

fn is_font(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
}

fn is_archive(p: &str) -> bool {
    let l = p.to_lowercase();
    l.ends_with(".zip") || l.ends_with(".tar.xz") || l.ends_with(".tar.gz") || l.ends_with(".tgz")
}

fn fonts_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            fonts_in(&p, out);
        } else if is_font(&p) {
            out.push(p);
        }
    }
}

/// The folder of the fonts of the user.
fn fonts_dir(ctx: &Ctx<'_>) -> PathBuf {
    match ctx.host.os {
        Os::MacOs => ctx.home.join("Library").join("Fonts"),
        Os::Linux => ctx.home.join(".local").join("share").join("fonts"),
        Os::Windows => ctx
            .env
            .get("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.home.join("AppData").join("Local"))
            .join("Microsoft")
            .join("Windows")
            .join("Fonts"),
    }
}

fn unpack(ctx: &mut Ctx<'_>, archive: &Path, work: &Path) -> Result<PathBuf> {
    let dest = work.join("unpacked");
    ctx.make_dir(&dest)?;
    let (a, d) = (
        archive.to_string_lossy().into_owned(),
        dest.to_string_lossy().into_owned(),
    );
    let out = ctx.change("tar", &["-xf", &a, "-C", &d])?;
    if !out.success() {
        bail!("tar could not unpack {a}: {}", out.stderr.trim());
    }
    Ok(dest)
}

fn install(ctx: &mut Ctx<'_>, arg: &str, work: &Path) -> Result<()> {
    let nerd_name = !arg.contains(['/', '\\', '.']);
    let source: PathBuf = if nerd_name {
        let file = if ctx.host.os == Os::Linux {
            format!("{arg}.tar.xz")
        } else {
            format!("{arg}.zip")
        };
        let url = format!("{NERD}/{file}");
        ctx.say(&format!("Downloading {url}"));
        let bytes = ctx.http.get_bytes(&url)?;
        let archive = work.join(&file);
        ctx.make_dir(work)?;
        if !ctx.dry_run {
            std::fs::write(&archive, bytes)?;
        }
        unpack(ctx, &archive, work)?
    } else if is_archive(arg) {
        unpack(ctx, Path::new(arg), work)?
    } else {
        PathBuf::from(arg)
    };
    let mut fonts = Vec::new();
    if source.is_dir() {
        fonts_in(&source, &mut fonts);
    } else if is_font(&source) {
        fonts.push(source.clone());
    }
    if fonts.is_empty() {
        bail!("{arg} has no .ttf or .otf file");
    }
    let dir = fonts_dir(ctx);
    let mut added = 0;
    for font in &fonts {
        let Some(name) = font.file_name() else {
            continue;
        };
        let dest = dir.join(name);
        if dest.exists() {
            continue;
        }
        let bytes = std::fs::read(font)?;
        let step = Undo::RestoreFile {
            path: dest.clone(),
            before: None,
            after_sha256: state::sha256_hex(&bytes),
        };
        state::record(ctx, NAME, step)?;
        ctx.copy_file(font, &dest)?;
        if ctx.host.os == Os::Windows {
            let stem = font
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let value = format!("{stem} (TrueType)");
            let before = crate::winreg::read(ctx, FONTS_KEY, &value)?;
            state::record(
                ctx,
                NAME,
                crate::winreg::undo_step(FONTS_KEY, &value, before),
            )?;
            crate::winreg::write(ctx, FONTS_KEY, &value, "REG_SZ", &dest.to_string_lossy())?;
        }
        added += 1;
    }
    if ctx.host.os == Os::Linux && added > 0 && ctx.runner.on_path("fc-cache") {
        ctx.change("fc-cache", &["-f"])?;
    }
    let skipped = fonts.len() - added;
    ctx.say(&format!(
        "{added} font file(s) are in {}. {skipped} were there already.",
        dir.display()
    ));
    if added > 0 {
        ctx.say("Restart an app to see a new font in it.");
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
        let argv = ["odd", "font"].iter().chain(args).map(Into::into).collect();
        dispatch(argv, &mut t.ctx(), &[&Font])
    }

    /// The fonts that a fake `tar` "unpacks": the test puts them where odd
    /// unpacks, before the run.
    fn unpacked(t: &TestCtx) {
        let dir = t.state_dir().join("font-work").join("unpacked").join("sub");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Mono-Regular.ttf"), "ttf").unwrap();
        std::fs::write(dir.join("Mono-Bold.otf"), "otf").unwrap();
        std::fs::write(dir.join("README.md"), "no").unwrap();
    }

    fn nerd(t: &TestCtx, file: &str) {
        t.http.bytes(
            &format!("https://github.com/ryanoasis/nerd-fonts/releases/latest/download/{file}"),
            b"archive".to_vec(),
        );
        t.runner.answer("tar", &[], Output::ok(""));
        t.runner.answer("fc-cache", &[], Output::ok(""));
        t.runner.with_path(&["fc-cache"]);
    }

    #[test]
    fn a_name_downloads_the_nerd_font() {
        let mut t = TestCtx::new(Os::MacOs);
        nerd(&t, "JetBrainsMono.zip");
        unpacked(&t);
        assert_eq!(odd(&mut t, &["JetBrainsMono"]), Exit::Done);
        let tar = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "tar")
            .unwrap();
        assert!(tar[2].ends_with("JetBrainsMono.zip"), "{tar:?}");
        assert!(t.home().join("Library/Fonts/Mono-Regular.ttf").exists());
        assert!(
            !t.state_dir().join("font-work").exists(),
            "the work folder goes away"
        );
    }

    #[test]
    fn linux_uses_tar_xz_and_fc_cache() {
        let mut t = TestCtx::new(Os::Linux);
        nerd(&t, "Hack.tar.xz");
        unpacked(&t);
        assert_eq!(odd(&mut t, &["Hack"]), Exit::Done);
        assert!(t.home().join(".local/share/fonts/Mono-Bold.otf").exists());
        assert!(t.runner.calls().iter().any(|c| c[0] == "fc-cache"));
    }

    #[test]
    fn finds_fonts_in_folders() {
        let mut t = TestCtx::new(Os::MacOs);
        let dir = t.dir.path().join("fonts/inner");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("A.ttf"), "a").unwrap();
        std::fs::write(dir.join("notes.txt"), "n").unwrap();
        let arg = t.dir.path().join("fonts").to_string_lossy().into_owned();
        assert_eq!(odd(&mut t, &[&arg]), Exit::Done);
        assert!(t.home().join("Library/Fonts/A.ttf").exists());
        assert!(!t.home().join("Library/Fonts/notes.txt").exists());
    }

    #[test]
    fn windows_registers_each_font() {
        let mut t = TestCtx::new(Os::Windows);
        t.env.insert(
            "LOCALAPPDATA".into(),
            t.dir.path().join("local").to_string_lossy().into_owned(),
        );
        t.runner
            .answer("reg", &["query"], Output::fail(1, "not found"));
        t.runner.answer("reg", &["add"], Output::ok(""));
        let file = t.dir.path().join("Code.ttf");
        std::fs::write(&file, "c").unwrap();
        assert_eq!(odd(&mut t, &[&file.to_string_lossy()]), Exit::Done);
        let add = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[1] == "add")
            .unwrap();
        assert_eq!(add[4], "Code (TrueType)");
        assert!(add[8].ends_with("Code.ttf"));
    }

    #[test]
    fn a_font_that_is_there_is_skipped() {
        let mut t = TestCtx::new(Os::MacOs);
        let fonts = t.home().join("Library/Fonts");
        std::fs::create_dir_all(&fonts).unwrap();
        std::fs::write(fonts.join("A.ttf"), "old").unwrap();
        let file = t.dir.path().join("A.ttf");
        std::fs::write(&file, "new").unwrap();
        odd(&mut t, &[&file.to_string_lossy()]);
        assert_eq!(std::fs::read_to_string(fonts.join("A.ttf")).unwrap(), "old");
    }

    #[test]
    fn undo_deletes_the_copied_files() {
        let mut t = TestCtx::new(Os::MacOs);
        let file = t.dir.path().join("A.ttf");
        std::fs::write(&file, "a").unwrap();
        odd(&mut t, &[&file.to_string_lossy()]);
        assert_eq!(odd(&mut t, &["--undo"]), Exit::Done);
        assert!(!t.home().join("Library/Fonts/A.ttf").exists());
    }
}
