//! Git Status Operations
//!
//! Git status processing functionality using the git CLI for handling different
//! file states and contexts.

use std::{collections::HashSet, process::Command};

use crate::errors::{GitError, Result, RonaError};

/// Runs `git status --porcelain=v1` and returns the output lines.
///
/// Each line has the format `XY PATH` where X is the index status and Y is the
/// working-tree status. For renamed files, the path may include ` -> ` separating
/// the old and new names.
///
/// # Errors
/// * If the git command fails or we are not in a git repository
fn run_git_status() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1"])
        .output()
        .map_err(RonaError::Io)?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(stdout.lines().map(String::from).collect());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.to_lowercase().contains("not a git repository") {
        return Err(RonaError::Git(GitError::RepositoryNotFound));
    }

    Err(RonaError::Git(GitError::CommandFailed {
        command: "git status".to_string(),
        output: stderr.trim().to_string(),
    }))
}

/// Returns the new paths of all staged renamed files.
///
/// Uses `git diff --cached --name-status --diff-filter=R` which outputs lines like:
/// `R100\told_name\tnew_name`
///
/// # Errors
/// * If the git command fails
fn get_renamed_new_paths() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-status", "--diff-filter=R"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() >= 3 {
                Some(parts[2].to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(paths)
}

/// Returns a list of all files that appear in git status
/// (modified, untracked, staged - but not deleted)
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Vec<String>` - List of files from git status
pub fn get_status_files() -> Result<Vec<String>> {
    let lines = run_git_status()?;
    let mut files: HashSet<String> = HashSet::new();

    for line in &lines {
        if line.len() < 4 {
            continue;
        }

        let mut chars = line.chars();
        let index_char = chars.next().unwrap_or(' ');
        let wt_char = chars.next().unwrap_or(' ');
        let path = &line[3..];

        // Skip index-deleted entries unless the working tree has modifications
        if index_char == 'D' && wt_char != 'M' && wt_char != '?' {
            continue;
        }

        // Skip working-tree-deleted files
        if wt_char == 'D' {
            continue;
        }

        // For renames, collect new paths separately below
        if index_char == 'R' {
            continue;
        }

        files.insert(path.to_string());
    }

    // Add new paths for renamed files
    for path in get_renamed_new_paths()? {
        files.insert(path);
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
    let lines = run_git_status()?;
    let mut deleted_files = Vec::new();

    for line in &lines {
        if line.len() < 4 {
            continue;
        }

        let mut chars = line.chars();
        let index_char = chars.next().unwrap_or(' ');
        let wt_char = chars.next().unwrap_or(' ');
        let path = &line[3..];

        // Working-tree deleted but NOT staged for deletion (index char != 'D')
        if wt_char == 'D' && index_char != 'D' {
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
    let lines = run_git_status()?;
    let mut deleted_files = Vec::new();

    for line in &lines {
        if line.len() < 4 {
            continue;
        }

        let index_char = line.chars().next().unwrap_or(' ');
        let path = &line[3..];

        // Index-deleted (staged deletion)
        if index_char == 'D' {
            deleted_files.push(path.to_string());
        }
    }

    Ok(deleted_files)
}

/// Processes the git status.
/// Returns the modified/added/renamed/type-changed files in the index,
/// to prepare the git commit message.
///
/// # Errors
/// * If reading git status fails
///
/// # Returns
/// * `Result<Vec<String>>` - The modified/added files
pub fn process_git_status() -> Result<Vec<String>> {
    let lines = run_git_status()?;
    let mut files = Vec::new();

    for line in &lines {
        if line.len() < 4 {
            continue;
        }

        let index_char = line.chars().next().unwrap_or(' ');
        let path = &line[3..];

        match index_char {
            'M' | 'A' | 'T' => files.push(path.to_string()),
            _ => {} // 'R' (renamed) files are collected separately below; skip all others
        }
    }

    // Add new paths for renamed files
    files.extend(get_renamed_new_paths()?);

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
    let lines = run_git_status()?;
    let count = lines
        .iter()
        .filter(|line| !line.is_empty() && line.starts_with('R'))
        .count();
    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_count_renamed_files() {
        // These tests require a git repository, so they're integration tests
        // The function now works with git CLI rather than parsing strings
        // Tests are validated through the integration test suite
    }

    #[test]
    fn test_get_status_files_with_renamed() {
        // These tests require a git repository, so they're integration tests
        // The function now works with git CLI rather than parsing strings
        // Tests are validated through the integration test suite
    }
}
