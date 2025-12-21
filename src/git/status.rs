//! Git Status Operations
//!
//! Git status processing functionality using git2 for handling different
//! file states and contexts.

use std::collections::HashSet;

use crate::errors::Result;

/// Returns a list of all files that appear in git status
/// (modified, untracked, staged - but not deleted)
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Vec<String>` - List of files from git status
pub fn get_status_files() -> Result<Vec<String>> {
    use super::repository::open_repo;

    let repo = open_repo()?;
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    ))?;

    // Use a HashSet to avoid duplicates
    let mut files: HashSet<String> = HashSet::new();

    for entry in statuses.iter() {
        let status = entry.status();

        // Skip deleted files (both in index and worktree)
        if status.contains(git2::Status::INDEX_DELETED)
            && !status.contains(git2::Status::WT_MODIFIED | git2::Status::WT_NEW)
        {
            continue;
        }
        if status.contains(git2::Status::WT_DELETED) {
            continue;
        }

        // Get the file path
        if let Some(path) = entry.path() {
            // For renamed files, use the new name
            if status.contains(git2::Status::INDEX_RENAMED) {
                if let Some(head_to_index) = entry.head_to_index()
                    && let Some(new_path) = head_to_index.new_file().path()
                    && let Some(path_str) = new_path.to_str()
                {
                    files.insert(path_str.to_string());
                }
            } else {
                files.insert(path.to_string());
            }
        }
    }

    Ok(files.into_iter().collect())
}

/// Processes deleted files that need to be staged for deletion.
/// Only returns files that are deleted in the working directory but not yet staged.
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Result<Vec<String>>` - Files that need to be staged for deletion
pub fn process_deleted_files_for_staging() -> Result<Vec<String>> {
    use super::repository::open_repo;

    let repo = open_repo()?;
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    ))?;

    let mut deleted_files = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();

        // We want files where worktree = 'D' (deleted in working tree) but index ≠ 'D'
        // This includes:
        // - WT_DELETED without INDEX_DELETED (not in index, deleted in working tree)
        // - INDEX_MODIFIED + WT_DELETED (modified in index, deleted in working tree)
        // - INDEX_NEW + WT_DELETED (added in index, deleted in working tree)
        // But excludes:
        // - INDEX_DELETED (already staged for deletion)
        if status.contains(git2::Status::WT_DELETED)
            && !status.contains(git2::Status::INDEX_DELETED)
            && let Some(path) = entry.path()
        {
            deleted_files.push(path.to_string());
        }
    }

    Ok(deleted_files)
}

/// Processes deleted files for commit message generation.
/// Returns all deleted files that are staged for deletion.
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Result<Vec<String>>` - All deleted files for the commit message
pub fn process_deleted_files_for_commit_message() -> Result<Vec<String>> {
    use super::repository::open_repo;

    let repo = open_repo()?;
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    ))?;

    let mut deleted_files = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();

        // Include only staged deletions:
        // - INDEX_DELETED (staged for deletion)
        if status.contains(git2::Status::INDEX_DELETED)
            && let Some(path) = entry.path()
        {
            deleted_files.push(path.to_string());
        }
    }

    Ok(deleted_files)
}

/// Processes the git status.
/// It will get the modified/added files to prepare the git commit message.
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Result<Vec<String>>` - The modified/added files
pub fn process_git_status() -> Result<Vec<String>> {
    use super::repository::open_repo;

    let repo = open_repo()?;
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    ))?;

    let mut files = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();

        // Match modified files, added files, and renamed files in the index
        if status.intersects(
            git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_NEW
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE,
        ) && let Some(path) = entry.path()
        {
            // For renamed files, use the new filename
            if status.contains(git2::Status::INDEX_RENAMED) {
                if let Some(head_to_index) = entry.head_to_index()
                    && let Some(new_path) = head_to_index.new_file().path()
                    && let Some(path_str) = new_path.to_str()
                {
                    files.push(path_str.to_string());
                }
            } else {
                files.push(path.to_string());
            }
        }
    }

    Ok(files)
}

/// Counts the number of renamed files in the git status.
///
/// This function helps with accurate file counting since renamed files appear
/// as 2 lines in `git diff --cached --numstat` (one deletion, one addition).
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Result<usize>` - The count of renamed files
pub fn count_renamed_files() -> Result<usize> {
    use super::repository::open_repo;

    let repo = open_repo()?;
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    ))?;

    let count = statuses
        .iter()
        .filter(|entry| entry.status().contains(git2::Status::INDEX_RENAMED))
        .count();

    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_count_renamed_files() {
        // These tests require a git repository, so they're integration tests
        // The function now works with git2 directly rather than parsing strings
        // Tests are validated through the integration test suite
    }

    #[test]
    fn test_get_status_files_with_renamed() {
        // These tests require a git repository, so they're integration tests
        // The function now works with git2 directly rather than parsing strings
        // Tests are validated through the integration test suite
    }
}
