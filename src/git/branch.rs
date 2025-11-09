//! Branch Operations
//!
//! Git branch-related functionality including branch information retrieval
//! and branch name formatting utilities.

use crate::{
    errors::{GitError, Result, RonaError},
    git::{commit::get_current_commit_nb, repository::open_repo},
};

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

    match config.get_string("init.defaultBranch") {
        Ok(branch) => Ok(branch),
        Err(_) => Ok("main".to_string()), // Default to "main" if not configured
    }
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
                    .ok_or(RonaError::Git(GitError::CommandFailed {
                        command: "get current branch".to_string(),
                        output: "Failed to get branch shorthand name".to_string(),
                    }))?
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
/// * `commit_types` - An array of commit type prefixes to remove (e.g., `["feat", "fix", "chore", "test"]`)
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
/// ```
///
/// # Use Cases
///
/// This is particularly useful for:
/// - Generating clean commit messages
/// - Creating readable branch displays in UI
/// - Normalizing branch names for processing
#[must_use]
pub fn format_branch_name(commit_types: &[&str; 4], branch: &str) -> String {
    let mut formatted_branch = branch.to_owned();

    for commit_type in commit_types {
        if formatted_branch.contains(commit_type) {
            // Remove the `/commit_type` from the branch name
            formatted_branch = formatted_branch.replace(&format!("{commit_type}/"), "");
        }
    }

    formatted_branch
}
