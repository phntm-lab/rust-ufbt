/// Severity level of a log message.
///
/// Levels are ordered by [`severity`](LogLevel::severity): [`LogLevel::Debug`] is the
/// lowest and [`LogLevel::Critical`] the highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum LogLevel {
    /// Diagnostic detail, hidden unless verbose logging is enabled.
    Debug = 10,
    /// Normal progress reporting.
    Info = 20,
    /// Something unexpected that does not stop the operation.
    Warning = 30,
    /// An operation failed.
    Error = 40,
    /// A failure that prevents any further work.
    Critical = 50,
}

impl LogLevel {
    /// Numeric severity of the level, from `10` for [`LogLevel::Debug`] to `50` for
    /// [`LogLevel::Critical`].
    #[must_use]
    pub const fn severity(self) -> u8 {
        self as u8
    }

    /// Upper-case name of the level, such as `"DEBUG"`.
    #[must_use]
    pub const fn level_name(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
            Self::Critical => "CRITICAL",
        }
    }

    /// First letter of [`level_name`](LogLevel::level_name), used in formatted messages.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Debug => 'D',
            Self::Info => 'I',
            Self::Warning => 'W',
            Self::Error => 'E',
            Self::Critical => 'C',
        }
    }
}
