use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, Mutex};

use chrono::{DateTime, Local, TimeZone};

use rust_ufbt::log::{ConsoleSink, LogEvent, LogLevel, LogSink, Progress, ProgressState};

static VECTORS: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    let raw = include_str!("fixtures/console_sink.json");
    serde_json::from_str(raw).expect("reference vectors are valid JSON")
});

fn expected(key: &str) -> &'static str {
    VECTORS
        .get(key)
        .unwrap_or_else(|| panic!("no reference vector named {key}"))
}

type Written = Arc<Mutex<String>>;

struct Console {
    sink: ConsoleSink,
    written: Written,
}

impl Console {
    fn new() -> Self {
        Self::configured(72, 300)
    }

    fn configured(bar_width: usize, units_per_hash: u64) -> Self {
        let written: Written = Arc::new(Mutex::new(String::new()));
        let target = written.clone();
        let sink = ConsoleSink::builder()
            .output(Box::new(move |text: &str| {
                target.lock().expect("output lock").push_str(text);
            }))
            .bar_width(bar_width)
            .units_per_hash(units_per_hash)
            .build();
        Self { sink, written }
    }

    fn feed(&self, event: &LogEvent) {
        self.sink.on_event(event);
    }

    fn feed_progress(&self, progress: Progress) {
        self.feed(&LogEvent::Progress {
            time: time(),
            progress,
        });
    }

    fn text(&self) -> String {
        self.written.lock().expect("output lock").clone()
    }
}

fn time() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 8, 23, 14, 5, 9)
        .earliest()
        .expect("local time exists")
        + chrono::Duration::milliseconds(7)
}

fn task(id: &str, state: ProgressState, current: u64, total: Option<u64>) -> Progress {
    named_task(id, "Downloading", state, current, total)
}

fn named_task(
    id: &str,
    title: &str,
    state: ProgressState,
    current: u64,
    total: Option<u64>,
) -> Progress {
    Progress::new(id, title, state, current, total, None)
}

#[test]
fn defaults_match_the_original() {
    let sink = ConsoleSink::new();
    assert_eq!(sink.bar_width(), 72);
    assert_eq!(sink.units_per_hash(), 300);
    assert_eq!(
        rust_ufbt::log::DEFAULT_UNITS_PER_HASH,
        sink.units_per_hash()
    );
}

#[test]
fn a_message_is_written_with_a_line_break() {
    let console = Console::new();
    console.feed(&LogEvent::Message {
        time: time(),
        level: LogLevel::Warning,
        message: "hi".to_owned(),
        formatted: "14:05:09.007 [W] hi".to_owned(),
    });
    assert_eq!(console.text(), expected("message"));
}

#[test]
fn a_build_step_is_written_with_a_line_break() {
    let console = Console::new();
    console.feed(&LogEvent::Build {
        time: time(),
        tag: "CC".to_owned(),
        value: "a.c".to_owned(),
        details: vec!["-Os".to_owned()],
        formatted: "\tCC\ta.c\n\t\t-Os".to_owned(),
    });
    assert_eq!(console.text(), expected("build"));
}

#[test]
fn raw_text_honours_its_line_break_flag() {
    let with_break = Console::new();
    with_break.feed(&LogEvent::Raw {
        time: time(),
        text: "plain".to_owned(),
        newline: true,
    });
    assert_eq!(with_break.text(), expected("raw_newline"));

    let without_break = Console::new();
    without_break.feed(&LogEvent::Raw {
        time: time(),
        text: "plain".to_owned(),
        newline: false,
    });
    assert_eq!(without_break.text(), expected("raw_no_newline"));
    assert_eq!(without_break.text(), "plain");
}

#[test]
fn a_started_task_announces_its_title() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Started, 0, Some(200)));
    assert_eq!(console.text(), expected("started"));
}

#[test]
fn a_started_task_without_a_title_writes_nothing() {
    let console = Console::new();
    console.feed_progress(named_task("a", "", ProgressState::Started, 0, Some(200)));
    assert_eq!(console.text(), expected("started_empty_title"));
    assert!(console.text().is_empty());
}

#[test]
fn a_determinate_task_redraws_its_bar_in_place() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Running, 50, Some(200)));
    assert_eq!(console.text(), expected("running_determinate"));
    assert!(console.text().starts_with('\r'));
}

#[test]
fn an_indeterminate_task_grows_a_row_of_hashes() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Started, 0, None));
    console.feed_progress(task("a", ProgressState::Running, 299, None));
    console.feed_progress(task("a", ProgressState::Running, 300, None));
    console.feed_progress(task("a", ProgressState::Running, 1500, None));
    console.feed_progress(task("a", ProgressState::Running, 1500, None));

    assert_eq!(console.text(), expected("running_indeterminate"));
    assert_eq!(console.text(), "Downloading\n#####");
}

#[test]
fn an_unannounced_indeterminate_task_starts_counting_from_zero() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Running, 900, None));
    assert_eq!(
        console.text(),
        expected("running_indeterminate_without_started")
    );
}

#[test]
fn a_finished_determinate_task_closes_its_bar() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Finished, 200, Some(200)));
    assert_eq!(console.text(), expected("finished_determinate"));
    assert!(console.text().ends_with(" 100.0%\n"));
}

#[test]
fn a_finished_indeterminate_task_closes_with_a_full_percentage() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Started, 0, None));
    console.feed_progress(task("a", ProgressState::Running, 600, None));
    console.feed_progress(task("a", ProgressState::Finished, 1000, None));

    assert_eq!(console.text(), expected("finished_indeterminate"));
    assert_eq!(console.text(), "Downloading\n### 100.0%\n");
}

#[test]
fn a_failed_task_only_closes_the_line() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Failed, 5, Some(200)));
    assert_eq!(console.text(), expected("failed"));
    assert_eq!(console.text(), "\n");
}

#[test]
fn concurrent_tasks_are_counted_separately() {
    let console = Console::new();
    console.feed_progress(named_task("a", "First", ProgressState::Started, 0, None));
    console.feed_progress(named_task("b", "Second", ProgressState::Started, 0, None));
    console.feed_progress(task("a", ProgressState::Running, 600, None));
    console.feed_progress(task("b", ProgressState::Running, 300, None));
    console.feed_progress(task("a", ProgressState::Running, 900, None));

    assert_eq!(console.text(), expected("two_tasks"));
    assert_eq!(console.text(), "First\nSecond\n####");
}

#[test]
fn a_reused_identifier_starts_counting_again() {
    let console = Console::new();
    console.feed_progress(task("a", ProgressState::Started, 0, None));
    console.feed_progress(task("a", ProgressState::Running, 900, None));
    console.feed_progress(task("a", ProgressState::Finished, 900, None));
    console.feed_progress(task("a", ProgressState::Started, 0, None));
    console.feed_progress(task("a", ProgressState::Running, 900, None));

    assert_eq!(console.text(), expected("reused_id_after_finish"));
}

#[test]
fn a_custom_bar_width_is_honoured() {
    let console = Console::configured(10, 300);
    console.feed_progress(task("a", ProgressState::Running, 1, Some(3)));
    assert_eq!(console.text(), expected("custom_bar_width"));
    assert_eq!(console.text(), "\r###         33.3%");
}

#[test]
fn a_custom_hash_size_is_honoured() {
    let console = Console::configured(72, 4);
    console.feed_progress(task("a", ProgressState::Started, 0, None));
    console.feed_progress(task("a", ProgressState::Running, 10, None));
    assert_eq!(console.text(), expected("custom_units_per_hash"));
    assert_eq!(console.text(), "Downloading\n##");
}

#[test]
fn a_console_sink_can_be_registered_with_a_logger() {
    let written: Written = Arc::new(Mutex::new(String::new()));
    let target = written.clone();
    let sink = Arc::new(ConsoleSink::with_output(Box::new(move |text: &str| {
        target.lock().expect("output lock").push_str(text);
    })));

    let logger = rust_ufbt::Logger::builder()
        .clock(Arc::new(time))
        .sink(sink)
        .build();
    logger.raw("hello");

    assert_eq!(written.lock().expect("output lock").as_str(), "hello\n");
}
