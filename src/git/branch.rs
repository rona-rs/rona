//! Branch Operations
//!
//! Git branch-related functionality including branch information retrieval
//! and branch name formatting utilities.

use crate::{
    errors::{Result, RonaError},
    git::handle_output,
};
use indicatif::{ProgressBar, ProgressDrawTarget};
use std::io::IsTerminal;
use std::process::Command;
use std::time::Duration;

/// Attempts to get the default branch name from git config.
///
/// This helper function tries to retrieve the default branch name using
/// `git config --get init.defaultBranch`. If successful, it returns the branch name.
/// If the config lookup fails, it returns a default of "main".
///
/// # Returns
///
/// * `Ok(String)` - The default branch name if successfully retrieved, or "main" as fallback
fn try_get_default_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["config", "--get", "init.defaultBranch"])
        .output()
        .map_err(RonaError::Io)?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Ok(branch);
        }
    }

    Ok("main".to_string())
}

/// Gets the current branch name.
///
/// This function returns the name of the currently checked out branch.
/// For detached HEAD states, it returns "HEAD".
/// For fresh repositories with no commits, it returns the configured default branch.
///
/// # Errors
///
/// Returns an error if:
/// - Not currently in a git repository
/// - Unable to determine the current branch (e.g., in a corrupted repository)
///
/// # Returns
///
/// The name of the current branch as a `String`
///
/// # Examples
///
/// ```no_run
/// use rona::git::branch::get_current_branch;
///
/// let branch = get_current_branch()?;
/// println!("Current branch: {}", branch);
///
/// // Use in conditional logic
/// if branch == "main" {
///     println!("On main branch");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn get_current_branch() -> Result<String> {
    // Primary: git symbolic-ref --short HEAD
    // Works for normal branches and fresh repositories (returns default branch name).
    // Fails with non-zero exit code for detached HEAD state.
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .map_err(RonaError::Io)?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Ok(branch);
        }
    }

    // Fallback: git rev-parse --abbrev-ref HEAD
    // Returns "HEAD" for detached HEAD state.
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map_err(RonaError::Io)?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Ok(branch);
        }
    }

    // Last resort: look up the configured default branch name
    try_get_default_branch()
}

/// Formats a branch name by removing commit type prefixes.
///
/// This function cleans up branch names that follow conventional naming patterns
/// like `feat/feature-name`, `fix/bug-name`, etc., by removing the commit type
/// prefix and slash, leaving just the descriptive part of the branch name.
///
/// # Arguments
///
/// * `commit_types` - A slice of commit type prefixes to remove (e.g., `&["feat", "fix", "chore", "test"]`)
/// * `branch` - The branch name to format
///
/// # Returns
///
/// A formatted branch name with commit type prefixes removed
///
/// # Examples
///
/// ```
/// use rona::git::branch::format_branch_name;
///
/// let commit_types = ["feat", "fix", "chore", "test"];
///
/// assert_eq!(
///     format_branch_name(&commit_types, "feat/user-authentication"),
///     "user-authentication"
/// );
///
/// assert_eq!(
///     format_branch_name(&commit_types, "fix/memory-leak"),
///     "memory-leak"
/// );
///
/// // Branch names without prefixes are unchanged
/// assert_eq!(
///     format_branch_name(&commit_types, "main"),
///     "main"
/// );
///
/// // Multiple prefixes are handled
/// assert_eq!(
///     format_branch_name(&commit_types, "feat/fix/complex-branch"),
///     "fix/complex-branch"  // Only first matching prefix is removed
/// );
///
/// // Works with any number of commit types
/// assert_eq!(
///     format_branch_name(&["feat", "fix"], "fix/bug"),
///     "bug"
/// );
/// ```
///
/// # Use Cases
///
/// This is particularly useful for:
/// - Generating clean commit messages
/// - Creating readable branch displays in UI
/// - Normalizing branch names for processing
#[must_use]
pub fn format_branch_name(commit_types: &[&str], branch: &str) -> String {
    let mut formatted_branch = branch.to_owned();

    for commit_type in commit_types {
        if formatted_branch.contains(commit_type) {
            // Remove the `/commit_type` from the branch name
            formatted_branch = formatted_branch.replace(&format!("{commit_type}/"), "");
        }
    }

    formatted_branch
}

/// Switches to a different branch using `git switch`.
///
/// # Arguments
/// * `branch_name` - The name of the branch to switch to
///
/// # Errors
/// * If the branch doesn't exist
/// * If there are uncommitted changes that would be lost
/// * If the switch operation fails
#[tracing::instrument]
pub fn git_switch(branch_name: &str) -> Result<()> {
    tracing::debug!("Switching to branch: {branch_name}");

    let output = Command::new("git")
        .args(["switch", branch_name])
        .output()
        .map_err(RonaError::Io)?;

    handle_output("switch", &output)
}

/// Creates a new branch and switches to it using `git switch -c`.
///
/// This is equivalent to `git switch -c <branch_name>`. It creates a new branch
/// at the current HEAD commit and checks it out.
///
/// # Arguments
/// * `branch_name` - The name of the branch to create
///
/// # Errors
/// * If a branch with that name already exists
/// * If there is no HEAD commit (empty repository)
/// * If the operation fails
#[tracing::instrument]
pub fn git_create_branch(branch_name: &str) -> Result<()> {
    tracing::debug!("Creating new branch: {branch_name}");

    let output = Command::new("git")
        .args(["switch", "-c", branch_name])
        .output()
        .map_err(RonaError::Io)?;

    handle_output("create branch", &output)
}

/// Pulls changes from the remote repository.
///
/// # Arguments
/// * `verbose` - Whether to print verbose output during the operation
///
/// # Errors
/// * If there's no remote repository configured
/// * If the git pull command fails
/// * If there are merge conflicts
///
/// # Panics
/// * If the internal git pull thread panics (should not happen in normal use)
pub fn git_pull(verbose: bool) -> Result<()> {
    tracing::debug!("Pulling latest changes...");

    let show_spinner = !verbose && std::io::stderr().is_terminal();
    let output = if show_spinner {
        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_message("Pulling...");
        pb.enable_steady_tick(Duration::from_millis(80));
        let handle = std::thread::spawn(|| Command::new("git").arg("pull").output());
        let result = handle.join().expect("git pull thread panicked");
        pb.finish_and_clear();
        result?
    } else {
        Command::new("git").arg("pull").output()?
    };

    handle_output("pull", &output)
}

/// Merges a branch into the current branch.
///
/// # Arguments
/// * `branch_name` - The name of the branch to merge
/// * `verbose` - Whether to print verbose output during the operation
///
/// # Errors
/// * If there are merge conflicts
/// * If the git merge command fails
///
/// # Panics
/// * If the internal git merge thread panics (should not happen in normal use)
pub fn git_merge(branch_name: &str, verbose: bool) -> Result<()> {
    tracing::debug!("Merging {branch_name} into current branch...");

    let show_spinner = !verbose && std::io::stderr().is_terminal();
    let branch_owned = branch_name.to_string();
    let output = if show_spinner {
        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_message(format!("Merging {branch_name}..."));
        pb.enable_steady_tick(Duration::from_millis(80));
        let handle = std::thread::spawn(move || {
            Command::new("git").arg("merge").arg(&branch_owned).output()
        });
        let result = handle.join().expect("git merge thread panicked");
        pb.finish_and_clear();
        result?
    } else {
        Command::new("git").arg("merge").arg(branch_name).output()?
    };

    handle_output("merge", &output)
}

/// Rebases the current branch onto another branch.
///
/// # Arguments
/// * `branch_name` - The name of the branch to rebase onto
/// * `verbose` - Whether to print verbose output during the operation
///
/// # Errors
/// * If there are rebase conflicts
/// * If the git rebase command fails
///
/// # Panics
/// * If the internal git rebase thread panics (should not happen in normal use)
pub fn git_rebase(branch_name: &str, verbose: bool) -> Result<()> {
    tracing::debug!("Rebasing onto {branch_name}...");

    let show_spinner = !verbose && std::io::stderr().is_terminal();
    let branch_owned = branch_name.to_string();
    let output = if show_spinner {
        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_message(format!("Rebasing onto {branch_name}..."));
        pb.enable_steady_tick(Duration::from_millis(80));
        let handle = std::thread::spawn(move || {
            Command::new("git")
                .arg("rebase")
                .arg(&branch_owned)
                .output()
        });
        let result = handle.join().expect("git rebase thread panicked");
        pb.finish_and_clear();
        result?
    } else {
        Command::new("git")
            .arg("rebase")
            .arg(branch_name)
            .output()?
    };

    handle_output("rebase", &output)
}
