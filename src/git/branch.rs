//! Branch Operations
//!
//! Git branch-related functionality including branch information retrieval
//! and branch name formatting utilities.

use crate::{
    errors::{GitError, Result, RonaError},
    git::{commit::get_current_commit_nb, handle_output, repository::open_repo},
};
use indicatif::{ProgressBar, ProgressDrawTarget};
use std::io::IsTerminal;
use std::process::Command;
use std::time::Duration;

/// Attempts to get the default branch name from git config.
///
/// This helper function tries to retrieve the default branch name using
/// the git2 config API. If successful, it returns the branch name.
/// If the config lookup fails, it returns a default of "main".
///
/// # Returns
///
/// * `Ok(String)` - The default branch name if successfully retrieved, or "main" as fallback
fn try_get_default_branch() -> Result<String> {
    let repo = open_repo()?;
    let config = repo.config()?;

    config
        .get_string("init.defaultBranch")
        .map_or_else(|_| Ok("main".to_string()), Ok)
}

/// Gets the current branch name.
///
/// This function returns the name of the currently checked out branch.
/// For detached HEAD states, it returns "HEAD".
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
    let repo = open_repo()?;

    // Try to get the current branch reference
    let head = repo.head();

    match head {
        Ok(reference) => {
            // Check if HEAD is pointing to a branch
            if reference.is_branch() {
                // Get the branch name
                let branch_name = reference
                    .shorthand()
                    .ok_or_else(|| {
                        RonaError::Git(GitError::CommandFailed {
                            command: "get current branch".to_string(),
                            output: "Failed to get branch shorthand name".to_string(),
                        })
                    })?
                    .to_string();
                Ok(branch_name)
            } else {
                // Detached HEAD state
                Ok("HEAD".to_string())
            }
        }
        Err(_) => {
            // HEAD doesn't exist, likely a fresh repository with no commits
            // Check if there are any commits
            match get_current_commit_nb() {
                Ok(0) => {
                    // Fresh repository with no commits
                    // Try to get the default branch name
                    try_get_default_branch()
                }
                Ok(_) => {
                    // Repository has commits, something is wrong
                    Err(RonaError::Git(GitError::CommandFailed {
                        command: "get current branch".to_string(),
                        output: "Failed to get HEAD reference".to_string(),
                    }))
                }
                Err(_) => {
                    // Can't determine commit count, likely a fresh repo with no HEAD
                    // Try to get the default branch name
                    try_get_default_branch()
                }
            }
        }
    }
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

/// Switches to a different branch using git2's checkout API.
///
/// Uses safe checkout mode, which refuses to overwrite uncommitted changes
/// that would be lost during the switch (matching `git switch` behavior).
///
/// # Arguments
/// * `branch_name` - The name of the branch to switch to
/// * `verbose` - Whether to print verbose output during the operation
///
/// # Errors
/// * If the branch doesn't exist
/// * If there are uncommitted changes that would be lost
/// * If the checkout operation fails
pub fn git_switch(branch_name: &str, verbose: bool) -> Result<()> {
    if verbose {
        println!("\nSwitching to branch: {branch_name}");
    }

    let repo = open_repo()?;

    // Resolve the branch name to a tree-ish object and its reference
    let (object, reference) = repo.revparse_ext(branch_name).map_err(|_| {
        RonaError::Git(GitError::CommandFailed {
            command: "switch".to_string(),
            output: format!("Branch '{branch_name}' not found"),
        })
    })?;

    // Update working directory (safe mode won't overwrite dirty files)
    repo.checkout_tree(&object, Some(git2::build::CheckoutBuilder::new().safe()))?;

    // Update HEAD to point to the branch
    let ref_name = reference
        .and_then(|r| r.name().map(String::from))
        .unwrap_or_else(|| format!("refs/heads/{branch_name}"));
    repo.set_head(&ref_name)?;

    if verbose {
        println!("switch successful!");
    }

    Ok(())
}

/// Creates a new branch and switches to it using git2.
///
/// This is equivalent to `git switch -c <branch_name>`. It creates a new branch
/// at the current HEAD commit and checks it out.
///
/// # Arguments
/// * `branch_name` - The name of the branch to create
/// * `verbose` - Whether to print verbose output during the operation
///
/// # Errors
/// * If a branch with that name already exists
/// * If there is no HEAD commit (empty repository)
/// * If the checkout operation fails
pub fn git_create_branch(branch_name: &str, verbose: bool) -> Result<()> {
    if verbose {
        println!("\nCreating new branch: {branch_name}");
    }

    let repo = open_repo()?;

    // Get the current HEAD commit to create the branch from
    let head_commit = repo.head()?.peel_to_commit().map_err(|_| {
        RonaError::Git(GitError::CommandFailed {
            command: "create branch".to_string(),
            output: "Cannot create branch: no commits in repository".to_string(),
        })
    })?;

    // Create the new branch pointing at HEAD (false = don't force-overwrite)
    let branch = repo.branch(branch_name, &head_commit, false)?;

    // Point HEAD to the new branch
    let ref_name = branch
        .into_reference()
        .name()
        .ok_or_else(|| {
            RonaError::Git(GitError::CommandFailed {
                command: "create branch".to_string(),
                output: "Branch reference has no name".to_string(),
            })
        })?
        .to_string();
    repo.set_head(&ref_name)?;

    // Update working directory to match
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().safe()))?;

    if verbose {
        println!("create branch successful!");
    }

    Ok(())
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
    if verbose {
        println!("\nPulling latest changes...");
    }

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

    handle_output("pull", &output, verbose)
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
    if verbose {
        println!("\nMerging {branch_name} into current branch...");
    }

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

    handle_output("merge", &output, verbose)
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
    if verbose {
        println!("\nRebasing onto {branch_name}...");
    }

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

    handle_output("rebase", &output, verbose)
}
