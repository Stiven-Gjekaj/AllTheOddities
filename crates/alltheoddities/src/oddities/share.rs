//! `odd share`: serve a file or a folder on the local network until Ctrl+C.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command, value_parser};

use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::host::Os;
use crate::oddity::Oddity;

/// `odd share`.
pub struct Share;

impl Oddity for Share {
    fn name(&self) -> &'static str {
        "share"
    }
    fn summary(&self) -> &'static str {
        "Serve a file or a folder on the local network, with a secret link and a QR code, until Ctrl+C."
    }
    fn hosts(&self) -> &'static [Os] {
        &[Os::MacOs, Os::Linux, Os::Windows]
    }
    fn args(&self, cmd: Command) -> Command {
        cmd.arg(
            Arg::new("path")
                .required(true)
                .help("The file or the folder to share"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .default_value("0")
                .value_parser(value_parser!(u16))
                .help("The port. 0 takes a free one"),
        )
    }
    fn can_undo(&self) -> bool {
        false
    }
    fn run(&self, m: &ArgMatches, ctx: &mut Ctx<'_>) -> Result<Exit> {
        let path = m.get_one::<String>("path").cloned().unwrap_or_default();
        let root =
            std::fs::canonicalize(&path).with_context(|| format!("{path} does not exist"))?;
        let port = m.get_one::<u16>("port").copied().unwrap_or(0);
        if ctx.dry_run {
            ctx.say(&format!("would serve {} on port {port}", root.display()));
            return Ok(Exit::Done);
        }
        let listener = TcpListener::bind(("0.0.0.0", port)).context("could not open the port")?;
        let port = listener.local_addr()?.port();
        let url = format!("http://{}:{port}/{}/", lan_ip(), token()?);
        ctx.say(&format!("Sharing {} until Ctrl+C at:", root.display()));
        ctx.say(&url);
        ctx.say("");
        ctx.say(crate::qr::render(&url).trim_end());
        ctx.say("Only a device with this link can read the files.");
        let _ = ctx.out.flush();
        let token = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_owned();
        serve(listener, &root, &token);
        Ok(Exit::Done)
    }
}

/// 16 random bytes as hex.
pub fn token() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no random data: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The address of this computer on the local network: the local address of
/// a UDP socket connected to 1.1.1.1. A UDP connect sends no packet.
fn lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("1.1.1.1:80").map(|()| s))
        .and_then(|s| s.local_addr())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

/// Answers each connection on its own thread. It runs until the process ends.
pub fn serve(listener: TcpListener, root: &Path, token: &str) {
    // The check that a path stays inside the root compares real paths.
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_owned());
    for stream in listener.incoming().flatten() {
        let root = root.to_owned();
        let token = token.to_owned();
        std::thread::spawn(move || {
            let _ = answer(stream, &root, &token);
        });
    }
}

fn respond(mut s: &TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        s,
        "HTTP/1.0 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    s.write_all(body)
}

fn not_found(s: &TcpStream) -> std::io::Result<()> {
    respond(s, "404 Not Found", "text/plain", b"Not found.\n")
}

fn answer(stream: TcpStream, root: &Path, token: &str) -> Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    // Read the headers, and ignore them.
    let mut header = String::new();
    while reader.read_line(&mut header)? > 2 && header.len() < 16 * 1024 {
        header.clear();
    }
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if method != "GET" {
        respond(
            &stream,
            "405 Method Not Allowed",
            "text/plain",
            b"GET only.\n",
        )?;
        return Ok(());
    }
    let prefix = format!("/{token}/");
    let Some(rest) = target
        .split('?')
        .next()
        .and_then(|p| p.strip_prefix(&prefix))
    else {
        not_found(&stream)?;
        return Ok(());
    };
    let Some(rest) = decode(rest) else {
        not_found(&stream)?;
        return Ok(());
    };
    match resolve(root, &rest) {
        None => not_found(&stream)?,
        Some(path) if path.is_dir() => respond(
            &stream,
            "200 OK",
            "text/html; charset=utf-8",
            listing(&path).as_bytes(),
        )?,
        Some(_) if rest.is_empty() => {
            // A share of one file: a page with a link to it.
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            respond(
                &stream,
                "200 OK",
                "text/html; charset=utf-8",
                page(&[(name, false)]).as_bytes(),
            )?
        }
        Some(path) => {
            let bytes = std::fs::read(&path)?;
            respond(&stream, "200 OK", "application/octet-stream", &bytes)?
        }
    }
    Ok(())
}

/// The path of a request inside the root, or None when it leaves the root.
fn resolve(root: &Path, rest: &str) -> Option<PathBuf> {
    if root.is_file() {
        let name = root.file_name()?.to_string_lossy().into_owned();
        return match rest {
            "" => Some(root.to_owned()),
            r if r == name => Some(root.to_owned()),
            _ => None,
        };
    }
    let mut path = root.to_owned();
    for c in Path::new(rest).components() {
        match c {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    let real = std::fs::canonicalize(&path).ok()?;
    real.starts_with(root).then_some(real)
}

/// `%XX` to bytes. None for a bad escape or text that is not UTF-8.
fn decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn listing(dir: &Path) -> String {
    let mut entries: Vec<(String, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        e.path().is_dir(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    page(&entries)
}

fn page(entries: &[(String, bool)]) -> String {
    let mut out = String::from(
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><title>odd share</title><ul>\n",
    );
    for (name, dir) in entries {
        let slash = if *dir { "/" } else { "" };
        out.push_str(&format!(
            "<li><a href=\"{}{slash}\">{}{slash}</a></li>\n",
            encode(name),
            html(name)
        ));
    }
    out.push_str("</ul>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    const TOKEN: &str = "00112233445566778899aabbccddeeff";

    /// A server on a free port of 127.0.0.1 for `root`. Gives its port.
    fn start(root: &std::path::Path) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let root = root.to_owned();
        std::thread::spawn(move || serve(listener, &root, TOKEN));
        port
    }

    fn get(port: u16, path: &str) -> (u16, String) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut text = String::new();
        s.read_to_string(&mut text).unwrap();
        let code = text.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = text
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_owned())
            .unwrap_or_default();
        (code, body)
    }

    fn folder() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("shared");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a b.txt"), "hello").unwrap();
        std::fs::write(root.join("sub/c.txt"), "deep").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "no").unwrap();
        (dir, root)
    }

    #[test]
    fn a_file_is_served() {
        let (_d, root) = folder();
        let port = start(&root);
        assert_eq!(
            get(port, &format!("/{TOKEN}/a%20b.txt")),
            (200, "hello".into())
        );
        assert_eq!(
            get(port, &format!("/{TOKEN}/sub/c.txt")),
            (200, "deep".into())
        );
    }

    #[test]
    fn a_folder_is_listed() {
        let (_d, root) = folder();
        let port = start(&root);
        let (code, body) = get(port, &format!("/{TOKEN}/"));
        assert_eq!(code, 200);
        assert!(
            body.contains("a%20b.txt") && body.contains("a b.txt") && body.contains("sub/"),
            "{body}"
        );
    }

    #[test]
    fn no_token_gives_404() {
        let (_d, root) = folder();
        let port = start(&root);
        assert_eq!(get(port, "/a%20b.txt").0, 404);
        assert_eq!(get(port, "/wrongtoken/a%20b.txt").0, 404);
    }

    #[test]
    fn dot_dot_gives_404() {
        let (_d, root) = folder();
        let port = start(&root);
        assert_eq!(get(port, &format!("/{TOKEN}/../secret.txt")).0, 404);
        assert_eq!(get(port, &format!("/{TOKEN}/%2e%2e/secret.txt")).0, 404);
        assert_eq!(get(port, &format!("/{TOKEN}/sub/../../secret.txt")).0, 404);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_root_gives_404() {
        let (d, root) = folder();
        std::os::unix::fs::symlink(d.path().join("secret.txt"), root.join("link.txt")).unwrap();
        let port = start(&root);
        assert_eq!(get(port, &format!("/{TOKEN}/link.txt")).0, 404);
    }

    #[test]
    fn a_single_file_share_serves_only_that_file() {
        let (_d, root) = folder();
        let port = start(&root.join("a b.txt"));
        assert_eq!(
            get(port, &format!("/{TOKEN}/a%20b.txt")),
            (200, "hello".into())
        );
        assert_eq!(get(port, &format!("/{TOKEN}/sub/c.txt")).0, 404);
        assert!(get(port, &format!("/{TOKEN}/")).1.contains("a%20b.txt"));
    }

    #[test]
    fn a_post_gives_405() {
        let (_d, root) = folder();
        let port = start(&root);
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "POST /{TOKEN}/ HTTP/1.1\r\n\r\n").unwrap();
        let mut text = String::new();
        s.read_to_string(&mut text).unwrap();
        assert!(text.starts_with("HTTP/1.0 405"), "{text}");
    }

    #[test]
    fn the_token_is_32_hex_characters() {
        let a = token().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, token().unwrap());
    }
}
