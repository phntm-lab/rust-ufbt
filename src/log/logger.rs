use std::fmt;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock, RwLockWriteGuard};

use chrono::{DateTime, Local};

use super::format;
use super::{LogEvent, LogLevel, LogSink, Progress, ProgressState, ProgressThrottle};

/// Source of timestamps for a [`Logger`].
///
/// Injecting one makes formatted output reproducible in tests.
pub type Clock = Arc<dyn Fn() -> DateTime<Local> + Send + Sync>;

/// Producer of [`LogEvent`]s, fanning each one out to every registered [`LogSink`].
///
/// A logger is shared rather than owned: every method takes `&self`, so an `Arc<Logger>` can
/// be handed to several concurrent operations at once.
///
/// Messages below the configured [`level`](Logger::level) are dropped. Build steps and raw
/// text are not levelled and are always emitted.
pub struct Logger {
    level: AtomicU8,
    progress_seq: AtomicU64,
    sinks: RwLock<Vec<Arc<dyn LogSink>>>,
    clock: Clock,
}

impl Logger {
    /// Creates a logger at [`LogLevel::Info`] with no sinks and the system clock.
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Starts configuring a logger with a sink, a level or an injected clock.
    #[must_use]
    pub fn builder() -> LoggerBuilder {
        LoggerBuilder::new()
    }

    /// Lowest level that is still emitted.
    #[must_use]
    pub fn level(&self) -> LogLevel {
        LogLevel::from_severity(self.severity()).expect("logger holds a valid severity")
    }

    /// Sets the lowest level that is still emitted.
    pub fn set_level(&self, level: LogLevel) {
        self.level.store(level.severity(), Ordering::Relaxed);
    }

    /// Whether the logger is at [`LogLevel::Debug`].
    #[must_use]
    pub fn verbose(&self) -> bool {
        self.severity() <= LogLevel::Debug.severity()
    }

    /// Switches the logger between [`LogLevel::Debug`] and [`LogLevel::Info`].
    pub fn set_verbose(&self, verbose: bool) {
        self.set_level(if verbose {
            LogLevel::Debug
        } else {
            LogLevel::Info
        });
    }

    /// Whether a message at `level` would be emitted.
    #[must_use]
    pub fn is_enabled_for(&self, level: LogLevel) -> bool {
        level.severity() >= self.severity()
    }

    /// Registers a sink.
    pub fn add_sink(&self, sink: Arc<dyn LogSink>) {
        self.sinks_mut().push(sink);
    }

    /// Removes the first sink pointing at the same value as `sink`, reporting whether one was
    /// found.
    pub fn remove_sink(&self, sink: &Arc<dyn LogSink>) -> bool {
        let mut sinks = self.sinks_mut();
        let Some(index) = sinks.iter().position(|other| Arc::ptr_eq(other, sink)) else {
            return false;
        };
        sinks.remove(index);
        true
    }

    /// Removes every sink.
    pub fn clear_sinks(&self) {
        self.sinks_mut().clear();
    }

    /// Hands `event` to every registered sink.
    ///
    /// The sinks are captured before the first one runs, so a sink may add or remove sinks
    /// while it is being called; the change takes effect from the next event onwards.
    pub fn emit(&self, event: &LogEvent) {
        let sinks = self
            .sinks
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for sink in &sinks {
            sink.on_event(event);
        }
    }

    /// Emits `message` at `level`, unless the level is filtered out.
    pub fn log(&self, level: LogLevel, message: &str) {
        if !self.is_enabled_for(level) {
            return;
        }
        let time = self.now();
        self.emit(&LogEvent::Message {
            time,
            level,
            message: message.to_owned(),
            formatted: format::message(time, level, message),
        });
    }

    /// Emits `message` at [`LogLevel::Debug`].
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Emits `message` at [`LogLevel::Info`].
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Emits `message` at [`LogLevel::Warning`].
    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    /// Emits `message` at [`LogLevel::Error`].
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    /// Emits `message` at [`LogLevel::Critical`].
    pub fn critical(&self, message: &str) {
        self.log(LogLevel::Critical, message);
    }

    /// Emits a build step. Build steps are not levelled and are never filtered out.
    pub fn build(&self, tag: &str, value: &str, details: &[String]) {
        self.emit(&LogEvent::Build {
            time: self.now(),
            tag: tag.to_owned(),
            value: value.to_owned(),
            details: details.to_vec(),
            formatted: format::build(tag, value, details),
        });
    }

    /// Emits verbatim text followed by a line break. Raw text is never filtered out.
    pub fn raw(&self, text: &str) {
        self.emit_raw(text, true);
    }

    /// Emits verbatim text without a trailing line break.
    pub fn raw_no_newline(&self, text: &str) {
        self.emit_raw(text, false);
    }

    /// Starts a task and emits its [`ProgressState::Started`] event.
    ///
    /// The task is identified as `progress-1`, `progress-2` and so on, counted per logger.
    /// Pass [`None`] as `total` for a task whose size is not known in advance.
    pub fn progress(&self, title: &str, total: Option<u64>) -> ProgressTask<'_> {
        self.progress_builder(title).total(total).start()
    }

    /// Starts configuring a task with an explicit identifier or throttle.
    pub fn progress_builder(&self, title: &str) -> ProgressBuilder<'_> {
        ProgressBuilder {
            logger: self,
            title: title.to_owned(),
            total: None,
            id: None,
            throttle: None,
        }
    }

    fn emit_raw(&self, text: &str, newline: bool) {
        self.emit(&LogEvent::Raw {
            time: self.now(),
            text: text.to_owned(),
            newline,
        });
    }

    fn emit_progress(&self, progress: &Progress) {
        self.emit(&LogEvent::Progress {
            time: self.now(),
            progress: progress.clone(),
        });
    }

    fn next_progress_id(&self) -> String {
        let sequence = self.progress_seq.fetch_add(1, Ordering::Relaxed) + 1;
        format!("progress-{sequence}")
    }

    fn now(&self) -> DateTime<Local> {
        (self.clock)()
    }

    fn severity(&self) -> u8 {
        self.level.load(Ordering::Relaxed)
    }

    fn sinks_mut(&self) -> RwLockWriteGuard<'_, Vec<Arc<dyn LogSink>>> {
        self.sinks.write().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Logger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sinks = self
            .sinks
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        formatter
            .debug_struct("Logger")
            .field("level", &self.level())
            .field("sinks", &sinks)
            .finish_non_exhaustive()
    }
}

/// Configures a [`Logger`].
pub struct LoggerBuilder {
    level: LogLevel,
    sinks: Vec<Arc<dyn LogSink>>,
    clock: Option<Clock>,
}

impl LoggerBuilder {
    fn new() -> Self {
        Self {
            level: LogLevel::Info,
            sinks: Vec::new(),
            clock: None,
        }
    }

    /// Registers a sink. May be called more than once.
    #[must_use]
    pub fn sink(mut self, sink: Arc<dyn LogSink>) -> Self {
        self.sinks.push(sink);
        self
    }

    /// Sets the lowest level that is still emitted. Defaults to [`LogLevel::Info`].
    #[must_use]
    pub fn level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// Replaces the system clock, which makes timestamps reproducible.
    #[must_use]
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Builds the logger.
    #[must_use]
    pub fn build(self) -> Logger {
        Logger {
            level: AtomicU8::new(self.level.severity()),
            progress_seq: AtomicU64::new(0),
            sinks: RwLock::new(self.sinks),
            clock: self.clock.unwrap_or_else(|| Arc::new(Local::now)),
        }
    }
}

impl Default for LoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for LoggerBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoggerBuilder")
            .field("level", &self.level)
            .field("sinks", &self.sinks.len())
            .finish_non_exhaustive()
    }
}

/// Configures a task before it is started.
#[derive(Debug)]
pub struct ProgressBuilder<'a> {
    logger: &'a Logger,
    title: String,
    total: Option<u64>,
    id: Option<String>,
    throttle: Option<ProgressThrottle>,
}

impl<'a> ProgressBuilder<'a> {
    /// Sets the total amount of work. [`None`] leaves the task indeterminate.
    #[must_use]
    pub fn total(mut self, total: Option<u64>) -> Self {
        self.total = total;
        self
    }

    /// Overrides the generated identifier.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Replaces the default [`ProgressThrottle`].
    #[must_use]
    pub fn throttle(mut self, throttle: ProgressThrottle) -> Self {
        self.throttle = Some(throttle);
        self
    }

    /// Starts the task, emitting its [`ProgressState::Started`] event.
    pub fn start(self) -> ProgressTask<'a> {
        let id = self.id.unwrap_or_else(|| self.logger.next_progress_id());
        let progress = Progress::new(id, self.title, ProgressState::Started, 0, self.total, None);
        self.logger.emit_progress(&progress);
        ProgressTask {
            logger: self.logger,
            progress,
            throttle: self.throttle.unwrap_or_default(),
        }
    }
}

/// Handle to a running task, emitting a [`LogEvent::Progress`] as it advances.
///
/// Updates are rate limited by a [`ProgressThrottle`]; the events that start and end the task
/// are always emitted. Once the task is done, further updates are ignored.
#[derive(Debug)]
pub struct ProgressTask<'a> {
    logger: &'a Logger,
    progress: Progress,
    throttle: ProgressThrottle,
}

impl ProgressTask<'_> {
    /// Latest state of the task.
    #[must_use]
    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    /// Reports new counters or a new message, moving the task to [`ProgressState::Running`].
    ///
    /// [`None`] leaves a field as it was, so a total or a message cannot be cleared once set.
    /// Whether this emits an event is up to the throttle.
    pub fn update(&mut self, current: Option<u64>, total: Option<u64>, message: Option<&str>) {
        if self.progress.is_done() {
            return;
        }
        self.progress =
            self.progress
                .copy_with(Some(ProgressState::Running), current, total, message);
        if !self.throttle.should_emit(self.progress.fraction()) {
            return;
        }
        self.logger.emit_progress(&self.progress);
    }

    /// Adds `delta` to the amount of work done.
    pub fn advance(&mut self, delta: u64) {
        self.update(Some(self.progress.current() + delta), None, None);
    }

    /// Completes the task, counting it as fully done.
    pub fn finish(&mut self) {
        self.complete(None);
    }

    /// Completes the task with a closing message.
    pub fn finish_with_message(&mut self, message: &str) {
        self.complete(Some(message));
    }

    /// Aborts the task, leaving its counters where they were.
    pub fn fail(&mut self) {
        self.abort(None);
    }

    /// Aborts the task with a closing message.
    pub fn fail_with_message(&mut self, message: &str) {
        self.abort(Some(message));
    }

    fn complete(&mut self, message: Option<&str>) {
        if self.progress.is_done() {
            return;
        }
        let current = self.progress.total().unwrap_or(self.progress.current());
        self.progress =
            self.progress
                .copy_with(Some(ProgressState::Finished), Some(current), None, message);
        self.logger.emit_progress(&self.progress);
    }

    fn abort(&mut self, message: Option<&str>) {
        if self.progress.is_done() {
            return;
        }
        self.progress = self
            .progress
            .copy_with(Some(ProgressState::Failed), None, None, message);
        self.logger.emit_progress(&self.progress);
    }
}
