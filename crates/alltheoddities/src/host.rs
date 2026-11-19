//! Which host runs `odd`, and with which rights.

use std::fmt;

/// The operating system of the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Os::MacOs => "macOS",
            Os::Linux => "Linux",
            Os::Windows => "Windows",
        })
    }
}

/// The host: its system, and on Linux the `ID` of its distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub os: Os,
    pub distro: Option<String>,
}

impl Host {
    /// The host that runs this binary.
    pub fn current() -> Host {
        let os = if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        };
        let distro = match os {
            Os::Linux => std::fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|text| os_release_id(&text)),
            _ => None,
        };
        Host { os, distro }
    }
}

/// The value of `ID` in the text of `/etc/os-release`, without quotes.
pub fn os_release_id(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("ID="))
        .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_owned())
        .filter(|v| !v.is_empty())
}

/// True when this process has the rights of an administrator.
#[cfg(unix)]
#[allow(unsafe_code)]
pub fn is_admin() -> bool {
    // SAFETY: geteuid has no arguments, never fails and touches no memory.
    unsafe { libc::geteuid() == 0 }
}

/// True when this process runs elevated, as a member of Administrators.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn is_admin() -> bool {
    // SAFETY: IsUserAnAdmin has no arguments and touches no memory of ours.
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_id_with_and_without_quotes() {
        assert_eq!(os_release_id("NAME=x\nID=ubuntu\n"), Some("ubuntu".into()));
        assert_eq!(os_release_id("ID=\"fedora\"\n"), Some("fedora".into()));
        assert_eq!(os_release_id("ID_LIKE=debian\n"), None);
    }
}
