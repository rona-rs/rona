//! Repository Operations
//!
//! Core repository-level operations for Git repositories including repository detection,
//! path resolution, and basic repository information.

use std::path::PathBuf;

use crate::errors::{GitError, Result, RonaError};

/// Opens the git repository from the current directory or any parent directory.
///
/// This function discovers the git repository by searching upward from the current
/// working directory. It's a helper function used internally by other repository operations.
///
/// # Errors
///
/// Returns an error if:
/// - Not currently in a git repository
/// - Unable to open the repository
///
/// # Returns
///
/// A `git2::Repository` object for the discovered repository
pub fn open_repo() -> Result<git2::Repository> {
    git2::Repository::discover(".").map_err(|_| RonaError::Git(GitError::RepositoryNotFound))
}

/// Finds the root directory of the git repository.
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
    let repo = open_repo()?;

    repo.path()
        .to_path_buf()
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
    let repo = open_repo()?;

    repo.workdir()
        .ok_or(RonaError::Git(GitError::RepositoryNotFound))
        .map(std::path::Path::to_path_buf)
}
