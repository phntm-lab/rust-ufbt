use chrono::{DateTime, Local};

use super::{LogLevel, Progress};

/// A single entry produced by a logger and handed to every registered sink.
///
/// The enum is deliberately exhaustive: a consumer that renders log output is expected to
/// handle every variant, and adding one is a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// A levelled text message.
    Message {
        /// When the event was produced.
        time: DateTime<Local>,
        /// Severity the message was logged at.
        level: LogLevel,
        /// The message itself, without decoration.
        message: String,
        /// The message rendered for display, as produced by [`format::message`].
        ///
        /// [`format::message`]: super::format::message
        formatted: String,
    },
    /// A build step reported as a tag with a value and optional detail lines.
    Build {
        /// When the event was produced.
        time: DateTime<Local>,
        /// Short name of the step, such as `CC` or `LINK`.
        tag: String,
        /// What the step acted on, usually a path.
        value: String,
        /// Additional lines describing the step.
        details: Vec<String>,
        /// The step rendered for display, as produced by [`format::build`].
        ///
        /// [`format::build`]: super::format::build
        formatted: String,
    },
    /// Text passed through verbatim, without a timestamp or a level prefix.
    Raw {
        /// When the event was produced.
        time: DateTime<Local>,
        /// The text to write.
        text: String,
        /// Whether a line break follows the text.
        newline: bool,
    },
    /// An update of a long-running task.
    Progress {
        /// When the event was produced.
        time: DateTime<Local>,
        /// State of the task at that moment.
        progress: Progress,
    },
}

impl LogEvent {
    /// When the event was produced.
    #[must_use]
    pub fn time(&self) -> DateTime<Local> {
        match self {
            Self::Message { time, .. }
            | Self::Build { time, .. }
            | Self::Raw { time, .. }
            | Self::Progress { time, .. } => *time,
        }
    }
}
