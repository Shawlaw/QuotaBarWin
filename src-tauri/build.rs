use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-env-changed=QUOTABARWIN_GIT_COMMIT");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=QUOTABARWIN_UPDATE_PUBLIC_KEY");
    emit_git_rerun_directives();

    let git_commit = env_commit().or_else(git_short_commit).unwrap_or_default();
    println!("cargo:rustc-env=QUOTABARWIN_GIT_SHORT_HASH={git_commit}");

    tauri_build::build();
}

fn env_commit() -> Option<String> {
    std::env::var("QUOTABARWIN_GIT_COMMIT")
        .or_else(|_| std::env::var("GITHUB_SHA"))
        .ok()
        .and_then(|value| short_hash(value.trim()))
}

fn git_short_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    short_hash(String::from_utf8_lossy(&output.stdout).trim())
}

fn short_hash(value: &str) -> Option<String> {
    let hash: String = value
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .take(7)
        .collect();
    (!hash.is_empty()).then_some(hash)
}

fn emit_git_rerun_directives() {
    let git_dir = PathBuf::from("../.git");
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );

    if let Some(ref_path) = current_head_ref_path(&git_dir) {
        println!("cargo:rerun-if-changed={}", ref_path.display());
    }
}

fn current_head_ref_path(git_dir: &Path) -> Option<PathBuf> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let ref_name = head.trim().strip_prefix("ref: ")?;
    Some(git_dir.join(ref_name))
}
