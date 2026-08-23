use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Local, TimeZone};

use rust_ufbt::log::{
    Clock, LogEvent, LogLevel, LogSink, Logger, Progress, ProgressState, ProgressThrottle,
};

type Recorded = Arc<Mutex<Vec<LogEvent>>>;

fn recorder() -> (Arc<dyn LogSink>, Recorded) {
    let store: Recorded = Arc::new(Mutex::new(Vec::new()));
    let sink_store = store.clone();
    let sink: Arc<dyn LogSink> = Arc::new(move |event: &LogEvent| {
        sink_store
            .lock()
            .expect("recorder lock")
            .push(event.clone());
    });
    (sink, store)
}

fn recorded(store: &Recorded) -> Vec<LogEvent> {
    store.lock().expect("recorder lock").clone()
}

fn fixed_time() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 8, 23, 14, 5, 9)
        .earliest()
        .expect("local time exists")
        + chrono::Duration::milliseconds(7)
}

fn fixed_clock() -> Clock {
    Arc::new(fixed_time)
}

fn eager_throttle() -> ProgressThrottle {
    ProgressThrottle::with_limits(0.0, Duration::ZERO)
}

fn logger_with_recorder(level: LogLevel) -> (Logger, Recorded) {
    let (sink, store) = recorder();
    let logger = Logger::builder()
        .level(level)
        .clock(fixed_clock())
        .sink(sink)
        .build();
    (logger, store)
}

fn message_texts(store: &Recorded) -> Vec<String> {
    recorded(store)
        .into_iter()
        .filter_map(|event| match event {
            LogEvent::Message { message, .. } => Some(message),
            _ => None,
        })
        .collect()
}

fn snapshots(store: &Recorded) -> Vec<Progress> {
    recorded(store)
        .into_iter()
        .filter_map(|event| match event {
            LogEvent::Progress { progress, .. } => Some(progress),
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_logger_starts_at_info_without_sinks() {
    let logger = Logger::new();
    assert_eq!(logger.level(), LogLevel::Info);
    assert!(!logger.verbose());
    logger.info("goes nowhere");
}

#[test]
fn is_enabled_for_compares_severity() {
    let logger = Logger::new();
    logger.set_level(LogLevel::Warning);

    assert!(!logger.is_enabled_for(LogLevel::Debug));
    assert!(!logger.is_enabled_for(LogLevel::Info));
    assert!(logger.is_enabled_for(LogLevel::Warning));
    assert!(logger.is_enabled_for(LogLevel::Error));
    assert!(logger.is_enabled_for(LogLevel::Critical));
}

#[test]
fn verbose_switches_between_debug_and_info() {
    let logger = Logger::new();
    assert!(!logger.verbose());

    logger.set_verbose(true);
    assert_eq!(logger.level(), LogLevel::Debug);
    assert!(logger.verbose());

    logger.set_verbose(false);
    assert_eq!(logger.level(), LogLevel::Info);
    assert!(!logger.verbose());

    logger.set_level(LogLevel::Debug);
    assert!(logger.verbose());
    logger.set_level(LogLevel::Critical);
    assert!(!logger.verbose());
}

#[test]
fn messages_below_the_level_are_dropped() {
    let (logger, store) = logger_with_recorder(LogLevel::Warning);

    logger.debug("debug");
    logger.info("info");
    logger.warning("warning");
    logger.error("error");
    logger.critical("critical");

    assert_eq!(message_texts(&store), ["warning", "error", "critical"]);
}

#[test]
fn every_level_passes_at_debug() {
    let (logger, store) = logger_with_recorder(LogLevel::Debug);

    logger.debug("debug");
    logger.info("info");
    logger.warning("warning");
    logger.error("error");
    logger.critical("critical");

    assert_eq!(
        message_texts(&store),
        ["debug", "info", "warning", "error", "critical"]
    );
}

#[test]
fn log_records_level_time_and_formatted_text() {
    let (logger, store) = logger_with_recorder(LogLevel::Debug);

    logger.warning("SDK is out of date");

    let events = recorded(&store);
    assert_eq!(events.len(), 1);
    let LogEvent::Message {
        time,
        level,
        message,
        formatted,
    } = &events[0]
    else {
        panic!("expected a message event");
    };
    assert_eq!(*time, fixed_time());
    assert_eq!(*level, LogLevel::Warning);
    assert_eq!(message, "SDK is out of date");
    assert_eq!(formatted, "14:05:09.007 [W] SDK is out of date");
}

#[test]
fn build_and_raw_are_never_filtered_out() {
    let (logger, store) = logger_with_recorder(LogLevel::Critical);

    logger.info("dropped");
    logger.build("CC", "src/main.c", &["-Os".to_owned()]);
    logger.raw("with a line break");
    logger.raw_no_newline("without one");

    let events = recorded(&store);
    assert_eq!(events.len(), 3);

    let LogEvent::Build {
        tag,
        value,
        details,
        formatted,
        ..
    } = &events[0]
    else {
        panic!("expected a build event");
    };
    assert_eq!(tag, "CC");
    assert_eq!(value, "src/main.c");
    assert_eq!(details, &["-Os".to_owned()]);
    assert_eq!(formatted, "\tCC\tsrc/main.c\n\t\t-Os");

    assert_eq!(
        events[1],
        LogEvent::Raw {
            time: fixed_time(),
            text: "with a line break".to_owned(),
            newline: true,
        }
    );
    assert_eq!(
        events[2],
        LogEvent::Raw {
            time: fixed_time(),
            text: "without one".to_owned(),
            newline: false,
        }
    );
}

#[test]
fn several_sinks_all_receive_every_event() {
    let (first, first_store) = recorder();
    let (second, second_store) = recorder();
    let logger = Logger::new();
    logger.add_sink(first);
    logger.add_sink(second);

    logger.info("shared");

    assert_eq!(message_texts(&first_store), ["shared"]);
    assert_eq!(message_texts(&second_store), ["shared"]);
}

#[test]
fn a_removed_sink_stops_receiving_events() {
    let (first, first_store) = recorder();
    let (second, second_store) = recorder();
    let logger = Logger::new();
    logger.add_sink(first.clone());
    logger.add_sink(second);

    logger.info("both");
    assert!(logger.remove_sink(&first));
    logger.info("only the second");

    assert_eq!(message_texts(&first_store), ["both"]);
    assert_eq!(message_texts(&second_store), ["both", "only the second"]);
}

#[test]
fn removing_an_unregistered_sink_reports_false() {
    let (sink, _) = recorder();
    let (other, _) = recorder();
    let logger = Logger::new();
    logger.add_sink(sink);

    assert!(!logger.remove_sink(&other));
}

#[test]
fn clear_sinks_removes_all_of_them() {
    let (sink, store) = recorder();
    let logger = Logger::new();
    logger.add_sink(sink);

    logger.info("before");
    logger.clear_sinks();
    logger.info("after");

    assert_eq!(message_texts(&store), ["before"]);
}

#[test]
fn a_sink_may_register_another_sink_while_it_runs() {
    let logger = Arc::new(Logger::new());
    let (late, late_store) = recorder();

    let weak = Arc::downgrade(&logger);
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = calls.clone();
    let registrar: Arc<dyn LogSink> = Arc::new(move |_: &LogEvent| {
        if sink_calls.fetch_add(1, Ordering::Relaxed) == 0
            && let Some(logger) = weak.upgrade()
        {
            logger.add_sink(late.clone());
        }
    });
    logger.add_sink(registrar);

    logger.info("first");
    logger.info("second");

    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(message_texts(&late_store), ["second"]);
}

#[test]
fn a_sink_may_remove_itself_while_it_runs() {
    let logger = Arc::new(Logger::new());
    let (counted, counted_store) = recorder();

    let weak = Arc::downgrade(&logger);
    let target = counted.clone();
    let remover: Arc<dyn LogSink> = Arc::new(move |_: &LogEvent| {
        if let Some(logger) = weak.upgrade() {
            logger.remove_sink(&target);
        }
    });
    logger.add_sink(remover);
    logger.add_sink(counted);

    logger.info("first");
    logger.info("second");

    assert_eq!(message_texts(&counted_store), ["first"]);
}

#[test]
fn progress_emits_started_immediately() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);

    let task = logger.progress("Downloading", Some(200));

    let events = snapshots(&store);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].state(), ProgressState::Started);
    assert_eq!(events[0].id(), "progress-1");
    assert_eq!(events[0].title(), "Downloading");
    assert_eq!(events[0].current(), 0);
    assert_eq!(events[0].total(), Some(200));
    assert_eq!(task.progress(), &events[0]);
}

#[test]
fn generated_identifiers_count_up_per_logger() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);

    let _first = logger.progress("one", None);
    let _second = logger.progress("two", None);
    let _third = logger.progress("three", None);

    let ids: Vec<_> = snapshots(&store)
        .iter()
        .map(|progress| progress.id().to_owned())
        .collect();
    assert_eq!(ids, ["progress-1", "progress-2", "progress-3"]);
}

#[test]
fn an_explicit_identifier_does_not_consume_a_number() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);

    let _named = logger.progress_builder("named").id("unpack").start();
    let _generated = logger.progress("generated", None);

    let ids: Vec<_> = snapshots(&store)
        .iter()
        .map(|progress| progress.id().to_owned())
        .collect();
    assert_eq!(ids, ["unpack", "progress-1"]);
}

#[test]
fn update_moves_the_task_to_running() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Downloading")
        .total(Some(200))
        .throttle(eager_throttle())
        .start();

    task.update(Some(50), None, Some("half way"));

    let events = snapshots(&store);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].state(), ProgressState::Running);
    assert_eq!(events[1].current(), 50);
    assert_eq!(events[1].message(), Some("half way"));
    assert_eq!(events[1].fraction(), Some(0.25));
}

#[test]
fn update_keeps_fields_it_is_not_given() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Downloading")
        .throttle(eager_throttle())
        .start();

    task.update(Some(5), Some(200), Some("first"));
    task.update(Some(6), None, None);

    let events = snapshots(&store);
    assert_eq!(events[2].current(), 6);
    assert_eq!(events[2].total(), Some(200));
    assert_eq!(events[2].message(), Some("first"));
}

#[test]
fn the_default_throttle_holds_back_a_burst_of_updates() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger.progress("Downloading", Some(200));

    for current in 1..=100 {
        task.update(Some(current), None, None);
    }

    assert_eq!(snapshots(&store).len(), 1);
    assert_eq!(task.progress().current(), 100);
}

#[test]
fn advance_adds_to_the_current_count() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Unpacking")
        .total(Some(10))
        .throttle(eager_throttle())
        .start();

    task.advance(1);
    task.advance(4);

    let events = snapshots(&store);
    assert_eq!(events[1].current(), 1);
    assert_eq!(events[2].current(), 5);
    assert_eq!(task.progress().current(), 5);
}

#[test]
fn finish_counts_the_task_as_complete() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger.progress("Downloading", Some(200));

    task.finish_with_message("done");

    let events = snapshots(&store);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].state(), ProgressState::Finished);
    assert_eq!(events[1].current(), 200);
    assert_eq!(events[1].message(), Some("done"));
    assert!(events[1].is_done());
}

#[test]
fn finish_without_a_total_keeps_the_current_count() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Downloading")
        .throttle(eager_throttle())
        .start();

    task.advance(7);
    task.finish();

    let events = snapshots(&store);
    assert_eq!(events.last().expect("an event").current(), 7);
    assert_eq!(
        events.last().expect("an event").state(),
        ProgressState::Finished
    );
}

#[test]
fn fail_leaves_the_counters_alone() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Downloading")
        .total(Some(200))
        .throttle(eager_throttle())
        .start();

    task.update(Some(5), None, None);
    task.fail_with_message("connection lost");

    let events = snapshots(&store);
    let last = events.last().expect("an event");
    assert_eq!(last.state(), ProgressState::Failed);
    assert_eq!(last.current(), 5);
    assert_eq!(last.message(), Some("connection lost"));
}

#[test]
fn finish_and_fail_bypass_the_throttle() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut finished = logger.progress("one", Some(200));
    let mut failed = logger.progress("two", Some(200));

    finished.finish();
    failed.fail();

    assert_eq!(snapshots(&store).len(), 4);
}

#[test]
fn a_finished_task_ignores_further_updates() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Downloading")
        .total(Some(200))
        .throttle(eager_throttle())
        .start();

    task.finish();
    let after_finish = snapshots(&store).len();

    task.update(Some(1), None, Some("ignored"));
    task.advance(5);
    task.finish();
    task.fail();

    assert_eq!(snapshots(&store).len(), after_finish);
    assert_eq!(task.progress().state(), ProgressState::Finished);
    assert_eq!(task.progress().current(), 200);
    assert_eq!(task.progress().message(), None);
}

#[test]
fn a_failed_task_ignores_further_updates() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger
        .progress_builder("Downloading")
        .total(Some(200))
        .throttle(eager_throttle())
        .start();

    task.fail_with_message("gave up");
    let after_fail = snapshots(&store).len();

    task.update(Some(1), None, None);
    task.finish();

    assert_eq!(snapshots(&store).len(), after_fail);
    assert_eq!(task.progress().state(), ProgressState::Failed);
    assert_eq!(task.progress().message(), Some("gave up"));
}

#[test]
fn progress_events_carry_the_injected_time() {
    let (logger, store) = logger_with_recorder(LogLevel::Info);
    let mut task = logger.progress("Downloading", Some(200));
    task.finish();

    for event in recorded(&store) {
        assert_eq!(event.time(), fixed_time());
    }
}
