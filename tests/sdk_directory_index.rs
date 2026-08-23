use rust_ufbt::sdk::{DirectoryIndex, FileType, IndexFile, IndexVersion};
use serde_json::json;

fn parse(value: serde_json::Value) -> DirectoryIndex {
    serde_json::from_value(value).unwrap()
}

fn full_index() -> DirectoryIndex {
    parse(json!({
        "channels": [
            {
                "id": "development",
                "title": "Development",
                "description": "Bleeding edge",
                "versions": [
                    {
                        "version": "0.104.0-rc",
                        "changelog": "Nothing to see here",
                        "timestamp": 1_700_000_000,
                        "files": [
                            {
                                "url": "https://update.flipperzero.one/dev/sdk.zip",
                                "target": "f7",
                                "type": "sdk_zip",
                                "sha256": "abc123"
                            },
                            {
                                "url": "https://update.flipperzero.one/dev/lib.zip",
                                "target": "f7",
                                "type": "lib_zip"
                            }
                        ]
                    }
                ]
            },
            {
                "id": "release",
                "versions": []
            }
        ]
    }))
}

#[test]
fn a_full_index_is_read_field_for_field() {
    let index = full_index();

    assert_eq!(index.channels().len(), 2);

    let development = index.find_channel("development").unwrap();
    assert_eq!(development.id(), "development");
    assert_eq!(development.title(), Some("Development"));
    assert_eq!(development.description(), Some("Bleeding edge"));
    assert_eq!(development.versions().len(), 1);

    let version = &development.versions()[0];
    assert_eq!(version.version(), "0.104.0-rc");
    assert_eq!(version.changelog(), Some("Nothing to see here"));
    assert_eq!(version.timestamp(), Some(1_700_000_000));
    assert_eq!(version.files().len(), 2);

    let file = &version.files()[0];
    assert_eq!(file.url(), "https://update.flipperzero.one/dev/sdk.zip");
    assert_eq!(file.target(), "f7");
    assert_eq!(file.file_type(), "sdk_zip");
    assert_eq!(file.sha256(), Some("abc123"));

    assert_eq!(version.files()[1].sha256(), None);
}

#[test]
fn absent_fields_read_as_empty_strings_and_nothing() {
    let file: IndexFile = serde_json::from_value(json!({})).unwrap();

    assert_eq!(file.url(), "");
    assert_eq!(file.target(), "");
    assert_eq!(file.file_type(), "");
    assert_eq!(file.sha256(), None);

    let version: IndexVersion = serde_json::from_value(json!({})).unwrap();

    assert_eq!(version.version(), "");
    assert_eq!(version.changelog(), None);
    assert_eq!(version.timestamp(), None);
    assert!(version.files().is_empty());
}

#[test]
fn null_fields_read_as_empty_strings_and_nothing() {
    let file: IndexFile = serde_json::from_value(json!({
        "url": null,
        "target": null,
        "type": null,
        "sha256": null
    }))
    .unwrap();

    assert_eq!(file.url(), "");
    assert_eq!(file.target(), "");
    assert_eq!(file.file_type(), "");
    assert_eq!(file.sha256(), None);
}

#[test]
fn fields_of_the_wrong_type_read_as_absent() {
    let file: IndexFile = serde_json::from_value(json!({
        "url": 42,
        "target": ["f7"],
        "type": { "id": "sdk_zip" },
        "sha256": true
    }))
    .unwrap();

    assert_eq!(file.url(), "");
    assert_eq!(file.target(), "");
    assert_eq!(file.file_type(), "");
    assert_eq!(file.sha256(), None);

    let version: IndexVersion = serde_json::from_value(json!({
        "version": 1,
        "timestamp": "yesterday",
        "files": "none"
    }))
    .unwrap();

    assert_eq!(version.version(), "");
    assert_eq!(version.timestamp(), None);
    assert!(version.files().is_empty());
}

#[test]
fn array_elements_that_are_not_objects_are_skipped() {
    let index = parse(json!({
        "channels": [
            "release",
            42,
            null,
            ["release"],
            {
                "id": "release",
                "versions": [
                    "0.1.0",
                    null,
                    {
                        "version": "0.1.0",
                        "files": [7, null, { "type": "sdk_zip", "target": "f7" }]
                    }
                ]
            }
        ]
    }));

    assert_eq!(index.channels().len(), 1);

    let release = index.find_channel("release").unwrap();
    assert_eq!(release.versions().len(), 1);
    assert_eq!(release.versions()[0].files().len(), 1);
    assert_eq!(release.versions()[0].files()[0].file_type(), "sdk_zip");
}

#[test]
fn an_index_without_channels_is_empty() {
    assert!(parse(json!({})).channels().is_empty());
    assert!(parse(json!({ "channels": null })).channels().is_empty());
    assert!(parse(json!({ "channels": [] })).channels().is_empty());
    assert!(
        parse(json!({ "channels": { "release": {} } }))
            .channels()
            .is_empty()
    );
}

#[test]
fn a_document_that_is_not_an_object_is_empty() {
    assert!(parse(json!([])).channels().is_empty());
    assert!(parse(json!("release")).channels().is_empty());
    assert!(parse(json!(null)).channels().is_empty());
}

#[test]
fn find_file_matches_both_type_and_target() {
    let index = full_index();
    let version = &index.find_channel("development").unwrap().versions()[0];

    let found = version.find_file(FileType::SdkZip, "f7").unwrap();
    assert_eq!(found.url(), "https://update.flipperzero.one/dev/sdk.zip");

    assert_eq!(
        version.find_file(FileType::LibZip, "f7").unwrap().url(),
        "https://update.flipperzero.one/dev/lib.zip"
    );
}

#[test]
fn find_file_yields_nothing_for_a_type_or_target_that_is_absent() {
    let index = full_index();
    let version = &index.find_channel("development").unwrap().versions()[0];

    assert!(version.find_file(FileType::ResourcesTgz, "f7").is_none());
    assert!(version.find_file(FileType::SdkZip, "f18").is_none());
    assert!(version.find_file(FileType::SdkZip, "").is_none());
}

#[test]
fn find_file_yields_the_first_match() {
    let version: IndexVersion = serde_json::from_value(json!({
        "files": [
            { "url": "first", "target": "f7", "type": "sdk_zip" },
            { "url": "second", "target": "f7", "type": "sdk_zip" }
        ]
    }))
    .unwrap();

    assert_eq!(
        version.find_file(FileType::SdkZip, "f7").unwrap().url(),
        "first"
    );
}

#[test]
fn find_channel_matches_the_identifier_exactly() {
    let index = full_index();

    assert!(index.find_channel("release").is_some());
    assert!(index.find_channel("Release").is_none());
    assert!(index.find_channel("release-candidate").is_none());
    assert!(index.find_channel("").is_none());
}

#[test]
fn a_channel_without_an_identifier_is_found_by_the_empty_string() {
    let index = parse(json!({ "channels": [{ "versions": [] }] }));

    assert_eq!(index.find_channel("").unwrap().id(), "");
}
