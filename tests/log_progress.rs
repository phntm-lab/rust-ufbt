use std::thread::sleep;
use std::time::Duration;

use rust_ufbt::log::{Progress, ProgressState, ProgressThrottle};

fn progress(state: ProgressState, current: u64, total: Option<u64>) -> Progress {
    Progress::new("progress-1", "Downloading", state, current, total, None)
}

#[test]
fn indeterminate_when_total_is_missing_or_zero() {
    assert!(progress(ProgressState::Running, 5, None).indeterminate());
    assert!(progress(ProgressState::Running, 5, Some(0)).indeterminate());
    assert!(!progress(ProgressState::Running, 5, Some(1)).indeterminate());
}

#[test]
fn fraction_is_none_for_indeterminate() {
    assert_eq!(progress(ProgressState::Running, 5, None).fraction(), None);
    assert_eq!(
        progress(ProgressState::Running, 5, Some(0)).fraction(),
        None
    );
    assert_eq!(progress(ProgressState::Running, 5, None).percent(), None);
    assert_eq!(progress(ProgressState::Running, 5, Some(0)).percent(), None);
}

#[test]
fn fraction_divides_current_by_total() {
    assert_eq!(
        progress(ProgressState::Running, 0, Some(200)).fraction(),
        Some(0.0)
    );
    assert_eq!(
        progress(ProgressState::Running, 50, Some(200)).fraction(),
        Some(0.25)
    );
    assert_eq!(
        progress(ProgressState::Running, 200, Some(200)).fraction(),
        Some(1.0)
    );
}

#[test]
fn fraction_is_clamped_when_current_exceeds_total() {
    assert_eq!(
        progress(ProgressState::Running, 500, Some(200)).fraction(),
        Some(1.0)
    );
    assert_eq!(
        progress(ProgressState::Running, 500, Some(200)).percent(),
        Some(100.0)
    );
}

#[test]
fn percent_scales_fraction_by_hundred() {
    assert_eq!(
        progress(ProgressState::Running, 50, Some(200)).percent(),
        Some(25.0)
    );
    assert_eq!(
        progress(ProgressState::Running, 0, Some(200)).percent(),
        Some(0.0)
    );
}

#[test]
fn is_done_only_for_finished_and_failed() {
    assert!(!progress(ProgressState::Started, 0, Some(10)).is_done());
    assert!(!progress(ProgressState::Running, 5, Some(10)).is_done());
    assert!(progress(ProgressState::Finished, 10, Some(10)).is_done());
    assert!(progress(ProgressState::Failed, 5, Some(10)).is_done());
}

#[test]
fn accessors_return_constructor_values() {
    let progress = Progress::new(
        "progress-7",
        "Unpacking",
        ProgressState::Running,
        3,
        Some(9),
        Some("entry.bin".to_owned()),
    );

    assert_eq!(progress.id(), "progress-7");
    assert_eq!(progress.title(), "Unpacking");
    assert_eq!(progress.state(), ProgressState::Running);
    assert_eq!(progress.current(), 3);
    assert_eq!(progress.total(), Some(9));
    assert_eq!(progress.message(), Some("entry.bin"));
}

#[test]
fn throttle_holds_back_the_first_update() {
    let mut throttle = ProgressThrottle::new();
    assert!(!throttle.should_emit(Some(0.0)));
    assert!(!throttle.should_emit(Some(0.5)));
}

#[test]
fn throttle_holds_back_updates_before_min_interval() {
    let mut throttle = ProgressThrottle::with_limits(0.002, Duration::from_millis(40));
    sleep(Duration::from_millis(60));
    assert!(throttle.should_emit(Some(0.1)));
    assert!(!throttle.should_emit(Some(0.9)));
}

#[test]
fn throttle_holds_back_updates_below_min_delta() {
    let mut throttle = ProgressThrottle::with_limits(0.002, Duration::from_millis(40));
    sleep(Duration::from_millis(60));
    assert!(throttle.should_emit(Some(0.5)));
    sleep(Duration::from_millis(60));
    assert!(!throttle.should_emit(Some(0.5005)));
    assert!(throttle.should_emit(Some(0.6)));
}

#[test]
fn throttle_always_emits_the_first_completion() {
    let mut throttle = ProgressThrottle::new();
    assert!(throttle.should_emit(Some(1.0)));
    assert!(!throttle.should_emit(Some(1.0)));
}

#[test]
fn throttle_emits_completion_even_after_a_recent_update() {
    let mut throttle = ProgressThrottle::with_limits(0.002, Duration::from_millis(40));
    sleep(Duration::from_millis(60));
    assert!(throttle.should_emit(Some(0.5)));
    assert!(throttle.should_emit(Some(1.0)));
}

#[test]
fn throttle_ticks_on_time_alone_when_fraction_is_unknown() {
    let mut throttle = ProgressThrottle::with_limits(0.002, Duration::from_millis(40));
    assert!(!throttle.should_emit(None));
    sleep(Duration::from_millis(60));
    assert!(throttle.should_emit(None));
    assert!(!throttle.should_emit(None));
    sleep(Duration::from_millis(60));
    assert!(throttle.should_emit(None));
}

#[test]
fn throttle_exposes_default_limits() {
    assert!((ProgressThrottle::DEFAULT_MIN_DELTA - 0.002).abs() < f64::EPSILON);
    assert_eq!(
        ProgressThrottle::DEFAULT_MIN_INTERVAL,
        Duration::from_millis(150)
    );
    assert!((ProgressThrottle::default().min_delta() - 0.002).abs() < f64::EPSILON);
}
