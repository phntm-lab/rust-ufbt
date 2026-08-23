use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Mutex, PoisonError};

use super::format;
use super::{LogEvent, LogSink, Progress, ProgressState};

/// Destination [`ConsoleSink`] writes to.
///
/// The text is written verbatim: it already carries its own line breaks and carriage returns.
pub type ConsoleOutput = Box<dyn Fn(&str) + Send + Sync>;

/// Amount of work one `#` stands for while a task's total is unknown.
pub const DEFAULT_UNITS_PER_HASH: u64 = 300;

/// [`LogSink`] that renders events as the terminal output uFBT produces.
///
/// Messages and build steps are written in their formatted form, raw text passes through, and
/// a task with a known total is drawn as a progress bar redrawn in place. A task whose total is
/// unknown grows a row of `#`, one per [`units_per_hash`](ConsoleSink::units_per_hash) units of
/// work.
pub struct ConsoleSink {
    output: ConsoleOutput,
    bar_width: usize,
    units_per_hash: u64,
    hashes: Mutex<HashMap<String, u64>>,
}

impl ConsoleSink {
    /// Creates a sink writing to standard output.
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Creates a sink writing through `output`.
    #[must_use]
    pub fn with_output(output: ConsoleOutput) -> Self {
        Self::builder().output(output).build()
    }

    /// Starts configuring a sink with its own destination, bar width or hash size.
    #[must_use]
    pub fn builder() -> ConsoleSinkBuilder {
        ConsoleSinkBuilder::new()
    }

    /// Width of the progress bar this sink draws.
    #[must_use]
    pub fn bar_width(&self) -> usize {
        self.bar_width
    }

    /// Amount of work one `#` stands for while a task's total is unknown.
    #[must_use]
    pub fn units_per_hash(&self) -> u64 {
        self.units_per_hash
    }

    fn write(&self, text: &str) {
        (self.output)(text);
    }

    fn on_progress(&self, progress: &Progress) {
        match progress.state() {
            ProgressState::Started => {
                self.set_hashes(progress.id(), 0);
                if !progress.title().is_empty() {
                    self.write(&format!("{}\n", progress.title()));
                }
            }
            ProgressState::Running => {
                if progress.indeterminate() {
                    self.append_hashes(progress);
                } else {
                    self.write(&format!(
                        "\r{}",
                        format::progress_bar(progress, self.bar_width)
                    ));
                }
            }
            ProgressState::Finished => {
                if progress.indeterminate() {
                    self.append_hashes(progress);
                    self.write(&format!(" {}\n", format::percent(100.0)));
                } else {
                    self.write(&format!(
                        "\r{}\n",
                        format::progress_bar(progress, self.bar_width)
                    ));
                }
                self.forget(progress.id());
            }
            ProgressState::Failed => {
                self.write("\n");
                self.forget(progress.id());
            }
        }
    }

    fn append_hashes(&self, progress: &Progress) {
        let mut hashes = self.hashes.lock().unwrap_or_else(PoisonError::into_inner);
        let printed = hashes.get(progress.id()).copied().unwrap_or(0);
        let expected = progress.current() / self.units_per_hash;
        if expected <= printed {
            return;
        }
        hashes.insert(progress.id().to_owned(), expected);
        let added = usize::try_from(expected - printed).unwrap_or(usize::MAX);
        self.write(&std::iter::repeat_n(format::BAR_CHAR, added).collect::<String>());
    }

    fn set_hashes(&self, id: &str, count: u64) {
        self.hashes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.to_owned(), count);
    }

    fn forget(&self, id: &str) {
        self.hashes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
    }
}

impl LogSink for ConsoleSink {
    fn on_event(&self, event: &LogEvent) {
        match event {
            LogEvent::Message { formatted, .. } | LogEvent::Build { formatted, .. } => {
                self.write(&format!("{formatted}\n"));
            }
            LogEvent::Raw { text, newline, .. } => {
                if *newline {
                    self.write(&format!("{text}\n"));
                } else {
                    self.write(text);
                }
            }
            LogEvent::Progress { progress, .. } => self.on_progress(progress),
        }
    }
}

impl Default for ConsoleSink {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ConsoleSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsoleSink")
            .field("bar_width", &self.bar_width)
            .field("units_per_hash", &self.units_per_hash)
            .finish_non_exhaustive()
    }
}

/// Configures a [`ConsoleSink`].
pub struct ConsoleSinkBuilder {
    output: Option<ConsoleOutput>,
    bar_width: usize,
    units_per_hash: u64,
}

impl ConsoleSinkBuilder {
    fn new() -> Self {
        Self {
            output: None,
            bar_width: format::DEFAULT_BAR_WIDTH,
            units_per_hash: DEFAULT_UNITS_PER_HASH,
        }
    }

    /// Replaces standard output as the destination.
    #[must_use]
    pub fn output(mut self, output: ConsoleOutput) -> Self {
        self.output = Some(output);
        self
    }

    /// Sets the progress bar width. Defaults to
    /// [`format::DEFAULT_BAR_WIDTH`](super::format::DEFAULT_BAR_WIDTH).
    #[must_use]
    pub fn bar_width(mut self, width: usize) -> Self {
        self.bar_width = width;
        self
    }

    /// Sets how much work one `#` stands for. Defaults to [`DEFAULT_UNITS_PER_HASH`].
    #[must_use]
    pub fn units_per_hash(mut self, units: u64) -> Self {
        self.units_per_hash = units;
        self
    }

    /// Builds the sink.
    #[must_use]
    pub fn build(self) -> ConsoleSink {
        ConsoleSink {
            output: self.output.unwrap_or_else(|| Box::new(write_to_stdout)),
            bar_width: self.bar_width,
            units_per_hash: self.units_per_hash,
            hashes: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for ConsoleSinkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ConsoleSinkBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsoleSinkBuilder")
            .field("bar_width", &self.bar_width)
            .field("units_per_hash", &self.units_per_hash)
            .finish_non_exhaustive()
    }
}

fn write_to_stdout(text: &str) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}
