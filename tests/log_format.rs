use std::collections::BTreeMap;
use std::sync::LazyLock;

use chrono::{DateTime, Duration, Local, TimeZone};

use rust_ufbt::log::format::{
    BAR_CHAR, DEFAULT_BAR_WIDTH, build, message, percent, progress_bar, timestamp,
};
use rust_ufbt::log::{LogLevel, Progress, ProgressState};

static VECTORS: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    let raw = include_str!("fixtures/log_format.json");
    serde_json::from_str(raw).expect("reference vectors are valid JSON")
});

fn expected(key: &str) -> &'static str {
    VECTORS
        .get(key)
        .unwrap_or_else(|| panic!("no reference vector named {key}"))
}

fn at(hour: u32, minute: u32, second: u32, micros: i64) -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 8, 23, hour, minute, second)
        .earliest()
        .expect("local time exists")
        + Duration::microseconds(micros)
}

fn sample_time() -> DateTime<Local> {
    at(14, 5, 9, 7_000)
}

fn running(current: u64, total: Option<u64>) -> Progress {
    Progress::new(
        "progress-1",
        "t",
        ProgressState::Running,
        current,
        total,
        None,
    )
}

fn in_state(state: ProgressState, current: u64, total: Option<u64>) -> Progress {
    Progress::new("progress-1", "t", state, current, total, None)
}

#[test]
fn constants_match_original() {
    assert_eq!(DEFAULT_BAR_WIDTH, 72);
    assert_eq!(BAR_CHAR, '#');
}

#[test]
fn timestamp_pads_every_field() {
    assert_eq!(timestamp(sample_time()), expected("ts"));
    assert_eq!(timestamp(sample_time()), "14:05:09.007");

    let midnight = Local
        .with_ymd_and_hms(2026, 1, 2, 0, 0, 0)
        .earliest()
        .expect("local time exists");
    assert_eq!(timestamp(midnight), expected("ts_min"));

    let last_moment = Local
        .with_ymd_and_hms(2026, 12, 31, 23, 59, 59)
        .earliest()
        .expect("local time exists")
        + Duration::milliseconds(999);
    assert_eq!(timestamp(last_moment), expected("ts_max"));
}

#[test]
fn timestamp_drops_sub_millisecond_precision() {
    assert_eq!(timestamp(at(14, 5, 9, 7_999)), expected("ts_micro"));
    assert_eq!(timestamp(at(14, 5, 9, 7_999)), "14:05:09.007");
}

#[test]
fn message_carries_the_level_letter() {
    let time = sample_time();
    assert_eq!(
        message(time, LogLevel::Debug, "hello world"),
        expected("msg_debug")
    );
    assert_eq!(
        message(time, LogLevel::Info, "hello world"),
        expected("msg_info")
    );
    assert_eq!(
        message(time, LogLevel::Warning, "hello world"),
        expected("msg_warning")
    );
    assert_eq!(
        message(time, LogLevel::Error, "hello world"),
        expected("msg_error")
    );
    assert_eq!(
        message(time, LogLevel::Critical, "hello world"),
        expected("msg_critical")
    );
    assert_eq!(message(time, LogLevel::Info, ""), expected("msg_empty"));
}

#[test]
fn build_indents_with_tabs() {
    assert_eq!(build("CC", "src/main.c", &[]), expected("build_none"));
    assert_eq!(build("CC", "src/main.c", &[]), "\tCC\tsrc/main.c");

    assert_eq!(
        build("CC", "src/main.c", &["-Os".to_owned()]),
        expected("build_one")
    );
    assert_eq!(
        build("LINK", "app.fap", &["-Os".to_owned(), "-Wall".to_owned()]),
        expected("build_many")
    );
    assert_eq!(
        build("LINK", "app.fap", &["-Os".to_owned(), "-Wall".to_owned()]),
        "\tLINK\tapp.fap\n\t\t-Os\n\t\t-Wall"
    );
    assert_eq!(build("", "", &[String::new()]), expected("build_empty"));
}

#[test]
fn percent_is_padded_to_five_characters() {
    assert_eq!(percent(0.0), expected("pct_0"));
    assert_eq!(percent(0.0), "  0.0%");
    assert_eq!(percent(5.5), expected("pct_5_5"));
    assert_eq!(percent(5.5), "  5.5%");
    assert_eq!(percent(100.0), expected("pct_100"));
    assert_eq!(percent(100.0), "100.0%");
    assert_eq!(percent(12.34), expected("pct_12_34"));
    assert_eq!(percent(99.99), expected("pct_99_99"));
    assert_eq!(percent(33.333), expected("pct_33_333"));

    for key in ["pct_0", "pct_5_5", "pct_100", "pct_12_34"] {
        assert_eq!(expected(key).len(), 6);
    }
}

#[test]
fn percent_rounds_halves_away_from_zero() {
    assert_eq!(percent(0.25), expected("pct_0_25"));
    assert_eq!(percent(0.25), "  0.3%");
    assert_eq!(percent(0.75), expected("pct_0_75"));
    assert_eq!(percent(7.25), expected("pct_7_25"));
    assert_eq!(percent(7.25), "  7.3%");
    assert_eq!(percent(33.75), expected("pct_33_75"));
    assert_eq!(percent(33.75), " 33.8%");
}

#[test]
fn percent_rounds_near_halves_by_their_exact_value() {
    assert_eq!(percent(0.05), expected("pct_0_05"));
    assert_eq!(percent(0.05), "  0.1%");
    assert_eq!(percent(0.15), expected("pct_0_15"));
    assert_eq!(percent(0.15), "  0.1%");
}

#[test]
fn progress_bar_fills_by_fraction() {
    assert_eq!(
        progress_bar(&running(0, Some(200)), DEFAULT_BAR_WIDTH),
        expected("bar_0")
    );
    assert_eq!(
        progress_bar(&running(100, Some(200)), DEFAULT_BAR_WIDTH),
        expected("bar_50")
    );
    assert_eq!(
        progress_bar(&running(200, Some(200)), DEFAULT_BAR_WIDTH),
        expected("bar_100")
    );
    assert_eq!(
        progress_bar(&running(7, Some(72)), DEFAULT_BAR_WIDTH),
        expected("bar_7_of_72")
    );

    let bar = progress_bar(&running(100, Some(200)), DEFAULT_BAR_WIDTH);
    assert_eq!(bar.chars().filter(|c| *c == BAR_CHAR).count(), 36);
    assert_eq!(bar.len(), DEFAULT_BAR_WIDTH + " 100.0%".len());
}

#[test]
fn progress_bar_clamps_current_above_total() {
    assert_eq!(
        progress_bar(&running(500, Some(200)), DEFAULT_BAR_WIDTH),
        expected("bar_over")
    );
    assert_eq!(
        progress_bar(&running(500, Some(200)), DEFAULT_BAR_WIDTH),
        expected("bar_100")
    );
}

#[test]
fn progress_bar_reads_indeterminate_as_empty() {
    assert_eq!(
        progress_bar(&running(5, None), DEFAULT_BAR_WIDTH),
        expected("bar_indeterminate")
    );
    assert_eq!(
        progress_bar(&running(5, None), DEFAULT_BAR_WIDTH),
        expected("bar_0")
    );
}

#[test]
fn progress_bar_reads_a_done_task_as_complete() {
    assert_eq!(
        progress_bar(
            &in_state(ProgressState::Finished, 5, None),
            DEFAULT_BAR_WIDTH
        ),
        expected("bar_indeterminate_finished")
    );
    assert_eq!(
        progress_bar(
            &in_state(ProgressState::Failed, 1, Some(200)),
            DEFAULT_BAR_WIDTH
        ),
        expected("bar_failed")
    );
    assert_eq!(
        progress_bar(
            &in_state(ProgressState::Failed, 1, Some(200)),
            DEFAULT_BAR_WIDTH
        ),
        expected("bar_100")
    );
}

#[test]
fn progress_bar_honours_a_custom_width() {
    assert_eq!(
        progress_bar(&running(1, Some(3)), 0),
        expected("bar_width_0")
    );
    assert_eq!(
        progress_bar(&running(1, Some(3)), 1),
        expected("bar_width_1")
    );
    assert_eq!(
        progress_bar(&running(1, Some(3)), 10),
        expected("bar_width_10")
    );
    assert_eq!(progress_bar(&running(1, Some(3)), 10), "###         33.3%");
    assert_eq!(
        progress_bar(&in_state(ProgressState::Finished, 1, Some(3)), 10),
        expected("bar_width_10_done")
    );
}
