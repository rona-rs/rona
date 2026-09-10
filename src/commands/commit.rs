//! `rona commit` and `rona push`: turning the prepared commit message into a commit, and
//! sending commits to the remote.

use std::fs::read_to_string;

use crate::{
    config::Config,
    errors::{GitError, Result, RonaError},
    git::{COMMIT_MESSAGE_FILE_PATH, get_top_level_path, git_commit, git_push},
    prompt,
};

/// Commits the working tree using the message prepared in `commit_message.md`.
///
/// # Arguments
/// * `args` - Additional arguments passed through to `git commit`. They belong to the commit only:
///   the push triggered by `push` runs without them, since a commit flag such as `-s` is not a
///   valid `git push` flag. Use `rona push` directly to pass extra arguments to the push.
/// * `push` - Whether to push the commit once it is created
/// * `unsigned` - Whether to skip commit signing
/// * `yes` - Whether to skip the confirmation prompt
/// * `copy` - Whether to copy the message to the clipboard instead of committing
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If the commit message file does not exist or cannot be read
/// * If the clipboard cannot be reached (with `copy`)
/// * If the commit or the push fails
#[allow(clippy::fn_params_excessive_bools)]
pub(crate) fn commit(
    args: &[String],
    push: bool,
    unsigned: bool,
    yes: bool,
    copy: bool,
    config: &Config,
) -> Result<()> {
    let commit_file_path = get_top_level_path()?.join(COMMIT_MESSAGE_FILE_PATH);

    if !commit_file_path.exists() {
        return Err(RonaError::Git(GitError::CommitMessageNotFound));
    }

    let commit_message = read_to_string(&commit_file_path)?;

    if copy {
        return copy_to_clipboard(&commit_message);
    }

    if !yes && !config.dry_run {
        let question = format!("Commit with message:\n{}", commit_message.trim());
        if !prompt::confirm(&question, true) {
            println!("Commit cancelled.");
            return Ok(());
        }
    }

    git_commit(args, unsigned, config.dry_run)?;

    if push {
        git_push(&[], config.verbose, config.dry_run)?;
    }

    Ok(())
}

/// Pushes the current branch to the remote repository.
///
/// # Arguments
/// * `args` - Additional arguments passed through to `git push`
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If the push fails
pub(crate) fn push(args: &[String], config: &Config) -> Result<()> {
    git_push(args, config.verbose, config.dry_run)
}

/// Copies the commit message to the system clipboard instead of committing.
///
/// # Errors
/// * If the clipboard cannot be opened or written to
fn copy_to_clipboard(commit_message: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| {
        RonaError::Io(std::io::Error::other(format!(
            "Failed to access clipboard: {e}"
        )))
    })?;

    clipboard.set_text(commit_message).map_err(|e| {
        RonaError::Io(std::io::Error::other(format!(
            "Failed to copy to clipboard: {e}"
        )))
    })?;

    println!("Commit message copied to clipboard");

    Ok(())
}
