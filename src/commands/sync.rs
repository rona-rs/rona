//! `rona sync`: brings another branch (usually `main`) into the current one.

use colored::Colorize;

use crate::{
    config::Config,
    errors::Result,
    git::{
        get_current_branch, git_create_branch, git_merge, git_pull, git_rebase, git_switch,
        has_local_changes, restore_stash, stash_local_changes,
    },
};

/// Syncs the current branch with `source_branch`.
///
/// Local changes to tracked files are stashed before the first branch switch and restored once
/// the sync is over, unless `no_stash` is set. When the sync fails the stash is deliberately
/// left in place, and where to find it is printed.
///
/// # Arguments
/// * `source_branch` - The branch to sync from (e.g. `main`)
/// * `rebase` - Whether to rebase instead of merge
/// * `new_branch` - Optional name of a branch to create before syncing
/// * `no_stash` - Whether to leave local changes in place instead of stashing them
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If the source branch does not exist
/// * If any of the stash, switch, pull, merge or rebase operations fail
pub(crate) fn sync(
    source_branch: &str,
    rebase: bool,
    new_branch: Option<&str>,
    no_stash: bool,
    config: &Config,
) -> Result<()> {
    let original_branch = get_current_branch()?;
    let target_branch = new_branch.unwrap_or(&original_branch);

    if config.dry_run {
        return describe_sync(source_branch, target_branch, rebase, new_branch, no_stash);
    }

    let stash = if no_stash {
        None
    } else {
        stash_local_changes(&format!("rona sync: auto-stash from {original_branch}"))?
    };

    if stash.is_some() {
        println!("Stashed local changes");
    }

    let sync_result = new_branch
        .map_or(Ok(()), git_create_branch)
        .and_then(|()| sync_with_source(source_branch, target_branch, rebase, config));

    if let Some(stash) = stash {
        if sync_result.is_ok() {
            restore_stash(&stash)?;
            println!("Restored the stashed changes");
        } else {
            // Popping onto a half-synced tree would only add conflicts to the failure.
            println!(
                "\n{}",
                format!(
                    "Your local changes are still stashed as {}. Restore them with 'git stash pop --index'.",
                    stash.short_commit()
                )
                .yellow()
            );
        }
    }

    sync_result?;

    println!("\nSuccessfully synced '{target_branch}' with '{source_branch}'");

    Ok(())
}

/// Prints what a sync would do, without touching the repository.
///
/// # Errors
/// * If reading the working tree state fails
fn describe_sync(
    source_branch: &str,
    target_branch: &str,
    rebase: bool,
    new_branch: Option<&str>,
    no_stash: bool,
) -> Result<()> {
    let would_stash = !no_stash && has_local_changes()?;

    if would_stash {
        println!("Would stash local changes");
    }
    if let Some(branch_name) = new_branch {
        println!("Would create new branch: {branch_name}");
    }
    println!("Would switch to: {source_branch}");
    println!("Would pull latest changes");
    println!("Would switch back to: {target_branch}");
    if rebase {
        println!("Would rebase with: {source_branch}");
    } else {
        println!("Would merge with: {source_branch}");
    }
    if would_stash {
        println!("Would restore the stashed changes");
    }

    Ok(())
}

/// Pulls `source_branch` and brings it into `target_branch`, which is left checked out.
///
/// # Errors
/// * If any of the switch, pull, merge or rebase operations fail
fn sync_with_source(
    source_branch: &str,
    target_branch: &str,
    rebase: bool,
    config: &Config,
) -> Result<()> {
    git_switch(source_branch)?;
    git_pull(config.verbose)?;

    git_switch(target_branch)?;

    if rebase {
        git_rebase(source_branch, config.verbose)
    } else {
        git_merge(source_branch, config.verbose)
    }
}
