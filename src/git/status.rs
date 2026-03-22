//! Git Status Operations
//!
//! Git status processing functionality using the git CLI for handling different
//! file states and contexts.

use std::{collections::HashSet, process::Command};

use crate::errors::{GitError, Result, RonaError};

/// Unquotes a git path.
///
/// When a path contains special characters (spaces, non-ASCII bytes, etc.),
/// git wraps it in double quotes and uses C-style escape sequences. This
/// function strips the surrounding quotes and unescapes the content.
fn unquote_git_path(path: &str) -> String {
    if path.starts_with('"') && path.ends_with('"') && path.len() >= 2 {
        let inner = &path[1..path.len() - 1];
        let mut result = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                result.push(ch);
                continue;
            }
            match chars.next() {
                Some('\\') | None => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some(c @ '0'..='7') => {
                    // Octal escape: up to 3 digits
                    let mut octal = String::from(c);
                    for _ in 0..2 {
                        match chars.peek() {
                            Some(&d) if d.is_ascii_digit() && d <= '7' => {
                                octal.push(d);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    if let Ok(byte) = u8::from_str_radix(&octal, 8) {
                        result.push(byte as char);
                    }
                }
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
            }
        }
        return result;
    }
    path.to_string()
}

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
        let path = unquote_git_path(&line[3..]);

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

        files.insert(path);
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
        let path = unquote_git_path(&line[3..]);

        // Working-tree deleted but NOT staged for deletion (index char != 'D')
        if wt_char == 'D' && index_char != 'D' {
            deleted_files.push(path);
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
        let path = unquote_git_path(&line[3..]);

        // Index-deleted (staged deletion)
        if index_char == 'D' {
            deleted_files.push(path);
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
        let path = unquote_git_path(&line[3..]);

        match index_char {
            'M' | 'A' | 'T' => files.push(path),
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
    use super::unquote_git_path;

    #[test]
    fn test_unquote_plain_path() {
        assert_eq!(unquote_git_path("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn test_unquote_quoted_path_with_spaces() {
        assert_eq!(
            unquote_git_path("\"assets/foo bar/file.txt\""),
            "assets/foo bar/file.txt"
        );
    }

    #[test]
    fn test_unquote_escape_sequences() {
        assert_eq!(unquote_git_path("\"a\\\\b\""), "a\\b");
        assert_eq!(unquote_git_path("\"a\\\"b\""), "a\"b");
        assert_eq!(unquote_git_path("\"a\\nb\""), "a\nb");
    }

    #[test]
    fn test_unquote_octal_escape() {
        // Space is octal 040
        assert_eq!(unquote_git_path("\"a\\040b\""), "a b");
    }
}
