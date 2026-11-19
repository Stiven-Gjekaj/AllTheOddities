//! Fakes of the host, for the tests. No test touches the real system.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::ask::Ask;
use crate::ctx::Ctx;
use crate::host::{Host, Os};
use crate::net::Http;
use crate::run::{Output, Runner};

/// A runner that records each call and gives prepared answers.
#[derive(Default)]
pub struct FakeRunner {
    answers: RefCell<Vec<(String, Vec<String>, Output)>>,
    calls: RefCell<Vec<Vec<String>>>,
    path: RefCell<Vec<String>>,
}

impl FakeRunner {
    /// The answer to each call of `program` whose arguments start with
    /// `prefix`. When several answers match, the longest prefix wins, and
    /// then the newest answer. So a test can change one answer of a shared
    /// set of answers.
    pub fn answer(&self, program: &str, prefix: &[&str], out: Output) {
        let prefix = prefix.iter().map(|s| (*s).to_owned()).collect();
        self.answers
            .borrow_mut()
            .push((program.to_owned(), prefix, out));
    }

    pub fn with_path(&self, programs: &[&str]) {
        self.path
            .borrow_mut()
            .extend(programs.iter().map(|s| (*s).to_owned()));
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        let mut call = vec![program.to_owned()];
        call.extend(args.iter().map(|s| (*s).to_owned()));
        self.calls.borrow_mut().push(call.clone());
        let answers = self.answers.borrow();
        let best = answers
            .iter()
            .enumerate()
            .filter(|(_, (p, prefix, _))| {
                p == program
                    && prefix.len() <= args.len()
                    && prefix.iter().zip(args).all(|(a, b)| a == b)
            })
            .max_by_key(|(i, (_, prefix, _))| (prefix.len(), *i));
        match best {
            Some((_, (_, _, out))) => Ok(out.clone()),
            None => panic!("unexpected call: {}", call.join(" ")),
        }
    }

    fn interactive(&self, program: &str, args: &[&str]) -> Result<i32> {
        Ok(self.run(program, args)?.status)
    }

    fn on_path(&self, program: &str) -> bool {
        self.path.borrow().iter().any(|p| p == program)
    }
}

/// Answers in the order given. A question with no answer left panics.
#[derive(Default)]
pub struct Scripted(pub RefCell<VecDeque<String>>);

impl Scripted {
    pub fn push(&self, answer: &str) {
        self.0.borrow_mut().push_back(answer.to_owned());
    }
    fn next(&self, question: &str) -> String {
        self.0
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| panic!("no answer for: {question}"))
    }
}

impl Ask for Scripted {
    fn yes_no(&self, question: &str, _default: bool) -> bool {
        matches!(self.next(question).as_str(), "y" | "yes")
    }
    fn text(&self, question: &str, _default: Option<&str>) -> Option<String> {
        Some(self.next(question)).filter(|s| !s.is_empty())
    }
    fn secret(&self, question: &str) -> Option<String> {
        self.text(question, None)
    }
}

/// Pages and files at fixed URLs. Another URL gives an error.
#[derive(Default)]
pub struct FakeHttp {
    pages: RefCell<HashMap<String, Vec<u8>>>,
    pub fail: RefCell<bool>,
}

impl FakeHttp {
    pub fn page(&self, url: &str, text: &str) {
        self.pages
            .borrow_mut()
            .insert(url.to_owned(), text.as_bytes().to_vec());
    }
    pub fn bytes(&self, url: &str, bytes: Vec<u8>) {
        self.pages.borrow_mut().insert(url.to_owned(), bytes);
    }
}

impl Http for FakeHttp {
    fn get(&self, url: &str) -> Result<String> {
        Ok(String::from_utf8(self.get_bytes(url)?)?)
    }
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        if *self.fail.borrow() {
            bail!("no network");
        }
        match self.pages.borrow().get(url) {
            Some(b) => Ok(b.clone()),
            None => bail!("no page at {url}"),
        }
    }
}

/// The fakes of one test, and a temporary home directory.
pub struct TestCtx {
    pub dir: tempfile::TempDir,
    pub os: Os,
    pub distro: Option<String>,
    pub runner: FakeRunner,
    pub ask: Scripted,
    pub http: FakeHttp,
    pub env: HashMap<String, String>,
    pub is_admin: bool,
    pub dry_run: bool,
    pub out: Vec<u8>,
}

impl TestCtx {
    pub fn new(os: Os) -> TestCtx {
        TestCtx {
            dir: tempfile::tempdir().unwrap(),
            os,
            distro: None,
            runner: FakeRunner::default(),
            ask: Scripted::default(),
            http: FakeHttp::default(),
            env: HashMap::new(),
            is_admin: true,
            dry_run: false,
            out: Vec::new(),
        }
    }

    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.dir.path().join("state")
    }

    /// A path inside the temporary directory, for a file of the system.
    pub fn sys(&self, path: &str) -> PathBuf {
        self.dir
            .path()
            .join("sys")
            .join(Path::new(path.trim_start_matches(['/', '\\'])))
    }

    pub fn ctx(&mut self) -> Ctx<'_> {
        Ctx {
            host: Host {
                os: self.os,
                distro: self.distro.clone(),
            },
            home: self.home(),
            state_dir: self.state_dir(),
            env: self.env.clone(),
            root: Some(self.dir.path().join("sys")),
            is_admin: self.is_admin,
            runner: &self.runner,
            ask: &self.ask,
            http: &self.http,
            dry_run: self.dry_run,
            out: &mut self.out,
        }
    }

    pub fn output(&self) -> String {
        String::from_utf8_lossy(&self.out).into_owned()
    }
}
