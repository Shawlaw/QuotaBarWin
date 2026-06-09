use std::fs;

#[test]
fn updater_config_is_present() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(root.join("tauri.conf.json")).expect("tauri config");
    let value: serde_json::Value = serde_json::from_str(&config).expect("json config");

    assert!(value.pointer("/plugins/updater").is_some());
    assert!(value.pointer("/bundle/createUpdaterArtifacts").is_some());
}

#[test]
fn release_workflow_yaml_is_valid() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf();
    let workflow =
        fs::read_to_string(root.join(".github/workflows/release.yml")).expect("release workflow");
    let value: serde_yaml::Value = serde_yaml::from_str(&workflow).expect("valid yaml");

    assert!(value.get("jobs").is_some());
    assert!(workflow.contains("tauri build"));
    assert!(workflow.contains("portable"));
}
