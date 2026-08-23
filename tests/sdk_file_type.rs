use rust_ufbt::sdk::{FileType, SdkError, UpdateChannel};

const ALL_FILE_TYPES: [FileType; 14] = [
    FileType::SdkZip,
    FileType::LibZip,
    FileType::Core2FirmwareTgz,
    FileType::ResourcesTgz,
    FileType::ScriptsTgz,
    FileType::UpdateTgz,
    FileType::FirmwareElf,
    FileType::FullBin,
    FileType::FullDfu,
    FileType::FullJson,
    FileType::UpdaterBin,
    FileType::UpdaterDfu,
    FileType::UpdaterElf,
    FileType::UpdaterJson,
];

const ALL_CHANNELS: [UpdateChannel; 3] = [
    UpdateChannel::Dev,
    UpdateChannel::Rc,
    UpdateChannel::Release,
];

#[test]
fn every_file_type_reports_its_identifier() {
    assert_eq!(FileType::SdkZip.id(), "sdk_zip");
    assert_eq!(FileType::LibZip.id(), "lib_zip");
    assert_eq!(FileType::Core2FirmwareTgz.id(), "core2_firmware_tgz");
    assert_eq!(FileType::ResourcesTgz.id(), "resources_tgz");
    assert_eq!(FileType::ScriptsTgz.id(), "scripts_tgz");
    assert_eq!(FileType::UpdateTgz.id(), "update_tgz");
    assert_eq!(FileType::FirmwareElf.id(), "firmware_elf");
    assert_eq!(FileType::FullBin.id(), "full_bin");
    assert_eq!(FileType::FullDfu.id(), "full_dfu");
    assert_eq!(FileType::FullJson.id(), "full_json");
    assert_eq!(FileType::UpdaterBin.id(), "updater_bin");
    assert_eq!(FileType::UpdaterDfu.id(), "updater_dfu");
    assert_eq!(FileType::UpdaterElf.id(), "updater_elf");
    assert_eq!(FileType::UpdaterJson.id(), "updater_json");
}

#[test]
fn file_type_identifiers_are_unique() {
    for (index, left) in ALL_FILE_TYPES.iter().enumerate() {
        for right in &ALL_FILE_TYPES[index + 1..] {
            assert_ne!(left.id(), right.id());
        }
    }
}

#[test]
fn every_channel_reports_its_identifier() {
    assert_eq!(UpdateChannel::Dev.id(), "development");
    assert_eq!(UpdateChannel::Rc.id(), "release-candidate");
    assert_eq!(UpdateChannel::Release.id(), "release");
}

#[test]
fn every_channel_reports_its_key() {
    assert_eq!(UpdateChannel::Dev.key(), "dev");
    assert_eq!(UpdateChannel::Rc.key(), "rc");
    assert_eq!(UpdateChannel::Release.key(), "release");
}

#[test]
fn python_name_is_the_upper_case_key() {
    assert_eq!(UpdateChannel::Dev.python_name(), "DEV");
    assert_eq!(UpdateChannel::Rc.python_name(), "RC");
    assert_eq!(UpdateChannel::Release.python_name(), "RELEASE");

    for channel in ALL_CHANNELS {
        assert_eq!(channel.python_name(), channel.key().to_uppercase());
    }
}

#[test]
fn by_key_resolves_every_channel() {
    for channel in ALL_CHANNELS {
        assert_eq!(UpdateChannel::by_key(channel.key()).unwrap(), channel);
    }
}

#[test]
fn by_key_ignores_case() {
    assert_eq!(UpdateChannel::by_key("DEV").unwrap(), UpdateChannel::Dev);
    assert_eq!(UpdateChannel::by_key("Rc").unwrap(), UpdateChannel::Rc);
    assert_eq!(
        UpdateChannel::by_key("ReLeAsE").unwrap(),
        UpdateChannel::Release
    );
}

#[test]
fn by_key_rejects_an_unknown_key() {
    let error = UpdateChannel::by_key("nightly").unwrap_err();

    assert!(matches!(
        &error,
        SdkError::InvalidChannel { channel } if channel == "nightly"
    ));
    assert_eq!(error.to_string(), "Invalid channel: nightly");
}

#[test]
fn by_key_reports_the_key_as_it_was_given() {
    let error = UpdateChannel::by_key("Nightly").unwrap_err();

    assert_eq!(error.to_string(), "Invalid channel: Nightly");
}

#[test]
fn by_key_rejects_a_channel_identifier() {
    let error = UpdateChannel::by_key("release-candidate").unwrap_err();

    assert_eq!(error.to_string(), "Invalid channel: release-candidate");
}
