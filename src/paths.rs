//! Layout of the uFBT home directory.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path, PathBuf};

/// Name of the file the deployment state is stored in.
pub const STATE_FILE_NAME: &str = "ufbt_state.json";

/// Name of the directory the ARM GCC toolchain is unpacked into.
pub const TOOLCHAIN_SUBDIR: &str = "toolchain";

/// Name of the file extra environment variables are read from.
pub const ENV_FILE_NAME: &str = ".env";

/// Every path uFBT works with, derived from the uFBT home directory and an optional
/// separate toolchain location.
///
/// Both locations are made absolute when the value is constructed, and a single trailing
/// separator is dropped. Nothing else is normalised: `.` and `..` segments survive, and a
/// path is never checked against the file system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Paths {
    state_dir: PathBuf,
    toolchain_path: PathBuf,
}

impl Paths {
    /// Builds the layout for a uFBT home directory, with the toolchain either alongside the
    /// state or at a location of its own.
    ///
    /// Both arguments are resolved against the current working directory when they are
    /// relative, and lose a single trailing separator when they are longer than one
    /// character. On Windows this means a bare drive root such as `C:\` becomes `C:`.
    #[must_use]
    pub fn new(home: impl AsRef<Path>, toolchain_path: Option<impl AsRef<Path>>) -> Self {
        let home = home.as_ref();
        let toolchain = toolchain_path.as_ref().map_or(home, AsRef::as_ref);
        Self {
            state_dir: absolute(home),
            toolchain_path: absolute(toolchain),
        }
    }

    /// Builds the layout the way uFBT does when it starts: the process environment
    /// overlaid with the [`.env`](ENV_FILE_NAME) file of the project directory, then
    /// `UFBT_HOME` for the home directory and `FBT_TOOLCHAIN_PATH` for the toolchain.
    ///
    /// Values from the `.env` file win over the ones already in the environment. Passing
    /// `None` for `env` reads the environment of the current process, and passing `None`
    /// for `project_dir` looks for the `.env` file in the current working directory.
    #[must_use]
    pub fn resolve(project_dir: Option<&Path>, env: Option<&HashMap<String, String>>) -> Self {
        let mut merged = match env {
            Some(env) => env.clone(),
            None => process_environment(),
        };
        merged.extend(load_env_file(project_dir));
        let home = merged
            .get("UFBT_HOME")
            .map_or_else(|| default_ufbt_home(&merged), PathBuf::from);
        let toolchain_path = merged.get("FBT_TOOLCHAIN_PATH").map(PathBuf::from);
        Self::new(home, toolchain_path)
    }

    /// Directory holding the deployed state: the current SDK, the downloads and, unless it
    /// was moved elsewhere, the toolchain.
    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Directory downloaded archives are written to.
    #[must_use]
    pub fn download_dir(&self) -> PathBuf {
        join(&self.state_dir, "download")
    }

    /// Directory the currently deployed SDK is unpacked into.
    #[must_use]
    pub fn current_sdk_dir(&self) -> PathBuf {
        join(&self.state_dir, "current")
    }

    /// Directory the toolchain versions are unpacked into.
    #[must_use]
    pub fn toolchain_dir(&self) -> PathBuf {
        join(&self.toolchain_path, TOOLCHAIN_SUBDIR)
    }

    /// File the deployment state is stored in.
    #[must_use]
    pub fn state_file(&self) -> PathBuf {
        join(&self.current_sdk_dir(), STATE_FILE_NAME)
    }

    /// Directory of the scripts shipped with the SDK.
    #[must_use]
    pub fn sdk_scripts_dir(&self) -> PathBuf {
        join(&self.current_sdk_dir(), "scripts")
    }

    /// Shell script the SDK uses to enter the toolchain environment.
    #[must_use]
    pub fn fbtenv_script(&self) -> PathBuf {
        join(
            &join(&self.sdk_scripts_dir(), TOOLCHAIN_SUBDIR),
            "fbtenv.sh",
        )
    }

    /// Batch script the SDK uses to enter the toolchain environment.
    #[must_use]
    pub fn fbtenv_cmd(&self) -> PathBuf {
        join(
            &join(&self.sdk_scripts_dir(), TOOLCHAIN_SUBDIR),
            "fbtenv.cmd",
        )
    }

    /// Directory the toolchain for the given architecture is unpacked into.
    #[must_use]
    pub fn toolchain_arch_dir(&self, arch_dir: &str) -> PathBuf {
        join(&self.toolchain_dir(), arch_dir)
    }

    /// Symbolic link pointing at the toolchain that is currently in use.
    #[must_use]
    pub fn toolchain_current_link(&self) -> PathBuf {
        join(&self.toolchain_dir(), "current")
    }
}

/// Returns the uFBT home directory to use when `UFBT_HOME` is not set: `.ufbt` inside the
/// directory named by `HOME`, or by `USERPROFILE` when `HOME` is absent.
///
/// With neither variable present the result is a bare `.ufbt` under the platform
/// separator, which is what uFBT produces as well.
#[must_use]
pub fn default_ufbt_home(env: &HashMap<String, String>) -> PathBuf {
    let home = env
        .get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map_or("", String::as_str);
    PathBuf::from(format!("{home}{MAIN_SEPARATOR}.ufbt"))
}

/// Reads the [`.env`](ENV_FILE_NAME) file of the given directory, defaulting to the current
/// working directory.
///
/// A missing or unreadable file yields no variables. Each line is trimmed as a whole, blank
/// lines and lines starting with `#` are skipped, and the first `=` splits the rest into a
/// key and a value. Neither side is trimmed, quotes are kept as written, and an `export`
/// prefix is not understood.
#[must_use]
pub fn load_env_file(dir: Option<&Path>) -> HashMap<String, String> {
    let dir = dir.map_or_else(|| env::current_dir().unwrap_or_default(), Path::to_path_buf);
    let mut vars = HashMap::new();
    let Ok(contents) = fs::read_to_string(join(&dir, ENV_FILE_NAME)) else {
        return vars;
    };
    for raw in contents.split(['\r', '\n']) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(index) = line.find('=') else {
            continue;
        };
        vars.insert(line[..index].to_owned(), line[index + 1..].to_owned());
    }
    vars
}

fn process_environment() -> HashMap<String, String> {
    env::vars_os()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

fn join(base: &Path, name: &str) -> PathBuf {
    let mut joined = base.as_os_str().to_os_string();
    joined.push(MAIN_SEPARATOR_STR);
    joined.push(name);
    PathBuf::from(joined)
}

fn absolute(path: &Path) -> PathBuf {
    let path = path.to_string_lossy();
    let mut resolved = if is_absolute(&path) {
        path.into_owned()
    } else {
        resolve_against_current_dir(&path)
    };
    if resolved.len() > 1 && (resolved.ends_with('/') || resolved.ends_with('\\')) {
        resolved.pop();
    }
    PathBuf::from(resolved)
}

fn current_dir() -> String {
    env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn is_absolute(path: &str) -> bool {
    path.starts_with('/')
}

#[cfg(not(windows))]
fn resolve_against_current_dir(path: &str) -> String {
    let current = current_dir();
    if current.ends_with('/') {
        format!("{current}{path}")
    } else {
        format!("{current}{MAIN_SEPARATOR}{path}")
    }
}

#[cfg(windows)]
fn is_absolute(path: &str) -> bool {
    if path.starts_with("\\\\") {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

#[cfg(windows)]
fn drive_letter(path: &str) -> Option<u8> {
    let bytes = path.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' {
        return None;
    }
    let upper = bytes[0] & !0x20;
    (0x41..=0x5b).contains(&upper).then_some(upper)
}

#[cfg(windows)]
fn resolve_against_current_dir(path: &str) -> String {
    let current = current_dir();
    if path.starts_with('\\') {
        return resolve_drive_relative(&current, path);
    }
    let relative = match drive_letter(path) {
        Some(drive) if Some(drive) != drive_letter(&current) => {
            return format!("{}:\\{path}", &path[..1]);
        }
        Some(_) => &path[2..],
        None => path,
    };
    if current.ends_with('\\') || current.ends_with('/') {
        format!("{current}{relative}")
    } else {
        format!("{current}\\{relative}")
    }
}

#[cfg(windows)]
fn resolve_drive_relative(current: &str, path: &str) -> String {
    if drive_letter(current).is_some() {
        return format!("{}:{path}", &current[..1]);
    }
    if let Some(after_prefix) = current.strip_prefix("\\\\") {
        if let Some(server_end) = after_prefix.find('\\').map(|index| index + 2) {
            let share_end = current[server_end + 1..]
                .find('\\')
                .map_or(current.len(), |index| index + server_end + 1);
            return format!("{}{path}", &current[..share_end]);
        }
    }
    path.to_owned()
}
