use std::collections::HashMap;
use std::fs;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use rust_ufbt::paths::{
    ENV_FILE_NAME, Paths, STATE_FILE_NAME, TOOLCHAIN_SUBDIR, default_ufbt_home, load_env_file,
};

const NO_TOOLCHAIN: Option<&Path> = None;

fn env_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn absolute_home() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("C:\\ufbt-home")
    } else {
        PathBuf::from("/ufbt-home")
    }
}

fn write_env_file(dir: &Path, contents: &str) {
    fs::write(dir.join(ENV_FILE_NAME), contents).expect("write .env");
}

fn joined(base: &Path, tail: &[&str]) -> PathBuf {
    let mut path = base.as_os_str().to_os_string();
    for segment in tail {
        path.push(MAIN_SEPARATOR.to_string());
        path.push(segment);
    }
    PathBuf::from(path)
}

#[test]
fn constants_match_ufbt() {
    assert_eq!(STATE_FILE_NAME, "ufbt_state.json");
    assert_eq!(TOOLCHAIN_SUBDIR, "toolchain");
    assert_eq!(ENV_FILE_NAME, ".env");
}

#[test]
fn getters_derive_from_state_dir() {
    let home = absolute_home();
    let paths = Paths::new(&home, NO_TOOLCHAIN);

    assert_eq!(paths.state_dir(), home);
    assert_eq!(paths.download_dir(), joined(&home, &["download"]));
    assert_eq!(paths.current_sdk_dir(), joined(&home, &["current"]));
    assert_eq!(paths.toolchain_dir(), joined(&home, &["toolchain"]));
    assert_eq!(
        paths.state_file(),
        joined(&home, &["current", "ufbt_state.json"])
    );
    assert_eq!(
        paths.sdk_scripts_dir(),
        joined(&home, &["current", "scripts"])
    );
    assert_eq!(
        paths.fbtenv_script(),
        joined(&home, &["current", "scripts", "toolchain", "fbtenv.sh"])
    );
    assert_eq!(
        paths.fbtenv_cmd(),
        joined(&home, &["current", "scripts", "toolchain", "fbtenv.cmd"])
    );
    assert_eq!(
        paths.toolchain_arch_dir("darwin-arm64"),
        joined(&home, &["toolchain", "darwin-arm64"])
    );
    assert_eq!(
        paths.toolchain_current_link(),
        joined(&home, &["toolchain", "current"])
    );
}

#[test]
fn separate_toolchain_path_moves_only_the_toolchain() {
    let home = absolute_home();
    let toolchain = if cfg!(windows) {
        PathBuf::from("C:\\elsewhere")
    } else {
        PathBuf::from("/elsewhere")
    };
    let paths = Paths::new(&home, Some(&toolchain));

    assert_eq!(paths.state_dir(), home);
    assert_eq!(paths.download_dir(), joined(&home, &["download"]));
    assert_eq!(paths.toolchain_dir(), joined(&toolchain, &["toolchain"]));
    assert_eq!(
        paths.toolchain_current_link(),
        joined(&toolchain, &["toolchain", "current"])
    );
}

#[test]
fn relative_home_is_resolved_against_the_current_directory() {
    let paths = Paths::new("relative-home", NO_TOOLCHAIN);
    let expected = joined(&std::env::current_dir().unwrap(), &["relative-home"]);

    assert_eq!(paths.state_dir(), expected);
}

#[test]
fn trailing_separator_is_dropped() {
    let home = absolute_home();
    let mut with_separator = home.as_os_str().to_os_string();
    with_separator.push(MAIN_SEPARATOR.to_string());

    assert_eq!(
        Paths::new(PathBuf::from(with_separator), NO_TOOLCHAIN).state_dir(),
        home
    );
}

#[test]
fn only_one_trailing_separator_is_dropped() {
    let home = absolute_home();
    let mut doubled = home.as_os_str().to_os_string();
    doubled.push(MAIN_SEPARATOR.to_string());
    doubled.push(MAIN_SEPARATOR.to_string());
    let mut expected = home.as_os_str().to_os_string();
    expected.push(MAIN_SEPARATOR.to_string());

    assert_eq!(
        Paths::new(PathBuf::from(doubled), NO_TOOLCHAIN).state_dir(),
        PathBuf::from(expected)
    );
}

#[test]
fn dot_segments_are_kept() {
    let home = absolute_home();
    let unnormalised = joined(&home, &["a", "..", "b"]);

    assert_eq!(
        Paths::new(&unnormalised, NO_TOOLCHAIN).state_dir(),
        unnormalised
    );
}

#[cfg(not(windows))]
#[test]
fn root_is_left_alone() {
    assert_eq!(Paths::new("/", NO_TOOLCHAIN).state_dir(), Path::new("/"));
    assert_eq!(
        Paths::new("/", NO_TOOLCHAIN).download_dir(),
        Path::new("//download")
    );
}

#[cfg(windows)]
#[test]
fn drive_root_loses_its_separator() {
    assert_eq!(
        Paths::new("C:\\", NO_TOOLCHAIN).state_dir(),
        Path::new("C:")
    );
    assert_eq!(Paths::new("C:/", NO_TOOLCHAIN).state_dir(), Path::new("C:"));
    assert_eq!(
        Paths::new("C:\\", NO_TOOLCHAIN).download_dir(),
        Path::new("C:\\download")
    );
}

#[cfg(windows)]
#[test]
fn a_path_without_a_drive_takes_the_drive_of_the_current_directory() {
    let current = std::env::current_dir().unwrap();
    let drive = current.to_string_lossy()[..1].to_owned();

    assert_eq!(
        Paths::new("\\ufbt-home", NO_TOOLCHAIN).state_dir(),
        PathBuf::from(format!("{drive}:\\ufbt-home"))
    );
}

#[cfg(windows)]
#[test]
fn a_forward_slash_path_stays_relative_on_windows() {
    let expected = joined(&std::env::current_dir().unwrap(), &[]);
    let mut expected = expected.into_os_string();
    expected.push("//ufbt-home");

    assert_eq!(
        Paths::new("/ufbt-home", NO_TOOLCHAIN).state_dir(),
        PathBuf::from(expected)
    );
}

#[cfg(windows)]
#[test]
fn a_unc_path_is_absolute() {
    assert_eq!(
        Paths::new("\\\\server\\share", NO_TOOLCHAIN).state_dir(),
        Path::new("\\\\server\\share")
    );
    assert_eq!(
        Paths::new("\\\\server\\share\\", NO_TOOLCHAIN).state_dir(),
        Path::new("\\\\server\\share")
    );
}

#[test]
fn default_ufbt_home_prefers_home() {
    assert_eq!(
        default_ufbt_home(&env_of(&[("HOME", "/h"), ("USERPROFILE", "/u")])),
        PathBuf::from(format!("/h{MAIN_SEPARATOR}.ufbt"))
    );
}

#[test]
fn default_ufbt_home_falls_back_to_userprofile() {
    assert_eq!(
        default_ufbt_home(&env_of(&[("USERPROFILE", "/u")])),
        PathBuf::from(format!("/u{MAIN_SEPARATOR}.ufbt"))
    );
}

#[test]
fn default_ufbt_home_without_either_variable() {
    assert_eq!(
        default_ufbt_home(&HashMap::new()),
        PathBuf::from(format!("{MAIN_SEPARATOR}.ufbt"))
    );
}

#[test]
fn default_ufbt_home_treats_an_empty_home_as_set() {
    assert_eq!(
        default_ufbt_home(&env_of(&[("HOME", ""), ("USERPROFILE", "/u")])),
        PathBuf::from(format!("{MAIN_SEPARATOR}.ufbt"))
    );
}

#[test]
fn missing_env_file_yields_no_variables() {
    let dir = tempfile::tempdir().unwrap();

    assert!(load_env_file(Some(dir.path())).is_empty());
}

#[test]
fn env_file_skips_blank_lines_and_comments() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(
        dir.path(),
        "\n   \n# comment\n  # indented comment\nA=1\n\nB=2\n",
    );

    assert_eq!(
        load_env_file(Some(dir.path())),
        env_of(&[("A", "1"), ("B", "2")])
    );
}

#[test]
fn env_file_skips_lines_without_a_separator() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "NO_SEPARATOR\nA=1\n");

    assert_eq!(load_env_file(Some(dir.path())), env_of(&[("A", "1")]));
}

#[test]
fn env_file_keeps_spaces_around_key_and_value() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "   A B = 1 2   \n");

    assert_eq!(load_env_file(Some(dir.path())), env_of(&[("A B ", " 1 2")]));
}

#[test]
fn env_file_keeps_quotes() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "A=\"quoted\"\nB='single'\n");

    assert_eq!(
        load_env_file(Some(dir.path())),
        env_of(&[("A", "\"quoted\""), ("B", "'single'")])
    );
}

#[test]
fn env_file_does_not_understand_export() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "export A=1\n");

    assert_eq!(
        load_env_file(Some(dir.path())),
        env_of(&[("export A", "1")])
    );
}

#[test]
fn env_file_splits_on_the_first_separator() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "A=1=2=3\n");

    assert_eq!(load_env_file(Some(dir.path())), env_of(&[("A", "1=2=3")]));
}

#[test]
fn env_file_accepts_an_empty_value_and_an_empty_key() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "A=\n=value\n");

    assert_eq!(
        load_env_file(Some(dir.path())),
        env_of(&[("A", ""), ("", "value")])
    );
}

#[test]
fn env_file_accepts_every_line_ending() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "A=1\r\nB=2\rC=3\nD=4");

    assert_eq!(
        load_env_file(Some(dir.path())),
        env_of(&[("A", "1"), ("B", "2"), ("C", "3"), ("D", "4")])
    );
}

#[test]
fn later_lines_win_over_earlier_ones() {
    let dir = tempfile::tempdir().unwrap();
    write_env_file(dir.path(), "A=first\nA=second\n");

    assert_eq!(load_env_file(Some(dir.path())), env_of(&[("A", "second")]));
}

#[test]
fn resolve_reads_ufbt_home_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let home = absolute_home();
    let env = env_of(&[("UFBT_HOME", home.to_str().unwrap())]);
    let paths = Paths::resolve(Some(dir.path()), Some(&env));

    assert_eq!(paths.state_dir(), home);
    assert_eq!(paths.toolchain_dir(), joined(&home, &["toolchain"]));
}

#[test]
fn resolve_reads_the_toolchain_path_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let home = absolute_home();
    let toolchain = if cfg!(windows) {
        "C:\\elsewhere"
    } else {
        "/elsewhere"
    };
    let env = env_of(&[
        ("UFBT_HOME", home.to_str().unwrap()),
        ("FBT_TOOLCHAIN_PATH", toolchain),
    ]);
    let paths = Paths::resolve(Some(dir.path()), Some(&env));

    assert_eq!(paths.state_dir(), home);
    assert_eq!(
        paths.toolchain_dir(),
        joined(Path::new(toolchain), &["toolchain"])
    );
}

#[test]
fn resolve_falls_back_to_the_default_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = absolute_home();
    let env = env_of(&[("HOME", home.to_str().unwrap())]);
    let paths = Paths::resolve(Some(dir.path()), Some(&env));

    assert_eq!(paths.state_dir(), joined(&home, &[".ufbt"]));
}

#[test]
fn resolve_lets_the_env_file_override_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let home = absolute_home();
    let from_file = joined(&home, &["from-file"]);
    write_env_file(
        dir.path(),
        &format!("UFBT_HOME={}\n", from_file.to_str().unwrap()),
    );
    let env = env_of(&[("UFBT_HOME", home.to_str().unwrap())]);
    let paths = Paths::resolve(Some(dir.path()), Some(&env));

    assert_eq!(paths.state_dir(), from_file);
}

#[test]
fn resolve_lets_the_env_file_supply_the_home_directory() {
    let dir = tempfile::tempdir().unwrap();
    let home = absolute_home();
    write_env_file(dir.path(), &format!("HOME={}\n", home.to_str().unwrap()));
    let paths = Paths::resolve(Some(dir.path()), Some(&HashMap::new()));

    assert_eq!(paths.state_dir(), joined(&home, &[".ufbt"]));
}

#[test]
fn resolve_without_an_env_file_uses_the_environment_alone() {
    let dir = tempfile::tempdir().unwrap();
    let home = absolute_home();
    let env = env_of(&[("UFBT_HOME", home.to_str().unwrap())]);

    assert_eq!(
        Paths::resolve(Some(dir.path()), Some(&env)).state_dir(),
        home
    );
}
