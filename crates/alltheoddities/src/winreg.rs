//! The registry of Windows, through `reg.exe`, so the tests run on each host.

use anyhow::{Result, bail};

use crate::ctx::Ctx;
use crate::state::Undo;

/// The type, such as `REG_SZ`, and the data of a value, as `reg query`
/// prints them. None when the value does not exist.
pub fn read(ctx: &Ctx<'_>, key: &str, name: &str) -> Result<Option<(String, String)>> {
    let out = ctx.query("reg", &["query", key, "/v", name])?;
    if !out.success() {
        return Ok(None);
    }
    Ok(out.stdout.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix(name)?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let rest = rest.trim_start();
        let kind_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let kind = &rest[..kind_end];
        if !kind.starts_with("REG_") {
            return None;
        }
        Some((kind.to_owned(), rest[kind_end..].trim().to_owned()))
    }))
}

/// Sets a value.
pub fn write(ctx: &mut Ctx<'_>, key: &str, name: &str, kind: &str, data: &str) -> Result<()> {
    let out = ctx.change(
        "reg",
        &["add", key, "/v", name, "/t", kind, "/d", data, "/f"],
    )?;
    if !out.success() {
        bail!("reg add {key} {name} failed: {}", out.stderr.trim());
    }
    Ok(())
}

/// The undo step of a change to a value: put back the type and data
/// before, or delete a value that did not exist.
pub fn undo_step(key: &str, name: &str, before: Option<(String, String)>) -> Undo {
    let args: Vec<&str> = match &before {
        Some((kind, data)) => vec!["add", key, "/v", name, "/t", kind, "/d", data, "/f"],
        None => vec!["delete", key, "/v", name, "/f"],
    };
    Undo::Run {
        program: "reg".into(),
        args: args.into_iter().map(str::to_owned).collect(),
    }
}

/// Tells open programs, such as Explorer, that the environment changed.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn broadcast_change() {
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

/// Tells open programs that the environment changed. Only Windows has this.
#[cfg(not(windows))]
pub fn broadcast_change() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::run::Output;
    use crate::testing::TestCtx;

    #[test]
    fn undo_step_deletes_a_value_that_did_not_exist() {
        let step = undo_step(r"HKCU\Environment", "FOO", None);
        assert_eq!(
            step,
            Undo::Run {
                program: "reg".into(),
                args: ["delete", r"HKCU\Environment", "/v", "FOO", "/f"]
                    .map(String::from)
                    .to_vec(),
            }
        );
    }

    #[test]
    fn undo_step_puts_back_the_old_type_and_data() {
        let before = Some(("REG_DWORD".to_owned(), "0x1".to_owned()));
        let Undo::Run { args, .. } = undo_step("HKCU\\X", "Hidden", before) else {
            panic!()
        };
        assert_eq!(
            args,
            [
                "add",
                "HKCU\\X",
                "/v",
                "Hidden",
                "/t",
                "REG_DWORD",
                "/d",
                "0x1",
                "/f"
            ]
        );
    }

    #[test]
    fn read_parses_the_type_and_the_data() {
        let mut t = TestCtx::new(Os::Windows);
        let out =
            "\r\nHKEY_CURRENT_USER\\Environment\r\n    Path    REG_EXPAND_SZ    a b;c\r\n\r\n";
        t.runner.answer("reg", &["query"], Output::ok(out));
        let got = read(&t.ctx(), r"HKCU\Environment", "Path").unwrap();
        assert_eq!(got, Some(("REG_EXPAND_SZ".into(), "a b;c".into())));
    }

    #[test]
    fn read_of_a_missing_value_is_none() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer(
            "reg",
            &["query"],
            Output::fail(
                1,
                "ERROR: The system was unable to find the specified registry key or value.",
            ),
        );
        assert_eq!(read(&t.ctx(), r"HKCU\Environment", "Nope").unwrap(), None);
    }

    #[test]
    fn read_of_an_empty_value_is_empty() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer(
            "reg",
            &["query"],
            Output::ok("\r\nHKEY_CURRENT_USER\\X\r\n    V    REG_SZ    \r\n"),
        );
        assert_eq!(
            read(&t.ctx(), "HKCU\\X", "V").unwrap(),
            Some(("REG_SZ".into(), String::new()))
        );
    }
}
