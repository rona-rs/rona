use std::process::Command;

use colored::Colorize;

use crate::{
    errors::{GitError, Result, RonaError, pretty_print_error},
    git::handle_output,
};

/// A stash entry that rona created and is expected to restore later.
///
/// The handle keeps the commit id of the stash entry so that [`restore_stash`] can check that
/// the entry it pops is the one rona pushed, and not something the user or another tool stacked
/// on top in the meantime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashHandle {
    commit: String,
}

impl StashHandle {
    /// Returns the commit id of the stash entry.
    #[must_use]
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// Returns the abbreviated commit id of the stash entry, for user-facing messages.
    #[must_use]
    pub fn short_commit(&self) -> &str {
        let len = self.commit.len().min(7);
        &self.commit[..len]
    }
}

/// Reports whether the working tree has changes to tracked files.
///
/// Untracked files are ignored: git carries them across a branch switch on its own, so stashing
/// them would be needless churn. Only staged and unstaged changes to tracked files can make
/// `git switch` refuse to move.
///
/// # Errors
/// * If the git status command cannot be run
/// * If the git status command fails
///
/// # Examples
///
/// ```no_run
/// use rona::git::stash::has_local_changes;
///
/// if has_local_changes()? {
///     println!("Commit or stash before switching branches");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[tracing::instrument]
pub fn has_local_changes() -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        return handle_output("status", &output).map(|()| false);
    }

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Returns the commit id of the entry on top of the stash stack, if there is one.
///
/// # Errors
/// * If the git command cannot be run
fn top_stash_commit() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "refs/stash"])
        .output()
        .map_err(RonaError::Io)?;

    if !output.status.success() {
        return Ok(None);
    }

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if commit.is_empty() {
        Ok(None)
    } else {
        Ok(Some(commit))
    }
}

/// Stashes changes to tracked files so that a branch switch can go through.
///
/// Does nothing and returns `Ok(None)` when the working tree has no tracked changes. Pass the
/// returned handle to [`restore_stash`] to put the changes back.
///
/// # Arguments
/// * `message` - The stash message, shown by `git stash list`
///
/// # Errors
/// * If the git stash command cannot be run
/// * If the git stash command fails
///
/// # Examples
///
/// ```no_run
/// use rona::git::stash::{restore_stash, stash_local_changes};
///
/// let stash = stash_local_changes("rona sync: auto-stash from feature")?;
/// // ... switch branches, pull, merge ...
/// if let Some(stash) = stash {
///     restore_stash(&stash)?;
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[tracing::instrument]
pub fn stash_local_changes(message: &str) -> Result<Option<StashHandle>> {
    if !has_local_changes()? {
        tracing::debug!("Working tree is clean, nothing to stash");

        return Ok(None);
    }

    tracing::debug!("Stashing local changes");

    let output = Command::new("git")
        .args(["stash", "push", "--message", message])
        .output()
        .map_err(RonaError::Io)?;

    handle_output("stash", &output)?;

    Ok(top_stash_commit()?.map(|commit| StashHandle { commit }))
}

/// Restores a stash entry created by [`stash_local_changes`].
///
/// The staged and unstaged split is restored as well, through `git stash pop --index`.
///
/// # Arguments
/// * `stash` - The handle returned by [`stash_local_changes`]
///
/// # Errors
/// * If the entry is no longer on top of the stash stack
/// * If the git stash pop command cannot be run
/// * If the changes conflict with the branch content, in which case git keeps the stash entry
#[tracing::instrument]
pub fn restore_stash(stash: &StashHandle) -> Result<()> {
    if top_stash_commit()?.as_deref() != Some(stash.commit()) {
        return Err(RonaError::Git(GitError::CommandFailed {
            command: "stash pop".to_string(),
            output: format!(
                "the stash entry rona created ({}) is no longer on top of the stash stack - \
                 find it with 'git stash list' and restore it yourself",
                stash.short_commit()
            ),
        }));
    }

    tracing::debug!("Restoring stashed changes");

    let output = Command::new("git")
        .args(["stash", "pop", "--index"])
        .output()
        .map_err(RonaError::Io)?;

    if output.status.success() {
        return handle_output("stash pop", &output);
    }

    // A failed pop still applies what it can, most often with conflict markers, and git keeps the
    // stash entry. Say so, because the git message on its own ("Index was not unstashed") does not.
    let details = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    println!(
        "\n{}",
        "Restoring your stashed changes failed:".red().bold()
    );
    pretty_print_error(&details);
    println!(
        "{}",
        format!(
            "Your changes are still in the stash as {}. Check 'git status', resolve any conflict, \n\
             then remove the entry with 'git stash drop'.",
            stash.short_commit()
        )
        .yellow()
    );

    Err(RonaError::Git(GitError::CommandFailed {
        command: "stash pop".to_string(),
        output: details.trim().to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_commit_truncates_to_seven_characters() {
        let stash = StashHandle {
            commit: "0123456789abcdef".to_string(),
        };

        assert_eq!(stash.short_commit(), "0123456");
        assert_eq!(stash.commit(), "0123456789abcdef");
    }

    #[test]
    fn short_commit_handles_shorter_ids() {
        let stash = StashHandle {
            commit: "abc".to_string(),
        };

        assert_eq!(stash.short_commit(), "abc");
    }
}
