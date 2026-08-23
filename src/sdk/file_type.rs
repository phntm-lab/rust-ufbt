use super::SdkError;

/// Kind of a file offered by the update server for a single firmware version.
///
/// The identifier returned by [`id`](FileType::id) is the value the update server uses in
/// the `type` field of `directory.json`, and the same value the branch listing loader
/// reconstructs from a file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileType {
    /// Application SDK bundle, `sdk_zip`.
    SdkZip,
    /// Firmware library bundle, `lib_zip`.
    LibZip,
    /// Core2 firmware archive, `core2_firmware_tgz`.
    Core2FirmwareTgz,
    /// Resources archive, `resources_tgz`.
    ResourcesTgz,
    /// Scripts archive, `scripts_tgz`.
    ScriptsTgz,
    /// Update package archive, `update_tgz`.
    UpdateTgz,
    /// Firmware ELF image, `firmware_elf`.
    FirmwareElf,
    /// Full firmware binary, `full_bin`.
    FullBin,
    /// Full firmware DFU image, `full_dfu`.
    FullDfu,
    /// Full firmware JSON descriptor, `full_json`.
    FullJson,
    /// Updater binary, `updater_bin`.
    UpdaterBin,
    /// Updater DFU image, `updater_dfu`.
    UpdaterDfu,
    /// Updater ELF image, `updater_elf`.
    UpdaterElf,
    /// Updater JSON descriptor, `updater_json`.
    UpdaterJson,
}

impl FileType {
    /// Identifier of the file type as it appears in the directory index.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SdkZip => "sdk_zip",
            Self::LibZip => "lib_zip",
            Self::Core2FirmwareTgz => "core2_firmware_tgz",
            Self::ResourcesTgz => "resources_tgz",
            Self::ScriptsTgz => "scripts_tgz",
            Self::UpdateTgz => "update_tgz",
            Self::FirmwareElf => "firmware_elf",
            Self::FullBin => "full_bin",
            Self::FullDfu => "full_dfu",
            Self::FullJson => "full_json",
            Self::UpdaterBin => "updater_bin",
            Self::UpdaterDfu => "updater_dfu",
            Self::UpdaterElf => "updater_elf",
            Self::UpdaterJson => "updater_json",
        }
    }
}

/// Firmware update channel published by the update server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdateChannel {
    /// Development builds, `development`.
    Dev,
    /// Release candidate builds, `release-candidate`.
    Rc,
    /// Stable release builds, `release`.
    Release,
}

impl UpdateChannel {
    const VALUES: [Self; 3] = [Self::Dev, Self::Rc, Self::Release];

    /// Identifier of the channel as it appears in the directory index.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dev => "development",
            Self::Rc => "release-candidate",
            Self::Release => "release",
        }
    }

    /// Short key of the channel, as accepted by [`by_key`](UpdateChannel::by_key) and as
    /// stored in the persisted deployment state.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Rc => "rc",
            Self::Release => "release",
        }
    }

    /// Upper-case form of [`key`](UpdateChannel::key), matching the channel names used by
    /// the Python implementation of uFBT.
    #[must_use]
    pub const fn python_name(self) -> &'static str {
        match self {
            Self::Dev => "DEV",
            Self::Rc => "RC",
            Self::Release => "RELEASE",
        }
    }

    /// Resolves a channel from its short [`key`](UpdateChannel::key), ignoring case.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError::InvalidChannel`] when no channel carries the given key. The
    /// error reports the key exactly as it was passed in, not its lower-case form.
    pub fn by_key(key: &str) -> Result<Self, SdkError> {
        let lowered = key.to_lowercase();
        Self::VALUES
            .into_iter()
            .find(|channel| channel.key() == lowered)
            .ok_or_else(|| SdkError::InvalidChannel {
                channel: key.to_string(),
            })
    }
}
