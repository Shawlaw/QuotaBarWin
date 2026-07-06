use std::fs;

#[test]
fn tauri_config_is_portable_only() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(root.join("tauri.conf.json")).expect("tauri config");
    let value: serde_json::Value = serde_json::from_str(&config).expect("json config");

    assert_eq!(
        value.pointer("/bundle/active"),
        Some(&serde_json::json!(false))
    );
    assert!(value.pointer("/bundle/createUpdaterArtifacts").is_none());
    assert!(value.pointer("/bundle/targets").is_none());
    assert!(value.pointer("/plugins/updater").is_none());
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
    assert_eq!(
        value
            .get("permissions")
            .and_then(|permissions| permissions.get("contents"))
            .and_then(serde_yaml::Value::as_str),
        Some("write")
    );
    assert!(workflow.contains("tauri -- build --no-bundle"));
    assert!(workflow.contains("portable"));
    assert!(workflow.contains("quotabarwin.portable"));
    assert!(workflow.contains("Compute release metadata"));
    assert!(workflow
        .contains("zip_name=QuotaBarWin_$($versionName)_windows_x64_portable_$commitId.zip"));
    assert!(workflow.contains("softprops/action-gh-release@v2"));
    assert!(!workflow.contains("TAURI_SIGNING_PRIVATE_KEY"));
    assert!(!workflow.contains("Collect installer artifacts"));
    assert!(workflow.contains("artifact_name=quotabarwin-release($($versionName)_$commitId)"));
    assert!(workflow.contains("steps.release_meta.outputs.artifact_name"));
    assert!(workflow.contains("steps.release_meta.outputs.zip_name"));
}
