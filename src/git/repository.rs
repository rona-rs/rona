//! Repository Operations
//!
//! Core repository-level operations for Git repositories including repository detection
//! and path resolution using the git CLI.

use std::{path::PathBuf, process::Command};

use crate::errors::{GitError, Result, RonaError};

/// Finds the root directory of the git repository (the `.git` directory).
///
/// This function locates the `.git` directory of the current repository.
/// It works from any subdirectory within a git repository.
///
/// # Errors
///
/// Returns an error if:
/// - Not currently in a git repository
/// - Unable to determine the git directory path
///
/// # Returns
///
/// - `Ok(PathBuf)` - Path to the `.git` directory
/// - `Err(RonaError::Git(GitError::RepositoryNotFound))` - If not in a git repository
///
/// # Examples
///
/// ```no_run
/// use rona::git::repository::find_git_root;
///
/// match find_git_root() {
///     Ok(git_dir) => println!("Git directory: {}", git_dir.display()),
///     Err(e) => eprintln!("Not in a git repository: {}", e),
/// }
/// ```
pub fn find_git_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        return Err(RonaError::Git(GitError::RepositoryNotFound));
    }

    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    PathBuf::from(&path_str)
        .canonicalize()
        .map_err(RonaError::Io)
}

/// Retrieves the top-level path of the git repository.
///
/// This function returns the root directory of the git working tree,
/// which is the directory containing the `.git` folder. This is useful
/// for operations that need to work relative to the repository root.
///
/// # Errors
///
/// Returns an error if:
/// - Not currently in a git repository
/// - Unable to determine the working directory
///
/// # Returns
///
/// The absolute path to the repository root directory
///
/// # Examples
///
/// ```no_run
/// use rona::git::repository::get_top_level_path;
/// use std::env;
///
/// let repo_root = get_top_level_path()?;
/// env::set_current_dir(&repo_root)?;
/// println!("Changed to repository root: {}", repo_root.display());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn get_top_level_path() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        return Err(RonaError::Git(GitError::RepositoryNotFound));
    }

    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path_str))
}
