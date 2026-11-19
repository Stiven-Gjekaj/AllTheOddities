//! The questions that an oddity asks the person.

/// Asks the person. The tests give scripted answers.
pub trait Ask {
    fn yes_no(&self, question: &str, default: bool) -> bool;
    fn text(&self, question: &str, default: Option<&str>) -> Option<String>;
    /// Hidden input. Never echoed, never logged.
    fn secret(&self, question: &str) -> Option<String>;
}
