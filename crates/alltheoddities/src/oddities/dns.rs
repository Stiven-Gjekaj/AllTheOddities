//! `odd dns`: set the DNS servers of the network in use.

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;
use crate::state::{self, Undo};

const NAME: &str = "dns";

const PRESETS: &[(&str, &[&str])] = &[
    ("cloudflare", &["1.1.1.1", "1.0.0.1"]),
    ("google", &["8.8.8.8", "8.8.4.4"]),
    ("quad9", &["9.9.9.9", "149.112.112.112"]),
];

/// The index of the interface of the default route, and its static name
/// servers from the registry. An empty list means the servers of DHCP.
const WINDOWS_READ: &str = "$r = Get-NetRoute -DestinationPrefix 0.0.0.0/0 | Sort-Object RouteMetric | Select-Object -First 1; \
$g = (Get-NetAdapter -InterfaceIndex $r.InterfaceIndex).InterfaceGuid; \
$ns = (Get-ItemProperty \"HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\$g\").NameServer; \
\"$($r.InterfaceIndex)|$ns\"";

/// `odd dns`.
pub struct Dns;

impl Oddity for Dns {
    fn name(&self) -> &'static str {
        NAME
    }
    fn summary(&self) -> &'static str {
        "Set the DNS servers of the network in use, with presets such as cloudflare and quad9."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.subcommand(
            Command::new("set")
                .about("Set the servers: cloudflare, google, quad9, or addresses")
                .arg(Arg::new("servers").required(true).action(ArgAction::Append)),
        )
        .subcommand(Command::new("reset").about("Go back to the servers of DHCP"))
        .subcommand(Command::new("show").about("Show the network in use and its servers"))
    }
    fn needs_admin(&self, m: &ArgMatches, _ctx: &Ctx<'_>) -> bool {
        matches!(m.subcommand_name(), Some("set" | "reset"))
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let want: Option<Vec<String>> = match m.subcommand() {
            Some(("set", a)) => {
                let words: Vec<String> = a
                    .get_many::<String>("servers")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                match servers(&words) {
                    Ok(list) => Some(list),
                    Err(why) => {
                        ctx.say(&why);
                        return Ok(Exit::Usage);
                    }
                }
            }
            Some(("reset", _)) => Some(Vec::new()),
            _ => None,
        };
        if ctx.host.os == Os::Linux
            && let Some(list) = &want
            && list.iter().any(|ip| ip.contains(':'))
        {
            ctx.say("On Linux, odd dns sets IPv4 servers only.");
            return Ok(Exit::Usage);
        }
        let net = network(ctx)?;
        match want {
            None => {
                let shown = if net.servers.is_empty() {
                    "the servers of DHCP".to_owned()
                } else {
                    net.servers.join(" ")
                };
                ctx.say(&format!("{}: {shown}", net.label));
            }
            Some(list) => {
                set(ctx, &net, &list)?;
                let shown = if list.is_empty() {
                    "the servers of DHCP".to_owned()
                } else {
                    list.join(" ")
                };
                ctx.say(&format!("{} uses {shown}.", net.label));
            }
        }
        Ok(Exit::Done)
    }
}

/// The addresses of presets and of addresses. An error names the word that
/// is neither.
pub fn servers(words: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for w in words {
        if let Some((_, ips)) = PRESETS.iter().find(|(n, _)| *n == w.as_str()) {
            out.extend(ips.iter().map(|s| (*s).to_owned()));
        } else if w.parse::<std::net::IpAddr>().is_ok() {
            out.push(w.clone());
        } else {
            return Err(format!(
                "{w} is not an address or a preset (cloudflare, google, quad9)."
            ));
        }
    }
    Ok(out)
}

/// The network in use, and its static servers. No servers means DHCP.
struct Network {
    /// The macOS service, the Linux connection, or the Windows interface index.
    id: String,
    label: String,
    servers: Vec<String>,
}

fn network(ctx: &Ctx<'_>) -> Result<Network> {
    match ctx.host.os {
        Os::MacOs => {
            let route = ctx.query("route", &["-n", "get", "default"])?;
            let Some(dev) = route.stdout.lines().find_map(|l| {
                l.trim()
                    .strip_prefix("interface:")
                    .map(|v| v.trim().to_owned())
            }) else {
                bail!("this Mac has no default route, so odd cannot find the network in use");
            };
            let ports = ctx.query("networksetup", &["-listallhardwareports"])?;
            let mut service = None;
            let mut last = None;
            for line in ports.stdout.lines() {
                if let Some(p) = line.strip_prefix("Hardware Port: ") {
                    last = Some(p.trim().to_owned());
                } else if line.strip_prefix("Device: ").map(str::trim) == Some(dev.as_str()) {
                    service = last.clone();
                }
            }
            let Some(service) = service else {
                bail!("odd cannot find the network service of {dev}");
            };
            let now = ctx.query("networksetup", &["-getdnsservers", &service])?;
            let servers = now
                .stdout
                .lines()
                .map(str::trim)
                .filter(|l| l.parse::<std::net::IpAddr>().is_ok())
                .map(str::to_owned)
                .collect();
            Ok(Network {
                id: service.clone(),
                label: service,
                servers,
            })
        }
        Os::Linux => {
            if !ctx.runner.on_path("nmcli") {
                bail!(
                    "odd dns needs NetworkManager on Linux, and this host has no nmcli. It changed nothing"
                );
            }
            let route = ctx.query("ip", &["route", "show", "default"])?;
            let dev = route
                .stdout
                .split_whitespace()
                .skip_while(|w| *w != "dev")
                .nth(1)
                .map(str::to_owned);
            let Some(dev) = dev else {
                bail!("this host has no default route, so odd cannot find the network in use");
            };
            let active = ctx.query(
                "nmcli",
                &["-t", "-f", "NAME,DEVICE", "connection", "show", "--active"],
            )?;
            let conn = active.stdout.lines().find_map(|l| {
                let (name, d) = l.rsplit_once(':')?;
                (d == dev).then(|| name.replace("\\:", ":"))
            });
            let Some(conn) = conn else {
                bail!("NetworkManager does not manage {dev}");
            };
            let now = ctx.query(
                "nmcli",
                &[
                    "-g",
                    "ipv4.dns,ipv4.ignore-auto-dns",
                    "connection",
                    "show",
                    &conn,
                ],
            )?;
            let mut lines = now.stdout.lines();
            let dns = lines.next().unwrap_or("").to_owned();
            let ignore = lines.next().unwrap_or("no").trim() == "yes";
            let servers = if ignore {
                dns.split([',', ' '])
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            } else {
                Vec::new()
            };
            Ok(Network {
                id: conn.clone(),
                label: conn,
                servers,
            })
        }
        Os::Windows => {
            let out = ctx.query("powershell", &["-NoProfile", "-Command", WINDOWS_READ])?;
            let line = out.stdout.lines().next().unwrap_or("").trim().to_owned();
            let Some((index, ns)) = line.split_once('|') else {
                bail!("odd cannot find the network in use: {}", out.stderr.trim());
            };
            let servers = ns
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            Ok(Network {
                id: index.to_owned(),
                label: format!("The interface {index}"),
                servers,
            })
        }
    }
}

/// The host command that sets `list`, or DHCP when it is empty.
fn commands(ctx: &Ctx<'_>, net: &Network, list: &[String]) -> Vec<(String, Vec<String>)> {
    match ctx.host.os {
        Os::MacOs => {
            let mut args = vec!["-setdnsservers".to_owned(), net.id.clone()];
            if list.is_empty() {
                args.push("Empty".into());
            } else {
                args.extend(list.iter().cloned());
            }
            vec![("networksetup".into(), args)]
        }
        Os::Linux => {
            let ignore = if list.is_empty() { "no" } else { "yes" };
            vec![
                (
                    "nmcli".into(),
                    [
                        "connection",
                        "modify",
                        &net.id,
                        "ipv4.dns",
                        &list.join(" "),
                        "ipv4.ignore-auto-dns",
                        ignore,
                    ]
                    .map(String::from)
                    .to_vec(),
                ),
                (
                    "nmcli".into(),
                    ["connection", "up", &net.id].map(String::from).to_vec(),
                ),
            ]
        }
        Os::Windows => {
            let tail = if list.is_empty() {
                "-ResetServerAddresses".to_owned()
            } else {
                let quoted: Vec<String> = list.iter().map(|ip| format!("'{ip}'")).collect();
                format!("-ServerAddresses ({})", quoted.join(","))
            };
            let line = format!(
                "Set-DnsClientServerAddress -InterfaceIndex {} {tail}",
                net.id
            );
            vec![(
                "powershell".into(),
                vec!["-NoProfile".into(), "-Command".into(), line],
            )]
        }
    }
}

fn set(ctx: &mut Ctx<'_>, net: &Network, list: &[String]) -> Result<()> {
    if net.servers == list {
        return Ok(());
    }
    // Undo runs newest first, so record the steps of the old servers in the
    // reverse order.
    for (program, args) in commands(ctx, net, &net.servers).into_iter().rev() {
        state::record(ctx, NAME, Undo::Run { program, args })?;
    }
    for (program, args) in commands(ctx, net, list) {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = ctx.change(&program, &refs)?;
        if !out.success() {
            bail!("{program} failed: {}", out.stderr.trim());
        }
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
        let argv = ["odd", "dns"].iter().chain(args).map(Into::into).collect();
        dispatch(argv, &mut t.ctx(), &[&Dns])
    }

    #[test]
    fn preset_names_give_the_addresses() {
        assert_eq!(
            servers(&["cloudflare".into()]).unwrap(),
            ["1.1.1.1", "1.0.0.1"]
        );
        assert_eq!(
            servers(&["quad9".into()]).unwrap(),
            ["9.9.9.9", "149.112.112.112"]
        );
        assert_eq!(servers(&["8.8.8.8".into()]).unwrap(), ["8.8.8.8"]);
    }

    #[test]
    fn a_bad_address_exits_2() {
        let mut t = TestCtx::new(Os::MacOs);
        assert_eq!(odd(&mut t, &["set", "1.2.3"]), Exit::Usage);
        assert!(t.runner.calls().is_empty());
    }

    const PORTS: &str = "\nHardware Port: Ethernet\nDevice: en1\nEthernet Address: x\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: y\n";

    fn macos(before: &str) -> TestCtx {
        let t = TestCtx::new(Os::MacOs);
        t.runner.answer(
            "route",
            &[],
            Output::ok("   route to: default\ndestination: default\n  interface: en0\n"),
        );
        t.runner.answer(
            "networksetup",
            &["-listallhardwareports"],
            Output::ok(PORTS),
        );
        t.runner
            .answer("networksetup", &["-getdnsservers"], Output::ok(before));
        t.runner
            .answer("networksetup", &["-setdnsservers"], Output::ok(""));
        t
    }

    #[test]
    fn macos_sets_the_service_of_the_default_route() {
        let mut t = macos("There aren't any DNS Servers set on Wi-Fi.\n");
        assert_eq!(odd(&mut t, &["set", "cloudflare"]), Exit::Done);
        let set = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[1] == "-setdnsservers")
            .unwrap();
        assert_eq!(set[2..], ["Wi-Fi", "1.1.1.1", "1.0.0.1"]);
    }

    #[test]
    fn undo_goes_back_to_dhcp() {
        let mut t = macos("There aren't any DNS Servers set on Wi-Fi.\n");
        odd(&mut t, &["set", "google"]);
        odd(&mut t, &["--undo"]);
        let last = t.runner.calls().pop().unwrap();
        assert_eq!(last[1..], ["-setdnsservers", "Wi-Fi", "Empty"]);
    }

    #[test]
    fn undo_puts_back_the_servers_before() {
        let mut t = macos("192.168.1.1\n");
        odd(&mut t, &["set", "google"]);
        odd(&mut t, &["--undo"]);
        let last = t.runner.calls().pop().unwrap();
        assert_eq!(last[1..], ["-setdnsservers", "Wi-Fi", "192.168.1.1"]);
    }

    #[test]
    fn linux_without_nmcli_changes_nothing() {
        let mut t = TestCtx::new(Os::Linux);
        assert_eq!(odd(&mut t, &["set", "cloudflare"]), Exit::Failed);
        assert!(t.output().contains("NetworkManager"), "{}", t.output());
        assert!(t.runner.calls().is_empty());
    }

    fn linux() -> TestCtx {
        let t = TestCtx::new(Os::Linux);
        t.runner.with_path(&["nmcli"]);
        t.runner.answer(
            "ip",
            &[],
            Output::ok("default via 192.168.1.1 dev wlan0 proto dhcp metric 600\n"),
        );
        t.runner.answer(
            "nmcli",
            &["-t"],
            Output::ok("Wired:eth0\nHome\\:5G:wlan0\n"),
        );
        t.runner.answer("nmcli", &["-g"], Output::ok("\nno\n"));
        t.runner
            .answer("nmcli", &["connection", "modify"], Output::ok(""));
        t.runner
            .answer("nmcli", &["connection", "up"], Output::ok(""));
        t
    }

    #[test]
    fn linux_sets_the_active_connection() {
        let mut t = linux();
        assert_eq!(odd(&mut t, &["set", "quad9"]), Exit::Done);
        let modify = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c.get(2).is_some_and(|a| a == "modify"))
            .unwrap();
        assert_eq!(
            modify[3..],
            [
                "Home:5G",
                "ipv4.dns",
                "9.9.9.9 149.112.112.112",
                "ipv4.ignore-auto-dns",
                "yes"
            ]
        );
    }

    #[test]
    fn linux_refuses_ipv6() {
        let mut t = linux();
        assert_eq!(odd(&mut t, &["set", "2606:4700:4700::1111"]), Exit::Usage);
    }

    #[test]
    fn windows_sets_the_default_interface() {
        let mut t = TestCtx::new(Os::Windows);
        t.runner.answer("powershell", &[], Output::ok("12|\r\n"));
        assert_eq!(odd(&mut t, &["set", "cloudflare"]), Exit::Done);
        let set = t
            .runner
            .calls()
            .into_iter()
            .find(|c| c[3].starts_with("Set-Dns"))
            .unwrap();
        assert_eq!(
            set[3],
            "Set-DnsClientServerAddress -InterfaceIndex 12 -ServerAddresses ('1.1.1.1','1.0.0.1')"
        );
        odd(&mut t, &["--undo"]);
        let last = t.runner.calls().pop().unwrap();
        assert_eq!(
            last[3],
            "Set-DnsClientServerAddress -InterfaceIndex 12 -ResetServerAddresses"
        );
    }

    #[test]
    fn show_needs_no_admin() {
        let mut t = macos("1.1.1.1\n");
        t.is_admin = false;
        assert_eq!(odd(&mut t, &["show"]), Exit::Done);
        assert!(
            t.output().contains("Wi-Fi") && t.output().contains("1.1.1.1"),
            "{}",
            t.output()
        );
    }
}
