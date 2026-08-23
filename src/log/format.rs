//! Rendering of log entries into the exact text form uFBT writes.

use chrono::{DateTime, Local, Timelike};

use super::{LogLevel, Progress};

/// Width of a progress bar when no other width is given.
pub const DEFAULT_BAR_WIDTH: usize = 72;

/// Character a progress bar is filled with.
pub const BAR_CHAR: char = '#';

/// Formats the time of day as `HH:MM:SS.mmm`.
///
/// Anything finer than a millisecond is dropped.
#[must_use]
pub fn timestamp(time: DateTime<Local>) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        time.hour(),
        time.minute(),
        time.second(),
        time.timestamp_subsec_millis()
    )
}

/// Formats a levelled message as `HH:MM:SS.mmm [L] message`.
#[must_use]
pub fn message(time: DateTime<Local>, level: LogLevel, message: &str) -> String {
    format!("{} [{}] {}", timestamp(time), level.letter(), message)
}

/// Formats a build step as a tab-indented `tag` and `value`, with each detail line on its
/// own line indented one level deeper.
#[must_use]
pub fn build(tag: &str, value: &str, details: &[String]) -> String {
    let head = format!("\t{tag}\t{value}");
    if details.is_empty() {
        return head;
    }
    let body = details
        .iter()
        .map(|line| format!("\t\t{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{head}\n{body}")
}

/// Formats a percentage with one decimal place, right-aligned to a width of five characters
/// and followed by a percent sign.
#[must_use]
pub fn percent(value: f64) -> String {
    format!("{:>5}%", to_fixed_one_decimal(value))
}

/// Renders a progress bar of the given width, followed by the completed
/// [`percent`](percent()).
///
/// A task that is done reads as complete regardless of its counters, and an indeterminate
/// task reads as empty.
#[must_use]
pub fn progress_bar(progress: &Progress, width: usize) -> String {
    let fraction = if progress.is_done() {
        1.0
    } else {
        progress.fraction().unwrap_or(0.0)
    };
    let filled = (fraction * width as f64).round() as usize;
    let bar: String = std::iter::repeat_n(BAR_CHAR, filled).collect();
    let padding: String = std::iter::repeat_n(' ', width - filled).collect();
    format!("{bar}{padding} {}", percent(fraction * 100.0))
}

fn to_fixed_one_decimal(value: f64) -> String {
    if is_exact_half_at_tenths(value) {
        return format!("{:.1}", (value * 10.0).round() / 10.0);
    }
    format!("{value:.1}")
}

fn is_exact_half_at_tenths(value: f64) -> bool {
    (value * 4.0).fract() == 0.0 && (value * 2.0).fract() != 0.0
}
