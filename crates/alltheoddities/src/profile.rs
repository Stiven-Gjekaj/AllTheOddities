//! The login profiles of a shell on macOS and Linux.

use std::path::PathBuf;

use crate::ctx::Ctx;

/// The files that a login shell reads. bash reads ~/.bash_profile in place
/// of ~/.profile when it exists, and zsh reads ~/.zprofile.
pub fn profiles(ctx: &Ctx<'_>, shell: &str) -> Vec<PathBuf> {
    let mut files = vec![ctx.home.join(".profile")];
    if shell.ends_with("zsh") {
        files.push(ctx.home.join(".zprofile"));
    }
    let bash_profile = ctx.home.join(".bash_profile");
    if shell.ends_with("bash") && bash_profile.exists() {
        files.push(bash_profile);
    }
    files
}

/// A string in single quotes for fish, which takes `\'` and `\\` there.
pub fn fish_quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Os;
    use crate::testing::TestCtx;

    #[test]
    fn zsh_also_gets_the_zprofile() {
        let mut t = TestCtx::new(Os::MacOs);
        let files = profiles(&t.ctx(), "/bin/zsh");
        assert_eq!(
            files,
            vec![t.home().join(".profile"), t.home().join(".zprofile")]
        );
    }

    #[test]
    fn fish_quote_escapes_quotes_and_backslashes() {
        assert_eq!(fish_quote(r"a'b\c"), r"'a\'b\\c'");
    }
}
