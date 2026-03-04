//! Integration tests for the Rona CLI tool.
//!
//! These tests verify the core functionality of Rona's command-line interface,
//! including help, version, file staging, and commit operations. Each test
//! runs in an isolated temporary directory to prevent interference with the
//! user's actual git repositories and configuration.
//!
//! # Test Structure
//!
//! - Each test creates a temporary directory for isolation
//! - Git repositories are initialized in these temp directories
//! - Tests verify both successful operations and error cases
//! - File operations are verified using git status and log commands
//!
//! # Environment
//!
//! Tests require:
//! - Git to be installed and available in PATH
//! - Write permissions for temporary directories
//! - No interference with user's actual git configuration

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use mockall::predicate;
use std::fs;
use tempfile::TempDir;

/// Tests the help command functionality.
///
/// Verifies that:
/// - The help command executes successfully
/// - Output contains usage information
/// - Output contains options documentation
#[test]
fn test_help_command() {
    let mut cmd = cargo_bin_cmd!("rona");
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("USAGE"))
        .stdout(predicate::str::contains("OPTIONS"));
}

/// Tests the version command functionality.
///
/// Verifies that:
/// - The version command executes successfully
/// - Output contains the correct version number from Cargo.toml
#[test]
fn test_version_command() {
    let mut cmd = cargo_bin_cmd!("rona");
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// Tests the file staging functionality with pattern exclusion.
///
/// Verifies that:
/// - Files matching the pattern are staged
/// - Files not matching the pattern remain unstaged
/// - Git status shows correct staging state
#[test]
fn test_add_command() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repository
    let mut git_init = Command::new("git");
    git_init.current_dir(temp_path).arg("init");
    git_init.assert().success();

    // Create test files
    fs::write(temp_path.join("test.txt"), "test content").unwrap();
    fs::write(temp_path.join("test2.md"), "test content").unwrap();
    fs::write(temp_path.join("test3.md"), "test content").unwrap();

    // Test rona add with pattern exclusion
    let mut cmd = cargo_bin_cmd!("rona");
    cmd.current_dir(temp_path).arg("-a").arg(r"*.md"); // exclude all markdown files
    cmd.assert().success();

    // Verify file staging status
    let mut git_status = Command::new("git");
    git_status
        .current_dir(temp_path)
        .args(["status", "--porcelain", "-u"]);

    git_status
        .assert()
        .success()
        .stdout(predicate::str::contains(r"A  test.txt")) // .txt file added
        .stdout(predicate::str::contains(r"?? test2.md")) // .md file excluded
        .stdout(predicate::str::contains(r"?? test3.md")); // .md file excluded
}

/// Tests that `rona -a` correctly stages files when run from a subdirectory.
///
/// Regression test for the doubled-path bug: `git status --porcelain=v1` returns
/// paths relative to the repo root, but if `git add` is invoked without
/// `.current_dir(repo_root)` it inherits the CWD, causing git to interpret those
/// paths as relative to the subdirectory and producing a doubled path such as
/// `packages/preview/pkg/1.0/packages/preview/pkg/1.0/file.png`.
///
/// Verifies that:
/// - Files in a deeply nested subdirectory are staged correctly
/// - Paths are not doubled in the git index
/// - `git add` succeeds when the user's CWD is not the repo root
#[test]
fn test_add_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repository at the root
    Command::new("git")
        .current_dir(temp_path)
        .arg("init")
        .assert()
        .success();

    // Create a deep nested directory structure (mirrors the reported scenario)
    let subdir = temp_path.join("packages/preview/clean-cnam-template/1.6.4");
    fs::create_dir_all(&subdir).unwrap();

    // Create files inside the nested directory
    fs::write(subdir.join("thumbnail.png"), "fake png").unwrap();
    fs::write(subdir.join("README.md"), "# readme").unwrap();

    // Run `rona -a` from the subdirectory, not from the repo root
    let mut cmd = cargo_bin_cmd!("rona");
    cmd.current_dir(&subdir).arg("-a");
    cmd.assert().success();

    // Verify files are staged with correct (non-doubled) paths
    let git_status = Command::new("git")
        .current_dir(temp_path)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();

    let status_output = String::from_utf8_lossy(&git_status.stdout);

    // Both files must appear as staged (index 'A') with their repo-root-relative paths
    assert!(
        status_output.contains("A  packages/preview/clean-cnam-template/1.6.4/thumbnail.png"),
        "thumbnail.png should be staged with correct path, got:\n{status_output}"
    );
    assert!(
        status_output.contains("A  packages/preview/clean-cnam-template/1.6.4/README.md"),
        "README.md should be staged with correct path, got:\n{status_output}"
    );

    // Ensure no doubled path appears in output
    assert!(
        !status_output.contains("packages/preview/clean-cnam-template/1.6.4/packages"),
        "Paths must not be doubled, got:\n{status_output}"
    );
}

/// Tests that `rona -a` correctly handles deleted files when run from a subdirectory.
///
/// Extends the subdirectory regression test to also cover the `git rm --cached`
/// path, ensuring deleted files are also resolved relative to the repo root.
///
/// Verifies that:
/// - A file deleted in a subdirectory is staged for deletion correctly
/// - The deletion is reflected in `git status` without path doubling
#[test]
fn test_add_deleted_file_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .current_dir(temp_path)
        .arg("init")
        .assert()
        .success();

    // Configure git user so we can commit
    for (key, val) in [
        ("user.name", "Test"),
        ("user.email", "t@t.com"),
        ("commit.gpgsign", "false"),
    ] {
        Command::new("git")
            .current_dir(temp_path)
            .args(["config", "--local", key, val])
            .assert()
            .success();
    }

    // Create the nested directory and a file, then commit it
    let subdir = temp_path.join("packages/preview/mypkg/1.0");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("asset.png"), "data").unwrap();

    Command::new("git")
        .current_dir(temp_path)
        .args(["add", "--all"])
        .assert()
        .success();

    Command::new("git")
        .current_dir(temp_path)
        .args(["commit", "-m", "initial"])
        .assert()
        .success();

    // Delete the file
    fs::remove_file(subdir.join("asset.png")).unwrap();

    // Run `rona -a` from the subdirectory
    let mut cmd = cargo_bin_cmd!("rona");
    cmd.current_dir(&subdir).arg("-a");
    cmd.assert().success();

    // Deleted file must be staged (index 'D') with correct non-doubled path
    let git_status = Command::new("git")
        .current_dir(temp_path)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();

    let status_output = String::from_utf8_lossy(&git_status.stdout);

    assert!(
        status_output.contains("D  packages/preview/mypkg/1.0/asset.png"),
        "Deleted file should be staged with correct path, got:\n{status_output}"
    );

    assert!(
        !status_output.contains("packages/preview/mypkg/1.0/packages"),
        "Paths must not be doubled for deleted files, got:\n{status_output}"
    );
}

/// Tests the commit functionality.
///
/// Verifies that:
/// - Git user configuration is properly set
/// - Files can be staged and committed
/// - Commit message is correctly applied
/// - Git log shows the commit with correct message
#[test]
fn test_commit_command() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repository
    let mut git_init = Command::new("git");
    git_init.current_dir(temp_path).arg("init");
    git_init.assert().success();

    // Configure git user (using --local to only affect this test repository)
    let mut git_config = Command::new("git");
    git_config
        .current_dir(temp_path)
        .args(["config", "--local", "user.name", "Test User"]);
    git_config.assert().success();

    let mut git_config_email = Command::new("git");
    git_config_email.current_dir(temp_path).args([
        "config",
        "--local",
        "user.email",
        "test@example.com",
    ]);
    git_config_email.assert().success();

    // Ensure GPG signing does not interfere with the test (using --local)
    let mut git_disable_gpgsign = Command::new("git");
    git_disable_gpgsign.current_dir(temp_path).args([
        "config",
        "--local",
        "commit.gpgsign",
        "false",
    ]);
    git_disable_gpgsign.assert().success();

    // Create and stage a test file
    fs::write(temp_path.join("test.txt"), "test content").unwrap();
    let mut git_add = Command::new("git");
    git_add.current_dir(temp_path).args(["add", "test.txt"]);
    git_add.assert().success();

    // Create commit message file with proper format
    let commit_msg = "[1] (feat on main)\n\n- `test.txt`:\n\n\t\n";
    fs::write(temp_path.join("commit_message.md"), commit_msg).unwrap();

    // Test rona commit with --yes to skip confirmation
    let mut cmd = cargo_bin_cmd!("rona");
    cmd.current_dir(temp_path).arg("-c").arg("--yes");
    cmd.assert().success();

    // Verify commit message in git log
    let mut git_log = Command::new("git");
    git_log
        .current_dir(temp_path)
        .args(["log", "-1", "--oneline"]);

    git_log
        .assert()
        .success()
        .stdout(predicate::str::contains("feat"));
}
