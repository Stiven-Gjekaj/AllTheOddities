//! A block of lines that `odd` owns inside a text file of the person.
//!
//! The block starts with `# >>> odd <name> >>>` and ends with
//! `# <<< odd <name> <<<`. `odd` changes only the lines between, so it never
//! touches a line that the person wrote. It keeps the line ends of the file.

/// The lines of a text, its line end, and whether it ends with a line end.
struct Lines {
    lines: Vec<String>,
    eol: &'static str,
    final_eol: bool,
}

impl Lines {
    fn parse(text: &str) -> Lines {
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        if text.is_empty() {
            return Lines {
                lines: Vec::new(),
                eol,
                final_eol: true,
            };
        }
        let mut lines: Vec<String> = text.split(eol).map(str::to_owned).collect();
        let final_eol = text.ends_with(eol);
        if final_eol {
            lines.pop();
        }
        Lines {
            lines,
            eol,
            final_eol,
        }
    }

    fn join(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        let mut text = self.lines.join(self.eol);
        if self.final_eol {
            text.push_str(self.eol);
        }
        text
    }

    /// The first and the last line of the block, both markers included.
    fn find(&self, name: &str, comment: &str) -> Option<(usize, usize)> {
        let (open, close) = markers(name, comment);
        let start = self.lines.iter().position(|l| l.trim() == open)?;
        let end = self.lines[start..].iter().position(|l| l.trim() == close)? + start;
        Some((start, end))
    }

    fn append(&mut self, block: Vec<String>) {
        // After a final line end, a blank line sets the block apart. With no
        // final line end, the block starts on the next line, so remove can
        // give back the text exactly.
        if !self.lines.is_empty() && self.final_eol {
            self.lines.push(String::new());
        }
        self.lines.extend(block);
        self.final_eol = true;
    }
}

fn markers(name: &str, comment: &str) -> (String, String) {
    (
        format!("{comment} >>> odd {name} >>>"),
        format!("{comment} <<< odd {name} <<<"),
    )
}

fn block_lines(name: &str, body: &str, comment: &str) -> Vec<String> {
    let (open, close) = markers(name, comment);
    std::iter::once(open)
        .chain(body.lines().map(str::to_owned))
        .chain(std::iter::once(close))
        .collect()
}

/// Puts `body` in the block `name`. Replaces the block if the text has it,
/// else adds it at the end.
pub fn upsert(text: &str, name: &str, body: &str, comment: &str) -> String {
    upsert_before(text, name, body, comment, &|_: &str| false)
}

/// As [`upsert`], but a new block goes before the first line for which
/// `before` is true. In `sshd_config`, a line after `Match` applies only to
/// that match, so the block of `odd` must come first.
pub fn upsert_before(
    text: &str,
    name: &str,
    body: &str,
    comment: &str,
    before: &dyn Fn(&str) -> bool,
) -> String {
    let mut l = Lines::parse(text);
    let block = block_lines(name, body, comment);
    if let Some((start, end)) = l.find(name, comment) {
        l.lines.splice(start..=end, block);
    } else if let Some(at) = l.lines.iter().position(|line| before(line)) {
        l.lines.splice(at..at, block);
    } else {
        l.append(block);
    }
    l.join()
}

/// Removes the block `name`, and the blank line that [`upsert`] put before
/// it. A text with no such block comes back the same.
pub fn remove(text: &str, name: &str, comment: &str) -> String {
    let mut l = Lines::parse(text);
    let Some((start, end)) = l.find(name, comment) else {
        return text.to_owned();
    };
    let at_end = end + 1 == l.lines.len();
    l.lines.drain(start..=end);
    if at_end && start > 0 {
        if l.lines[start - 1].is_empty() {
            l.lines.pop();
            l.final_eol = true;
        } else {
            l.final_eol = false;
        }
    }
    l.join()
}

/// True when the text has the block `name`.
pub fn contains(text: &str, name: &str, comment: &str) -> bool {
    Lines::parse(text).find(name, comment).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_to_a_file_with_no_final_newline() {
        let out = upsert("Host old\n    HostName x", "github", "Host github.com", "#");
        assert!(
            out.starts_with("Host old\n    HostName x\n# >>> odd github >>>\n"),
            "{out:?}"
        );
        assert!(
            out.ends_with("Host github.com\n# <<< odd github <<<\n"),
            "{out:?}"
        );
    }

    #[test]
    fn leaves_a_blank_line_after_a_file_with_a_final_newline() {
        let out = upsert("a\n", "x", "b", "#");
        assert_eq!(out, "a\n\n# >>> odd x >>>\nb\n# <<< odd x <<<\n");
    }

    #[test]
    fn keeps_crlf() {
        let out = upsert("a\r\nb\r\n", "x", "line one\nline two", "#");
        assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
        assert!(out.contains("line one\r\nline two\r\n"), "{out:?}");
    }

    #[test]
    fn replaces_and_does_not_repeat() {
        let once = upsert("a\n", "x", "first", "#");
        let twice = upsert(&once, "x", "second", "#");
        assert_eq!(twice.matches(">>> odd x >>>").count(), 1);
        assert!(
            twice.contains("second") && !twice.contains("first"),
            "{twice:?}"
        );
    }

    #[test]
    fn remove_gives_back_the_text_before() {
        for t in ["", "a\n", "a", "a\r\nb\r\n", "a\n\n"] {
            assert_eq!(remove(&upsert(t, "x", "b", "#"), "x", "#"), t, "{t:?}");
        }
    }

    #[test]
    fn remove_of_a_missing_block_changes_nothing() {
        assert_eq!(remove("a\n", "x", "#"), "a\n");
    }

    #[test]
    fn two_blocks_stay_apart() {
        let both = upsert(&upsert("", "one", "1", "#"), "two", "2", "#");
        assert_eq!(remove(&both, "two", "#"), upsert("", "one", "1", "#"));
        assert!(contains(&both, "one", "#") && contains(&both, "two", "#"));
    }

    #[test]
    fn inserts_before_the_first_match() {
        let text = "Port 22\nMatch Group administrators\n    AuthorizedKeysFile x\n";
        let is_match = |l: &str| l.trim_start().starts_with("Match ");
        let out = upsert_before(
            text,
            "keys-only",
            "PasswordAuthentication no",
            "#",
            &is_match,
        );
        let block = out.find(">>> odd keys-only").unwrap();
        assert!(block < out.find("Match Group").unwrap(), "{out}");
        assert_eq!(remove(&out, "keys-only", "#"), text);
    }

    #[test]
    fn inserts_at_the_end_when_nothing_matches() {
        let out = upsert_before("Port 22\n", "x", "y", "#", &|_: &str| false);
        assert_eq!(out, upsert("Port 22\n", "x", "y", "#"));
    }
}
