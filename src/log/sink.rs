use super::LogEvent;

/// Receiver of [`LogEvent`]s produced by a [`Logger`](super::Logger).
///
/// Sinks are called synchronously, in the order they were added, on whichever thread produced
/// the event. A sink must therefore be quick and must not block: anything slow belongs behind
/// a channel the sink writes to.
///
/// Any `Fn(&LogEvent) + Send + Sync` is a sink, so a closure can be registered directly.
pub trait LogSink: Send + Sync {
    /// Handles one event.
    fn on_event(&self, event: &LogEvent);
}

impl<F> LogSink for F
where
    F: Fn(&LogEvent) + Send + Sync,
{
    fn on_event(&self, event: &LogEvent) {
        self(event);
    }
}
