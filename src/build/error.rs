use std::io;
use std::path::PathBuf;

/// Failure of `application.fam` parsing.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FamParseError {
    /// An expected token is missing at the given offset.
    #[error("Expected \"{token}\" at offset {position} in application.fam")]
    Expected {
        /// Token the parser expected to find.
        token: String,
        /// Offset the parser stopped at.
        position: usize,
    },
    /// A unary minus was applied to something other than a number.
    #[error("Unary minus on a non-number")]
    UnaryMinusOnNonNumber,
    /// A binary operator outside the supported subset was used.
    #[error("Unsupported operation \"{operator}\" in application.fam")]
    UnsupportedOperation {
        /// Operator that is not supported.
        operator: String,
    },
    /// The manifest ended while a value was still expected.
    #[error("Unexpected end of application.fam")]
    UnexpectedEnd,
    /// A character that cannot start a value was encountered.
    #[error("Unexpected character \"{character}\" in application.fam")]
    UnexpectedCharacter {
        /// Character the parser stopped at.
        character: char,
    },
    /// A string literal was never closed.
    #[error("Unterminated string")]
    UnterminatedString,
    /// A list or a tuple was never closed.
    #[error("Unterminated sequence")]
    UnterminatedSequence,
    /// A dictionary literal was never closed.
    #[error("Unterminated dict")]
    UnterminatedDict,
    /// A call argument list was never closed.
    #[error("Unterminated call")]
    UnterminatedCall,
}

/// Failure of application manifest loading or validation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    /// The application directory has no `application.fam`.
    #[error("App manifest not found at {}", .path.display())]
    NotFound {
        /// Path the manifest was looked up at.
        path: PathBuf,
    },
    /// The manifest contains no `App()` call.
    #[error("No App() found in {}", .path.display())]
    NoAppFound {
        /// Path of the manifest that was parsed.
        path: PathBuf,
    },
    /// The application identifier is missing or malformed.
    #[error("Invalid appid '{}'. Must match regex '{pattern}'", render_appid(.appid))]
    InvalidAppId {
        /// Identifier as declared in the manifest, if any.
        appid: Option<String>,
        /// Regular expression the identifier must match.
        pattern: String,
    },
    /// The declared application type is not known.
    #[error("Unknown apptype for {appid}")]
    UnknownAppType {
        /// Identifier of the offending application.
        appid: String,
    },
    /// A version component is not an integer.
    #[error("Invalid version '{text}'. Must be in the form 'major.minor'")]
    InvalidVersionForm {
        /// Version as declared in the manifest.
        text: String,
    },
    /// The version has fewer than two components.
    #[error("Invalid version '{text}'. Not enough version components")]
    NotEnoughVersionComponents {
        /// Version as declared in the manifest.
        text: String,
    },
    /// The manifest could not be parsed.
    #[error(transparent)]
    Parse(#[from] FamParseError),
    /// The manifest could not be read.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Failure of ELF parsing.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ElfError {
    /// The file does not start with the ELF magic.
    #[error("Not an ELF file")]
    NotElf,
    /// The file is not a 32-bit ELF.
    #[error("Only ELF32 is supported")]
    NotElf32,
    /// The file is not little-endian.
    #[error("Only little-endian ELF is supported")]
    NotLittleEndian,
    /// The ELF carries no symbol table.
    #[error("No symbol table found")]
    NoSymbolTable,
    /// A relocation section name does not follow the expected convention.
    #[error("Unknown relocation section name: {name}")]
    UnknownRelocationSection {
        /// Name of the offending section.
        name: String,
    },
    /// The ELF could not be read.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Failure of icon decoding or icon assets compilation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IconError {
    /// The image could not be decoded by any known decoder.
    #[error("Failed to decode image {}", .path.display())]
    DecodeFailed {
        /// Path of the offending image.
        path: PathBuf,
    },
    /// The icons directory does not exist.
    #[error("Icons directory not found, path = '{}'", .path.display())]
    IconsDirectoryNotFound {
        /// Path the icons directory was looked up at.
        path: PathBuf,
    },
    /// Animation frames within one directory have different dimensions.
    #[error("Animation frames differ in size in {}", .path.display())]
    AnimationFramesDifferInSize {
        /// Path of the offending animation directory.
        path: PathBuf,
    },
    /// An animation directory holds no usable frames.
    #[error("Invalid animation in {}", .path.display())]
    InvalidAnimation {
        /// Path of the offending animation directory.
        path: PathBuf,
    },
    /// The image exceeds the dimensions the icon format can encode.
    #[error(
        "Image {} is too big ({width}x{height} vs. {max_width}x{max_height})",
        .path.display()
    )]
    ImageTooBig {
        /// Path of the offending image.
        path: PathBuf,
        /// Width of the offending image.
        width: u32,
        /// Height of the offending image.
        height: u32,
        /// Largest width the icon format can encode.
        max_width: u32,
        /// Largest height the icon format can encode.
        max_height: u32,
    },
    /// An image or a directory entry could not be read.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Failure of file assets bundling.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BundleError {
    /// The assets directory does not exist.
    #[error("Assets directory {} does not exist", .path.display())]
    AssetsDirectoryNotFound {
        /// Path the assets directory was looked up at.
        path: PathBuf,
    },
    /// An asset could not be read or written.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Failure of an application build.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// The SDK is not deployed, so nothing can be compiled.
    #[error("SDK is not deployed: components.json not found")]
    SdkNotDeployed,
    /// A toolchain executable is missing.
    #[error("Toolchain binary not found: {}", .path.display())]
    ToolchainBinaryNotFound {
        /// Path the executable was looked up at.
        path: PathBuf,
    },
    /// The manifest declares no application to build.
    #[error("No application found in manifest")]
    NoApplicationInManifest,
    /// The application does not list the requested hardware target.
    #[error("{appid} does not support target f{hardware}")]
    UnsupportedTarget {
        /// Identifier of the application being built.
        appid: String,
        /// Hardware target that was requested.
        hardware: String,
    },
    /// No source file matched the application sources.
    #[error("No source files found for {appid}")]
    NoSourceFiles {
        /// Identifier of the application being built.
        appid: String,
    },
    /// A private library directory declared by the manifest is missing.
    #[error("Private library not found: {}", .path.display())]
    PrivateLibraryNotFound {
        /// Path the library was looked up at.
        path: PathBuf,
    },
    /// No source file matched a private library.
    #[error("No sources gathered for private library {name}")]
    NoPrivateLibrarySources {
        /// Name of the offending library.
        name: String,
    },
    /// The symbol listing tool failed.
    #[error("nm failed: {stderr}")]
    NmFailed {
        /// Standard error output of the tool.
        stderr: String,
    },
    /// The application imports symbols the firmware API does not provide.
    #[error(
        "{}: app may not be runnable. Symbols not resolved using firmware's API: {{{}}}{}",
        .fap.display(),
        .unresolved.join(", "),
        render_disabled(.disabled)
    )]
    UnresolvedSymbols {
        /// Path of the application binary that was checked.
        fap: PathBuf,
        /// Symbols that could not be resolved, sorted.
        unresolved: Vec<String>,
        /// Subset of the unresolved symbols that the API declares as disabled.
        disabled: Vec<String>,
    },
    /// A build tool exited with a non-zero status.
    #[error("{tool} failed with exit code {exit_code}")]
    ToolFailed {
        /// File name of the tool that failed.
        tool: String,
        /// Status the tool exited with.
        exit_code: i32,
    },
    /// The application icon does not fit into the manifest field.
    #[error("Flipper app icon must be 32 bytes or less, but {length} bytes were given")]
    IconTooLarge {
        /// Size of the encoded icon.
        length: usize,
    },
    /// The application icon has the wrong dimensions.
    #[error("Flipper app icon must be 10x10 pixels, but {width}x{height} was given")]
    IconWrongSize {
        /// Width of the offending icon.
        width: u32,
        /// Height of the offending icon.
        height: u32,
    },
    /// The manifest could not be loaded.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// An ELF produced or consumed by the build could not be parsed.
    #[error(transparent)]
    Elf(#[from] ElfError),
    /// Icon assets could not be compiled.
    #[error(transparent)]
    Icon(#[from] IconError),
    /// File assets could not be bundled.
    #[error(transparent)]
    Bundle(#[from] BundleError),
    /// A file system operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl BuildError {
    /// Reports whether the build failed because the SDK or the toolchain is missing,
    /// as opposed to an application that fails to build.
    ///
    /// The distinction matters to callers that may run the same build somewhere else.
    pub fn is_environment(&self) -> bool {
        matches!(
            self,
            Self::SdkNotDeployed | Self::ToolchainBinaryNotFound { .. }
        )
    }
}

fn render_appid(appid: &Option<String>) -> &str {
    match appid {
        Some(appid) => appid,
        None => "null",
    }
}

fn render_disabled(disabled: &[String]) -> String {
    if disabled.is_empty() {
        return String::new();
    }
    format!(" (in API, but disabled: {{{}}})", disabled.join(", "))
}
