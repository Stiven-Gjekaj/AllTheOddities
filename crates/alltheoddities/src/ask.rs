//! The questions that an oddity asks the person.

/// Asks the person. The tests give scripted answers.
pub trait Ask {
    fn yes_no(&self, question: &str, default: bool) -> bool;
    fn text(&self, question: &str, default: Option<&str>) -> Option<String>;
    /// Hidden input. Never echoed, never logged.
    fn secret(&self, question: &str) -> Option<String>;
}

/// `--yes`: each question takes its default, and a secret has none.
pub struct Defaults;

impl Ask for Defaults {
    fn yes_no(&self, _question: &str, default: bool) -> bool {
        default
    }
    fn text(&self, _question: &str, default: Option<&str>) -> Option<String> {
        default.map(str::to_owned)
    }
    fn secret(&self, _question: &str) -> Option<String> {
        None
    }
}

/// Where the answers come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The standard input is the keyboard.
    Stdin,
    /// The standard input is a pipe, as in `curl ... | sh`, so read the
    /// terminal itself: `/dev/tty`, or `CONIN$` on Windows.
    Tty,
    /// No terminal at all. Each question takes its default.
    None,
}

/// Picks the source of the answers.
pub fn pick_source(stdin_is_terminal: bool, tty_opens: bool) -> Source {
    match (stdin_is_terminal, tty_opens) {
        (true, _) => Source::Stdin,
        (false, true) => Source::Tty,
        (false, false) => Source::None,
    }
}

#[cfg(unix)]
const TTY: &str = "/dev/tty";
#[cfg(windows)]
const TTY: &str = "CONIN$";

/// The questions of a real run. The questions go to the error stream, so
/// the output stream stays clean for `--json`.
pub struct Terminal {
    source: Source,
}

impl Terminal {
    /// Gives a terminal, or [`Defaults`] when the host has no terminal.
    pub fn open() -> Box<dyn Ask> {
        use std::io::IsTerminal;
        let tty_opens = std::fs::File::open(TTY).is_ok();
        match pick_source(std::io::stdin().is_terminal(), tty_opens) {
            Source::None => {
                eprintln!("No terminal: odd takes the default of each question.");
                Box::new(Defaults)
            }
            source => Box::new(Terminal { source }),
        }
    }

    fn line(&self, prompt: &str) -> Option<String> {
        use std::io::{BufRead, Write};
        eprint!("{prompt}");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        let read = match self.source {
            Source::Tty => std::fs::File::open(TTY)
                .ok()
                .map(|f| std::io::BufReader::new(f).read_line(&mut line)),
            _ => Some(std::io::stdin().lock().read_line(&mut line)),
        };
        match read {
            Some(Ok(n)) if n > 0 => Some(line.trim().to_owned()),
            _ => None,
        }
    }
}

impl Ask for Terminal {
    fn yes_no(&self, question: &str, default: bool) -> bool {
        let hint = if default { "[Y/n]" } else { "[y/N]" };
        match self.line(&format!("{question} {hint} ")).as_deref() {
            Some("y" | "Y" | "yes" | "Yes") => true,
            Some("n" | "N" | "no" | "No") => false,
            _ => default,
        }
    }

    fn text(&self, question: &str, default: Option<&str>) -> Option<String> {
        let prompt = match default {
            Some(d) => format!("{question} [{d}] "),
            None => format!("{question} "),
        };
        match self.line(&prompt) {
            Some(answer) if !answer.is_empty() => Some(answer),
            _ => default.map(str::to_owned),
        }
    }

    fn secret(&self, question: &str) -> Option<String> {
        // rpassword reads the terminal itself, so a pipe on stdin is no problem.
        rpassword::prompt_password(format!("{question} "))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Scripted;

    #[test]
    fn defaults_accept_each_default() {
        assert!(!Defaults.yes_no("q", false));
        assert!(Defaults.yes_no("q", true));
        assert_eq!(Defaults.text("q", Some("x")), Some("x".to_owned()));
        assert_eq!(Defaults.text("q", None), None);
        assert_eq!(Defaults.secret("q"), None);
    }

    #[test]
    fn scripted_answers_in_order() {
        let s = Scripted::default();
        s.push("y");
        s.push("name");
        assert!(s.yes_no("a", false));
        assert_eq!(s.text("b", None), Some("name".to_owned()));
    }

    #[test]
    fn a_terminal_on_stdin_reads_stdin() {
        assert_eq!(pick_source(true, true), Source::Stdin);
    }

    #[test]
    fn a_pipe_on_stdin_still_reads_the_terminal() {
        assert_eq!(pick_source(false, true), Source::Tty);
    }

    #[test]
    fn no_terminal_at_all_takes_the_defaults() {
        assert_eq!(pick_source(false, false), Source::None);
    }
}
