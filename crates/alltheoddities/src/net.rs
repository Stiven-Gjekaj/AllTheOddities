//! Read a page or a file from the network.

use anyhow::{Result, bail};

/// Reads from HTTPS. The tests use a fake.
pub trait Http {
    fn get(&self, url: &str) -> Result<String>;
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>>;
}

/// Reads with the `curl` of the host, or `wget` when the host has no curl.
///
/// macOS, Windows 10 and later, and most Linux hosts have curl. A Rust TLS
/// library would compile C code for each target, which the release builds
/// do not allow.
pub struct HostHttp;

impl Http for HostHttp {
    fn get(&self, url: &str) -> Result<String> {
        String::from_utf8(self.get_bytes(url)?)
            .map_err(|_| anyhow::anyhow!("{url} gave text that is not UTF-8"))
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        use std::process::{Command, Stdio};
        if !url.starts_with("https://") {
            bail!("odd reads HTTPS only, not {url}");
        }
        let tries: [(&str, Vec<&str>); 2] = [
            ("curl", vec!["-fsSL", "--proto", "=https", "--tlsv1.2", url]),
            ("wget", vec!["-qO-", "--https-only", url]),
        ];
        for (program, args) in tries {
            match Command::new(program)
                .args(&args)
                .stdin(Stdio::null())
                .output()
            {
                Ok(out) if out.status.success() => return Ok(out.stdout),
                Ok(out) => {
                    let why = String::from_utf8_lossy(&out.stderr);
                    bail!("{program} could not read {url}: {}", why.trim());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => bail!("could not start {program}: {e}"),
            }
        }
        bail!("this host has no curl and no wget, so odd cannot read {url}")
    }
}
