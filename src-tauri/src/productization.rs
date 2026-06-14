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
fn release_version_is_consistent() {
    let tauri_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = tauri_root.parent().expect("repo root").to_path_buf();
    let release_version = env!("CARGO_PKG_VERSION");

    let tauri_config =
        fs::read_to_string(tauri_root.join("tauri.conf.json")).expect("tauri config");
    let tauri_value: serde_json::Value =
        serde_json::from_str(&tauri_config).expect("json tauri config");
    assert_eq!(tauri_value["version"], release_version);
    assert_eq!(
        tauri_value["app"]["windows"][0]["title"],
        format!("QuotaBarWin V{release_version}")
    );

    let package = fs::read_to_string(repo_root.join("package.json")).expect("package json");
    let package_value: serde_json::Value = serde_json::from_str(&package).expect("json package");
    assert_eq!(package_value["version"], release_version);

    let lockfile = fs::read_to_string(repo_root.join("package-lock.json")).expect("package lock");
    let lockfile_value: serde_json::Value =
        serde_json::from_str(&lockfile).expect("json package lock");
    assert_eq!(lockfile_value["version"], release_version);
    assert_eq!(lockfile_value["packages"][""]["version"], release_version);
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
