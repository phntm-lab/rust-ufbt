use chrono::{Local, TimeZone};

use rust_ufbt::log::{LogEvent, LogLevel, Progress, ProgressState};

fn time(millis: u32) -> chrono::DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 8, 23, 14, 5, 9)
        .single()
        .expect("unambiguous local time")
        + chrono::Duration::milliseconds(i64::from(millis))
}

#[test]
fn message_event_keeps_every_field() {
    let event = LogEvent::Message {
        time: time(0),
        level: LogLevel::Warning,
        message: "SDK is out of date".to_owned(),
        formatted: "14:05:09.000 [W] SDK is out of date".to_owned(),
    };

    let LogEvent::Message {
        time: at,
        level,
        message,
        formatted,
    } = &event
    else {
        panic!("expected a message event");
    };
    assert_eq!(*at, time(0));
    assert_eq!(*level, LogLevel::Warning);
    assert_eq!(message, "SDK is out of date");
    assert_eq!(formatted, "14:05:09.000 [W] SDK is out of date");
}

#[test]
fn build_event_keeps_every_field() {
    let event = LogEvent::Build {
        time: time(0),
        tag: "CC".to_owned(),
        value: "src/main.c".to_owned(),
        details: vec!["-Os".to_owned(), "-Wall".to_owned()],
        formatted: "\tCC\tsrc/main.c\n\t\t-Os\n\t\t-Wall".to_owned(),
    };

    let LogEvent::Build {
        tag,
        value,
        details,
        formatted,
        ..
    } = &event
    else {
        panic!("expected a build event");
    };
    assert_eq!(tag, "CC");
    assert_eq!(value, "src/main.c");
    assert_eq!(details, &["-Os".to_owned(), "-Wall".to_owned()]);
    assert_eq!(formatted, "\tCC\tsrc/main.c\n\t\t-Os\n\t\t-Wall");
}

#[test]
fn raw_event_keeps_every_field() {
    let event = LogEvent::Raw {
        time: time(0),
        text: "plain output".to_owned(),
        newline: false,
    };

    let LogEvent::Raw { text, newline, .. } = &event else {
        panic!("expected a raw event");
    };
    assert_eq!(text, "plain output");
    assert!(!*newline);
}

#[test]
fn progress_event_keeps_every_field() {
    let snapshot = Progress::new(
        "progress-1",
        "Downloading",
        ProgressState::Running,
        50,
        Some(200),
        None,
    );
    let event = LogEvent::Progress {
        time: time(0),
        progress: snapshot.clone(),
    };

    let LogEvent::Progress { progress, .. } = &event else {
        panic!("expected a progress event");
    };
    assert_eq!(*progress, snapshot);
    assert_eq!(progress.fraction(), Some(0.25));
}

#[test]
fn time_is_available_on_every_variant() {
    let events = [
        LogEvent::Message {
            time: time(1),
            level: LogLevel::Info,
            message: String::new(),
            formatted: String::new(),
        },
        LogEvent::Build {
            time: time(2),
            tag: String::new(),
            value: String::new(),
            details: Vec::new(),
            formatted: String::new(),
        },
        LogEvent::Raw {
            time: time(3),
            text: String::new(),
            newline: true,
        },
        LogEvent::Progress {
            time: time(4),
            progress: Progress::new("id", "title", ProgressState::Started, 0, None, None),
        },
    ];

    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.time(), time(index as u32 + 1));
    }
}

#[test]
fn events_compare_by_value() {
    let first = LogEvent::Raw {
        time: time(0),
        text: "same".to_owned(),
        newline: true,
    };
    let second = first.clone();
    let other = LogEvent::Raw {
        time: time(0),
        text: "same".to_owned(),
        newline: false,
    };

    assert_eq!(first, second);
    assert_ne!(first, other);
}
