# odd, plan 1: the core, `odd path`, `odd ssh`, the installers and CI

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `odd` binary with its shared core, `odd list`, `odd path`,
`odd ssh` and `odd update`, the two one-line installers, and the CI and
release workflows.

**Architecture:** A Cargo workspace with one crate, `alltheoddities`, that
builds the binary `odd`, in the layout of AllTheTools. Each oddity is one
file that implements the `Oddity` trait, and one line in a registry array.
Each oddity reaches the host only through a `Ctx`: host commands go through a
`Runner`, questions through an `Ask`, file changes through `Ctx` methods that
`--dry-run` turns into printed lines, and each change goes into a state file
for `--undo`.

**Tech Stack:** Rust 1.97.1, edition 2024, clap 4 (builder API), anyhow,
serde and serde_json, ureq for HTTPS, sha2, rpassword, windows-sys (Windows
only), tempfile (tests only). POSIX sh and Windows PowerShell 5.1 for the
installers.

**Spec:** `docs/superpowers/specs/2026-11-19-odd-design.md`

**Later plans:** plan 2 has `odd git`, `odd env`, `odd tweaks`,
`odd hostname`, `odd hosts`, `odd dns`. Plan 3 has `odd carry`,
`odd expose`, `odd share`, `odd wifi-pass`, `odd remote`, `odd firewall`.
Plan 4 has `odd font`, `odd devcert`, `odd autostart`, `odd cache-purge`,
`odd apps`, `odd dualboot-clock`, the Homebrew, Scoop and winget packages
(spec 6.4), and the first release. Each plan uses the
interfaces of this one.

## Global Constraints

- The rules of `AGENTS.md`: Simplified Technical English in code, comments,
  messages and docs; no em-dash; no emoji; one change in each commit; code and
  its tests in one commit; docs in a different commit; subject in the present
  tense, with no version number.
- Git identity: `Stiven Gjekaj <stivenagostingjekaj@gmail.com>`. No
  `Co-Authored-By`, no tool footer. Do not push.
- Toolchain `1.97.1`, edition `2024`, `rust-version = "1.97"`, as AllTheTools.
- Each dependency version is exact (`=x.y.z`). A new dependency or version is
  a commit of its own.
- Lints of AllTheTools: `unsafe_code = "deny"`, clippy `all` and
  `unwrap_used` as warnings, `-D warnings` in CI.
- Package `alltheoddities`, binary `odd`.
- Release files: `odd-<target>`, and `odd-<target>.exe` on Windows, plus
  `SHA256SUMS`. Six targets: `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`,
  `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`,
  `aarch64-pc-windows-msvc`.
- Version `YYYY.MM.N`. `odd --version` prints `odd <version> (<Codename>)`.
- Exit codes: 0 done, 1 a step failed, 2 usage or host not supported,
  3 needs an administrator and changed nothing.
- `odd` never raises its own rights.
- No test touches the real system: no real host command that changes
  anything, no file outside a temporary directory.
- No secret goes on a command line, into the state file, or into output.
- State file: `~/.local/state/odd/<oddity>.json` on macOS and Linux,
  `%LOCALAPPDATA%\odd\state\<oddity>.json` on Windows.
- Each new oddity has the label `experimental`.

## Review Focus

1. Run-once mode, `curl ... | sh -s -- ssh`: the standard input of `odd`
   is the pipe of `curl`, not the keyboard. `odd` must still ask its
   questions and read the auth key from the terminal. Test in Task 3 and
   Task 13.
2. A config file with CRLF line ends, or with no newline at the end (an
   `sshd_config` on Windows, a shell profile that an editor saved). The
   managed block keeps the line ends of the file and does not join a line.
   Test in Task 4.
3. A Windows host in a language other than English, where the group is not
   called "Administrators". The access list of
   `administrators_authorized_keys` uses the SIDs `*S-1-5-32-544` and
   `*S-1-5-18`, and never the names. Test in Task 10.
4. A second run after a run that failed in the middle. `odd ssh` does each
   missing step once, and the state file has no undo step twice. Test in
   Task 5 and Task 12.
5. A GitHub user with no public keys, or no network when `odd` reads the
   keys. The keys-only step does not run, and passwords stay on. Test in
   Task 11.

---

### Task 1: The workspace and `odd --version`

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`,
  `.cargo/config.toml`, `crates/alltheoddities/Cargo.toml`,
  `crates/alltheoddities/src/main.rs`, `crates/alltheoddities/src/lib.rs`,
  `crates/alltheoddities/src/version.rs`
- Modify: `.gitignore` (add `/target/`)

**Interfaces:**
- Produces: `pub fn run() -> i32` in `lib.rs`; `main.rs` is
  `std::process::exit(alltheoddities::run())`.
- Produces: `version::CODENAMES: [&str; 26]`, `version::CODENAME: &str`,
  `version::long() -> String`.

- [ ] **Step 1:** Copy `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`
  and `.cargo/config.toml` from AllTheTools as they are. Write the workspace
  `Cargo.toml` with the `[workspace.package]`, lints and release profile of
  AllTheTools, `version = "2026.11.0"`, the repository URL of AllTheOddities,
  and these dependencies: `anyhow = "=1.0.104"`,
  `clap = { version = "=4.6.6", features = ["cargo"] }`,
  `serde = { version = "=1.0.229", features = ["derive"] }`,
  `serde_json = "=1.0.151"`, `windows-sys = "=0.61.2"`. The crate
  `Cargo.toml` has `name = "alltheoddities"` and `[[bin]] name = "odd"`.

- [ ] **Step 2: Write the failing tests** in `version.rs`:

```rust
#[test]
fn the_codenames_go_from_a_to_z() {
    assert_eq!(CODENAMES.len(), 26);
    for (i, name) in CODENAMES.iter().enumerate() {
        assert_eq!(name.as_bytes()[0], b'A' + i as u8, "{name}");
    }
}

#[test]
fn the_long_version_names_the_codename() {
    assert_eq!(long(), format!("{} ({})", env!("CARGO_PKG_VERSION"), CODENAME));
    assert!(CODENAMES.contains(&CODENAME));
}
```

- [ ] **Step 3:** Run `cargo test -p alltheoddities version`. Expected: the
  build fails, because `CODENAMES` does not exist.

- [ ] **Step 4:** Add `CODENAMES` with the 26 names of spec section 8.2, in
  that order, `CODENAME = "Astrolabe"`, and `long()`. `run()` builds a
  `clap::Command` named `odd` with `.version(version::long())`.

- [ ] **Step 5:** Run `cargo test -p alltheoddities` and
  `cargo run -q -- --version`. Expected: PASS, and
  `odd 2026.11.0 (Astrolabe)`.

- [ ] **Step 6:** Run `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check`. Expected: no output. Commit
  `feat: add the workspace and the version of odd`.

### Task 2: The host, the runner and the context

**Files:**
- Create: `src/host.rs`, `src/run.rs`, `src/ctx.rs`, `src/exit.rs`,
  `src/testing.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces in `host.rs`:
  `pub enum Os { MacOs, Linux, Windows }`,
  `pub struct Host { pub os: Os, pub distro: Option<String> }` (`distro` is
  the `ID` of `/etc/os-release` on Linux),
  `Host::current() -> Host`, `pub fn is_admin() -> bool` (effective uid 0 on
  unix; membership of the Administrators SID with an elevated token on
  Windows, through windows-sys, in a shim that allows `unsafe_code` with its
  reason).
- Produces in `run.rs`:
  `pub struct Output { pub status: i32, pub stdout: String, pub stderr: String }`,
  `pub trait Runner { fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<Output>; fn on_path(&self, program: &str) -> bool; }`,
  `pub struct HostRunner;` (uses `std::process::Command`, stdin null; a
  missing program is an error that names it).
- Produces in `exit.rs`:
  `#[repr(i32)] pub enum Exit { Done = 0, Failed = 1, Usage = 2, NeedsAdmin = 3 }`.
- Produces in `ctx.rs`:

```rust
pub struct Ctx<'a> {
    pub host: Host,
    pub home: PathBuf,
    pub state_dir: PathBuf,
    pub env: HashMap<String, String>,
    pub is_admin: bool,
    pub runner: &'a dyn Runner,
    pub ask: &'a dyn Ask,
    pub http: &'a dyn Http,
    pub dry_run: bool,
    pub out: &'a mut dyn Write,
}
impl Ctx<'_> {
    /// A command that reads. It runs in dry-run too.
    pub fn query(&self, program: &str, args: &[&str]) -> Result<Output>;
    /// A command that changes the host. In dry-run it prints "would run: ..."
    /// and gives a status-0 Output with empty text.
    pub fn change(&mut self, program: &str, args: &[&str]) -> Result<Output>;
    /// Writes a whole file. In dry-run it prints "would write: <path>".
    pub fn write_file(&mut self, path: &Path, text: &str) -> Result<()>;
    pub fn say(&mut self, line: &str);
}
```

- Produces in `net.rs`:
  `pub trait Http { fn get(&self, url: &str) -> Result<String>; fn get_bytes(&self, url: &str) -> Result<Vec<u8>>; }`.
  Task 10 adds the real `HostHttp`. Until then, `run()` uses a `NoHttp` that
  gives the error "this build has no network".
- Produces in `ask.rs`: the `Ask` trait only (Task 3 adds the types).
- Produces in `testing.rs`: `FakeRunner` with
  `FakeRunner::new()`, `.answer(program, args_prefix, Output)` to queue an
  answer, `.calls() -> Vec<Vec<String>>`, and `.with_path(&[&str])` for
  `on_path`. An unexpected call panics with the program and its arguments.
  `FakeHttp` with `.page(url, text)` and `.bytes(url, Vec<u8>)`; an unknown
  URL gives an error.
  `fn test_ctx(os: Os) -> TestCtx`, which holds a `TempDir`, a `FakeRunner`,
  a `Scripted` ask, a `FakeHttp` and an empty `env`, and gives a `Ctx` with
  `home` and `state_dir` in the temporary directory and `is_admin = true`.

- [ ] **Step 1: Write the failing tests** in `ctx.rs`:
  `dry_run_prints_a_change_and_does_not_run_it` (a `FakeRunner` with no
  answers; `ctx.change("systemctl", &["start", "ssh"])` in dry-run gives
  status 0, `calls()` is empty, the output holds
  `would run: systemctl start ssh`);
  `a_query_runs_in_dry_run` (the runner gets the call);
  `dry_run_writes_no_file` (the path does not exist after `write_file`).
- [ ] **Step 2:** Run `cargo test -p alltheoddities ctx`. Expected: FAIL to
  compile.
- [ ] **Step 3:** Implement the files above.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the host, the runner and the context`.

### Task 3: Questions

**Files:**
- Create: `src/ask.rs`
- Modify: `Cargo.toml` (add `rpassword = "=<latest at build time>"`)

**Interfaces:**
- Produces:

```rust
pub trait Ask {
    fn yes_no(&self, question: &str, default: bool) -> bool;
    fn text(&self, question: &str, default: Option<&str>) -> Option<String>;
    /// Hidden input. Never echoed, never logged.
    fn secret(&self, question: &str) -> Option<String>;
}
pub struct Terminal;                 // reads stdin; secret uses rpassword
pub struct Defaults;                 // --yes: yes_no -> default, text -> default, secret -> None
#[cfg(test)] pub struct Scripted(RefCell<VecDeque<String>>); // answers in order; panics when empty
```

- [ ] **Step 1: Write the failing tests:** `defaults_accept_each_default`
  (`yes_no(_, false)` is false, `text(_, Some("x"))` is `Some("x")`,
  `secret` is `None`); `scripted_answers_in_order`.
- [ ] **Step 2:** Run them. Expected: FAIL to compile.
- [ ] **Step 3:** Implement. `Terminal::yes_no` prints `<question> [Y/n] `
  or `[y/N]`, and an empty line gives the default. When stdin is not a
  terminal, `Terminal` reads from `/dev/tty` on unix and from `CONIN$` on
  Windows. Only when it cannot open those, it behaves as `Defaults` and says
  once: "No terminal: odd takes the default of each question."
  Add the test `a_pipe_on_stdin_still_reads_the_terminal` as a function
  `open_input(stdin_is_terminal: bool) -> Source` that gives `Source::Stdin`,
  `Source::Tty` or `Source::None`, and test its three cases.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the questions that an oddity asks`.

### Task 4: The managed block

**Files:**
- Create: `src/block.rs`

**Interfaces:**
- Produces:

```rust
/// Puts `body` between the lines "# >>> odd <name> >>>" and
/// "# <<< odd <name> <<<". Replaces the block if it exists, else appends it.
pub fn upsert(text: &str, name: &str, body: &str, comment: &str) -> String;
/// Inserts the block before the first line that `before` matches, or appends it.
pub fn upsert_before(text: &str, name: &str, body: &str, comment: &str, before: &dyn Fn(&str) -> bool) -> String;
pub fn remove(text: &str, name: &str, comment: &str) -> String;
pub fn contains(text: &str, name: &str, comment: &str) -> bool;
```

`comment` is the comment prefix of the file: `#` for shell and SSH files.

- [ ] **Step 1: Write the failing tests:**
  - `appends_to_a_file_with_no_final_newline`:
    `upsert("Host old\n    HostName x", "github", "Host github.com", "#")`
    keeps `"    HostName x\n"` as one whole line, then a blank line, then
    the block.
  - `keeps_crlf`: input with `\r\n` gives output where each line ends in
    `\r\n`, and no bare `\n`.
  - `replaces_and_does_not_repeat`: two `upsert` calls with other bodies give
    one block, with the second body.
  - `remove_gives_back_the_text_before`: `remove(upsert(t, ...))` equals `t`
    for `t` in `["", "a\n", "a", "a\r\nb\r\n"]`.
  - `inserts_before_the_first_match`: with
    `before = |l| l.trim_start().starts_with("Match ")`, the block comes
    before `Match Group administrators`.
- [ ] **Step 2:** Run them. Expected: FAIL to compile.
- [ ] **Step 3:** Implement. Find the line end of the file from its first
  line end; default `\n`.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the managed block of a text file`.

### Task 5: The record of changes, and undo

**Files:**
- Create: `src/state.rs`

**Interfaces:**
- Consumes: `Ctx` (Task 2), `block::remove` (Task 4).
- Produces:

```rust
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Undo {
    RemoveBlock { path: PathBuf, name: String, comment: String },
    RestoreFile { path: PathBuf, before: Option<String>, after_sha256: String },
    Run { program: String, args: Vec<String> },
    /// Calls `Oddity::undo_call(action, arg)` of the oddity that recorded it.
    Call { action: String, arg: String },
}
pub fn record(ctx: &mut Ctx, oddity: &str, step: Undo) -> Result<()>;
pub struct UndoReport { pub undone: Vec<String>, pub skipped: Vec<String> }
pub fn undo(ctx: &mut Ctx, oddity: &str) -> Result<UndoReport>;
```

`record` adds the step only if the same step is not in the file. In dry-run
it records nothing. `undo` runs the steps newest first. `RestoreFile` skips
a file whose SHA-256 is no longer `after_sha256`, and names it in `skipped`.
After `undo`, the state file of that oddity is gone, unless `undo` skipped a
step; then it keeps the skipped steps.

- [ ] **Step 1:** Add `sha2 = "=<latest at build time>"` to the workspace.
- [ ] **Step 2: Write the failing tests:** `the_same_step_twice_is_recorded_once`;
  `undo_runs_newest_first` (two `Run` steps; `FakeRunner::calls()` is in
  reverse order); `undo_skips_a_file_that_changed_again`;
  `undo_puts_back_a_file_that_did_not_exist` (`before: None` deletes it);
  `dry_run_records_nothing`.
- [ ] **Step 3:** Run them. Expected: FAIL to compile.
- [ ] **Step 4:** Implement.
- [ ] **Step 5:** Run them. Expected: PASS.
- [ ] **Step 6:** Commit `feat: add the record of changes and undo`.

### Task 6: The Oddity trait, the command line and `odd list`

**Files:**
- Create: `src/oddity.rs`, `src/cli.rs`, `src/oddities/mod.rs`,
  `src/oddities/list.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: Tasks 2 to 5.
- Produces:

```rust
pub enum Label { Experimental, Stable, Deprecated }
pub trait Oddity: Sync {
    fn name(&self) -> &'static str;
    fn summary(&self) -> &'static str;
    fn hosts(&self) -> &'static [Os];
    fn label(&self) -> Label { Label::Experimental }
    /// The arguments of the oddity. cli.rs adds --yes, --dry-run and --undo.
    fn args(&self, cmd: clap::Command) -> clap::Command { cmd }
    fn can_undo(&self) -> bool { true }
    /// True when this run needs an administrator. Checked before `run`.
    fn needs_admin(&self, m: &clap::ArgMatches, ctx: &Ctx) -> bool { false }
    fn run(&self, m: &clap::ArgMatches, ctx: &mut Ctx) -> anyhow::Result<Exit>;
    /// Reverts one `Undo::Call` step.
    fn undo_call(&self, action: &str, arg: &str, ctx: &mut Ctx) -> anyhow::Result<()> {
        anyhow::bail!("odd {} has no undo step named {action}", self.name())
    }
}
pub static ODDITIES: &[&dyn Oddity] = &[ /* one line for each oddity */ ];
pub fn dispatch(argv: Vec<OsString>, ctx: &mut Ctx) -> Exit;
```

`dispatch` handles, in this order: a host not in `hosts()` gives
`odd <name> does not run on <macOS|Linux|Windows> yet.` and `Exit::Usage`;
`--undo` on an oddity with `can_undo() == false` prints
`odd <name> changes nothing, so it has nothing to undo.` and gives
`Exit::Usage`; else it calls `state::undo` and prints the report; `needs_admin` true and
`host::is_admin()` false gives `Exit::NeedsAdmin` and the line
`Run again with: sudo odd <args>` on unix, or
`Run again in a PowerShell that runs as administrator: odd <args>` on
Windows; else `run`. An `Err` prints `odd <name>: <error>` and gives
`Exit::Failed`. The admin check reads `ctx.is_admin`, which `run()` sets
from `host::is_admin()`, so a test can set it.

`odd list` prints one row for each oddity: name, label, hosts, summary.
`--json` prints an array of objects with the fields `name`, `label`,
`hosts`, `summary`.

- [ ] **Step 1: Write the failing tests** with a test oddity in `oddity.rs`:
  `an_oddity_on_another_host_exits_2`; `needs_admin_exits_3_and_runs_nothing`
  (the `FakeRunner` has no calls); `undo_calls_the_state`;
  `list_json_has_each_oddity` (parse the output, check the four fields).
- [ ] **Step 2:** Run them. Expected: FAIL to compile.
- [ ] **Step 3:** Implement. `lib::run()` builds a real `Ctx` with
  `HostRunner`, `Terminal` or `Defaults` (`--yes`), the home directory from
  `HOME` or `USERPROFILE`, the state directory of the Global Constraints, and
  stdout.
- [ ] **Step 4:** Run the tests, then `cargo run -q -- list`. Expected: PASS,
  and the header row only. `list` is a command of `odd` and not an oddity, so
  the table stays empty until Task 7.
- [ ] **Step 5:** Commit `feat: add the oddity trait and odd list`.

### Task 7: `odd path`

**Files:**
- Create: `src/oddities/path.rs`, `src/win/env.rs` (Windows only)
- Modify: `src/oddity.rs` (one line in `ODDITIES`)

**Interfaces:**
- Consumes: `block::upsert/remove`, `state::record`, `Ctx`.
- Produces: `odd path add <dir>`, `odd path list`, `odd path --undo`.
  `pub fn same_entry(a: &str, b: &str, os: Os) -> bool` (Task 13 and plan 2
  use it).
- Produces in `win/env.rs`: `read_user(name) -> Option<String>`,
  `write_user(name, value)`, `broadcast_change()` (`SendMessageTimeoutW` with
  `HWND_BROADCAST`, `WM_SETTINGCHANGE`, `"Environment"`), through windows-sys
  with `Win32_System_Registry` and `Win32_UI_WindowsAndMessaging`. The
  registry value type stays `REG_EXPAND_SZ`.

Behaviour (spec 7.2): macOS and Linux write a block named `path` to
`~/.profile`, and also to `~/.zprofile` when `$SHELL` ends in `zsh`. Each
`add` keeps one block with each directory that `odd` added, one
`export PATH="<dir>:$PATH"` line each. With `$SHELL` ending in `fish`, it runs
`fish -c 'fish_add_path -U <dir>'`, and records
`Run { "fish", ["-c", "set -U fish_user_paths (string match -v -- <dir> $fish_user_paths)"] }`.
Windows adds the directory to the user `Path` and records
`Call { action: "remove-entry", arg: <dir> }`; `undo_call` removes each entry
for which `same_entry` is true, and broadcasts the change. `list`
prints each entry of the current PATH with `missing` or `repeat` beside it.

- [ ] **Step 1: Write the failing tests:**
  - `same_entry_ignores_a_trailing_slash` (`"/a/b/"`, `"/a/b"`, unix: true);
  - `same_entry_ignores_case_on_windows_only`
    (`r"C:\Tools"`, `r"c:\tools\"`: true on Windows, false on Linux);
  - `same_entry_keeps_spaces` (`"/a b"`, `"/a  b"`: false);
  - `add_twice_writes_one_line` (zsh: `~/.zprofile` has the dir once);
  - `add_of_a_dir_already_on_path_changes_nothing` (PATH in `Ctx` holds it
    with a trailing slash; no file is written);
  - `undo_removes_the_block`.
  The PATH comes from `ctx.env`, so the test sets it.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them. Expected: PASS. Run
  `cargo run -q -- path add /tmp/x --dry-run`. Expected: `would write:` with
  the path of the profile.
- [ ] **Step 5:** Commit `feat: add odd path`.

### Task 8: `odd ssh`, the "out" part

**Files:**
- Create: `src/oddities/ssh/mod.rs`, `src/oddities/ssh/out.rs`

**Interfaces:**
- Consumes: `Ctx`, `block`, `state`.
- Produces: `odd ssh` with flags `--out`, `--in`, `--tailscale`,
  `--tailscale-ssh`, `--keep-passwords`, `--comment <text>`,
  `--key <path>`, `--no-passphrase`, `--github-user <name>`. With none of
  `--out`, `--in`, `--tailscale`, it asks one question for each part, in that
  order (spec 7.1). `pub fn key_path(m, ctx) -> PathBuf` (default
  `<home>/.ssh/id_ed25519`).

Steps of "out", as `ssh/new-machine.sh` and `ssh/new-machine.ps1` do them
now: the directory (`chmod 700` through `ctx.change` on unix); the key with
`ssh-keygen -q -t ed25519 -C <comment> -f <key>` and `-N ""` only with
`--no-passphrase` (an empty argument goes as an empty `&str`; Rust passes it
correctly on Windows); a block named `github` in `~/.ssh/config` with the
lines of the current scripts, `UseKeychain yes` on macOS only, the key path
with forward slashes; the agent (`ssh-add --apple-use-keychain` on macOS,
`ssh-add` if `SSH_AUTH_SOCK` is in `ctx.env` on Linux, the `ssh-agent`
service on Windows: `sc query ssh-agent`, and if admin
`sc config ssh-agent start= auto` then `sc start ssh-agent`); Git on Windows
(`git config --global core.sshCommand <path of ssh.exe with />` if
`git config --global core.sshCommand` gives nothing); `gh ssh-key add` if
`gh auth status` gives 0, else print the key and
`https://github.com/settings/ssh/new`; the test with
`ssh -o StrictHostKeyChecking=accept-new -T git@github.com`, where the text
`successfully authenticated` means success. Record undo only for the config
block and the Git setting. The key is never deleted.

- [ ] **Step 1: Write the failing tests**, ported from
  `ssh/test-new-machine.sh` and `.ps1`, each with a `FakeRunner`:
  `out_makes_the_key_with_the_comment` (the call has `-C test@test`);
  `out_keeps_an_existing_key` (a file at the key path; no `ssh-keygen` call);
  `out_keeps_the_old_config_entry` (config `"Host old\n    HostName old.example"`);
  `out_twice_gives_one_block`; `use_keychain_on_macos_only`;
  `windows_sets_git_ssh_only_when_unset`;
  `the_test_reads_success_from_exit_1` (ssh gives status 1 and the GitHub
  text; the oddity gives `Exit::Done`).
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the out part of odd ssh`.

### Task 9: `odd ssh`, the server

**Files:**
- Create: `src/oddities/ssh/server.rs`

**Interfaces:**
- Consumes: Task 8's `odd ssh` arguments.
- Produces: `pub fn enable(ctx: &mut Ctx) -> Result<()>`,
  `pub fn config_path(os: Os) -> PathBuf` (`/etc/ssh/sshd_config`, or
  `C:\ProgramData\ssh\sshd_config`), `pub fn restart(ctx: &mut Ctx) -> Result<()>`.
  `needs_admin` of `odd ssh` is true when the in part or the Tailscale part
  will run.

Steps by host (spec 7.1, the "In" table). macOS:
`systemsetup -getremotelogin`, then `systemsetup -setremotelogin on`; if
the output holds `Full Disk Access`, stop with the steps
"System Settings > General > Sharing > Remote Login" and `Exit::Failed`.
Linux: the package manager from `host.distro` (`apt-get install -y
openssh-server` for debian, ubuntu and their family; `dnf install -y
openssh-server` for fedora and rhel; `pacman -S --noconfirm openssh` for
arch; `zypper install -y openssh` for opensuse; other: stop with a message),
then `systemctl enable --now ssh`, or `sshd` when `systemctl cat ssh` fails.
Windows: `powershell -NoProfile -Command "Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0"`
if `Get-WindowsCapability` says it is not installed, `sc config sshd start= auto`,
`sc start sshd`, and the firewall rule `OpenSSH-Server-In-TCP` with
`netsh advfirewall firewall add rule name=OpenSSH-Server-In-TCP dir=in action=allow protocol=TCP localport=22`
if `netsh advfirewall firewall show rule name=OpenSSH-Server-In-TCP` fails.
Record a `Run` undo for each part that `odd` turned on, and nothing for a
part that was on.

- [ ] **Step 1: Write the failing tests:** `macos_turns_on_remote_login`;
  `macos_full_disk_access_gives_the_settings_steps`;
  `linux_picks_the_package_manager` (one case for each distro family above);
  `linux_uses_sshd_when_ssh_is_not_a_unit`;
  `windows_adds_the_firewall_rule_only_when_missing`;
  `a_server_that_runs_already_records_no_undo`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: turn on the SSH server in odd ssh`.

### Task 10: `odd ssh`, the authorized keys

**Files:**
- Create: `src/oddities/ssh/keys.rs`, `src/net.rs`
- Modify: `Cargo.toml` (add `ureq = "=<latest at build time>"` with rustls)

**Interfaces:**
- Produces in `net.rs`: `pub struct HostHttp;`, which implements the `Http`
  trait of Task 2 with ureq. `run()` uses it in place of `NoHttp`.
- Produces in `keys.rs`: `pub fn add_from_github(ctx, user) -> Result<usize>`
  (gives the number of keys in the file after the step),
  `pub fn authorized_keys_path(ctx) -> PathBuf`.

`authorized_keys_path` is `<home>/.ssh/authorized_keys`, except on Windows
when the user is an administrator: `C:\ProgramData\ssh\administrators_authorized_keys`.
`odd` reads `https://github.com/<user>.keys` and adds each line that is not in
the file. Record `RestoreFile`. On Windows, for the administrators file, run
`icacls <file> /inheritance:r /grant *S-1-5-32-544:F /grant *S-1-5-18:F`.

- [ ] **Step 1: Write the failing tests:** `adds_only_new_keys`;
  `gives_the_count_after_the_step`;
  `windows_admin_uses_the_programdata_file`;
  `windows_acl_uses_sids_not_names` (the `icacls` call holds
  `*S-1-5-32-544:F` and `*S-1-5-18:F`, and not `Administrators`);
  `a_user_with_no_keys_adds_nothing`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the GitHub keys of a user in odd ssh`.

### Task 11: `odd ssh`, keys only

**Files:**
- Create: `src/oddities/ssh/keys_only.rs`

**Interfaces:**
- Consumes: `server::config_path`, `server::restart`, `keys::add_from_github`
  result, `block::upsert_before`.

The order of spec 7.1 "Keys only": stop if the count is 0, or if
`add_from_github` gave an error (say "Passwords stay on: <reason>");
write `PasswordAuthentication no` and `KbdInteractiveAuthentication no` to
`/etc/ssh/sshd_config.d/50-odd.conf` when `sshd_config` has an uncommented
`Include /etc/ssh/sshd_config.d/*.conf` line, else in a block named
`keys-only` before the first `Match` line; `sshd -t` (on Windows the full
path `C:\Windows\System32\OpenSSH\sshd.exe -t`); on failure put back the old
text and stop with `Exit::Failed`; `restart`; print
`Test a key login from a second window before you close this one.`
`--keep-passwords` skips the whole part.

- [ ] **Step 1: Write the failing tests:**
  `no_keys_keeps_passwords_on` (nothing written, `sshd -t` not called);
  `a_key_fetch_error_keeps_passwords_on`;
  `uses_the_drop_in_when_the_config_includes_it`;
  `goes_before_match_when_there_is_no_include`;
  `a_failed_check_puts_back_the_config` (`sshd -t` gives status 1; the file
  equals the text before; no restart call);
  `keep_passwords_skips_it`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: make the SSH server accept keys only in odd ssh`.

### Task 12: `odd ssh`, Tailscale, and the whole run

**Files:**
- Create: `src/oddities/ssh/tailscale.rs`
- Modify: `src/oddities/ssh/mod.rs` (the order of the parts, and undo)

Steps (spec 7.1 "Tailscale"): if `tailscale` is not on PATH, ask, then
install: Linux `sh -c "curl -fsSL https://tailscale.com/install.sh | sh"`;
macOS: download `https://pkgs.tailscale.com/stable/Tailscale-latest-macos.pkg`
with `ctx.http.get_bytes` and run `installer -pkg <file> -target /`;
Windows: `winget install --id Tailscale.Tailscale -e --accept-source-agreements --accept-package-agreements`.
Then the key: `--yes` reads `TS_AUTHKEY` from `ctx.env`, else `ask.secret`
with the text "Paste an ephemeral, single-use auth key from
https://login.tailscale.com/admin/settings/keys". No key: skip the part and
say so. Write the key to a file in a new temporary directory with mode 600
(on Windows, `icacls` to the current user only), run
`tailscale up --auth-key=file:<path>` plus `--ssh` with `--tailscale-ssh` on
macOS and Linux, and delete the file and the directory in a `Drop` guard, so
an error deletes it too. Then `tailscale status --json`, and print the
`Self.DNSName` without the final dot, the first `TailscaleIPs` entry, and
`ssh <user>@<name>`. Record `Run { tailscale, [logout] }`.

`--undo` of `odd ssh` runs the steps of Tasks 8 to 12 in reverse, through
`state::undo`, and keeps the key pair.

- [ ] **Step 0:** Confirm two facts in the Tailscale documentation, and stop
  and tell the owner if one is false: `tailscale up` takes
  `--auth-key=file:<path>`, and the macOS package URL above gives the latest
  package.
- [ ] **Step 1: Write the failing tests:**
  `the_key_is_never_an_argument` (no call has the key text in any argument;
  one argument starts with `--auth-key=file:`);
  `the_key_file_is_gone_after_an_error` (`tailscale up` gives status 1; the
  temporary directory does not exist after);
  `yes_reads_ts_authkey`; `no_key_skips_tailscale`;
  `tailscale_ssh_is_not_offered_on_windows`;
  `a_second_run_records_no_step_twice` (run `odd ssh --out --in` twice with
  the same fakes; the state file has each step once);
  `undo_keeps_the_key_pair`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run all tests, then `cargo run -q -- ssh --out --dry-run`.
  Expected: PASS, and `would run:` lines with no change on this computer.
- [ ] **Step 5:** Commit `feat: join Tailscale with an ephemeral key in odd ssh`.
- [ ] **Step 6:** Delete `ssh/new-machine.sh`, `ssh/new-machine.ps1` and
  their tests, and the jobs of `.github/workflows/ci.yml` that run them.
  Commit `refactor: remove the SSH scripts that odd ssh replaces`.

### Task 13: The installers

**Files:**
- Create: `install.sh`, `install.ps1`, `tests/install.sh`, `tests/install.ps1`

Behaviour (spec 6.1, 6.2): the base URL is
`https://github.com/Stiven-Gjekaj/AllTheOddities/releases/latest/download`,
or `.../releases/download/<ODD_VERSION>` when `ODD_VERSION` is set, or
`ODD_BASE_URL` when set (for the tests only; the README does not name it).
The target from `uname -s` and `uname -m` (`x86_64`/`amd64` to `x86_64`,
`arm64`/`aarch64` to `aarch64`), or `$env:PROCESSOR_ARCHITECTURE`
(`AMD64`, `ARM64`). `curl -fsSL`, else `wget -qO`. The checksum with
`sha256sum`, else `shasum -a 256`, or `Get-FileHash -Algorithm SHA256`,
against the line of `SHA256SUMS` for that file name. A mismatch prints
`The checksum of <file> does not match SHA256SUMS. Nothing is installed.`
and exits 1. With no arguments: copy to `~/.local/bin/odd` or
`%LOCALAPPDATA%\Programs\odd\odd.exe`, then run `odd path add <dir>`. With
arguments: run `odd <args>` from the temporary directory with
`< /dev/tty` when `/dev/tty` can be read (stdin is the pipe of `curl`), delete
the directory, and exit with the exit code of `odd`. `install.ps1` takes its arguments as a script
block parameter (`param([Parameter(ValueFromRemainingArguments)]$Args)`), and
runs in Windows PowerShell 5.1.

- [ ] **Step 1: Write the failing tests.** Each test makes a fake release
  directory with a small script named `odd-<target>` that prints its
  arguments, and its `SHA256SUMS`, serves it with
  `python3 -m http.server` on a free port, and sets `ODD_BASE_URL` and a
  temporary `HOME` / `LOCALAPPDATA`. Cases: `installs_and_adds_to_path`;
  `a_bad_checksum_installs_nothing` (exit 1, no file at the install path);
  `arguments_run_once_and_leave_nothing` (the output holds the arguments;
  the install path does not exist).
- [ ] **Step 2:** Run `sh tests/install.sh` and
  `pwsh -File tests/install.ps1`. Expected: FAIL, the installers do not exist.
- [ ] **Step 3:** Write the installers. Keep each under 100 lines.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the one-line installers`.

### Task 14: `odd update`

**Files:**
- Create: `src/oddities/update.rs`

`odd update` is a command of `odd`, like `list`. It reads
`https://api.github.com/repos/Stiven-Gjekaj/AllTheOddities/releases/latest`
through `ctx.http`, takes `tag_name`, and stops with
`odd <version> is the latest.` when it equals `v` + the running version.
Else it downloads `odd-<target>` and `SHA256SUMS`, checks the SHA-256 with
`sha2`, writes the new binary beside the running one, and replaces it: on
unix, a rename over the running file; on Windows, rename the running file to
`odd.old.exe`, then rename the new one into place. `lib::run()` deletes
`odd.old.exe` beside the binary on each start, and ignores an error.
It uses `ctx.http.get_bytes` for the files.

- [ ] **Step 1: Write the failing tests:** `the_same_version_changes_nothing`;
  `a_bad_checksum_changes_nothing` (the binary in a temporary directory has
  the same bytes after); `a_new_version_replaces_the_binary` (with a path
  argument for the binary, so the test does not replace itself).
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement. The target string is a `const` set from
  `env!("TARGET")` through a one-line `build.rs`.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add odd update`.

### Task 15: CI and the release workflow

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`, `scripts/smoke.sh`

Copy `ci.yml` and `release.yml` of AllTheTools, with each action pinned to the
same commit. Changes: package `alltheoddities`, binary `odd`; no Debian or
RPM step; the collect step names each file `odd-<target>` (`.exe` on
Windows); `SHA256SUMS` over those six files; the tag check compares the tag
with `v` + the version in `Cargo.toml`; the release title is
`<YYYY.MM> "<Codename>"`, read with `odd --version` from the Linux x64
binary. Add to `ci.yml` the jobs that run `tests/install.sh` on Linux and
macOS, and `tests/install.ps1` on Windows with `shell: powershell` and
`shell: pwsh`. `scripts/smoke.sh <odd>` runs `odd list`, `odd --version`,
`odd path list`, and `odd ssh --out --dry-run --yes`, and fails on a
non-zero exit.

- [ ] **Step 1:** Write the files.
- [ ] **Step 2:** Run `sh scripts/smoke.sh target/debug/odd`. Expected: each
  command exits 0.
- [ ] **Step 3:** Run `release.yml` by hand later, after the owner creates
  the GitHub repository. This plan does not push. Record in the commit body
  that the workflows have not run yet.
- [ ] **Step 4:** Commit `ci: build, test and release odd on each host`.

### Task 16: The documentation

Each step is one commit, in this order:

- [ ] **Step 1:** Rewrite the README in the style of AllTheTools for `odd`:
  install with the two one-liners, `odd list`, the table of oddities with
  labels, `odd ssh`, the exit codes. Commit
  `docs: describe odd and its installers in the README`.
- [ ] **Step 2:** Rewrite `docs/oddities.md` for `odd path` and `odd ssh`,
  each host, each step, what undo does. Commit
  `docs: describe odd path and odd ssh`.
- [ ] **Step 3:** Add `docs/releasing.md` (the version, the codename list,
  the labels, how to cut a release) and `docs/limits.md` (the facts of spec
  section 10, and what each host cannot do). Commit
  `docs: add the release rules and the limits`.
- [ ] **Step 4:** Add `CHANGELOG.md` with an "Unreleased" part that lists
  `odd path` and `odd ssh` as New. Commit `docs: add the changelog`.
