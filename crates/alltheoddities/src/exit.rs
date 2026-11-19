//! The exit codes of `odd`. The README gives the same table.

/// The end of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Exit {
    /// The oddity did its work.
    Done = 0,
    /// A step failed. The output says which step, and what to do.
    Failed = 1,
    /// The usage is not correct, or the oddity does not run on this host.
    Usage = 2,
    /// The oddity needs an administrator. It changed nothing.
    NeedsAdmin = 3,
}
