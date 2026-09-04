//! Commands that move files between the working tree, the index, and nowhere.
//!
//! `rona -a` stages, `rona reset` unstages, `rona restore` discards, and `rona -l` lists what
//! git status reports (used by shell completion). The three staging commands share the same
//! shape: `--interactive` picks the files from a checklist, otherwise the files given on the
//! command line are used.

use colored::Colorize;
use glob::Pattern;

use crate::{
    config::Config,
    errors::{Result, RonaError},
    git::{
        StatusEntry, get_restorable_files, get_stageable_files, get_staged_files, get_status_files,
        git_add_files, git_add_with_exclude_patterns, git_restore_files, git_unstage_files,
    },
    prompt,
};

/// Stages every changed file except those matching the given glob patterns.
///
/// # Arguments
/// * `exclude` - Glob patterns of files to keep out of the index
/// * `interactive` - Whether to pick the files from a checklist instead
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If any glob pattern is invalid
/// * If reading git status or staging the files fails
pub(crate) fn add(exclude: &[String], interactive: bool, config: &Config) -> Result<()> {
    if interactive {
        return add_interactively(exclude, config);
    }

    let patterns: Vec<Pattern> = exclude
        .iter()
        .map(|p| {
            Pattern::new(p)
                .map_err(|e| RonaError::InvalidInput(format!("Invalid glob pattern '{p}': {e}")))
        })
        .collect::<Result<Vec<Pattern>>>()?;

    git_add_with_exclude_patterns(&patterns, config.verbose, config.dry_run)
}

/// Stages the files picked from a checklist of everything with unstaged changes (`rona -a -i`).
///
/// Exclude patterns make no sense once the files are picked by hand, so they are only used to
/// warn that they are ignored.
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If staging the selected files fails
fn add_interactively(exclude: &[String], config: &Config) -> Result<()> {
    if !exclude.is_empty() {
        println!(
            "{} Exclude patterns are ignored in interactive mode (-i).",
            "WARNING:".yellow().bold()
        );
    }

    let paths = pick_files(
        "Select files to stage",
        &get_stageable_files()?,
        "No changes to stage.",
    )?;
    if paths.is_empty() {
        return Ok(());
    }

    git_add_files(&paths, config.dry_run)
}

/// Unstages files, moving them out of the index without touching the working tree.
///
/// Interactive mode picks the files from the staged ones; otherwise the listed files are
/// unstaged, or every staged file when none are listed.
///
/// # Arguments
/// * `files` - Explicit files to unstage (ignored in interactive mode)
/// * `interactive` - Whether to pick the files from a checklist
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If unstaging the files fails
pub(crate) fn reset(files: &[String], interactive: bool, config: &Config) -> Result<()> {
    let paths = if interactive {
        let picked = pick_files(
            "Select files to unstage",
            &get_staged_files()?,
            "No staged files to unstage.",
        )?;
        if picked.is_empty() {
            return Ok(());
        }
        picked
    } else if files.is_empty() {
        // No files given: unstage everything currently staged.
        get_staged_files()?
            .into_iter()
            .map(|entry| entry.path)
            .collect()
    } else {
        files.to_vec()
    };

    git_unstage_files(&paths, config.dry_run)
}

/// Discards working-tree changes, restoring files to their staged or committed state.
///
/// This is destructive: unstaged edits to the affected files are lost, so a confirmation is
/// asked for unless `yes` or `--dry-run` is set. Running it with neither files nor
/// `--interactive` is a no-op, since discarding every change at once is rarely intended.
///
/// # Arguments
/// * `files` - Explicit files to restore (ignored in interactive mode)
/// * `interactive` - Whether to pick the files from a checklist
/// * `yes` - Whether to skip the confirmation prompt
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If restoring the files fails
pub(crate) fn restore(
    files: &[String],
    interactive: bool,
    yes: bool,
    config: &Config,
) -> Result<()> {
    let paths = if interactive {
        pick_files(
            "Select files to restore",
            &get_restorable_files()?,
            "No changes to restore.",
        )?
    } else if files.is_empty() {
        println!(
            "{} Specify files to restore or use -i/--interactive to pick them.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    } else {
        files.to_vec()
    };

    if paths.is_empty() {
        return Ok(());
    }

    // Discarding changes is irreversible: confirm unless explicitly skipped.
    if !yes && !config.dry_run {
        let message = format!(
            "Discard working-tree changes to {} file(s)? This cannot be undone.",
            paths.len()
        );
        if !prompt::confirm(&message, false) {
            println!("Restore cancelled.");
            return Ok(());
        }
    }

    git_restore_files(&paths, config.dry_run)
}

/// Prints the files reported by git status, one per line, for shell completion.
///
/// # Errors
/// * If reading git status fails
pub(crate) fn list_status() -> Result<()> {
    for file in get_status_files()? {
        println!("{file}");
    }

    Ok(())
}

/// Shows `entries` as a checklist and returns the paths the user ticked.
///
/// Returns no paths when there is nothing to show (printing `empty_message`) or when the user
/// ticked nothing, which callers treat as "nothing to do".
///
/// # Errors
/// * If the user cancels the prompt
fn pick_files(
    prompt_text: &str,
    entries: &[StatusEntry],
    empty_message: &str,
) -> Result<Vec<String>> {
    if entries.is_empty() {
        println!("{empty_message}");
        return Ok(vec![]);
    }

    let selected = prompt::multi_select(prompt_text, entries)?;
    if selected.is_empty() {
        println!("No files selected.");
        return Ok(vec![]);
    }

    Ok(selected
        .into_iter()
        .map(|index| entries[index].path.clone())
        .collect())
}
