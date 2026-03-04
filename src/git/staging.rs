//! Git Staging Operations
//!
//! File staging functionality with pattern exclusion and dry-run capabilities.

use std::{io::IsTerminal, process::Command, time::Duration};

use glob::Pattern;
use indicatif::{ProgressBar, ProgressDrawTarget};

use crate::errors::{GitError, Result, RonaError};

use super::{
    repository::get_top_level_path,
    status::{count_renamed_files, get_status_files, process_deleted_files_for_staging},
};

/// Checks if a pattern matches a file path, considering both absolute (repo-relative)
/// and current-directory-relative patterns.
///
/// This function tries to match the pattern in multiple ways to provide intuitive behavior:
/// 1. Against the full path from repository root (e.g., "tp08-sujet/RESPONSE.md")
/// 2. Against the path relative to current directory (e.g., "RESPONSE.md" when in "tp08-sujet/")
/// 3. Against just the filename (for simple patterns like "RESPONSE.md")
///
/// # Arguments
/// * `pattern` - The glob pattern to match
/// * `file_path` - The file path relative to repository root
/// * `current_dir_rel_to_repo` - Current directory path relative to repo root (e.g., "tp08-sujet")
///
/// # Returns
/// `true` if the pattern matches the file in any of the supported ways, `false` otherwise
///
/// # Examples
/// ```
/// use glob::Pattern;
///
/// let pattern = Pattern::new("RESPONSE.md").unwrap();
/// let file_path = "tp08-sujet/RESPONSE.md";
/// let current_dir = Some("tp08-sujet");
///
/// // Matches because "RESPONSE.md" matches the filename
/// assert!(pattern_matches_file(&pattern, file_path, current_dir));
///
/// // Also matches with full path pattern
/// let pattern = Pattern::new("tp08-sujet/RESPONSE.md").unwrap();
/// assert!(pattern_matches_file(&pattern, file_path, None));
///
/// // Glob patterns work too
/// let pattern = Pattern::new("*/RESPONSE.md").unwrap();
/// assert!(pattern_matches_file(&pattern, file_path, None));
/// ```
fn pattern_matches_file(
    pattern: &Pattern,
    file_path: &str,
    current_dir_rel_to_repo: Option<&str>,
) -> bool {
    // Try matching against full path (root-relative or glob patterns)
    if pattern.matches(file_path) {
        return true;
    }

    // If we're in a subdirectory, also try matching against path relative to current dir
    if let Some(current_subdir) = current_dir_rel_to_repo
        && !current_subdir.is_empty()
    {
        // Remove the current directory prefix from the file path
        if let Some(relative_path) = file_path.strip_prefix(&format!("{current_subdir}/"))
            && pattern.matches(relative_path)
        {
            return true;
        }
    }

    // Also try matching just the filename (for simple patterns like "RESPONSE.md")
    if let Some(filename) = std::path::Path::new(file_path).file_name()
        && let Some(filename_str) = filename.to_str()
        && pattern.matches(filename_str)
    {
        return true;
    }

    false
}

/// Adds files to the git index.
///
/// # Errors
/// * If reading git status fails
/// * If adding files to git fails
/// * If getting git staged information fails
///
/// # Examples
/// ```no_run
/// use std::error::Error;
/// use glob::Pattern;
///
/// // Exclude all Rust source files
/// let patterns = vec![Pattern::new("*.rs").unwrap()];
/// git_add_with_exclude_patterns(&patterns, true)?;
///
/// // Exclude an entire directory
/// let patterns = vec![Pattern::new("target/**/*").unwrap()];
/// git_add_with_exclude_patterns(&patterns, false)?;
///
/// // Multiple exclusion patterns
/// let patterns = vec![
///     Pattern::new("*.log").unwrap(),
///     Pattern::new("temp/*").unwrap(),
///     Pattern::new("**/*.tmp").unwrap()
/// ];
/// git_add_with_exclude_patterns(&patterns, true)?;
///
/// // Complex wildcard pattern
/// let patterns = vec![Pattern::new("src/**/*_test.{rs,txt}").unwrap()];
/// git_add_with_exclude_patterns(&patterns, false)?;
///
/// // No exclusions (empty pattern list)
/// let patterns = vec![];
/// git_add_with_exclude_patterns(&patterns, true)?;
///
/// // Pattern with special characters
/// let patterns = vec![Pattern::new("[abc]*.rs").unwrap()];
/// git_add_with_exclude_patterns(&patterns, false)?;
///
/// // Error handling example
/// fn handle_git_add() -> Result<(), Box<dyn Error>> {
///     let patterns = vec![Pattern::new("*.rs")?];
///     git_add_with_exclude_patterns(&patterns, true)?;
///     Ok(())
/// }
/// ```
///
/// In these examples:
/// - `"*.rs"` excludes all Rust source files
/// - `"target/**/*"` excludes everything in the target directory and subdirectories
/// - Multiple patterns show how to exclude logs, temp files, and .tmp files
/// - `"src/**/*_test.{rs,txt}"` excludes test files with .rs or .txt extensions in src/
/// - Empty vector shows how to add all files without exclusions
/// - `"[abc]*.rs"` excludes Rust files starting with a, b, or c
/// - Error handling shows proper pattern creation with error propagation
///
/// # Arguments
/// * `exclude_patterns` - List of patterns to exclude
/// * `verbose` - Whether to print verbose output
/// * `dry_run` - If true, only show what would be added without actually staging files
#[tracing::instrument(skip(exclude_patterns))]
pub fn git_add_with_exclude_patterns(
    exclude_patterns: &[Pattern],
    verbose: bool,
    dry_run: bool,
) -> Result<()> {
    tracing::debug!("Adding files...");

    // Get current directory relative to repo root
    let repo_root = get_top_level_path()?;
    let current_dir_rel_to_repo = {
        use std::env;

        let current_dir = env::current_dir().map_err(RonaError::Io)?;

        // Calculate relative path from repo root to current directory
        current_dir
            .strip_prefix(&repo_root)
            .ok()
            .and_then(|p| p.to_str())
            .map(String::from)
    };

    let (deleted_files, files_to_add, staged_files_len) = {
        let deleted_files = process_deleted_files_for_staging()?;
        let staged_files = get_status_files()?;
        let staged_files_len = staged_files.len();

        let files_to_add: Vec<String> = staged_files
            .into_iter()
            .filter(|file| {
                !exclude_patterns.iter().any(|pattern| {
                    pattern_matches_file(pattern, file, current_dir_rel_to_repo.as_deref())
                })
            })
            .collect();

        (deleted_files, files_to_add, staged_files_len)
    };

    if files_to_add.is_empty() && deleted_files.is_empty() {
        println!("No files to add or delete");
        return Ok(());
    }

    let deleted_files_count = deleted_files.len();

    if dry_run {
        print_dry_run_summary(&files_to_add, &deleted_files, staged_files_len);
        return Ok(());
    }

    let total_ops = files_to_add.len() + deleted_files.len();
    let show_progress = total_ops > 15 && std::io::stderr().is_terminal() && !verbose;

    let pb = if show_progress {
        let bar = ProgressBar::new_spinner();
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_message("Staging files...");
        bar.enable_steady_tick(Duration::from_millis(80));
        Some(bar)
    } else {
        None
    };

    // Stage files to add as a single batched call
    if !files_to_add.is_empty() {
        let output = Command::new("git")
            .current_dir(&repo_root)
            .arg("add")
            .arg("--")
            .args(&files_to_add)
            .output()
            .map_err(RonaError::Io)?;

        if !output.status.success() {
            if let Some(bar) = pb {
                bar.finish_and_clear();
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RonaError::Git(GitError::CommandFailed {
                command: "git add".to_string(),
                output: stderr.trim().to_string(),
            }));
        }
    }

    // Stage deleted files as a single batched call
    if !deleted_files.is_empty() {
        let output = Command::new("git")
            .current_dir(&repo_root)
            .arg("rm")
            .arg("--cached")
            .arg("--")
            .args(&deleted_files)
            .output()
            .map_err(RonaError::Io)?;

        if !output.status.success() {
            if let Some(bar) = pb {
                bar.finish_and_clear();
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RonaError::Git(GitError::CommandFailed {
                command: "git rm".to_string(),
                output: stderr.trim().to_string(),
            }));
        }
    }

    if let Some(bar) = pb {
        bar.finish_and_clear();
    }

    // Get the new git status after staging to count renamed files
    let renamed_count = count_renamed_files()?;

    // Count the actual number of files staged
    let staged_count = files_to_add.len();
    let excluded_count = staged_files_len - files_to_add.len();

    println!(
        "Added {staged_count} files, deleted {deleted_files_count}, renamed {renamed_count} while excluding {excluded_count} files for commit."
    );

    Ok(())
}

/// Prints a detailed summary of files that would be affected by a git add operation in dry run mode.
///
/// This function provides a clear overview of:
/// - Files that would be added to the staging area
/// - Files that would be deleted
/// - Number of files that would be excluded based on patterns
///
/// The output is formatted as follows:
/// ```
/// Would add N files:
///   + file1.txt
///   + file2.rs
/// Would delete M files:
///   - deleted_file1.txt
///   - deleted_file2.rs
/// Would exclude K files
/// ```
///
/// # Arguments
/// * `files_to_add` - List of files that would be added to the staging area
/// * `deleted_files` - List of files that would be marked as deleted
/// * `staged_files_len` - Total number of files that would be staged (including excluded ones)
/// ```
fn print_dry_run_summary(
    files_to_add: &[String],
    deleted_files: &[String],
    staged_files_len: usize,
) {
    println!("Would add {} files:", files_to_add.len());
    for file in files_to_add {
        println!("  + {file}");
    }

    println!("Would delete {} files:", deleted_files.len());
    for file in deleted_files {
        println!("  - {file}");
    }

    let excluded_files_len = staged_files_len - files_to_add.len();
    println!("Would exclude {excluded_files_len} files");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matches_file_full_path() {
        let pattern = Pattern::new("tp08-sujet/RESPONSE.md").unwrap();
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should match with full path pattern
        assert!(pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_matches_file_relative_to_current_dir() {
        let pattern = Pattern::new("RESPONSE.md").unwrap();
        let file_path = "tp08-sujet/RESPONSE.md";
        let current_dir = Some("tp08-sujet");

        // Should match when pattern is relative to current directory
        assert!(pattern_matches_file(&pattern, file_path, current_dir));
    }

    #[test]
    fn test_pattern_matches_file_filename_only() {
        let pattern = Pattern::new("RESPONSE.md").unwrap();
        let file_path = "some/nested/dir/RESPONSE.md";

        // Should match just the filename even without current_dir context
        assert!(pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_matches_file_glob_pattern() {
        let pattern = Pattern::new("*/RESPONSE.md").unwrap();
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should match with glob pattern
        assert!(pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_matches_file_double_star_glob() {
        let pattern = Pattern::new("**/RESPONSE.md").unwrap();
        let file_path = "some/deep/nested/dir/RESPONSE.md";

        // Should match with double-star glob pattern
        assert!(pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_does_not_match() {
        let pattern = Pattern::new("README.md").unwrap();
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should not match different filename
        assert!(!pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_matches_relative_path_in_subdirectory() {
        let pattern = Pattern::new("src/main.java").unwrap();
        let file_path = "tp08-sujet/src/main.java";
        let current_dir = Some("tp08-sujet");

        // Should match relative path from current directory
        assert!(pattern_matches_file(&pattern, file_path, current_dir));
    }

    #[test]
    fn test_pattern_matches_nested_path_from_root() {
        let pattern = Pattern::new("tp08-sujet/src/main.java").unwrap();
        let file_path = "tp08-sujet/src/main.java";

        // Should match full path from repository root
        assert!(pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_with_extension_wildcard() {
        let pattern = Pattern::new("*.md").unwrap();
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should match with extension wildcard
        assert!(pattern_matches_file(&pattern, file_path, None));
    }

    #[test]
    fn test_pattern_at_repo_root() {
        let pattern = Pattern::new("README.md").unwrap();
        let file_path = "README.md";
        let current_dir = Some(""); // At repository root

        // Should match when at repository root
        assert!(pattern_matches_file(&pattern, file_path, current_dir));
    }
}
