//! Git Operations Module
//!
//! This module provides organized Git-related functionality for the Rona CLI tool.
//! It's organized into focused submodules for better maintainability and clear separation of concerns.
//!
//! ## Submodules
//!
//! - [`repository`] - Core repository operations (finding git root, top level path)
//! - [`branch`] - Branch operations (current branch, branch name formatting)
//! - [`commit`] - Commit operations (commit counting, committing, commit message generation)
//! - [`status`] - Git status parsing and processing
//! - [`staging`] - File staging operations with pattern exclusion
//! - [`remote`] - Remote operations (git push)
//! - [`files`] - File and exclusion handling utilities

use crate::errors::{GitError, Result, RonaError};
use regex::Regex;
use std::process::Output;

pub mod branch;
pub mod commit;
pub mod files;
pub mod remote;
pub mod repository;
pub mod staging;
pub mod status;

// Re-export commonly used functions for convenience
pub use branch::{format_branch_name, get_current_branch};
pub use commit::{
    COMMIT_MESSAGE_FILE_PATH, COMMIT_TYPES, generate_commit_message, get_current_commit_nb,
    git_commit,
};
pub use files::create_needed_files;
pub use remote::git_push;
pub use repository::{find_git_root, get_top_level_path, open_repo};
pub use staging::git_add_with_exclude_patterns;
pub use status::get_status_files;

/// Handles the output of git commands, providing consistent error handling and success messaging.
///
/// This function processes the output of git commands and:
/// - Prints success messages when verbose mode is enabled
/// - Displays command output if present
/// - Formats and prints error messages with suggestions when commands fail
///
/// # Arguments
/// * `method_name` - The name of the git command being executed (e.g., "commit", "push")
/// * `output` - The `Output` struct containing the command's stdout, stderr, and status
/// * `verbose` - Whether to print verbose output during the operation
///
/// # Returns
/// * `Result<()>` - `Ok(())` if the command succeeded, `Err(RonaError)` if it failed
#[doc(hidden)]
pub fn handle_output(method_name: &str, output: &Output, verbose: bool) -> Result<()> {
    use crate::errors::pretty_print_error;

    if output.status.success() {
        if verbose {
            println!("{method_name} successful!");
        }

        if !output.stdout.is_empty() {
            println!("{}", String::from_utf8_lossy(&output.stdout).trim());
        }

        Ok(())
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr);

        println!("\n🚨 Git {method_name} failed:");
        pretty_print_error(&error_message);

        Err(RonaError::Io(std::io::Error::other(format!(
            "Git {method_name} failed"
        ))))
    }
}

/// Extracts filenames from git status output using regex patterns.
///
/// This function compiles a regex pattern and extracts matching filenames from
/// the provided message. It handles renamed files by preferring the new filename
/// when multiple capture groups are available.
///
/// # Arguments
/// * `message` - The git status output message to parse
/// * `pattern` - The regex pattern to match filenames
///
/// # Returns
/// * `Result<Vec<String>>` - The extracted filenames or an error message
///
/// # Errors
/// * If the regex pattern fails to compile
#[doc(hidden)]
pub fn extract_filenames(message: &str, pattern: &str) -> Result<Vec<String>> {
    let regex = Regex::new(pattern).map_err(|e| {
        RonaError::Git(GitError::InvalidStatus {
            output: format!("Failed to compile regex pattern: {e}"),
        })
    })?;

    let mut result = Vec::new();
    for line in message.lines() {
        if regex.is_match(line)
            && let Some(captures) = regex.captures(line)
        {
            // If we have a second capture group (renamed file), use that
            // Otherwise use the first capture group
            if let Some(new_name) = captures.get(2) {
                result.push(new_name.as_str().to_string());
            } else if let Some(file_name) = captures.get(1) {
                result.push(file_name.as_str().to_string());
            }
        }
    }

    Ok(result)
}
