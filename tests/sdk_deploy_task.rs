use rust_ufbt::sdk::{DeployTask, UpdateChannel};
use rust_ufbt::state::State;
use serde_json::json;

fn state_of(value: serde_json::Value) -> State {
    State::new(serde_json::from_value(value).unwrap())
}

fn params_of(task: &DeployTask) -> Vec<(String, String)> {
    task.params()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

#[test]
fn defaults_deploy_the_release_channel_for_the_default_target() {
    let task = DeployTask::defaults();

    assert_eq!(task.hw_target(), Some("f7"));
    assert_eq!(DeployTask::DEFAULT_HW_TARGET, "f7");
    assert_eq!(task.mode(), Some("channel"));
    assert!(!task.force());
    assert_eq!(params_of(&task), pairs(&[("channel", "release")]));
}

#[test]
fn a_channel_task_carries_the_channel_key() {
    let task = DeployTask::channel(UpdateChannel::Dev).build();

    assert_eq!(task.hw_target(), Some("f7"));
    assert_eq!(task.mode(), Some("channel"));
    assert!(!task.force());
    assert_eq!(params_of(&task), pairs(&[("channel", "dev")]));
}

#[test]
fn a_channel_task_takes_a_target_an_index_and_a_forced_flag() {
    let task = DeployTask::channel(UpdateChannel::Rc)
        .hw_target("f18")
        .index_url("https://example.test/directory.json")
        .force(true)
        .build();

    assert_eq!(task.hw_target(), Some("f18"));
    assert!(task.force());
    assert_eq!(
        params_of(&task),
        pairs(&[
            ("channel", "rc"),
            ("json_index", "https://example.test/directory.json"),
        ])
    );
}

#[test]
fn a_channel_task_without_an_index_url_carries_no_index_key() {
    let task = DeployTask::channel(UpdateChannel::Release).build();

    assert_eq!(task.param("json_index"), None);
    assert!(!task.params().contains_key("json_index"));
}

#[test]
fn a_branch_task_carries_the_branch_name() {
    let task = DeployTask::branch("dev").build();

    assert_eq!(task.hw_target(), Some("f7"));
    assert_eq!(task.mode(), Some("branch"));
    assert_eq!(params_of(&task), pairs(&[("branch", "dev")]));
}

#[test]
fn a_branch_task_stores_the_index_url_as_the_branch_root() {
    let task = DeployTask::branch("feature/x")
        .hw_target("f18")
        .index_url("https://example.test/builds")
        .force(true)
        .build();

    assert_eq!(task.hw_target(), Some("f18"));
    assert!(task.force());
    assert_eq!(
        params_of(&task),
        pairs(&[
            ("branch", "feature/x"),
            ("branch_root", "https://example.test/builds"),
        ])
    );
    assert!(!task.params().contains_key("json_index"));
}

#[test]
fn a_branch_task_without_an_index_url_carries_no_branch_root_key() {
    let task = DeployTask::branch("dev").build();

    assert!(!task.params().contains_key("branch_root"));
}

#[test]
fn a_url_task_carries_the_address_and_the_given_target() {
    let task = DeployTask::url("https://example.test/sdk.zip", "f18").build();

    assert_eq!(task.hw_target(), Some("f18"));
    assert_eq!(task.mode(), Some("url"));
    assert!(!task.force());
    assert_eq!(
        params_of(&task),
        pairs(&[("url", "https://example.test/sdk.zip")])
    );
}

#[test]
fn a_local_task_carries_the_file_path_and_the_given_target() {
    let task = DeployTask::local("bundles/sdk.zip", "f7")
        .force(true)
        .build();

    assert_eq!(task.hw_target(), Some("f7"));
    assert_eq!(task.mode(), Some("local"));
    assert!(task.force());
    assert_eq!(task.param("file_path").unwrap(), "bundles/sdk.zip");
}

#[test]
fn from_state_repeats_the_target_and_the_mode_in_the_parameters() {
    let task = DeployTask::from_state(&state_of(json!({
        "hw_target": "f18",
        "mode": "channel",
        "channel": "dev",
        "version": "0.104.0"
    })));

    assert_eq!(task.hw_target(), Some("f18"));
    assert_eq!(task.mode(), Some("channel"));
    assert!(!task.force());
    assert_eq!(
        params_of(&task),
        pairs(&[
            ("hw_target", "f18"),
            ("mode", "channel"),
            ("channel", "dev"),
            ("version", "0.104.0"),
        ])
    );
}

#[test]
fn from_state_leaves_out_values_that_are_not_strings() {
    let task = DeployTask::from_state(&state_of(json!({
        "hw_target": "f7",
        "timestamp": 1_700_000_000,
        "files": ["sdk.zip"],
        "details": { "nested": true },
        "changelog": null,
        "version": "0.104.0"
    })));

    assert_eq!(
        params_of(&task),
        pairs(&[("hw_target", "f7"), ("version", "0.104.0")])
    );
}

#[test]
fn from_state_without_a_target_or_a_mode_carries_neither() {
    let task = DeployTask::from_state(&state_of(json!({ "version": "0.104.0" })));

    assert_eq!(task.hw_target(), None);
    assert_eq!(task.mode(), None);
    assert_eq!(params_of(&task), pairs(&[("version", "0.104.0")]));
}

#[test]
fn from_state_of_an_empty_state_is_an_empty_task() {
    let task = DeployTask::from_state(&State::default());

    assert_eq!(task.hw_target(), None);
    assert_eq!(task.mode(), None);
    assert!(!task.force());
    assert!(task.params().is_empty());
}

#[test]
fn update_from_takes_over_a_target_and_a_mode_that_are_given() {
    let mut task = DeployTask::defaults();
    let other = DeployTask::branch("dev").hw_target("f18").build();

    task.update_from(&other);

    assert_eq!(task.hw_target(), Some("f18"));
    assert_eq!(task.mode(), Some("branch"));
}

#[test]
fn update_from_keeps_a_target_and_a_mode_that_are_absent() {
    let mut task = DeployTask::defaults();
    let other = DeployTask::from_state(&State::default());

    task.update_from(&other);

    assert_eq!(task.hw_target(), Some("f7"));
    assert_eq!(task.mode(), Some("channel"));
}

#[test]
fn update_from_always_takes_over_the_forced_flag() {
    let mut task = DeployTask::channel(UpdateChannel::Release)
        .force(true)
        .build();
    let other = DeployTask::from_state(&State::default());

    task.update_from(&other);

    assert!(!task.force());

    let mut task = DeployTask::defaults();
    task.update_from(&DeployTask::channel(UpdateChannel::Dev).force(true).build());

    assert!(task.force());
}

#[test]
fn update_from_merges_parameters_and_keeps_the_ones_it_does_not_carry() {
    let mut task = DeployTask::channel(UpdateChannel::Release)
        .index_url("https://example.test/directory.json")
        .build();

    task.update_from(&DeployTask::channel(UpdateChannel::Dev).build());

    assert_eq!(
        params_of(&task),
        pairs(&[
            ("channel", "dev"),
            ("json_index", "https://example.test/directory.json"),
        ])
    );
}

#[test]
fn update_from_ignores_empty_parameter_values() {
    let mut task = DeployTask::defaults();
    let other = DeployTask::from_state(&state_of(json!({
        "channel": "",
        "version": "0.104.0"
    })));

    task.update_from(&other);

    assert_eq!(
        params_of(&task),
        pairs(&[("channel", "release"), ("version", "0.104.0")])
    );
}

#[test]
fn update_from_appends_parameters_it_alone_carries() {
    let mut task = DeployTask::defaults();

    task.update_from(&DeployTask::url("https://example.test/sdk.zip", "f18").build());

    assert_eq!(
        params_of(&task),
        pairs(&[
            ("channel", "release"),
            ("url", "https://example.test/sdk.zip"),
        ])
    );
    assert_eq!(task.mode(), Some("url"));
    assert_eq!(task.hw_target(), Some("f18"));
}

#[test]
fn display_matches_the_original_wording() {
    assert_eq!(
        DeployTask::defaults().to_string(),
        "SdkDeployTask(hw_target: f7, mode: channel, force: false, params: {channel: release})"
    );

    let task = DeployTask::branch("dev")
        .hw_target("f18")
        .index_url("https://example.test/builds")
        .force(true)
        .build();
    assert_eq!(
        task.to_string(),
        "SdkDeployTask(hw_target: f18, mode: branch, force: true, \
         params: {branch: dev, branch_root: https://example.test/builds})"
    );
}

#[test]
fn display_writes_null_for_a_target_and_a_mode_that_are_absent() {
    assert_eq!(
        DeployTask::from_state(&State::default()).to_string(),
        "SdkDeployTask(hw_target: null, mode: null, force: false, params: {})"
    );
}
