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
fn tauri_csp_is_enabled() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(root.join("tauri.conf.json")).expect("tauri config");
    let value: serde_json::Value = serde_json::from_str(&config).expect("json config");

    let csp = value
        .pointer("/app/security/csp")
        .and_then(serde_json::Value::as_object)
        .expect("release CSP should be configured");
    assert_eq!(csp.get("default-src"), Some(&serde_json::json!("'self'")));
    assert_eq!(
        csp.get("connect-src"),
        Some(&serde_json::json!("ipc: http://ipc.localhost"))
    );
    assert!(
        value
            .pointer("/app/security/dangerousDisableAssetCspModification")
            .is_none(),
        "Tauri CSP source injection should stay enabled"
    );

    let dev_csp = value
        .pointer("/app/security/devCsp")
        .and_then(serde_json::Value::as_object)
        .expect("dev CSP should be configured explicitly");
    assert!(dev_csp
        .get("connect-src")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|sources| sources.contains("ws://localhost:1420")));
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
fn open_source_metadata_is_present() {
    let tauri_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = tauri_root.parent().expect("repo root").to_path_buf();

    for file_name in [
        "LICENSE",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "CODE_OF_CONDUCT.md",
        ".github/workflows/ci.yml",
    ] {
        assert!(
            repo_root.join(file_name).is_file(),
            "{file_name} should exist before opening the repository"
        );
    }

    let package = fs::read_to_string(repo_root.join("package.json")).expect("package json");
    let package_value: serde_json::Value = serde_json::from_str(&package).expect("json package");
    assert_eq!(package_value["license"], "MIT");
    assert_eq!(
        package_value["repository"]["url"],
        "git+https://github.com/Shawlaw/QuotaBarWin.git"
    );
    assert_eq!(package_value["engines"]["node"], ">=22");

    let manifest = fs::read_to_string(tauri_root.join("Cargo.toml")).expect("cargo manifest");
    assert!(manifest.contains("license = \"MIT\""));
    assert!(manifest.contains("repository = \"https://github.com/Shawlaw/QuotaBarWin\""));
    assert!(manifest.contains("default-run = \"quotabarwin\""));

    let readme = fs::read_to_string(repo_root.join("README.md")).expect("readme");
    assert!(readme.contains("E2E 当前是维护者可选检查"));
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
    assert!(workflow.contains("Build CLI companion"));
    assert!(workflow.contains("--bin quotabarwin-cli"));
    assert!(workflow.contains("QuotaBarWin.Cli.exe"));
    assert!(workflow.contains("Build update helper"));
    assert!(workflow.contains("--bin quotabarwin-updater"));
    assert!(workflow.contains("QuotaBarWin.Updater.exe"));
    assert!(workflow.contains("QUOTABARWIN_UPDATE_PUBLIC_KEY"));
    assert!(workflow.contains("publish-portable-update@v0.1.4"));
    assert!(workflow.contains("DESKTOP_UPDATE_PRIVATE_KEY"));
    assert!(workflow.contains("public-key: ${{ vars.QUOTABARWIN_UPDATE_PUBLIC_KEY }}"));
    assert!(workflow.contains("portable"));
    assert!(workflow.contains("quotabarwin.portable"));
    assert!(workflow.contains("Compute release metadata"));
    assert!(workflow.contains("Generate release notes"));
    assert!(workflow.contains("CHANGELOG.md does not contain release notes"));
    assert!(workflow
        .contains("zip_name=QuotaBarWin_$($versionName)_windows_x64_portable_$commitId.zip"));
    assert!(workflow.contains("softprops/action-gh-release@v2"));
    assert!(!workflow.contains("TAURI_SIGNING_PRIVATE_KEY"));
    assert!(!workflow.contains("Collect installer artifacts"));
    assert!(workflow.contains("artifact_name=quotabarwin-release($($versionName)_$commitId)"));
    assert!(workflow.contains("steps.release_meta.outputs.artifact_name"));
    assert!(workflow.contains("steps.release_meta.outputs.zip_name"));
    assert!(workflow.contains("body_path: artifacts/release-notes.md"));
}
