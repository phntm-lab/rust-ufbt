use rust_ufbt::LogLevel;

const ALL: [LogLevel; 5] = [
    LogLevel::Debug,
    LogLevel::Info,
    LogLevel::Warning,
    LogLevel::Error,
    LogLevel::Critical,
];

#[test]
fn severity_matches_original() {
    assert_eq!(LogLevel::Debug.severity(), 10);
    assert_eq!(LogLevel::Info.severity(), 20);
    assert_eq!(LogLevel::Warning.severity(), 30);
    assert_eq!(LogLevel::Error.severity(), 40);
    assert_eq!(LogLevel::Critical.severity(), 50);
}

#[test]
fn level_name_is_upper_case_variant_name() {
    assert_eq!(LogLevel::Debug.level_name(), "DEBUG");
    assert_eq!(LogLevel::Info.level_name(), "INFO");
    assert_eq!(LogLevel::Warning.level_name(), "WARNING");
    assert_eq!(LogLevel::Error.level_name(), "ERROR");
    assert_eq!(LogLevel::Critical.level_name(), "CRITICAL");
}

#[test]
fn letter_is_first_character_of_level_name() {
    assert_eq!(LogLevel::Debug.letter(), 'D');
    assert_eq!(LogLevel::Info.letter(), 'I');
    assert_eq!(LogLevel::Warning.letter(), 'W');
    assert_eq!(LogLevel::Error.letter(), 'E');
    assert_eq!(LogLevel::Critical.letter(), 'C');

    for level in ALL {
        assert_eq!(
            level.letter(),
            level.level_name().chars().next().unwrap(),
            "letter must be the first character of {}",
            level.level_name()
        );
    }
}

#[test]
fn ordering_follows_severity() {
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warning);
    assert!(LogLevel::Warning < LogLevel::Error);
    assert!(LogLevel::Error < LogLevel::Critical);

    let mut shuffled = [
        LogLevel::Critical,
        LogLevel::Debug,
        LogLevel::Error,
        LogLevel::Info,
        LogLevel::Warning,
    ];
    shuffled.sort();
    assert_eq!(shuffled, ALL);

    for window in ALL.windows(2) {
        assert!(window[0].severity() < window[1].severity());
        assert!(window[0] < window[1]);
    }
}

#[test]
fn is_enabled_for_semantics_hold_via_severity() {
    for level in ALL {
        for message_level in ALL {
            assert_eq!(
                message_level.severity() >= level.severity(),
                message_level >= level
            );
        }
    }
}
