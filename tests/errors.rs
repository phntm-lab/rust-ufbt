use std::io;
use std::path::PathBuf;

use rust_ufbt::Error;
use rust_ufbt::build::{
    BuildError, BundleError, ElfError, FamParseError, IconError, ManifestError,
};
use rust_ufbt::net::FetchError;
use rust_ufbt::sdk::SdkError;
use rust_ufbt::toolchain::ToolchainError;

fn path(text: &str) -> PathBuf {
    PathBuf::from(text)
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn fetch_error_display() {
    assert_eq!(
        FetchError::Http {
            status: 404,
            reason: "Not Found".to_owned(),
        }
        .to_string(),
        "HTTP Error 404: Not Found"
    );
}

#[test]
fn sdk_error_display() {
    assert_eq!(
        SdkError::InvalidMode {
            mode: "branch".to_owned(),
        }
        .to_string(),
        "Invalid mode: branch"
    );
    assert_eq!(
        SdkError::MultipleVersions {
            first: "0.103.1".to_owned(),
            second: "0.104.0".to_owned(),
        }
        .to_string(),
        "Found multiple versions: 0.103.1 and 0.104.0"
    );
    assert_eq!(
        SdkError::SdkBundleNotFound {
            target: "7".to_owned(),
        }
        .to_string(),
        "SDK bundle not found for 7"
    );
    assert_eq!(
        SdkError::LoaderNotInitialized.to_string(),
        "Loader is not initialized"
    );
    assert_eq!(
        SdkError::InvalidJson {
            message: "Unexpected character".to_owned(),
        }
        .to_string(),
        "Invalid JSON: Unexpected character"
    );
    assert_eq!(
        SdkError::InvalidChannel {
            channel: "nightly".to_owned(),
        }
        .to_string(),
        "Invalid channel: nightly"
    );
    assert_eq!(
        SdkError::EmptyChannel {
            channel: "release".to_owned(),
        }
        .to_string(),
        "Empty channel: release"
    );
    assert_eq!(SdkError::EmptyFilesList.to_string(), "Empty files list");
    assert_eq!(
        SdkError::InvalidFileType {
            file_type: "SDK_ZIP".to_owned(),
        }
        .to_string(),
        "Invalid file type: FileType.SDK_ZIP"
    );
    assert_eq!(SdkError::InvalidFileUrl.to_string(), "Invalid file url");
}

#[test]
fn toolchain_error_display() {
    let error = ToolchainError::Process {
        program: "uname".to_owned(),
        source: io::Error::from(io::ErrorKind::NotFound),
    };
    assert_eq!(error.to_string(), "Failed to run uname");
}

#[test]
fn fam_parse_error_display() {
    assert_eq!(
        FamParseError::Expected {
            token: ")".to_owned(),
            position: 42,
        }
        .to_string(),
        "Expected \")\" at offset 42 in application.fam"
    );
    assert_eq!(
        FamParseError::UnaryMinusOnNonNumber.to_string(),
        "Unary minus on a non-number"
    );
    assert_eq!(
        FamParseError::UnsupportedOperation {
            operator: "**".to_owned(),
        }
        .to_string(),
        "Unsupported operation \"**\" in application.fam"
    );
    assert_eq!(
        FamParseError::UnexpectedEnd.to_string(),
        "Unexpected end of application.fam"
    );
    assert_eq!(
        FamParseError::UnexpectedCharacter { character: '@' }.to_string(),
        "Unexpected character \"@\" in application.fam"
    );
    assert_eq!(
        FamParseError::UnterminatedString.to_string(),
        "Unterminated string"
    );
    assert_eq!(
        FamParseError::UnterminatedSequence.to_string(),
        "Unterminated sequence"
    );
    assert_eq!(
        FamParseError::UnterminatedDict.to_string(),
        "Unterminated dict"
    );
    assert_eq!(
        FamParseError::UnterminatedCall.to_string(),
        "Unterminated call"
    );
}

#[test]
fn manifest_error_display() {
    assert_eq!(
        ManifestError::NotFound {
            path: path("/apps/hello/application.fam"),
        }
        .to_string(),
        "App manifest not found at /apps/hello/application.fam"
    );
    assert_eq!(
        ManifestError::NoAppFound {
            path: path("/apps/hello/application.fam"),
        }
        .to_string(),
        "No App() found in /apps/hello/application.fam"
    );
    assert_eq!(
        ManifestError::InvalidAppId {
            appid: Some("Hello".to_owned()),
            pattern: "^[a-z0-9_]+$".to_owned(),
        }
        .to_string(),
        "Invalid appid 'Hello'. Must match regex '^[a-z0-9_]+$'"
    );
    assert_eq!(
        ManifestError::UnknownAppType {
            appid: "hello".to_owned(),
        }
        .to_string(),
        "Unknown apptype for hello"
    );
    assert_eq!(
        ManifestError::InvalidVersionForm {
            text: "1.x".to_owned(),
        }
        .to_string(),
        "Invalid version '1.x'. Must be in the form 'major.minor'"
    );
    assert_eq!(
        ManifestError::NotEnoughVersionComponents {
            text: "1".to_owned(),
        }
        .to_string(),
        "Invalid version '1'. Not enough version components"
    );
}

#[test]
fn manifest_error_renders_missing_appid_as_null() {
    assert_eq!(
        ManifestError::InvalidAppId {
            appid: None,
            pattern: "^[a-z0-9_]+$".to_owned(),
        }
        .to_string(),
        "Invalid appid 'null'. Must match regex '^[a-z0-9_]+$'"
    );
}

#[test]
fn elf_error_display() {
    assert_eq!(ElfError::NotElf.to_string(), "Not an ELF file");
    assert_eq!(ElfError::NotElf32.to_string(), "Only ELF32 is supported");
    assert_eq!(
        ElfError::NotLittleEndian.to_string(),
        "Only little-endian ELF is supported"
    );
    assert_eq!(ElfError::NoSymbolTable.to_string(), "No symbol table found");
    assert_eq!(
        ElfError::UnknownRelocationSection {
            name: ".foo".to_owned(),
        }
        .to_string(),
        "Unknown relocation section name: .foo"
    );
}

#[test]
fn icon_error_display() {
    assert_eq!(
        IconError::DecodeFailed {
            path: path("/icons/a.png"),
        }
        .to_string(),
        "Failed to decode image /icons/a.png"
    );
    assert_eq!(
        IconError::IconsDirectoryNotFound {
            path: path("/icons"),
        }
        .to_string(),
        "Icons directory not found, path = '/icons'"
    );
    assert_eq!(
        IconError::AnimationFramesDifferInSize {
            path: path("/icons/anim"),
        }
        .to_string(),
        "Animation frames differ in size in /icons/anim"
    );
    assert_eq!(
        IconError::InvalidAnimation {
            path: path("/icons/anim"),
        }
        .to_string(),
        "Invalid animation in /icons/anim"
    );
    assert_eq!(
        IconError::ImageTooBig {
            path: path("/icons/a.png"),
            width: 70000,
            height: 3,
            max_width: 0xFFFF,
            max_height: 0xFFFF,
        }
        .to_string(),
        "Image /icons/a.png is too big (70000x3 vs. 65535x65535)"
    );
}

#[test]
fn bundle_error_display() {
    assert_eq!(
        BundleError::AssetsDirectoryNotFound {
            path: path("/apps/hello/files"),
        }
        .to_string(),
        "Assets directory /apps/hello/files does not exist"
    );
}

#[test]
fn build_error_display() {
    assert_eq!(
        BuildError::SdkNotDeployed.to_string(),
        "SDK is not deployed: components.json not found"
    );
    assert_eq!(
        BuildError::ToolchainBinaryNotFound {
            path: path("/toolchain/bin/arm-none-eabi-gcc"),
        }
        .to_string(),
        "Toolchain binary not found: /toolchain/bin/arm-none-eabi-gcc"
    );
    assert_eq!(
        BuildError::NoApplicationInManifest.to_string(),
        "No application found in manifest"
    );
    assert_eq!(
        BuildError::UnsupportedTarget {
            appid: "hello".to_owned(),
            hardware: "7".to_owned(),
        }
        .to_string(),
        "hello does not support target f7"
    );
    assert_eq!(
        BuildError::NoSourceFiles {
            appid: "hello".to_owned(),
        }
        .to_string(),
        "No source files found for hello"
    );
    assert_eq!(
        BuildError::PrivateLibraryNotFound {
            path: path("/apps/hello/lib/foo"),
        }
        .to_string(),
        "Private library not found: /apps/hello/lib/foo"
    );
    assert_eq!(
        BuildError::NoPrivateLibrarySources {
            name: "foo".to_owned(),
        }
        .to_string(),
        "No sources gathered for private library foo"
    );
    assert_eq!(
        BuildError::NmFailed {
            stderr: "nm: bad file".to_owned(),
        }
        .to_string(),
        "nm failed: nm: bad file"
    );
    assert_eq!(
        BuildError::ToolFailed {
            tool: "arm-none-eabi-gcc".to_owned(),
            exit_code: 1,
        }
        .to_string(),
        "arm-none-eabi-gcc failed with exit code 1"
    );
    assert_eq!(
        BuildError::IconTooLarge { length: 40 }.to_string(),
        "Flipper app icon must be 32 bytes or less, but 40 bytes were given"
    );
    assert_eq!(
        BuildError::IconWrongSize {
            width: 12,
            height: 9,
        }
        .to_string(),
        "Flipper app icon must be 10x10 pixels, but 12x9 was given"
    );
}

#[test]
fn unresolved_symbols_display_without_disabled() {
    assert_eq!(
        BuildError::UnresolvedSymbols {
            fap: path("/dist/hello.fap"),
            unresolved: strings(&["furi_foo", "furi_bar"]),
            disabled: Vec::new(),
        }
        .to_string(),
        "/dist/hello.fap: app may not be runnable. Symbols not resolved using \
         firmware's API: {furi_foo, furi_bar}"
    );
}

#[test]
fn unresolved_symbols_display_with_disabled() {
    assert_eq!(
        BuildError::UnresolvedSymbols {
            fap: path("/dist/hello.fap"),
            unresolved: strings(&["furi_foo", "furi_bar"]),
            disabled: strings(&["furi_bar"]),
        }
        .to_string(),
        "/dist/hello.fap: app may not be runnable. Symbols not resolved using \
         firmware's API: {furi_foo, furi_bar} (in API, but disabled: {furi_bar})"
    );
}

#[test]
fn environment_failures_are_distinguishable() {
    assert!(BuildError::SdkNotDeployed.is_environment());
    assert!(
        BuildError::ToolchainBinaryNotFound {
            path: path("/toolchain/bin/nm"),
        }
        .is_environment()
    );
    assert!(!BuildError::NoApplicationInManifest.is_environment());
    assert!(
        !BuildError::NoSourceFiles {
            appid: "hello".to_owned(),
        }
        .is_environment()
    );
}

fn fetch_into_sdk() -> Result<(), SdkError> {
    Err(FetchError::Http {
        status: 500,
        reason: "Internal Server Error".to_owned(),
    })?;
    Ok(())
}

fn fam_into_manifest() -> Result<(), ManifestError> {
    Err(FamParseError::UnterminatedDict)?;
    Ok(())
}

fn manifest_into_build() -> Result<(), BuildError> {
    fam_into_manifest()?;
    Ok(())
}

fn build_into_root() -> Result<(), Error> {
    manifest_into_build()?;
    Ok(())
}

fn sdk_into_root() -> Result<(), Error> {
    fetch_into_sdk()?;
    Ok(())
}

#[test]
fn conversion_chain_reaches_root_error() {
    let error = build_into_root().unwrap_err();
    assert!(matches!(
        error,
        Error::Build(BuildError::Manifest(ManifestError::Parse(
            FamParseError::UnterminatedDict
        )))
    ));
    assert_eq!(error.to_string(), "Unterminated dict");

    let error = sdk_into_root().unwrap_err();
    assert!(matches!(
        error,
        Error::Sdk(SdkError::Fetch(FetchError::Http { status: 500, .. }))
    ));
    assert_eq!(error.to_string(), "HTTP Error 500: Internal Server Error");
}

#[test]
fn io_errors_convert_into_every_module_error() {
    fn io_error() -> io::Error {
        io::Error::from(io::ErrorKind::PermissionDenied)
    }

    let expected = io_error().to_string();

    assert_eq!(FetchError::from(io_error()).to_string(), expected);
    assert_eq!(SdkError::from(io_error()).to_string(), expected);
    assert_eq!(ToolchainError::from(io_error()).to_string(), expected);
    assert_eq!(ManifestError::from(io_error()).to_string(), expected);
    assert_eq!(ElfError::from(io_error()).to_string(), expected);
    assert_eq!(IconError::from(io_error()).to_string(), expected);
    assert_eq!(BundleError::from(io_error()).to_string(), expected);
    assert_eq!(BuildError::from(io_error()).to_string(), expected);
    assert_eq!(Error::from(io_error()).to_string(), expected);
}

#[test]
fn root_error_accepts_every_module_error() {
    let errors: Vec<Error> = vec![
        FetchError::Http {
            status: 404,
            reason: "Not Found".to_owned(),
        }
        .into(),
        SdkError::EmptyFilesList.into(),
        ToolchainError::from(io::Error::from(io::ErrorKind::NotFound)).into(),
        FamParseError::UnterminatedCall.into(),
        ManifestError::UnknownAppType {
            appid: "hello".to_owned(),
        }
        .into(),
        BuildError::NoApplicationInManifest.into(),
        ElfError::NotElf.into(),
        IconError::DecodeFailed {
            path: path("/icons/a.png"),
        }
        .into(),
        BundleError::AssetsDirectoryNotFound {
            path: path("/files"),
        }
        .into(),
    ];
    assert_eq!(errors.len(), 9);
}
