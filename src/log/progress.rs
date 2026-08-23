use std::time::{Duration, Instant};

/// Lifecycle stage of a [`Progress`] report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgressState {
    /// The task has just been created and no work has been reported yet.
    Started,
    /// The task is running and reporting updates.
    Running,
    /// The task completed successfully.
    Finished,
    /// The task was aborted because of an error.
    Failed,
}

/// Rate limiter deciding which progress updates are worth emitting.
///
/// An update passes when the completed fraction moved by at least
/// [`min_delta`](ProgressThrottle::min_delta) *and* at least `min_interval` elapsed since the
/// last accepted update. Reaching completion always passes, and updates without a known
/// fraction pass on the interval alone.
#[derive(Debug)]
pub struct ProgressThrottle {
    min_delta: f64,
    min_interval: Duration,
    last: f64,
    marked_at: Instant,
}

impl ProgressThrottle {
    /// Smallest change in completed fraction that is worth emitting.
    pub const DEFAULT_MIN_DELTA: f64 = 0.002;

    /// Shortest time between two emitted updates.
    pub const DEFAULT_MIN_INTERVAL: Duration = Duration::from_millis(150);

    /// Creates a throttle with [`DEFAULT_MIN_DELTA`](ProgressThrottle::DEFAULT_MIN_DELTA) and
    /// [`DEFAULT_MIN_INTERVAL`](ProgressThrottle::DEFAULT_MIN_INTERVAL).
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(Self::DEFAULT_MIN_DELTA, Self::DEFAULT_MIN_INTERVAL)
    }

    /// Creates a throttle with custom limits.
    ///
    /// The interval starts running immediately, so an update reported right after
    /// construction is held back unless it reports completion.
    #[must_use]
    pub fn with_limits(min_delta: f64, min_interval: Duration) -> Self {
        Self {
            min_delta,
            min_interval,
            last: -1.0,
            marked_at: Instant::now(),
        }
    }

    /// Smallest change in completed fraction this throttle lets through.
    #[must_use]
    pub fn min_delta(&self) -> f64 {
        self.min_delta
    }

    /// Reports an update and answers whether it should be emitted.
    ///
    /// `fraction` is [`None`] for an indeterminate task, in which case only the interval is
    /// taken into account. Every call that returns `true` restarts the interval.
    pub fn should_emit(&mut self, fraction: Option<f64>) -> bool {
        let Some(fraction) = fraction else {
            return self.tick();
        };
        if fraction >= 1.0 && self.last < 1.0 {
            self.mark(fraction);
            return true;
        }
        if (fraction - self.last).abs() >= self.min_delta
            && self.marked_at.elapsed() >= self.min_interval
        {
            self.mark(fraction);
            return true;
        }
        false
    }

    fn tick(&mut self) -> bool {
        if self.marked_at.elapsed() >= self.min_interval {
            self.marked_at = Instant::now();
            return true;
        }
        false
    }

    fn mark(&mut self, fraction: f64) {
        self.last = fraction;
        self.marked_at = Instant::now();
    }
}

impl Default for ProgressThrottle {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable snapshot of a long-running task.
///
/// A task with no known `total` is *indeterminate*: it reports how much work is done but not
/// how much is left, so [`fraction`](Progress::fraction) and [`percent`](Progress::percent)
/// are unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    id: String,
    title: String,
    state: ProgressState,
    current: u64,
    total: Option<u64>,
    message: Option<String>,
}

impl Progress {
    /// Creates a snapshot.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        state: ProgressState,
        current: u64,
        total: Option<u64>,
        message: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            state,
            current,
            total,
            message,
        }
    }

    /// Identifier that ties every snapshot of one task together.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Human-readable name of the task.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Lifecycle stage of the task.
    #[must_use]
    pub fn state(&self) -> ProgressState {
        self.state
    }

    /// Amount of work reported as done.
    #[must_use]
    pub fn current(&self) -> u64 {
        self.current
    }

    /// Total amount of work, when it is known.
    #[must_use]
    pub fn total(&self) -> Option<u64> {
        self.total
    }

    /// Message attached to the latest update, when there is one.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Whether the total amount of work is unknown, either missing or zero.
    #[must_use]
    pub fn indeterminate(&self) -> bool {
        self.total.is_none_or(|total| total == 0)
    }

    /// Whether the task reached [`ProgressState::Finished`] or [`ProgressState::Failed`].
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.state, ProgressState::Finished | ProgressState::Failed)
    }

    /// Completed share of the work, clamped to `[0, 1]`.
    ///
    /// Returns [`None`] when the task is [`indeterminate`](Progress::indeterminate).
    #[must_use]
    pub fn fraction(&self) -> Option<f64> {
        if self.indeterminate() {
            return None;
        }
        let total = self.total?;
        Some((self.current as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// [`fraction`](Progress::fraction) expressed as a percentage in `[0, 100]`.
    #[must_use]
    pub fn percent(&self) -> Option<f64> {
        self.fraction().map(|value| value * 100.0)
    }
}
