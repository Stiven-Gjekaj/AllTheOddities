//! `odd wifi-pass`: the password of a saved Wi-Fi network, and its QR code.

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

/// `odd wifi-pass`.
pub struct WifiPass;

impl Oddity for WifiPass {
    fn name(&self) -> &'static str {
        "wifi-pass"
    }
    fn summary(&self) -> &'static str {
        "Show the password of the current or a named Wi-Fi network, and a QR code for a phone."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("network").help("The name of a saved network. The default is the current one"),
        )
        .arg(
            Arg::new("qr")
                .long("qr")
                .action(ArgAction::SetTrue)
                .help("Also print a QR code that a phone can scan"),
        )
    }
    fn can_undo(&self) -> bool {
        false
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let ssid = match m.get_one::<String>("network") {
            Some(n) => n.clone(),
            None => match current(ctx)? {
                Some(n) => n,
                None => bail!(
                    "this computer is on no Wi-Fi network. Name a saved network: odd wifi-pass <network>"
                ),
            },
        };
        let password = password(ctx, &ssid)?;
        ctx.say(&format!("Network:  {ssid}"));
        ctx.say(&format!(
            "Password: {}",
            password.as_deref().unwrap_or("(none, an open network)")
        ));
        if m.get_flag("qr") {
            ctx.say("");
            ctx.say(crate::qr::render(&wifi_qr_text(&ssid, password.as_deref())).trim_end());
        }
        Ok(Exit::Done)
    }
}

/// The text of a Wi-Fi QR code, as phones read it. `\ ; , : "` get a `\`.
pub fn wifi_qr_text(ssid: &str, password: Option<&str>) -> String {
    let esc = |s: &str| {
        let mut out = String::new();
        for c in s.chars() {
            if matches!(c, '\\' | ';' | ',' | ':' | '"') {
                out.push('\\');
            }
            out.push(c);
        }
        out
    };
    match password {
        Some(p) => format!("WIFI:T:WPA;S:{};P:{};;", esc(ssid), esc(p)),
        None => format!("WIFI:T:nopass;S:{};;", esc(ssid)),
    }
}

/// The value after `<label> :` on a line of `netsh` or `ipconfig`.
fn field(text: &str, label: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == label)
            .then(|| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    })
}

fn current(ctx: &Ctx<'_>) -> Result<Option<String>> {
    Ok(match ctx.host.os {
        Os::MacOs => {
            let ports = ctx.query("networksetup", &["-listallhardwareports"])?;
            let mut device = None;
            let mut wifi = false;
            for line in ports.stdout.lines() {
                if let Some(p) = line.strip_prefix("Hardware Port: ") {
                    wifi = p.trim() == "Wi-Fi" || p.trim() == "AirPort";
                } else if let Some(d) = line.strip_prefix("Device: ")
                    && wifi
                {
                    device = Some(d.trim().to_owned());
                }
            }
            let Some(device) = device else {
                return Ok(None);
            };
            let summary = ctx.query("ipconfig", &["getsummary", &device])?;
            field(&summary.stdout, "SSID")
        }
        Os::Linux => {
            let out = ctx.query("nmcli", &["-t", "-f", "active,ssid", "dev", "wifi"])?;
            out.stdout
                .lines()
                .find_map(|l| l.strip_prefix("yes:").map(|s| s.replace("\\:", ":")))
        }
        Os::Windows => {
            let out = ctx.query("netsh", &["wlan", "show", "interfaces"])?;
            field(&out.stdout, "SSID")
        }
    })
}

fn password(ctx: &Ctx<'_>, ssid: &str) -> Result<Option<String>> {
    let out = match ctx.host.os {
        Os::MacOs => ctx.query(
            "security",
            &[
                "find-generic-password",
                "-D",
                "AirPort network password",
                "-a",
                ssid,
                "-w",
            ],
        )?,
        Os::Linux => ctx.query(
            "nmcli",
            &[
                "-s",
                "-g",
                "802-11-wireless-security.psk",
                "connection",
                "show",
                ssid,
            ],
        )?,
        Os::Windows => {
            let name = format!("name={ssid}");
            let out = ctx.query("netsh", &["wlan", "show", "profile", &name, "key=clear"])?;
            if !out.success() {
                bail!("Windows has no saved network named {ssid}");
            }
            return Ok(field(&out.stdout, "Key Content"));
        }
    };
    if !out.success() {
        bail!(
            "could not read the password of {ssid}: {}",
            out.stderr.trim()
        );
    }
    let p = out.stdout.trim().to_owned();
    Ok((!p.is_empty()).then_some(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::oddity::dispatch;
    use crate::run::Output;
    use crate::testing::TestCtx;

    fn odd(t: &mut TestCtx, args: &[&str]) -> Exit {
        let argv = ["odd", "wifi-pass"]
            .iter()
            .chain(args)
            .map(Into::into)
            .collect();
        dispatch(argv, &mut t.ctx(), &[&WifiPass])
    }

    #[test]
    fn escapes_special_characters() {
        assert_eq!(
            wifi_qr_text(r#"a;b,c:d\e"f"#, Some("p;w")),
            r#"WIFI:T:WPA;S:a\;b\,c\:d\\e\"f;P:p\;w;;"#
        );
    }

    #[test]
    fn an_open_network_has_nopass() {
        assert_eq!(wifi_qr_text("cafe", None), "WIFI:T:nopass;S:cafe;;");
    }

    #[test]
    fn macos_reads_the_ssid_of_the_wifi_device() {
        let mut t = TestCtx::new(Os::MacOs);
        t.runner.answer(
            "networksetup",
            &["-listallhardwareports"],
            Output::ok("Hardware Port: Wi-Fi\nDevice: en0\n"),
        );
        t.runner.answer(
            "ipconfig",
            &["getsummary", "en0"],
            Output::ok("  BSSID : x\n  SSID : Home Net\n"),
        );
        t.runner.answer("security", &[], Output::ok("hunter2\n"));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(
            t.output().contains("Home Net") && t.output().contains("hunter2"),
            "{}",
            t.output()
        );
        let sec = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[0] == "security")
            .unwrap();
        assert!(sec.windows(2).any(|w| w == ["-a", "Home Net"]), "{sec:?}");
    }

    #[test]
    fn linux_reads_the_active_network() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner
            .answer("nmcli", &["-t"], Output::ok("no:Other\nyes:Home\n"));
        t.runner.answer("nmcli", &["-s"], Output::ok("hunter2\n"));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.output().contains("hunter2"));
    }

    #[test]
    fn windows_reads_the_key_content() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer("netsh", &["wlan", "show", "interfaces"], Output::ok("    Name                   : Wi-Fi\r\n    SSID                   : Home\r\n    BSSID                  : aa\r\n"));
        t.runner.answer("netsh", &["wlan", "show", "profile"], Output::ok("    Authentication         : WPA2-Personal\r\n    Key Content            : hunter2\r\n"));
        assert_eq!(odd(&mut t, &[]), Exit::Done);
        assert!(t.output().contains("hunter2"), "{}", t.output());
        let call = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c.get(3).is_some_and(|a| a == "profile"))
            .unwrap();
        assert!(
            call.contains(&"name=Home".to_owned()) && call.contains(&"key=clear".to_owned()),
            "{call:?}"
        );
    }

    #[test]
    fn a_named_network_skips_the_lookup() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("nmcli", &["-s"], Output::ok("pw\n"));
        assert_eq!(odd(&mut t, &["Cafe"]), Exit::Done);
        assert!(
            !t.runner
                .calls()
                .iter()
                .any(|c| c.get(1).is_some_and(|a| a == "-t"))
        );
    }

    #[test]
    fn no_wifi_fails_with_a_message() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("nmcli", &["-t"], Output::ok("no:Other\n"));
        assert_eq!(odd(&mut t, &[]), Exit::Failed);
        assert!(t.output().contains("Wi-Fi"), "{}", t.output());
    }

    #[test]
    fn qr_prints_a_code() {
        let mut t = TestCtx::new(Os::Linux);
        t.runner.answer("nmcli", &["-s"], Output::ok("pw\n"));
        odd(&mut t, &["Cafe", "--qr"]);
        assert!(t.output().contains('\u{2588}'), "{}", t.output());
    }
}
