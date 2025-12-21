//! Git Remote Operations
//!
//! Remote repository operations including push functionality with dry-run support.

use std::process::Command;

use crate::errors::Result;

/// Pushes committed changes to the remote repository.
///
/// This function pushes to the remote repository with optional additional arguments.
/// It provides feedback on the operation's success or failure.
///
/// Note: Uses the git command to properly handle authentication (SSH keys, credentials, etc.)
/// rather than git2's push API which requires complex callback setup.
///
/// # Arguments
/// * `args` - Additional arguments to pass to the git push command (e.g., `--force`, `origin main`)
/// * `verbose` - Whether to print verbose output during the operation
/// * `dry_run` - If true, only show what would be pushed without actually pushing
///
/// # Errors
/// * If the git push command fails
/// * If not in a git repository
/// * If no remote repository is configured
/// * If authentication fails
///
/// # Examples
///
/// ```no_run
/// use rona::git::remote::git_push;
///
/// // Basic push
/// git_push(&vec![], false, false)?;
///
/// // Push with force
/// git_push(&vec!["--force".to_string()], true, false)?;
///
/// // Push to specific remote and branch
/// git_push(&vec!["origin".to_string(), "main".to_string()], false, false)?;
///
/// // Dry run to preview the push
/// git_push(&vec![], false, true)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn git_push(args: &[String], verbose: bool, dry_run: bool) -> Result<()> {
    if verbose {
        println!("\nPushing...");
    }

    if dry_run {
        println!("Would push to remote repository");
        if !args.is_empty() {
            println!("With args: {args:?}");
        }
        return Ok(());
    }

    // Use the git command for push to properly handle authentication
    // git2's push API requires complex callback setup for SSH keys, credentials, etc.
    let output = Command::new("git").arg("push").args(args).output()?;

    handle_output("push", &output, verbose)
}

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
// Use the shared handle_output function from the parent module
use super::handle_output;
