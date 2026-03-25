//! Git Staging Operations
//!
//! File staging functionality with pattern exclusion and dry-run capabilities.

use std::{io::IsTerminal, process::Command, time::Duration};

use glob::Pattern;
use indicatif::{ProgressBar, ProgressDrawTarget};

use crate::errors::{GitError, Result, RonaError};

use super::{
    repository::get_top_level_path,
    status::{
        count_renamed_files, get_all_staged_file_paths, get_status_files,
        process_deleted_files_for_staging,
    },
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

    if dry_run {
        let deleted_files = process_deleted_files_for_staging()?;
        let all_files = get_status_files()?;
        let total_len = all_files.len() + deleted_files.len();

        let files_to_add: Vec<String> = all_files
            .into_iter()
            .filter(|f| {
                !exclude_patterns
                    .iter()
                    .any(|p| pattern_matches_file(p, f, current_dir_rel_to_repo.as_deref()))
            })
            .collect();
        let deleted_to_stage: Vec<String> = deleted_files
            .into_iter()
            .filter(|f| {
                !exclude_patterns
                    .iter()
                    .any(|p| pattern_matches_file(p, f, current_dir_rel_to_repo.as_deref()))
            })
            .collect();

        let excluded_count = total_len - files_to_add.len() - deleted_to_stage.len();
        print_dry_run_summary(&files_to_add, &deleted_to_stage, excluded_count);
        return Ok(());
    }

    let show_progress = std::io::stderr().is_terminal() && !verbose;
    let pb = if show_progress {
        let bar = ProgressBar::new_spinner();
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_message("Staging files...");
        bar.enable_steady_tick(Duration::from_millis(80));
        Some(bar)
    } else {
        None
    };

    // Stage everything at once
    let output = Command::new("git")
        .current_dir(&repo_root)
        .args(["add", "-A"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        if let Some(bar) = &pb {
            bar.finish_and_clear();
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RonaError::Git(GitError::CommandFailed {
            command: "git add -A".to_string(),
            output: stderr.trim().to_string(),
        }));
    }

    // Unstage files matching exclude patterns
    let staged_files = get_all_staged_file_paths()?;
    let total_staged = staged_files.len();

    let files_to_unstage: Vec<String> = staged_files
        .into_iter()
        .filter(|f| {
            exclude_patterns
                .iter()
                .any(|p| pattern_matches_file(p, f, current_dir_rel_to_repo.as_deref()))
        })
        .collect();

    if !files_to_unstage.is_empty() {
        let output = Command::new("git")
            .current_dir(&repo_root)
            .args(["rm", "--cached", "--"])
            .args(&files_to_unstage)
            .output()
            .map_err(RonaError::Io)?;

        if !output.status.success() {
            if let Some(bar) = &pb {
                bar.finish_and_clear();
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RonaError::Git(GitError::CommandFailed {
                command: "git rm --cached".to_string(),
                output: stderr.trim().to_string(),
            }));
        }
    }

    if let Some(bar) = pb {
        bar.finish_and_clear();
    }

    let excluded_count = files_to_unstage.len();
    let staged_count = total_staged - excluded_count;
    let renamed_count = count_renamed_files()?;

    println!(
        "Added {staged_count} files, renamed {renamed_count} while excluding {excluded_count} files for commit."
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
    fn test_pattern_matches_file_full_path() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let pattern = Pattern::new("tp08-sujet/RESPONSE.md")?;
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should match with full path pattern
        assert!(pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_matches_file_relative_to_current_dir()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("RESPONSE.md")?;
        let file_path = "tp08-sujet/RESPONSE.md";
        let current_dir = Some("tp08-sujet");

        // Should match when pattern is relative to current directory
        assert!(pattern_matches_file(&pattern, file_path, current_dir));
        Ok(())
    }

    #[test]
    fn test_pattern_matches_file_filename_only()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("RESPONSE.md")?;
        let file_path = "some/nested/dir/RESPONSE.md";

        // Should match just the filename even without current_dir context
        assert!(pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_matches_file_glob_pattern()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("*/RESPONSE.md")?;
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should match with glob pattern
        assert!(pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_matches_file_double_star_glob()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("**/RESPONSE.md")?;
        let file_path = "some/deep/nested/dir/RESPONSE.md";

        // Should match with double-star glob pattern
        assert!(pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_does_not_match() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("README.md")?;
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should not match different filename
        assert!(!pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_matches_relative_path_in_subdirectory()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("src/main.java")?;
        let file_path = "tp08-sujet/src/main.java";
        let current_dir = Some("tp08-sujet");

        // Should match relative path from current directory
        assert!(pattern_matches_file(&pattern, file_path, current_dir));
        Ok(())
    }

    #[test]
    fn test_pattern_matches_nested_path_from_root()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("tp08-sujet/src/main.java")?;
        let file_path = "tp08-sujet/src/main.java";

        // Should match full path from repository root
        assert!(pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_with_extension_wildcard() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let pattern = Pattern::new("*.md")?;
        let file_path = "tp08-sujet/RESPONSE.md";

        // Should match with extension wildcard
        assert!(pattern_matches_file(&pattern, file_path, None));
        Ok(())
    }

    #[test]
    fn test_pattern_at_repo_root() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pattern = Pattern::new("README.md")?;
        let file_path = "README.md";
        let current_dir = Some(""); // At repository root

        // Should match when at repository root
        assert!(pattern_matches_file(&pattern, file_path, current_dir));
        Ok(())
    }
}
