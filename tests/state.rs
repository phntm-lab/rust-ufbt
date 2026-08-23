use std::fs;

use serde_json::{Map, Value, json};

use rust_ufbt::state::{ALWAYS_UPDATE_VERSIONS, State};

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(values) => values,
        other => panic!("expected an object, got {other}"),
    }
}

fn reference_vectors() -> Map<String, Value> {
    let raw = fs::read_to_string("tests/fixtures/state_json.json").expect("read fixture");
    object(serde_json::from_str(&raw).expect("parse fixture"))
}

fn state_of(value: Value) -> State {
    State::new(object(value))
}

#[test]
fn always_update_versions_match_ufbt() {
    assert_eq!(ALWAYS_UPDATE_VERSIONS, ["unknown", "local"]);
}

#[test]
fn encoding_matches_the_reference_vectors() {
    let vectors = reference_vectors();
    assert!(!vectors.is_empty());
    for (name, vector) in &vectors {
        let vector = vector.as_object().expect("vector object");
        let values = vector["values"].clone();
        let expected = vector["encoded"].as_str().expect("encoded string");

        assert_eq!(state_of(values).to_json(), expected, "vector {name}");
    }
}

#[test]
fn writing_matches_the_reference_vectors_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ufbt_state.json");
    for (name, vector) in &reference_vectors() {
        let vector = vector.as_object().expect("vector object");
        let values = vector["values"].clone();
        let expected = vector["encoded"].as_str().expect("encoded string");

        state_of(values).write(&file).expect("write state");

        assert_eq!(
            fs::read(&file).unwrap(),
            expected.as_bytes(),
            "vector {name}"
        );
    }
}

#[test]
fn round_trip_preserves_values_and_key_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ufbt_state.json");
    let state = state_of(json!({
        "z": "1",
        "hw_target": "f7",
        "a": "2",
        "nested": {"b": [1, "two", null]},
    }));

    state.write(&file).expect("write state");
    let read = State::read(&file)
        .expect("read state")
        .expect("state present");

    assert_eq!(read, state);
    assert_eq!(
        read.values().keys().collect::<Vec<_>>(),
        ["z", "hw_target", "a", "nested"]
    );
    assert_eq!(read.to_json(), state.to_json());
}

#[test]
fn write_creates_the_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir
        .path()
        .join("deep")
        .join("deeper")
        .join("ufbt_state.json");

    state_of(json!({"hw_target": "f7"}))
        .write(&file)
        .expect("write state");

    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "{\n    \"hw_target\": \"f7\"\n}"
    );
}

#[test]
fn write_overwrites_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ufbt_state.json");
    state_of(json!({"version": "long previous contents"}))
        .write(&file)
        .expect("write first");

    state_of(json!({"version": "1"}))
        .write(&file)
        .expect("write second");

    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "{\n    \"version\": \"1\"\n}"
    );
}

#[test]
fn reading_a_missing_file_yields_nothing() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(
        State::read(dir.path().join("ufbt_state.json")).expect("read state"),
        None
    );
}

#[test]
fn reading_a_directory_yields_nothing() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(State::read(dir.path()).expect("read state"), None);
}

#[test]
fn reading_json_that_is_not_an_object_yields_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ufbt_state.json");
    for contents in ["[1, 2]", "\"text\"", "42", "null", "true"] {
        fs::write(&file, contents).unwrap();

        assert_eq!(State::read(&file).expect("read state"), None, "{contents}");
    }
}

#[test]
fn reading_malformed_json_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ufbt_state.json");
    fs::write(&file, "{\"hw_target\": ").unwrap();

    assert!(State::read(&file).is_err());
}

#[test]
fn reading_an_empty_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ufbt_state.json");
    fs::write(&file, "").unwrap();

    assert!(State::read(&file).is_err());
}

#[test]
fn getters_read_the_well_known_keys() {
    let state = state_of(json!({
        "hw_target": "f7",
        "mode": "channel",
        "version": "1.3.4",
        "json_index": "https://update.flipperzero.one/firmware/directory.json",
        "channel": "release",
    }));

    assert_eq!(state.hw_target(), Some("f7"));
    assert_eq!(state.mode(), Some("channel"));
    assert_eq!(state.version(), Some("1.3.4"));
    assert_eq!(
        state.json_index(),
        Some("https://update.flipperzero.one/firmware/directory.json")
    );
    assert_eq!(state.channel(), Some("release"));
}

#[test]
fn getters_yield_nothing_for_absent_keys() {
    let state = State::new(Map::new());

    assert_eq!(state.hw_target(), None);
    assert_eq!(state.mode(), None);
    assert_eq!(state.version(), None);
    assert_eq!(state.json_index(), None);
    assert_eq!(state.channel(), None);
}

#[test]
fn getters_yield_nothing_for_a_null_value() {
    let state = state_of(json!({"hw_target": null}));

    assert_eq!(state.hw_target(), None);
}

#[test]
fn getters_yield_nothing_for_a_value_that_is_not_a_string() {
    let state = state_of(json!({"version": 7, "channel": ["release"]}));

    assert_eq!(state.version(), None);
    assert_eq!(state.channel(), None);
}

#[test]
fn version_unknown_for_the_always_update_versions() {
    for version in ALWAYS_UPDATE_VERSIONS {
        assert!(
            state_of(json!({ "version": version })).is_version_unknown(),
            "{version}"
        );
    }
}

#[test]
fn version_unknown_without_a_version() {
    assert!(State::new(Map::new()).is_version_unknown());
    assert!(state_of(json!({"version": null})).is_version_unknown());
}

#[test]
fn version_known_for_an_ordinary_version() {
    assert!(!state_of(json!({"version": "1.3.4"})).is_version_unknown());
    assert!(!state_of(json!({"version": ""})).is_version_unknown());
    assert!(!state_of(json!({"version": "UNKNOWN"})).is_version_unknown());
}

#[test]
fn values_exposes_everything_that_was_stored() {
    let state = state_of(json!({"hw_target": "f7", "extra": 1}));

    assert_eq!(state.values().len(), 2);
    assert_eq!(state.values()["extra"], json!(1));
}
