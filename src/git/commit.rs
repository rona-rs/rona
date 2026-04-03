//! Commit Operations
//!
//! Git commit-related functionality including commit counting, commit message generation,
//! and commit execution operations.

use std::{
    fs::{File, OpenOptions, read_to_string, write},
    io::Write,
    path::Path,
    process::Command,
};

use colored::Colorize;

use crate::{
    errors::{GitError, Result, RonaError},
    git::branch::{format_branch_name, get_current_branch},
};

use super::{
    files::get_ignore_patterns,
    get_top_level_path,
    status::{process_deleted_files_for_commit_message, process_git_status},
};

pub const COMMIT_MESSAGE_FILE_PATH: &str = "commit_message.md";
pub const COMMIT_TYPES: [&str; 4] = ["chore", "feat", "fix", "test"];

/// Gets the total number of commits in the current branch.
///
/// This function counts all commits reachable from the current HEAD.
/// Returns 0 for a fresh repository with no commits.
///
/// # Errors
///
/// Returns an error if:
/// - Not currently in a git repository
/// - The commit count output cannot be parsed
///
/// # Returns
///
/// The total number of commits as a `u32`
///
/// # Examples
///
/// ```no_run
/// use rona::git::commit::get_current_commit_nb;
///
/// let commit_count = get_current_commit_nb()?;
/// println!("This repository has {} commits", commit_count);
///
/// // Use for commit numbering
/// let next_commit_number = commit_count + 1;
/// println!("Next commit will be #{}", next_commit_number);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn get_current_commit_nb() -> Result<u32> {
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        // Likely a fresh repository with no commits
        return Ok(0);
    }

    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    count_str.parse::<u32>().map_err(|_| {
        RonaError::Git(GitError::InvalidStatus {
            output: format!("Failed to parse commit count: {count_str}"),
        })
    })
}

/// Detects if GPG signing is configured in git.
///
/// Checks whether a signing key is configured via `git config --get user.signingkey`.
/// When this returns `true`, git will attempt to sign commits automatically.
///
/// # Returns
/// * `true` if a signing key is configured
/// * `false` if no signing key is configured
///
/// # Examples
///
/// ```no_run
/// use rona::git::commit::is_gpg_signing_available;
///
/// if is_gpg_signing_available() {
///     println!("GPG signing is configured");
/// } else {
///     println!("GPG signing is not configured, will create unsigned commit");
/// }
/// ```
#[must_use]
pub fn is_gpg_signing_available() -> bool {
    let output = Command::new("git")
        .args(["config", "--get", "user.signingkey"])
        .output();

    match output {
        Ok(out) => out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        Err(_) => false,
    }
}

/// Handles dry run output for commit operations.
///
/// # Arguments
/// * `file_content` - The commit message content
/// * `unsigned` - Whether the commit should be unsigned
/// * `filtered_args` - Additional git arguments
/// * `is_amend` - Whether this is an amend operation
fn handle_dry_run_output(
    file_content: &str,
    unsigned: bool,
    filtered_args: &[String],
    is_amend: bool,
) {
    println!("Would commit with message:");
    println!("---");
    println!("{}", file_content.trim());
    println!("---");

    if is_amend {
        println!("Would amend the previous commit");
    }

    let gpg_available = is_gpg_signing_available();
    let would_sign = !unsigned && gpg_available;

    if unsigned {
        println!("Would create unsigned commit");
    } else if would_sign {
        println!("Would sign commit with GPG");
    } else {
        println!("Would create unsigned commit (GPG signing not available)");
        if !gpg_available {
            println!(
                "{} GPG signing not available or not configured.",
                "WARNING:".yellow().bold()
            );
            println!("   To suppress this warning, use the --unsigned (-u) flag.");
        }
    }

    if !filtered_args.is_empty() {
        println!("With additional args: {filtered_args:?}");
    }
}

/// Commits files to the git repository using `git commit -F`.
///
/// This function reads the commit message from `commit_message.md` and creates
/// a git commit with that message. By using the git CLI directly, all git hooks
/// (pre-commit, commit-msg, post-commit, etc.) are triggered naturally.
///
/// GPG signing is handled by git's own configuration (`commit.gpgsign`,
/// `user.signingkey`). Pass `unsigned = true` to disable signing via
/// `--no-gpg-sign`.
///
/// # Arguments
/// * `args` - Additional arguments (supports `--amend` to amend the previous commit)
/// * `unsigned` - If true, creates an unsigned commit (passes `--no-gpg-sign`)
/// * `dry_run` - If true, only show what would be committed without actually committing
///
/// # Errors
/// * If the commit message file doesn't exist
/// * If reading the commit message file fails
/// * If the git commit command fails
/// * If not in a git repository
///
/// # Examples
///
/// ```no_run
/// use rona::git::commit::git_commit;
///
/// // Commit with automatic GPG detection (default)
/// git_commit(&[], false, false)?;
///
/// // Unsigned commit
/// git_commit(&[], true, false)?;
///
/// // Amend the previous commit
/// git_commit(&["--amend".to_string()], false, false)?;
///
/// // Dry run to preview the commit
/// git_commit(&[], false, true)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[tracing::instrument(skip_all)]
pub fn git_commit(args: &[String], unsigned: bool, dry_run: bool) -> Result<()> {
    tracing::debug!(unsigned, dry_run, "Committing files...");

    let project_root = get_top_level_path()?;
    let commit_file_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    if !commit_file_path.exists() {
        return Err(RonaError::Git(GitError::CommitMessageNotFound));
    }

    let file_content = read_to_string(&commit_file_path)?;

    // Detect --amend and filter out flags that don't apply to git commit -F
    let is_amend = args.iter().any(|arg| arg == "--amend");
    let filtered_args: Vec<String> = args
        .iter()
        .filter(|arg| !arg.starts_with("-c") && !arg.starts_with("--commit") && *arg != "--amend")
        .cloned()
        .collect();

    if dry_run {
        handle_dry_run_output(&file_content, unsigned, &filtered_args, is_amend);
        return Ok(());
    }

    // Warn if user expects signing but no key is configured
    if !unsigned && !is_gpg_signing_available() {
        println!(
            "{} GPG signing not available or not configured. Creating unsigned commit.",
            "WARNING:".yellow().bold()
        );
        println!("   To suppress this warning, use the --unsigned (-u) flag.");
    }

    let commit_file_str = commit_file_path.to_str().ok_or_else(|| {
        RonaError::Git(GitError::CommandFailed {
            command: "commit".to_string(),
            output: "Invalid path to commit message file".to_string(),
        })
    })?;

    let mut cmd = Command::new("git");
    cmd.arg("commit");

    if is_amend {
        cmd.arg("--amend");
    }

    if unsigned {
        cmd.arg("--no-gpg-sign");
    }

    cmd.args(["-F", commit_file_str]);

    // Use .status() so git inherits stdin/stdout/stderr.
    // This allows hooks to run and interactive GPG prompts to work.
    let status = cmd.status().map_err(RonaError::Io)?;

    if !status.success() {
        return Err(RonaError::Git(GitError::CommandFailed {
            command: "commit".to_string(),
            output: "git commit failed".to_string(),
        }));
    }

    tracing::debug!("commit successful!");

    Ok(())
}

/// Prepares the commit message.
/// It creates the commit message file and empties it if it already exists.
/// It also adds the modified / added files to the commit message file.
///
/// # Errors
/// * If we cannot write to the commit message file
/// * If we cannot read the git status
/// * If we cannot process either git status or deleted files from the git status
/// * If we cannot read the commitignore file
///
/// # Arguments
/// * `commit_type` - `&str` - The commit type
/// * `no_commit_number` - `bool` - Whether to include the commit number in the header
#[tracing::instrument(skip_all)]
pub fn generate_commit_message(commit_type: &str, no_commit_number: bool) -> Result<()> {
    let project_root = get_top_level_path()?;
    let commit_message_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    // Empty the file if it exists
    if commit_message_path.exists() {
        write(&commit_message_path, "")?;
    }

    // Get git status info
    let modified_files = process_git_status()?;
    let deleted_files = process_deleted_files_for_commit_message()?;

    // Open the commit file for writing
    let mut commit_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&commit_message_path)?;

    // Write header
    write_commit_header(&mut commit_file, commit_type, no_commit_number)?;

    // Get files to ignore
    let ignore_patterns = get_ignore_patterns()?;

    // Process modified files
    for file in modified_files {
        if !should_ignore_file(&file, &ignore_patterns)? {
            writeln!(commit_file, "- `{file}`:\n\n\t\n")?;
        }
    }

    // Process deleted files
    for file in deleted_files {
        writeln!(commit_file, "- `{file}`: deleted\n")?;
    }

    // Close the file
    commit_file.flush()?;

    tracing::debug!("{} created", commit_message_path.display());

    Ok(())
}

/// Writes the commit header to the commit file.
///
/// # Arguments
/// * `commit_file` - The file to write to
/// * `commit_type` - The type of commit
/// * `no_commit_number` - Whether to include the commit number in the header
///
/// # Errors
/// * If writing to the file fails
fn write_commit_header(
    commit_file: &mut File,
    commit_type: &str,
    no_commit_number: bool,
) -> Result<()> {
    let branch_name = format_branch_name(&COMMIT_TYPES, &get_current_branch()?);

    if no_commit_number {
        writeln!(commit_file, "({commit_type} on {branch_name})\n\n")?;
    } else {
        let commit_number = get_current_commit_nb()? + 1;
        writeln!(
            commit_file,
            "[{commit_number}] ({commit_type} on {branch_name})\n\n"
        )?;
    }

    Ok(())
}

/// Checks if a file should be ignored based on ignored patterns.
///
/// # Arguments
/// * `file` - The file to check
/// * `ignore_patterns` - Patterns to check against
///
/// # Errors
/// * If checking file paths fails
///
/// # Returns
/// * `true` if the file should be ignored, `false` otherwise
fn should_ignore_file(file: &str, ignore_patterns: &[String]) -> Result<bool> {
    use crate::utils::check_for_file_in_folder;

    // Check if the file is directly in the ignore list
    if ignore_patterns.contains(&file.to_string()) {
        return Ok(true);
    }

    // Check if the file is in a folder that's in the ignore list
    let file_path = Path::new(file);

    for item in ignore_patterns {
        let item_path = Path::new(item);

        if check_for_file_in_folder(file_path, item_path)? {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Tests that call set_current_dir must serialize — it is process-global state.
    static DIR_MUTEX: Mutex<()> = Mutex::new(());

    /// Initializes a minimal git repo in `path` suitable for making real commits.
    #[cfg(unix)]
    fn init_git_repo(
        path: &std::path::Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            Command::new("git").current_dir(path).args(&args).output()?;
        }
        Ok(())
    }

    #[test]
    fn test_gpg_signing_available() {
        // Verifies the function does not panic; result depends on system config.
        let _result = is_gpg_signing_available();
    }

    #[test]
    fn test_git_commit_dry_run_with_unsigned() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let _guard = DIR_MUTEX.lock().map_err(|e| e.to_string())?;

        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        Command::new("git")
            .current_dir(temp_path)
            .arg("init")
            .output()?;

        let commit_msg = "[1] (test on main)\n\n- `test.txt`:\n\n\t\n";
        write(temp_path.join("commit_message.md"), commit_msg)?;

        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(temp_path)?;

        let result = git_commit(&[], true, true);

        std::env::set_current_dir(original_dir)?;

        assert!(result.is_ok());
        Ok(())
    }

    /// Verifies that a `pre-commit` hook is triggered when `git_commit` runs.
    ///
    /// The hook writes a marker file. If it fires, the file exists after the commit.
    /// This test would have been impossible with git2 (which bypasses all hooks).
    #[test]
    #[cfg(unix)]
    fn test_pre_commit_hook_fires() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let _guard = DIR_MUTEX.lock().map_err(|e| e.to_string())?;

        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        init_git_repo(temp_path)?;

        // Stage a file so the commit has something to record.
        write(temp_path.join("test.txt"), "hello")?;
        Command::new("git")
            .current_dir(temp_path)
            .args(["add", "test.txt"])
            .output()?;

        // Install a pre-commit hook that writes a marker file.
        let hooks_dir = temp_path.join(".git/hooks");
        std::fs::create_dir_all(&hooks_dir)?;
        let hook_path = hooks_dir.join("pre-commit");
        write(&hook_path, "#!/bin/sh\ntouch HOOK_FIRED\n")?;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;

        write(
            temp_path.join("commit_message.md"),
            "(test on main)\n\n- `test.txt`:\n\n\t\n",
        )?;

        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(temp_path)?;

        let result = git_commit(&[], true, false);

        std::env::set_current_dir(&original_dir)?;

        assert!(result.is_ok(), "commit failed: {result:?}");
        assert!(
            temp_path.join("HOOK_FIRED").exists(),
            "pre-commit hook did not fire"
        );
        Ok(())
    }

    /// Verifies that a failing `pre-commit` hook prevents the commit.
    ///
    /// A hook that exits non-zero must cause `git_commit` to return an error.
    #[test]
    #[cfg(unix)]
    fn test_pre_commit_hook_blocks_commit() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let _guard = DIR_MUTEX.lock().map_err(|e| e.to_string())?;

        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        init_git_repo(temp_path)?;

        write(temp_path.join("test.txt"), "hello")?;
        Command::new("git")
            .current_dir(temp_path)
            .args(["add", "test.txt"])
            .output()?;

        // Install a pre-commit hook that always rejects the commit.
        let hooks_dir = temp_path.join(".git/hooks");
        std::fs::create_dir_all(&hooks_dir)?;
        let hook_path = hooks_dir.join("pre-commit");
        write(&hook_path, "#!/bin/sh\nexit 1\n")?;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;

        write(
            temp_path.join("commit_message.md"),
            "(test on main)\n\n- `test.txt`:\n\n\t\n",
        )?;

        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(temp_path)?;

        let result = git_commit(&[], true, false);

        std::env::set_current_dir(&original_dir)?;

        assert!(
            result.is_err(),
            "commit should have been blocked by the pre-commit hook"
        );
        Ok(())
    }
}
