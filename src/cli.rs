//! Command Line Interface (CLI) Module for Rona
//!
//! This module handles all command-line interface functionality for Rona, including
//! - Command parsing and execution
//! - Subcommand implementations
//! - CLI argument handling
//!
//! # Commands
//!
//! The CLI supports several commands:
//! - `add-with-exclude`: Add files to git while excluding specified patterns
//! - `commit`: Commit changes using the commit message from `commit_message.md`
//! - `config`: Create or manage local/global configuration files
//! - `generate`: Generate a new commit message file
//! - `init`: Initialize Rona configuration
//! - `list-status`: List git status files (for shell completion)
//! - `pr`: Open a pull or merge request for the current branch
//! - `push`: Push changes to remote repository
//! - `set-editor`: Configure the editor for commit messages
//!
//! # Features
//!
//! - Supports verbose mode for detailed operation logging
//! - Supports dry-run mode for previewing changes
//! - Integrates with git commands
//! - Provides shell completion capabilities
//! - Handles configuration management
//!

use clap::{
    Args, Command as ClapCommand, CommandFactory, Parser, Subcommand, ValueEnum, ValueHint,
};
use clap_complete::{Shell, generate};
use colored::Colorize;
use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect};
use glob::Pattern;
use std::{collections::HashMap, fs::read_to_string, io, path::Path, process::Command};

use crate::{
    config::{Config, find_config_sources},
    errors::{Result, RonaError},
    extra_fields::{
        BuiltInFieldConfig, ExtraField, MessagePrefetchConfig, prompt_extra_field,
        run_message_prefetch,
    },
    git::{
        COMMIT_MESSAGE_FILE_PATH, COMMIT_TYPES, Forge, RemoteInfo, add_to_git_exclude,
        create_needed_files, default_branch, detect_remote, format_branch_name,
        generate_commit_message, get_current_branch, get_current_commit_nb, get_restorable_files,
        get_stageable_files, get_staged_files, get_status_files, get_top_level_path, git_add_files,
        git_add_with_exclude_patterns, git_branch_only, git_commit, git_create_branch, git_push,
        git_restore_files, git_unstage_files, sanitize_branch_name,
    },
    pr::{
        PR_DESCRIPTION_FILE_PATH, PrBackend, PrRequest, body_file_path, branch_type_of,
        compose_description, default_title_source, find_description_templates, push_source_branch,
        resolve_backend, split_title_and_body, submit, write_body_file,
    },
    template::{
        BranchTemplateVariables, PrTemplateVariables, TemplateVariables, process_branch_template,
        process_pr_template, process_template, validate_branch_template, validate_pr_template,
        validate_template,
    },
    theme::prompt_theme,
};

/// Configuration scope for config command
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum ConfigScope {
    /// Local project configuration (.rona.toml)
    Local,
    /// Global configuration (~/.config/rona.toml)
    Global,
}

/// Subcommands for the `config` command
#[derive(Subcommand)]
pub(crate) enum ConfigSubcommand {
    /// Create or manage a local or global configuration file
    #[command(short_flag = 'c', name = "create")]
    Create {
        /// Scope of the configuration (local project or global)
        #[arg(value_enum)]
        scope: ConfigScope,

        /// Add .rona.toml to .git/info/exclude (only applies to local scope)
        #[arg(short = 'e', long, default_value_t = false)]
        exclude: bool,

        /// Show what would be created without actually creating the config file
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Show which configuration files would be used from a directory
    #[command(short_flag = 'w', name = "which", visible_alias = "find")]
    Which {
        /// Directory to check from (defaults to current directory)
        #[arg(value_name = "PATH", value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// Show the effective (merged) configuration values
        #[arg(short = 'e', long = "effective", default_value_t = false)]
        show_effective: bool,
    },
}

/// Arguments for the `pr` command.
///
/// Every field has a config counterpart, and the flag always wins. Anything left unset falls
/// back to the config file and then to a value derived from the repository.
// A command line flag struct is naturally bool-heavy; one field per switch is the clearest form.
#[allow(clippy::struct_excessive_bools)]
#[derive(Args, Debug)]
pub(crate) struct PrArgs {
    /// Branch to target (defaults to `pr_target`, then the remote's default branch)
    #[arg(short = 't', long = "target", value_name = "BRANCH")]
    pub(crate) target: Option<String>,

    /// Title of the request (overrides the document heading)
    #[arg(short = 'T', long = "title", value_name = "TITLE")]
    pub(crate) title: Option<String>,

    /// Markdown file holding the whole request (skips the editor)
    #[arg(short = 'b', long = "body-file", value_name = "PATH", value_hint = ValueHint::FilePath)]
    pub(crate) body_file: Option<String>,

    /// Open the request as a draft
    #[arg(short = 'd', long = "draft", default_value_t = false)]
    pub(crate) draft: bool,

    /// Label to apply (repeat for several)
    #[arg(short = 'l', long = "label", value_name = "LABEL")]
    pub(crate) labels: Vec<String>,

    /// Reviewer to request (repeat for several)
    #[arg(short = 'r', long = "reviewer", value_name = "USER")]
    pub(crate) reviewers: Vec<String>,

    /// Assignee to set (repeat for several)
    #[arg(short = 'A', long = "assignee", value_name = "USER")]
    pub(crate) assignees: Vec<String>,

    /// Backend used to open the request
    #[arg(
        long = "backend",
        value_name = "BACKEND",
        value_parser = ["auto", "gh", "glab", "push-options", "browser"]
    )]
    pub(crate) backend: Option<String>,

    /// Remote to open the request against (defaults to `pr_remote`, then `origin`)
    #[arg(long = "remote", value_name = "NAME")]
    pub(crate) remote: Option<String>,

    /// Open the pre-filled web form instead of using a CLI backend
    #[arg(short = 'w', long = "web", default_value_t = false)]
    pub(crate) web: bool,

    /// Use the request document as it is instead of opening the editor
    #[arg(long = "no-edit", default_value_t = false)]
    pub(crate) no_edit: bool,

    /// Do not push the source branch before opening the request
    #[arg(long = "no-push", default_value_t = false)]
    pub(crate) no_push: bool,

    /// Skip the confirmation prompt
    #[arg(short = 'y', long = "yes", default_value_t = false)]
    pub(crate) yes: bool,

    /// Show what would be opened without opening anything
    #[arg(long, default_value_t = false)]
    pub(crate) dry_run: bool,
}

/// CLI's commands
#[derive(Subcommand)]
pub(crate) enum CliCommand {
    /// Create a new branch interactively using a branch name template.
    #[command(name = "branch")]
    Branch {
        /// Show what would be created without actually creating the branch
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Create the branch without switching to it
        #[arg(long = "no-switch", default_value_t = false)]
        no_switch: bool,
    },

    /// Add all files to the `git add` command and exclude the patterns passed as positional arguments.
    #[command(short_flag = 'a', name = "add-with-exclude")]
    AddWithExclude {
        /// Patterns of files to exclude (supports glob patterns like `"node_modules/*"`)
        #[arg(value_name = "PATTERNS", value_hint = ValueHint::AnyPath)]
        to_exclude: Vec<String>,

        /// Interactively pick which changed files to stage (`MultiSelect` of git status)
        #[arg(short = 'i', long = "interactive", default_value_t = false)]
        interactive: bool,

        /// Show what would be added without actually adding files
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Directly commit the file with the text in `commit_message.md`.
    #[command(short_flag = 'c')]
    Commit {
        /// Whether to push the commit after committing
        #[arg(short = 'p', long = "push", default_value_t = false)]
        push: bool,

        /// Show what would be committed without actually committing
        #[arg(short = 'd', long, default_value_t = false)]
        dry_run: bool,

        /// Create unsigned commit (default is to auto-detect GPG availability and sign if possible)
        #[arg(short = 'u', long = "unsigned", default_value_t = false)]
        unsigned: bool,

        /// Skip confirmation prompt and commit directly
        #[arg(short = 'y', long = "yes", default_value_t = false)]
        yes: bool,

        /// Copy commit message to clipboard instead of committing
        #[arg(long = "copy", default_value_t = false)]
        copy: bool,

        /// Additional arguments to pass to the commit command
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Generate shell completions for your shell
    #[command(name = "completion")]
    Completion {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Manage configuration files (create or inspect)
    #[command(name = "config")]
    Config {
        #[command(subcommand)]
        subcommand: ConfigSubcommand,
    },

    /// Directly generate the `commit_message.md` file.
    #[command(short_flag = 'g')]
    Generate {
        /// Show what would be generated without creating files
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Interactive mode - input the commit message directly in the terminal
        #[arg(short = 'i', long = "interactive", default_value_t = false)]
        interactive: bool,

        /// No commit number
        #[arg(short = 'n', long = "no-commit-number", default_value_t = false)]
        no_commit_number: bool,
    },

    /// Initialize the rona configuration file.
    #[command(short_flag = 'i', name = "init")]
    Initialize {
        /// Editor to use for the commit message.
        #[arg(default_value_t = String::from("nano"))]
        editor: String,

        /// Show what would be initialized without creating files
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// List files from git status (for shell completion on the -a)
    #[command(short_flag = 'l')]
    ListStatus,

    /// Open a pull request (or merge request) for the current branch.
    #[command(name = "pr", visible_aliases = ["mr", "pull-request"])]
    Pr(PrArgs),

    /// Push to a git repository.
    #[command(short_flag = 'p')]
    Push {
        /// Show what would be pushed without actually pushing
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Additional arguments to pass to the push command
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Unstage files, moving them out of the staging area without losing changes.
    #[command(name = "reset")]
    Reset {
        /// Specific files to unstage (relative to the repo root). Unstages all staged files when omitted.
        #[arg(value_name = "FILES", value_hint = ValueHint::AnyPath)]
        files: Vec<String>,

        /// Interactively pick which staged files to unstage (`MultiSelect` of staged files)
        #[arg(short = 'i', long = "interactive", default_value_t = false)]
        interactive: bool,

        /// Show what would be unstaged without actually unstaging files
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Discard working-tree changes, restoring files to their staged or committed state.
    #[command(name = "restore")]
    Restore {
        /// Specific files to restore (relative to the repo root). Required unless `--interactive` is used.
        #[arg(value_name = "FILES", value_hint = ValueHint::AnyPath)]
        files: Vec<String>,

        /// Interactively pick which modified files to discard (`MultiSelect` of changed files)
        #[arg(short = 'i', long = "interactive", default_value_t = false)]
        interactive: bool,

        /// Skip the confirmation prompt before discarding changes
        #[arg(short = 'y', long = "yes", default_value_t = false)]
        yes: bool,

        /// Show what would be restored without actually discarding changes
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Set the editor to use for editing the commit message.
    #[command(short_flag = 's', name = "set-editor")]
    Set {
        /// The editor to use for the commit message
        #[arg(value_name = "EDITOR")]
        editor: String,

        /// Show what would be changed without modifying config
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Sync current branch with main (or another branch) by pulling and merging/rebasing.
    #[command(name = "sync")]
    Sync {
        /// Branch to sync from (default: main)
        #[arg(short = 'b', long = "branch", default_value = "main")]
        source_branch: String,

        /// Use rebase instead of merge
        #[arg(short = 'r', long = "rebase", default_value_t = false)]
        rebase: bool,

        /// Create a new branch before syncing
        #[arg(short = 'n', long = "new-branch")]
        new_branch: Option<String>,

        /// Keep local changes in place instead of stashing them during the sync
        #[arg(long = "no-stash", default_value_t = false)]
        no_stash: bool,

        /// Show what would be done without actually doing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

#[derive(Parser)]
#[command(about = "Simple program that can:\n\
\t- Commit with the current 'commit_message.md' file text.\n\
\t- Generate the 'commit_message.md' file.\n\
\t- Push to git repository.\n\
\t- Add files with pattern exclusion.\n\
\t- Open a pull or merge request for the current branch.\n\
\nAll commands support --dry-run to preview changes.")]
#[command(author = "Tom Planche <tomplanche@proton.me>")]
#[command(help_template = "{about}\nMade by: {author}\n\nUSAGE:\n{usage}\n\n{all-args}\n")]
#[command(name = "rona")]
#[command(version)]
pub(crate) struct Cli {
    /// Commands
    #[command(subcommand)]
    pub(crate) command: CliCommand,

    /// Verbose output - show detailed information about operations
    #[arg(short, long, default_value = "false")]
    verbose: bool,

    /// Config file to use instead of the default global/project hierarchy
    #[arg(short = 'f', long = "config-file", value_name = "PATH", value_hint = ValueHint::FilePath, global = true)]
    config: Option<String>,
}

/// Build the CLI command structure for generating completions
#[doc(hidden)]
fn build_cli() -> ClapCommand {
    Cli::command()
}

/// Print custom fish shell completions that enhance the auto-generated ones
#[doc(hidden)]
fn print_fish_custom_completions() {
    println!();
    println!("# === CUSTOM RONA COMPLETIONS ===");
    println!("# Helper function to get git status files");
    println!("function __rona_status_files");
    println!("    rona -l");
    println!("end");
    println!();
    println!("# Command-specific completions");
    println!("# add-with-exclude: Complete with git status files");
    println!(
        "complete -c rona -n '__fish_seen_subcommand_from add-with-exclude -a' -xa '(__rona_status_files)'"
    );
    println!("# reset / restore: Complete with git status files");
    println!("complete -c rona -n '__fish_seen_subcommand_from reset' -xa '(__rona_status_files)'");
    println!(
        "complete -c rona -n '__fish_seen_subcommand_from restore' -xa '(__rona_status_files)'"
    );
}

/// Prompt for branch description and any configured branch extra fields in the configured order.
///
/// The reserved name `"description"` positions the built-in description prompt. Extra fields not
/// listed in `field_order` are appended after all listed items.
///
/// # Errors
/// Returns an error if any prompt is cancelled or a validation regex is invalid.
fn prompt_branch_fields(
    extra_fields: &[ExtraField],
    field_order: &[String],
    needs_description: bool,
    description_config: Option<&BuiltInFieldConfig>,
) -> Result<(String, HashMap<String, String>)> {
    const DESCRIPTION_KEY: &str = "description";

    let description_disabled = description_config.is_some_and(|c| c.disabled);
    let effective_needs_description = needs_description && !description_disabled;

    let ordered = ordered_field_names(
        extra_fields,
        field_order,
        DESCRIPTION_KEY,
        effective_needs_description,
    );

    let mut description: Option<String> = None;
    let mut extra_values: HashMap<String, String> = HashMap::new();

    for name in &ordered {
        if name == DESCRIPTION_KEY {
            let prompt_text = description_config
                .and_then(|c| c.prompt.as_deref())
                .unwrap_or("Branch description");
            description = Some(prompt_text_field(
                prompt_text,
                description_config.and_then(|c| c.validation.as_deref()),
                None,
            )?);
        } else if let Some(field) = extra_fields.iter().find(|f| f.name == *name)
            && let Some(value) = prompt_extra_field(field)?
        {
            extra_values.insert(field.name.clone(), value);
        }
    }

    Ok((description.unwrap_or_default(), extra_values))
}

/// Returns the effective list of types shown in the `rona branch` type selector.
fn branch_effective_types(config: &Config) -> Vec<String> {
    let commit: Vec<String> = config.project_config.commit_types.as_ref().map_or_else(
        || COMMIT_TYPES.iter().map(|s| (*s).to_string()).collect(),
        Clone::clone,
    );
    match &config.project_config.branch_types {
        None => commit,
        Some(branch) => {
            if config.project_config.merge_branch_and_commit_types {
                let mut merged = branch.clone();
                for ct in &commit {
                    if !merged.contains(ct) {
                        merged.push(ct.clone());
                    }
                }
                merged
            } else {
                branch.clone()
            }
        }
    }
}

/// Handle the `Branch` command which creates a new branch from a template.
///
/// # Errors
/// * If branch creation fails
/// * If user cancels a prompt
#[allow(clippy::literal_string_with_formatting_args)]
fn handle_branch(no_switch: bool, config: &Config) -> Result<()> {
    let effective_types = branch_effective_types(config);
    let types_for_branch: Vec<&str> = effective_types.iter().map(String::as_str).collect();

    let default_template = "{branch_type}/{description}";
    let template = config
        .project_config
        .branch_template
        .as_deref()
        .unwrap_or(default_template);

    // Determine which built-in variables the template actually uses.
    let needs_branch_type =
        template.contains("{branch_type}") || template.contains("{?branch_type}");
    let needs_description =
        template.contains("{description}") || template.contains("{?description}");

    // A field is "referenced" when {name} or {?name} appears anywhere in the template.
    let is_referenced = |name: &str| {
        template.contains(&format!("{{{name}}}")) || template.contains(&format!("{{?{name}}}"))
    };

    // Build the effective field list for branch prompts. Only fields referenced in the template
    // are prompted: fields inherited from an extended config (or otherwise configured) but unused
    // by this template are skipped rather than prompted for a value that would be discarded.
    let mut effective_branch_fields: Vec<ExtraField> = Vec::new();
    for field in &config.project_config.branch_extra_fields {
        if is_referenced(&field.name) {
            effective_branch_fields.push(field.clone());
        } else {
            println!(
                "[NOTE] Branch extra field '{}' is not referenced in the template; skipping.",
                field.name
            );
        }
    }
    // Pull in any commit_extra_fields whose names are referenced in the template but not already
    // covered by a branch_extra_field with the same name.
    for commit_field in &config.project_config.commit_extra_fields {
        let already_present = effective_branch_fields
            .iter()
            .any(|f| f.name == commit_field.name);
        if is_referenced(&commit_field.name) && !already_present {
            effective_branch_fields.push(commit_field.clone());
        }
    }

    // Validate template before prompting the user.
    let extra_names: Vec<&str> = effective_branch_fields
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    if let Err(e) = validate_branch_template(template, &extra_names) {
        return Err(RonaError::InvalidInput(format!(
            "Branch template validation error: {e}"
        )));
    }

    let branch_type = if needs_branch_type {
        let index = FuzzySelect::with_theme(&prompt_theme())
            .with_prompt("Select branch type")
            .items(&types_for_branch)
            .default(0)
            .interact_opt()
            .map_err(|_| RonaError::UserCancelled)?
            .ok_or(RonaError::UserCancelled)?;
        types_for_branch[index].to_string()
    } else {
        String::new()
    };

    let (description, extra_values) = prompt_branch_fields(
        &effective_branch_fields,
        &config.project_config.branch_field_order,
        needs_description,
        config.project_config.branch_description.as_ref(),
    )?;

    if needs_description && description.trim().is_empty() {
        println!(
            "{} Empty description provided. Exiting.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    }

    let variables = BranchTemplateVariables::new(branch_type, description.trim().to_owned())?;

    let raw_name = process_branch_template(template, &variables, &extra_values)?;
    let branch_name = sanitize_branch_name(&raw_name);

    if branch_name.is_empty() {
        return Err(RonaError::InvalidInput(
            "Generated branch name is empty after sanitization.".to_string(),
        ));
    }

    if config.dry_run {
        println!("Would create branch: {branch_name}");
        if no_switch {
            println!("Would not switch to the new branch.");
        } else {
            println!("Would switch to the new branch.");
        }
        return Ok(());
    }

    if no_switch {
        git_branch_only(&branch_name)?;
        println!("Branch created: {branch_name}");
    } else {
        git_create_branch(&branch_name)?;
        println!("Switched to new branch: {branch_name}");
    }

    Ok(())
}

/// Handle the `AddWithExclude` command which adds files to git while excluding specified patterns.
///
/// # Arguments
/// * `exclude` - List of glob patterns for files to exclude from git add
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If any glob pattern is invalid
/// * If git add operation fails
/// * If reading git status fails
fn handle_add_with_exclude(exclude: &[String], interactive: bool, config: &Config) -> Result<()> {
    if interactive {
        return handle_add_interactive(exclude, config);
    }

    let patterns: Vec<Pattern> = exclude
        .iter()
        .map(|p| {
            Pattern::new(p)
                .map_err(|e| RonaError::InvalidInput(format!("Invalid glob pattern '{p}': {e}")))
        })
        .collect::<Result<Vec<Pattern>>>()?;

    git_add_with_exclude_patterns(&patterns, config.verbose, config.dry_run)?;
    Ok(())
}

/// Handle the interactive variant of the add command (`rona -a -i`).
///
/// Presents a `MultiSelect` of every file with unstaged changes and stages only
/// the ones the user selects. Exclude patterns are not used in this mode.
///
/// # Arguments
/// * `exclude` - Patterns passed on the command line (ignored, only used to warn)
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If staging the selected files fails
fn handle_add_interactive(exclude: &[String], config: &Config) -> Result<()> {
    if !exclude.is_empty() {
        println!(
            "{} Exclude patterns are ignored in interactive mode (-i).",
            "WARNING:".yellow().bold()
        );
    }

    let entries = get_stageable_files()?;
    if entries.is_empty() {
        println!("No changes to stage.");
        return Ok(());
    }

    let selected = MultiSelect::with_theme(&prompt_theme())
        .with_prompt("Select files to stage")
        .items(&entries)
        .interact_opt()
        .map_err(|_| RonaError::UserCancelled)?
        .ok_or(RonaError::UserCancelled)?;

    let paths: Vec<String> = selected
        .into_iter()
        .map(|index| entries[index].path.clone())
        .collect();
    git_add_files(&paths, config.dry_run)?;
    Ok(())
}

/// Handle the Reset command (`rona reset`), unstaging files from the index.
///
/// In interactive mode (`-i`) a `MultiSelect` of staged files is shown and only
/// the selected files are unstaged. Otherwise the explicitly listed files are
/// unstaged, or every staged file when none are given. Unstaging never touches
/// the working tree, so local edits are preserved.
///
/// # Arguments
/// * `files` - Explicit files to unstage (ignored in interactive mode)
/// * `interactive` - Whether to pick files from a checklist
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If unstaging the files fails
fn handle_reset(files: &[String], interactive: bool, config: &Config) -> Result<()> {
    if interactive {
        return handle_reset_interactive(config);
    }

    if !files.is_empty() {
        return git_unstage_files(files, config.dry_run);
    }

    // No files given: unstage everything currently staged.
    let staged: Vec<String> = get_staged_files()?
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    git_unstage_files(&staged, config.dry_run)
}

/// Handle the interactive variant of the reset command (`rona reset -i`).
///
/// Presents a `MultiSelect` of every staged file and unstages only the ones the
/// user selects.
///
/// # Arguments
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If unstaging the selected files fails
fn handle_reset_interactive(config: &Config) -> Result<()> {
    let entries = get_staged_files()?;
    if entries.is_empty() {
        println!("No staged files to unstage.");
        return Ok(());
    }

    let selected = MultiSelect::with_theme(&prompt_theme())
        .with_prompt("Select files to unstage")
        .items(&entries)
        .interact_opt()
        .map_err(|_| RonaError::UserCancelled)?
        .ok_or(RonaError::UserCancelled)?;

    let paths: Vec<String> = selected
        .into_iter()
        .map(|index| entries[index].path.clone())
        .collect();
    git_unstage_files(&paths, config.dry_run)
}

/// Handle the Restore command (`rona restore`), discarding working-tree changes.
///
/// This is destructive: unstaged edits to the affected files are lost. A
/// confirmation prompt is shown before anything is discarded unless `--yes` or
/// `--dry-run` is set. In interactive mode (`-i`) the files are chosen from a
/// `MultiSelect` of changed files; otherwise the explicitly listed files are used.
/// Running it with neither files nor `-i` is a no-op, since discarding every
/// change at once is rarely intended.
///
/// # Arguments
/// * `files` - Explicit files to restore (ignored in interactive mode)
/// * `interactive` - Whether to pick files from a checklist
/// * `yes` - Whether to skip the confirmation prompt
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If reading git status fails
/// * If the user cancels the prompt
/// * If restoring the files fails
fn handle_restore(files: &[String], interactive: bool, yes: bool, config: &Config) -> Result<()> {
    let paths: Vec<String> = if interactive {
        let entries = get_restorable_files()?;
        if entries.is_empty() {
            println!("No changes to restore.");
            return Ok(());
        }

        let selected = MultiSelect::with_theme(&prompt_theme())
            .with_prompt("Select files to restore")
            .items(&entries)
            .interact_opt()
            .map_err(|_| RonaError::UserCancelled)?
            .ok_or(RonaError::UserCancelled)?;

        selected
            .into_iter()
            .map(|index| entries[index].path.clone())
            .collect()
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
        println!("No files selected.");
        return Ok(());
    }

    // Discarding changes is irreversible: confirm unless explicitly skipped.
    if !yes && !config.dry_run {
        let message = format!(
            "Discard working-tree changes to {} file(s)? This cannot be undone.",
            paths.len()
        );
        let confirmed = Confirm::with_theme(&prompt_theme())
            .with_prompt(&message)
            .default(false)
            .interact()
            .unwrap_or(false);

        if !confirmed {
            println!("Restore cancelled.");
            return Ok(());
        }
    }

    git_restore_files(&paths, config.dry_run)
}

/// Handle the Commit command which commits changes using the message from `commit_message.md`.
///
/// # Arguments
/// * `args` - Additional arguments to pass to git commit
/// * `push` - Whether to push changes after committing
/// * `unsigned` - Whether to create an unsigned commit (skips -S flag)
/// * `yes` - Whether to skip the confirmation prompt
/// * `copy` - Whether to copy the commit message to clipboard instead of committing
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If git commit operation fails
/// * If push is true and git push operation fails
/// * If commit message file doesn't exist or cannot be read
/// * If user cancels the commit confirmation
/// * If clipboard operation fails
#[allow(clippy::fn_params_excessive_bools)]
fn handle_commit(
    args: &[String],
    push: bool,
    unsigned: bool,
    yes: bool,
    copy: bool,
    config: &Config,
) -> Result<()> {
    // Read the commit message file
    let project_root = get_top_level_path()?;
    let commit_file_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    if !commit_file_path.exists() {
        return Err(crate::errors::RonaError::Git(
            crate::errors::GitError::CommitMessageNotFound,
        ));
    }

    let commit_message = read_to_string(&commit_file_path)?;

    // If copy flag is set, copy to clipboard and exit
    if copy {
        use arboard::Clipboard;
        let mut clipboard = Clipboard::new().map_err(|e| {
            crate::errors::RonaError::Io(std::io::Error::other(format!(
                "Failed to access clipboard: {e}"
            )))
        })?;

        clipboard.set_text(&commit_message).map_err(|e| {
            crate::errors::RonaError::Io(std::io::Error::other(format!(
                "Failed to copy to clipboard: {e}"
            )))
        })?;

        println!("Commit message copied to clipboard");
        return Ok(());
    }

    // Show confirmation prompt unless --yes flag is set or in dry-run mode
    if !yes && !config.dry_run {
        // Show confirmation prompt
        let confirmation_message = format!("Commit with message:\n{}", commit_message.trim());
        let confirm = Confirm::with_theme(&prompt_theme())
            .with_prompt(&confirmation_message)
            .default(true)
            .interact()
            .unwrap_or(false);

        if !confirm {
            println!("Commit cancelled.");
            return Ok(());
        }
    }

    git_commit(args, unsigned, config.dry_run)?;

    if push {
        git_push(args, config.verbose, config.dry_run)?;
    }
    Ok(())
}

/// Handle the Completion command
#[doc(hidden)]
fn handle_completion(shell: Shell) {
    let mut cmd = build_cli();
    generate(shell, &mut cmd, "rona", &mut io::stdout());

    // Add custom completions for fish shell
    if matches!(shell, Shell::Fish) {
        print_fish_custom_completions();
    }
}

/// Prompt the commit message and any configured extra fields in the order defined by
/// `field_order`.
///
/// The reserved name `"message"` positions the built-in message prompt among the extra
/// fields. Extra fields not listed in `field_order` are appended after all listed items.
/// When `field_order` is empty the default order is: extra fields first, then message.
///
/// # Errors
/// Returns an error if any prompt is cancelled or a validation regex is invalid.
fn prompt_interactive_fields(
    extra_fields: &[ExtraField],
    field_order: &[String],
    message_prefetch: Option<&MessagePrefetchConfig>,
    message_config: Option<&BuiltInFieldConfig>,
) -> Result<(String, HashMap<String, String>)> {
    const MESSAGE_KEY: &str = "message";

    let message_disabled = message_config.is_some_and(|c| c.disabled);

    let ordered: Vec<String> = if field_order.is_empty() {
        let mut v: Vec<String> = extra_fields.iter().map(|f| f.name.clone()).collect();
        if !message_disabled {
            v.push(MESSAGE_KEY.to_string());
        }
        v
    } else {
        let mut v: Vec<String> = field_order.to_vec();
        // Append extra fields not explicitly listed
        for f in extra_fields {
            if !v.iter().any(|s| s == &f.name) {
                v.push(f.name.clone());
            }
        }
        // Guarantee message is always prompted unless disabled
        if !message_disabled && !v.iter().any(|s| s == MESSAGE_KEY) {
            v.push(MESSAGE_KEY.to_string());
        }
        v
    };

    let mut message: Option<String> = None;
    let mut extra_values: HashMap<String, String> = HashMap::new();

    for name in &ordered {
        if name == MESSAGE_KEY {
            let prompt_text = message_config
                .and_then(|c| c.prompt.as_deref())
                .unwrap_or("Message");
            let default = message_prefetch
                .map(run_message_prefetch)
                .transpose()?
                .flatten();
            let validator_pattern = message_config.and_then(|c| c.validation.as_deref());
            let theme = prompt_theme();
            let value = if let Some(pattern) = validator_pattern {
                let re = regex::Regex::new(pattern).map_err(|e| {
                    RonaError::InvalidInput(format!("Invalid validation regex for message: {e}"))
                })?;
                let pattern_owned = pattern.to_string();
                let mut text_prompt = Input::<String>::with_theme(&theme)
                    .with_prompt(prompt_text)
                    .allow_empty(true);
                if let Some(ref d) = default {
                    text_prompt = text_prompt.default(d.clone());
                }
                text_prompt
                    .validate_with(move |input: &String| -> std::result::Result<(), String> {
                        if re.is_match(input) {
                            Ok(())
                        } else {
                            Err(format!("Must match pattern: {pattern_owned}"))
                        }
                    })
                    .interact_text()
                    .map_err(|_| RonaError::UserCancelled)?
            } else {
                let mut text_prompt = Input::<String>::with_theme(&theme)
                    .with_prompt(prompt_text)
                    .allow_empty(true);
                if let Some(ref d) = default {
                    text_prompt = text_prompt.default(d.clone());
                }
                text_prompt
                    .interact_text()
                    .map_err(|_| RonaError::UserCancelled)?
            };
            message = Some(value);
        } else if let Some(field) = extra_fields.iter().find(|f| f.name == *name)
            && let Some(value) = prompt_extra_field(field)?
        {
            extra_values.insert(field.name.clone(), value);
        }
    }

    let message = message
        .ok_or_else(|| RonaError::InvalidInput("message prompt was not executed".to_string()))?;

    Ok((message, extra_values))
}

/// Returns whether `template` uses `name`, either as `{name}` or as a conditional block
/// `{?name}...{/name}`.
fn template_references(template: &str, name: &str) -> bool {
    template.contains(&format!("{{{name}}}")) || template.contains(&format!("{{?{name}}}"))
}

/// Show the commit type selector and return the chosen type.
///
/// # Errors
/// * If the user cancels the prompt
fn select_commit_type(config: &Config) -> Result<&str> {
    let commit_types_vec = config.project_config.commit_types.as_ref().map_or_else(
        || COMMIT_TYPES.to_vec(),
        |v| v.iter().map(String::as_str).collect::<Vec<&str>>(),
    );

    let index = FuzzySelect::with_theme(&prompt_theme())
        .with_prompt("Select commit type")
        .items(&commit_types_vec)
        .default(0)
        .interact_opt()
        .map_err(|_| RonaError::UserCancelled)?
        .ok_or(RonaError::UserCancelled)?;

    Ok(commit_types_vec[index])
}

/// The default commit-message template used when none is configured.
///
/// The conditional block `{?commit_number}...{/commit_number}` is only included when
/// `commit_number` has a value.
const DEFAULT_COMMIT_TEMPLATE: &str =
    "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}";

/// The default pull/merge request title template used when none is configured.
///
/// The title is normally written as the request document's heading, so the default only has to
/// seed that heading. The last commit subject is the closest thing to a title the repository
/// already holds, and it needs no prompt. Configure `{title}` to be asked for one instead.
const DEFAULT_PR_TITLE_TEMPLATE: &str = "{commit_subject}";

/// Handle the Generate command which creates a new commit message file.
///
/// # Arguments
/// * `interactive` - Whether to prompt for commit message in terminal
/// * `no_commit_number` - Whether to include commit number in message
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If creating needed files fails
/// * If generating commit message fails
/// * If writing commit message fails
/// * If launching editor fails (in non-interactive mode)
fn handle_generate(interactive: bool, no_commit_number: bool, config: &Config) -> Result<()> {
    if config.dry_run {
        println!("Would create files: commit_message.md, .commitignore");
        println!("Would add files to .git/info/exclude");
        return Ok(());
    }

    create_needed_files()?;

    let commit_template = config
        .project_config
        .commit_template
        .as_deref()
        .unwrap_or(DEFAULT_COMMIT_TEMPLATE);

    // The selector is only worth showing when the chosen type ends up in the message. Both modes
    // render the same template, so the template alone decides.
    let commit_type = if template_references(commit_template, "commit_type") {
        Some(select_commit_type(config)?)
    } else {
        None
    };

    if interactive {
        // Only prompt for extra fields referenced in the commit template. Fields inherited from
        // an extended config (or otherwise configured) but unused by this template are skipped
        // rather than prompted for a value that would be discarded.
        let referenced_fields: Vec<ExtraField> = config
            .project_config
            .commit_extra_fields
            .iter()
            .filter(|f| {
                let referenced = template_references(commit_template, &f.name);
                if !referenced {
                    println!(
                        "[NOTE] Extra field '{}' is not referenced in the template; skipping.",
                        f.name
                    );
                }
                referenced
            })
            .cloned()
            .collect();

        // In interactive mode, prompt all fields (including message) in configured order
        let (message, extra_values) = prompt_interactive_fields(
            &referenced_fields,
            &config.project_config.commit_fields_order,
            config.project_config.message_prefetch.as_ref(),
            config.project_config.commit_message.as_ref(),
        )?;
        handle_interactive_mode(
            commit_type,
            no_commit_number,
            &message,
            &extra_values,
            config,
        )?;
    } else {
        // Editor mode renders the same template with an empty message, so the file opens on a
        // header that already matches the configured format and only the message is missing.
        // Extra fields are never prompted for here, so they resolve to empty as well.
        let blank_extra_values: HashMap<String, String> = config
            .project_config
            .commit_extra_fields
            .iter()
            .filter(|f| template_references(commit_template, &f.name))
            .map(|f| {
                println!(
                    "[NOTE] Editor mode leaves the extra field '{}' empty. Complete it in your editor.",
                    f.name
                );
                (f.name.clone(), String::new())
            })
            .collect();

        let header = build_commit_message(
            commit_type,
            no_commit_number,
            "",
            &blank_extra_values,
            config,
        )?;

        // In editor mode, generate the scaffold file first, then open the editor
        generate_commit_message(&header)?;
        handle_editor_mode(config)?;
    }
    Ok(())
}

/// Render the configured commit template (or [`DEFAULT_COMMIT_TEMPLATE`]) for `message`.
///
/// `commit_type` is `None` when the template does not use `{commit_type}`, in which case no type
/// was ever selected and the variable resolves to an empty string. Editor mode passes an empty
/// `message`, which renders the header the user then completes in their editor.
///
/// When the template fails validation a warning is printed and the built-in
/// `[number] (type on branch) message` layout is used instead, keeping whichever parts are known.
///
/// # Errors
/// * If the current branch, commit count, or git author cannot be read
/// * If the template cannot be processed
fn build_commit_message(
    commit_type: Option<&str>,
    no_commit_number: bool,
    message: &str,
    extra_values: &HashMap<String, String>,
    config: &Config,
) -> Result<String> {
    let branch_name = format_branch_name(&COMMIT_TYPES, &get_current_branch()?);
    let commit_number = if no_commit_number {
        None
    } else {
        Some(get_current_commit_nb()? + 1)
    };

    // Get template from config or use default with conditional syntax
    let template = config
        .project_config
        .commit_template
        .as_deref()
        .unwrap_or(DEFAULT_COMMIT_TEMPLATE);

    // Validate template (including any extra field variable names)
    let extra_names: Vec<&str> = extra_values.keys().map(String::as_str).collect();
    if let Err(e) = validate_template(template, &extra_names) {
        println!(
            "{} Template validation error: {e}",
            "WARNING:".yellow().bold()
        );
        println!("Using fallback format...");
        let number_prefix = commit_number.map_or_else(String::new, |number| format!("[{number}] "));
        let type_prefix = commit_type.map_or_else(String::new, |commit_type| {
            format!("({commit_type} on {branch_name}) ")
        });
        return Ok(format!("{number_prefix}{type_prefix}{}", message.trim()));
    }

    // Create template variables
    let variables = TemplateVariables::new(
        commit_number,
        commit_type.unwrap_or_default().to_string(),
        branch_name,
        message.trim().to_string(),
    )?;

    // Process template (extra_values are substituted alongside built-in variables)
    process_template(template, &variables, extra_values)
}

/// Handle interactive mode for generate command
fn handle_interactive_mode(
    commit_type: Option<&str>,
    no_commit_number: bool,
    message: &str,
    extra_values: &HashMap<String, String>,
    config: &Config,
) -> Result<()> {
    let project_root = get_top_level_path()?;
    let commit_file_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    if message.trim().is_empty() {
        println!(
            "{} Empty message provided. Exiting.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    }

    let formatted_message =
        build_commit_message(commit_type, no_commit_number, message, extra_values, config)?;

    // Write the formatted message to commit_message.md
    std::fs::write(&commit_file_path, &formatted_message)?;

    println!("\n{} Commit message created!", "✓".green());
    println!("Message: {formatted_message}");
    Ok(())
}

/// Opens a file in the configured editor and waits for it to close.
///
/// # Errors
/// * If the editor cannot be launched or waited on
fn open_in_editor(path: &Path, config: &Config) -> Result<()> {
    let editor = config.get_editor()?;

    Command::new(&editor)
        .arg(path)
        .spawn()
        .map_err(|e| RonaError::CommandFailed {
            command: format!("Failed to spawn editor '{editor}': {e}"),
        })?
        .wait()
        .map_err(|e| RonaError::CommandFailed {
            command: format!("Failed to wait for editor '{editor}': {e}"),
        })?;

    Ok(())
}

/// Handle editor mode for generate command
fn handle_editor_mode(config: &Config) -> Result<()> {
    let project_root = get_top_level_path()?;
    let commit_file_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    open_in_editor(&commit_file_path, config)
}

/// Handle the Initialize command which creates the initial configuration file.
///
/// # Arguments
/// * `editor` - The editor command to configure
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If creating configuration file fails
fn handle_initialize(editor: &str, config: &Config) -> Result<()> {
    if config.dry_run {
        println!("Would create config file with editor: {editor}");
        return Ok(());
    }
    config.create_config_file(editor)?;
    Ok(())
}

/// Handle the `ListStatus` command
fn handle_list_status() -> Result<()> {
    let files = get_status_files()?;
    // Print each file on a new line for fish shell completion
    for file in files {
        println!("{file}");
    }
    Ok(())
}

/// Handle the Push command which pushes changes to the remote repository.
///
/// # Arguments
/// * `args` - Additional arguments to pass to git push
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If git push operation fails
fn handle_push(args: &[String], config: &Config) -> Result<()> {
    git_push(args, config.verbose, config.dry_run)?;
    Ok(())
}

/// Handle the Set command which updates the editor in the configuration.
///
/// # Arguments
/// * `editor` - The editor command to set
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If updating configuration file fails
fn handle_set(editor: &str, config: &Config) -> Result<()> {
    if config.dry_run {
        println!("Would set editor to: {editor}");
        return Ok(());
    }
    config.set_editor(editor)?;
    Ok(())
}

/// Handle the Sync command which syncs the current branch with another branch.
///
/// Local changes to tracked files are stashed before the first branch switch and restored once
/// the sync is over, unless `no_stash` is set.
///
/// # Arguments
/// * `source_branch` - The branch to sync from (e.g., "main")
/// * `rebase` - Whether to use rebase instead of merge
/// * `new_branch` - Optional name for a new branch to create before syncing
/// * `no_stash` - Whether to leave local changes in place instead of stashing them
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If git operations fail
/// * If the source branch doesn't exist
/// * If there are uncommitted changes that would be lost
fn handle_sync(
    source_branch: &str,
    rebase: bool,
    new_branch: Option<&str>,
    no_stash: bool,
    config: &Config,
) -> Result<()> {
    use crate::git::{git_create_branch, has_local_changes, restore_stash, stash_local_changes};

    // Get current branch before any operations
    let original_branch = get_current_branch()?;
    let target_branch = new_branch.unwrap_or(&original_branch);

    if config.dry_run {
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
        return Ok(());
    }

    let stash = if no_stash {
        None
    } else {
        stash_local_changes(&format!("rona sync: auto-stash from {original_branch}"))?
    };

    if stash.is_some() {
        println!("Stashed local changes");
    }

    // Create new branch if specified
    let sync_result = new_branch
        .map_or(Ok(()), git_create_branch)
        .and_then(|()| sync_with_source(source_branch, target_branch, rebase, config));

    if let Some(stash) = stash {
        if sync_result.is_ok() {
            restore_stash(&stash)?;
            println!("Restored the stashed changes");
        } else {
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

/// Pulls `source_branch` and brings it into `target_branch`.
///
/// # Arguments
/// * `source_branch` - The branch to sync from (e.g., "main")
/// * `target_branch` - The branch to sync into, which is left checked out
/// * `rebase` - Whether to use rebase instead of merge
/// * `config` - Global configuration including verbose settings
///
/// # Errors
/// * If any of the switch, pull, merge or rebase operations fail
fn sync_with_source(
    source_branch: &str,
    target_branch: &str,
    rebase: bool,
    config: &Config,
) -> Result<()> {
    use crate::git::{git_merge, git_pull, git_rebase, git_switch};

    // Switch to source branch and pull
    git_switch(source_branch)?;
    git_pull(config.verbose)?;

    // Switch back to target branch
    git_switch(target_branch)?;

    // Merge or rebase
    if rebase {
        git_rebase(source_branch, config.verbose)
    } else {
        git_merge(source_branch, config.verbose)
    }
}

/// Handle the `WhichConfig` command which shows which config files would be used.
///
/// # Arguments
/// * `path` - Optional directory to check from (defaults to current directory)
/// * `show_effective` - Whether to also show the effective merged configuration
///
/// # Errors
/// * If the directory does not exist
/// * If the home directory cannot be determined
fn handle_which_config(path: Option<&str>, show_effective: bool) -> Result<()> {
    use std::path::Path;

    let search_path = match path {
        Some(p) => {
            let path = Path::new(p);
            if !path.exists() {
                return Err(crate::errors::RonaError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Directory not found: {p}"),
                )));
            }
            Some(path)
        }
        None => None,
    };

    let config_info = find_config_sources(search_path)?;

    println!("Searching from: {}", config_info.search_directory.display());
    println!();

    // Check if any config exists
    let active_sources: Vec<_> = config_info.sources.iter().filter(|s| s.exists).collect();

    if active_sources.is_empty() {
        println!("! No configuration files found.");
        println!();
        println!("Possible config locations (in loading order):");
        for source in &config_info.sources {
            println!(
                "  ○ [priority {}] {}",
                source.priority,
                source.path.display()
            );
            println!("    └─ {}", source.description);
        }
        println!();
        println!("Run 'rona init' or 'rona config local/global' to create a config file.");
        return Ok(());
    }

    println!("Configuration sources (in loading order, later overrides earlier):");
    println!();

    for source in &config_info.sources {
        let status = if source.exists { "✓" } else { "○" };
        let exists_text = if source.exists {
            "(active)"
        } else {
            "(not found)"
        };

        println!(
            "  {} [priority {}] {}",
            status,
            source.priority,
            source.path.display()
        );
        println!("    └─ {} {}", source.description, exists_text);
    }

    // Show which config takes precedence
    if let Some(highest) = active_sources.iter().max_by_key(|s| s.priority) {
        println!();
        println!("Effective config from: {}", highest.path.display());
    }

    // Show effective configuration values if requested
    if show_effective {
        println!();
        println!("Effective configuration values:");
        println!();

        if let Some(cfg) = &config_info.effective_config {
            if let Some(editor) = &cfg.editor {
                println!("- editor = \"{editor}\"");
            }
            if let Some(commit_types) = &cfg.commit_types {
                println!("- commit_types = {commit_types:?}");
            }
            if let Some(template) = &cfg.commit_template {
                println!("- commit_template = \"{template}\"");
            }
        } else {
            println!("  (using defaults)");
        }
    }

    Ok(())
}

/// Handle the Config command which creates or manages configuration files.
///
/// Generates a commented TOML config file content with all supported options documented.
fn generate_commented_config() -> String {
    let default_commit_types = r#"["feat", "fix", "perf", "revert", "docs", "quality", "style", "chore", "refactor", "test", "build", "ci"]"#;
    format!(
        r#"# Editor used to open commit_message.md in non-interactive mode.
editor = "nano"

# Commit types shown in the selector.
commit_types = {default_commit_types}

##########
# COMMIT #
##########

# Template applied to the final commit message.
# Built-in variables:
#   {{commit_number}}  - sequential commit count on the current branch
#   {{commit_type}}    - the type chosen in the selector
#   {{branch_name}}    - current branch (prefix stripped, e.g. feat/x -> x)
#   {{message}}        - the message entered by the user
#   {{date}}           - YYYY-MM-DD
#   {{time}}           - HH:MM:SS
#   {{author}}         - git user.name
#   {{email}}          - git user.email
# Conditional blocks: {{?var}}...{{/var}} renders only when var has a value.
# Extra variables: add with [[commit_extra_fields]].
commit_template = "{{?commit_number}}[{{commit_number}}] {{/commit_number}}({{commit_type}} on {{branch_name}}) {{message}}"

# Order of prompts in interactive mode (-i).
# Use the reserved name "message" to position the built-in message prompt.
# Fields not listed are appended after all listed items.
# commit_fields_order = ["scope", "message", "ticket"]

# Overrides for the built-in message prompt (uncomment to customise or disable).
# [commit_message]
# prompt = "Commit message"
# validation = ""
# disabled = false

# [[commit_extra_fields]]
# name = "scope"
# prompt = "Select scope"
# kind = "select"
# required = false
# prefetch.source = "command"
# prefetch.command = "git log -50 --pretty=format:%s"
# prefetch.extract_regex = "\\w+\\((?P<value>[^)]*)\\):"
# prefetch.deduplicate = true

# [[commit_extra_fields]]
# name = "ticket"
# prompt = "Ticket:"
# kind = "text"
# required = false
# validation = "^[A-Z]+-[0-9]+$"
# prefetch.source = "branch"
# prefetch.extract_regex = "[A-Z]+-[0-9]+"

##########
# BRANCH #
##########

# Template applied to the generated branch name.
# Built-in variables:
#   {{branch_type}}   - the type chosen in the selector
#   {{description}}   - the description entered by the user
#   {{date}}          - YYYY-MM-DD
#   {{time}}          - HH:MM:SS
#   {{author}}        - git user.name
# Conditional blocks: {{?var}}...{{/var}} renders only when var has a value.
# Extra variables: add with [[branch_extra_fields]].
# Commit extra fields (from [[commit_extra_fields]]) can also be referenced here.
branch_template = "{{branch_type}}/{{description}}"

# Dedicated branch types (when absent, commit_types is used).
# branch_types = ["feat", "fix", "chore"]

# When true, branch_types and commit_types are merged in the selector.
# merge_branch_and_commit_types = false

# Order of prompts for branch creation.
# Use the reserved name "description" to position the built-in description prompt.
# branch_field_order = ["description", "ticket"]

# Overrides for the built-in description prompt (uncomment to customise or disable).
# [branch_description]
# prompt = "Branch description"
# validation = ""
# disabled = false

# [[branch_extra_fields]]
# name = "description"
# prompt = "Small description in kebab-case"
# kind = "text"
# required = true
# validation = "^[a-z][a-z0-9-]+$"
"#
    )
}

/// # Arguments
/// * `scope` - Whether to create local (.rona.toml) or global (~/.config/rona.toml) config
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If creating configuration file fails
/// * If writing configuration content fails
fn handle_config_command(scope: ConfigScope, exclude: bool, config: &Config) -> Result<()> {
    use std::io::Write;

    let config_path = {
        match scope {
            ConfigScope::Local => {
                let project_root = get_top_level_path()?;
                project_root.join(".rona.toml")
            }
            ConfigScope::Global => {
                let home = dirs::home_dir().ok_or(crate::errors::ConfigError::ConfigNotFound)?;
                home.join(".config/rona.toml")
            }
        }
    };

    if config.dry_run {
        println!(
            "Would create {} configuration file at: {}",
            match scope {
                ConfigScope::Local => "local",
                ConfigScope::Global => "global",
            },
            config_path.display()
        );
        if exclude {
            match scope {
                ConfigScope::Local => println!("Would add .rona.toml to .git/info/exclude"),
                ConfigScope::Global => {
                    println!("--exclude only applies to local scope, ignoring");
                }
            }
        }
        return Ok(());
    }

    // Check if config already exists
    if config_path.exists() {
        println!(
            "Configuration file already exists at: {}",
            config_path.display()
        );
        println!("Use 'rona set-editor <editor>' to modify the editor setting.");
    } else {
        // Create parent directory if it doesn't exist (for global config)
        if let Some(parent) = config_path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        let toml_content = generate_commented_config();

        // Write the config file
        let mut file = std::fs::File::create(&config_path)?;
        file.write_all(toml_content.as_bytes())?;

        println!("Configuration file created at: {}", config_path.display());
        println!("You can now edit this file to customize your settings.");
    }

    if exclude {
        match scope {
            ConfigScope::Local => {
                add_to_git_exclude(&[".rona.toml"])?;
                println!("Added .rona.toml to .git/info/exclude");
            }
            ConfigScope::Global => {
                println!("--exclude only applies to local scope, ignoring");
            }
        }
    }

    Ok(())
}

/// Prompts for a free-text value.
///
/// `validation` is a regex the answer must match, and `default_value` is offered as the
/// pre-filled answer so that pressing Enter accepts it.
///
/// # Errors
/// * If the validation regex is invalid
/// * If the user cancels the prompt
fn prompt_text_field(
    prompt_text: &str,
    validation: Option<&str>,
    default_value: Option<&str>,
) -> Result<String> {
    let theme = prompt_theme();
    let mut input = Input::<String>::with_theme(&theme)
        .with_prompt(prompt_text)
        .allow_empty(true);

    if let Some(default) = default_value.filter(|value| !value.trim().is_empty()) {
        input = input.default(default.to_string());
    }

    if let Some(pattern) = validation {
        let regex = regex::Regex::new(pattern).map_err(|e| {
            RonaError::InvalidInput(format!("Invalid validation regex '{pattern}': {e}"))
        })?;
        let pattern_owned = pattern.to_string();
        input = input.validate_with(move |value: &String| -> std::result::Result<(), String> {
            if regex.is_match(value) {
                Ok(())
            } else {
                Err(format!("Must match pattern: {pattern_owned}"))
            }
        });
    }

    input.interact_text().map_err(|_| RonaError::UserCancelled)
}

/// Prompts for the request title and any configured PR extra fields, in the configured order.
///
/// The reserved name `"title"` positions the built-in title prompt. Extra fields not listed in
/// `field_order` are appended after all listed items.
///
/// # Errors
/// * If any prompt is cancelled or a validation regex is invalid
fn prompt_pr_fields(
    extra_fields: &[ExtraField],
    field_order: &[String],
    needs_title: bool,
    title_config: Option<&BuiltInFieldConfig>,
    title_default: &str,
) -> Result<(String, HashMap<String, String>)> {
    const TITLE_KEY: &str = "title";

    let title_disabled = title_config.is_some_and(|c| c.disabled);
    let effective_needs_title = needs_title && !title_disabled;

    let ordered = ordered_field_names(extra_fields, field_order, TITLE_KEY, effective_needs_title);

    let mut title: Option<String> = None;
    let mut extra_values: HashMap<String, String> = HashMap::new();

    for name in &ordered {
        if name == TITLE_KEY {
            let prompt_text = title_config
                .and_then(|c| c.prompt.as_deref())
                .unwrap_or("Request title");
            title = Some(prompt_text_field(
                prompt_text,
                title_config.and_then(|c| c.validation.as_deref()),
                Some(title_default),
            )?);
        } else if let Some(field) = extra_fields.iter().find(|f| f.name == *name)
            && let Some(value) = prompt_extra_field(field)?
        {
            extra_values.insert(field.name.clone(), value);
        }
    }

    Ok((title.unwrap_or_default(), extra_values))
}

/// Builds the prompt order for a template that has one built-in field.
///
/// Fields named in `field_order` come first in that order, then any configured field the order
/// does not mention, then the built-in prompt when it is not already positioned.
fn ordered_field_names(
    extra_fields: &[ExtraField],
    field_order: &[String],
    builtin_key: &str,
    needs_builtin: bool,
) -> Vec<String> {
    let mut ordered: Vec<String> = if field_order.is_empty() {
        extra_fields.iter().map(|f| f.name.clone()).collect()
    } else {
        let mut listed = field_order.to_vec();
        for field in extra_fields {
            if !listed.iter().any(|name| name == &field.name) {
                listed.push(field.name.clone());
            }
        }
        listed
    };

    if needs_builtin && !ordered.iter().any(|name| name == builtin_key) {
        ordered.push(builtin_key.to_string());
    }

    ordered
}

/// Resolves the remote a request is opened against, and the forge behind it.
///
/// A `pr_forge` config value overrides detection, which is what self-hosted instances need
/// when their hostname does not name the product.
///
/// # Errors
/// * If the remote is not configured or its URL cannot be parsed
/// * If `pr_forge` names something rona does not know
fn resolve_pr_remote(args: &PrArgs, config: &Config) -> Result<(String, RemoteInfo)> {
    let remote = args
        .remote
        .clone()
        .or_else(|| config.project_config.pr_remote.clone())
        .unwrap_or_else(|| "origin".to_string());

    let mut info = detect_remote(&remote)?;

    if let Some(name) = &config.project_config.pr_forge {
        info.forge = Forge::parse(name).ok_or_else(|| {
            RonaError::InvalidInput(format!(
                "Unknown pr_forge '{name}'. Use github, gitlab, or bitbucket."
            ))
        })?;
    }

    Ok((remote, info))
}

/// Resolves the branch a request targets.
///
/// # Errors
/// * If no target can be determined
/// * If the target is the branch the request is opened from
fn resolve_pr_target(args: &PrArgs, config: &Config, remote: &str, source: &str) -> Result<String> {
    let target = args
        .target
        .clone()
        .or_else(|| config.project_config.pr_target.clone())
        .or_else(|| default_branch(remote))
        .ok_or_else(|| {
            RonaError::InvalidInput(format!(
                "Could not determine the target branch of '{remote}'. \
                 Pass --target or set `pr_target` in your rona config."
            ))
        })?;

    if target == source {
        return Err(RonaError::InvalidInput(format!(
            "'{source}' cannot target itself. Switch to your feature branch, or pass --target."
        )));
    }

    Ok(target)
}

/// Resolves the backend used to open the request.
///
/// `--web` is a shorthand for `--backend browser` and wins over the config.
///
/// # Errors
/// * If a configured backend name is not recognised
/// * If the forge is unknown and no backend was configured
fn resolve_pr_backend(args: &PrArgs, config: &Config, info: &RemoteInfo) -> Result<PrBackend> {
    if args.web {
        return Ok(PrBackend::Browser);
    }

    let configured = args
        .backend
        .clone()
        .or_else(|| config.project_config.pr_backend.clone());

    let backend = match configured {
        None => PrBackend::Auto,
        Some(name) => PrBackend::parse(&name).ok_or_else(|| {
            RonaError::InvalidInput(format!(
                "Unknown pr backend '{name}'. Use auto, gh, glab, push-options, or browser."
            ))
        })?,
    };

    resolve_backend(backend, info)
}

/// Builds the request title from the flag, or from the template and its prompts.
///
/// # Errors
/// * If the title template is invalid
/// * If the user cancels a prompt
fn build_pr_title(
    args: &PrArgs,
    config: &Config,
    source_branch: &str,
    target_branch: &str,
) -> Result<String> {
    if let Some(title) = &args.title {
        return Ok(title.clone());
    }

    let template = config
        .project_config
        .pr_title_template
        .as_deref()
        .unwrap_or(DEFAULT_PR_TITLE_TEMPLATE);

    // Only prompt for fields the template actually uses, matching `rona branch`.
    let referenced_fields: Vec<ExtraField> = config
        .project_config
        .pr_extra_fields
        .iter()
        .filter(|field| {
            let referenced = template_references(template, &field.name);
            if !referenced {
                println!(
                    "[NOTE] PR extra field '{}' is not referenced in the template; skipping.",
                    field.name
                );
            }
            referenced
        })
        .cloned()
        .collect();

    let extra_names: Vec<&str> = referenced_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    if let Err(e) = validate_pr_template(template, &extra_names) {
        return Err(RonaError::InvalidInput(format!(
            "PR title template validation error: {e}"
        )));
    }

    let (title, extra_values) = prompt_pr_fields(
        &referenced_fields,
        &config.project_config.pr_field_order,
        template_references(template, "title"),
        config.project_config.pr_title.as_ref(),
        &default_title_source(),
    )?;

    let variables = PrTemplateVariables::new(
        source_branch.to_string(),
        branch_type_of(source_branch),
        title.trim().to_string(),
        target_branch.to_string(),
        default_title_source(),
    )?;

    Ok(process_pr_template(template, &variables, &extra_values)?
        .trim()
        .to_string())
}

/// Prepares the request document and returns its content.
///
/// `--body-file` is read as it is. Otherwise `pr_description.md` at the repository root is
/// created, excluded from git, and opened in the configured editor unless `--no-edit` was
/// passed. A new file is seeded with `seed_title` as its heading followed by the repository's
/// own request template, so the whole request is one editable document.
///
/// The returned text still carries the title heading; [`split_title_and_body`] separates them.
///
/// # Errors
/// * If the document cannot be read or written
/// * If the editor cannot be launched
fn prepare_pr_document(
    args: &PrArgs,
    config: &Config,
    info: &RemoteInfo,
    seed_title: &str,
) -> Result<String> {
    if let Some(body_file) = &args.body_file {
        return read_to_string(body_file).map_err(|e| {
            RonaError::Io(std::io::Error::other(format!(
                "Could not read body file '{body_file}': {e}"
            )))
        });
    }

    let root = get_top_level_path()?;
    let path = root.join(PR_DESCRIPTION_FILE_PATH);

    if config.dry_run {
        println!("Would write the request to: {}", path.display());

        return if path.exists() {
            Ok(read_to_string(&path)?)
        } else {
            Ok(compose_description(seed_title, ""))
        };
    }

    if !path.exists() {
        let template = seed_pr_description(&root, info.forge)?;
        std::fs::write(&path, compose_description(seed_title, &template))?;
        add_to_git_exclude(&[PR_DESCRIPTION_FILE_PATH])?;
    }

    if !args.no_edit {
        println!("The first heading is the title; everything below it is the description.");
        open_in_editor(&path, config)?;
    }

    Ok(read_to_string(&path)?)
}

/// Returns the content a new description file starts from.
///
/// Repositories with a single forge template use it. With several, the user picks one. With
/// none, the file starts empty.
///
/// # Errors
/// * If a template file cannot be read
/// * If the user cancels the template picker
fn seed_pr_description(root: &Path, forge: Forge) -> Result<String> {
    let templates = find_description_templates(root, forge);

    let chosen = match templates.len() {
        0 => return Ok(String::new()),
        1 => templates[0].clone(),
        _ => {
            let labels: Vec<String> = templates
                .iter()
                .map(|path| {
                    path.strip_prefix(root)
                        .unwrap_or(path)
                        .display()
                        .to_string()
                })
                .collect();
            let index = FuzzySelect::with_theme(&prompt_theme())
                .with_prompt("Select a description template")
                .items(&labels)
                .default(0)
                .interact_opt()
                .map_err(|_| RonaError::UserCancelled)?
                .ok_or(RonaError::UserCancelled)?;
            templates[index].clone()
        }
    };

    println!(
        "Seeded {PR_DESCRIPTION_FILE_PATH} from {}",
        chosen.strip_prefix(root).unwrap_or(&chosen).display()
    );

    Ok(read_to_string(&chosen)?)
}

/// Warns about request fields the chosen backend cannot carry.
///
/// Silence here would be the worst outcome: the request would be created without the labels or
/// reviewers the user asked for, and nothing would say so.
fn warn_unsupported_pr_fields(backend: PrBackend, request: &PrRequest, forge: Forge) {
    let warn = |what: &str, why: &str| {
        println!("{} {what} {why}", "WARNING:".yellow().bold());
    };

    if backend == PrBackend::PushOptions && !request.reviewers.is_empty() {
        warn(
            "Reviewers are dropped:",
            "GitLab push options cannot set them. Use --backend glab instead.",
        );
    }

    if backend == PrBackend::Browser {
        if forge == Forge::GitLab
            && !(request.labels.is_empty()
                && request.reviewers.is_empty()
                && request.assignees.is_empty())
        {
            warn(
                "Labels, reviewers, and assignees are not pre-filled:",
                "the GitLab form takes only the branches, title, and description.",
            );
        }

        if forge == Forge::Bitbucket {
            warn(
                "The title and description are not pre-filled:",
                "the Bitbucket form takes only the branches.",
            );
        }

        if request.draft {
            warn(
                "Draft is not pre-filled:",
                "tick the draft box in the web form.",
            );
        }
    }
}

/// Shows what is about to be opened and asks for confirmation.
fn confirm_pr(request: &PrRequest, backend: PrBackend, info: &RemoteInfo) -> bool {
    let summary = format!(
        "Open a {} on {}\n  {} -> {}\n  Title: {}\n  Backend: {}\nProceed?",
        info.forge.change_request_name(),
        info.project_path(),
        request.source_branch,
        request.target_branch,
        request.title,
        backend.as_str()
    );

    Confirm::with_theme(&prompt_theme())
        .with_prompt(summary)
        .default(true)
        .interact()
        .unwrap_or(false)
}

/// Chooses the request title from the sources that can supply one.
///
/// The flag wins, then the heading the user wrote in the document, then whatever seeded that
/// heading. Blank candidates are skipped rather than accepted, so `--title ""` falls through to
/// the document instead of leaving the request untitled.
fn choose_pr_title(from_flag: Option<&str>, from_heading: Option<&str>, seed: &str) -> String {
    [from_flag, from_heading, Some(seed)]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|candidate| !candidate.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// Handle the `Pr` command, which opens a pull or merge request for the current branch.
///
/// The payload is assembled the same way whatever the forge is, then handed to the backend
/// resolved from the remote: `gh`, `glab`, GitLab push options, or a pre-filled web form.
///
/// # Errors
/// * If the remote is missing, unparsable, or of an unknown forge
/// * If the target branch cannot be determined
/// * If the user cancels a prompt
/// * If the backend fails to open the request
fn handle_pr(args: &PrArgs, config: &Config) -> Result<()> {
    let (remote, info) = resolve_pr_remote(args, config)?;
    let source_branch = get_current_branch()?;
    let target_branch = resolve_pr_target(args, config, &remote, &source_branch)?;
    let backend = resolve_pr_backend(args, config, &info)?;

    // The title template seeds the document's heading, which the user can then edit in place.
    let seed_title = build_pr_title(args, config, &source_branch, &target_branch)?;
    let document = prepare_pr_document(args, config, &info, &seed_title)?;
    let (heading, body) = split_title_and_body(&document);

    let title = choose_pr_title(args.title.as_deref(), heading.as_deref(), &seed_title);

    if title.is_empty() {
        println!(
            "{} No title found. Give the request a `# Heading` or pass --title.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    }

    let body_path = if config.dry_run {
        body_file_path()?
    } else {
        write_body_file(body.trim())?
    };

    let request = PrRequest {
        title,
        body: body.trim().to_string(),
        source_branch,
        target_branch,
        remote,
        draft: args.draft || config.project_config.pr_draft,
        labels: merged_pr_values(&args.labels, &config.project_config.pr_labels),
        reviewers: merged_pr_values(&args.reviewers, &config.project_config.pr_reviewers),
        assignees: merged_pr_values(&args.assignees, &config.project_config.pr_assignees),
    };

    warn_unsupported_pr_fields(backend, &request, info.forge);

    if !args.yes && !config.dry_run && !confirm_pr(&request, backend, &info) {
        println!("Request cancelled.");
        return Ok(());
    }

    // Push options carry the branch themselves; every other backend needs it on the remote first.
    if !args.no_push && !backend.pushes_branch() {
        push_source_branch(&request, config.verbose, config.dry_run)?;
    }

    let url = submit(
        &request,
        backend,
        &info,
        &body_path,
        args.yes,
        config.verbose,
        config.dry_run,
    )?;

    // A dry run has already printed the command it would have run, URL included.
    if let Some(url) = url
        && !config.dry_run
    {
        println!("\n{} {url}", "✓".green());
    }

    Ok(())
}

/// Combines command line values with their config defaults, keeping each value once.
fn merged_pr_values(from_flags: &[String], from_config: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = from_config.to_vec();

    for value in from_flags {
        if !merged.contains(value) {
            merged.push(value.clone());
        }
    }

    merged
}

/// Initializes structured logging for the CLI.
///
/// Respects the `RUST_LOG` environment variable; falls back to `debug` when
/// `--verbose` is set and `warn` otherwise. Safe to call once at startup.
fn init_logging(verbose: bool) {
    let log_level = if verbose { "debug" } else { "warn" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init()
        .ok();
}

/// Loads the configuration, either from an explicit file or from the global/project hierarchy.
///
/// # Errors
/// * If the named config file is missing or cannot be parsed
/// * If the home directory cannot be determined
fn load_config(config_path: Option<&str>) -> Result<Config> {
    config_path.map_or_else(Config::new, |path| {
        Config::new_with_config_file(Path::new(path))
    })
}

/// Dispatches the `config` subcommands.
///
/// # Arguments
/// * `subcommand` - The parsed `config` subcommand
/// * `config` - Global configuration, updated with the subcommand dry-run setting
///
/// # Errors
/// * If the underlying config handler fails
fn handle_config_subcommand(subcommand: ConfigSubcommand, config: &mut Config) -> Result<()> {
    match subcommand {
        ConfigSubcommand::Create {
            scope,
            exclude,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            handle_config_command(scope, exclude, config)
        }
        ConfigSubcommand::Which {
            path,
            show_effective,
        } => handle_which_config(path.as_deref(), show_effective),
    }
}

/// Runs the program by parsing command line arguments and executing the appropriate command.
///
/// # Errors
/// * If creating configuration fails
/// * If command execution fails
/// * If any operation fails based on command-specific errors
///
/// # Returns
/// * `Result<()>` - Ok if all operations succeed, Err with error details otherwise
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let mut config = load_config(cli.config.as_deref())?;
    config.set_verbose(cli.verbose);

    match cli.command {
        CliCommand::Branch { dry_run, no_switch } => {
            config.set_dry_run(dry_run);
            handle_branch(no_switch, &config)
        }

        CliCommand::AddWithExclude {
            to_exclude: exclude,
            interactive,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            handle_add_with_exclude(&exclude, interactive, &config)
        }

        CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } => {
            config.set_dry_run(dry_run);
            handle_commit(&args, push, unsigned, yes, copy, &config)
        }

        CliCommand::Completion { shell } => {
            handle_completion(shell);
            Ok(())
        }

        CliCommand::Config { subcommand } => handle_config_subcommand(subcommand, &mut config),

        CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } => {
            config.set_dry_run(dry_run);
            handle_generate(interactive, no_commit_number, &config)
        }

        CliCommand::Initialize { editor, dry_run } => {
            config.set_dry_run(dry_run);
            handle_initialize(&editor, &config)
        }

        CliCommand::ListStatus => handle_list_status(),

        CliCommand::Pr(pr_args) => {
            config.set_dry_run(pr_args.dry_run);
            handle_pr(&pr_args, &config)
        }

        CliCommand::Push { args, dry_run } => {
            config.set_dry_run(dry_run);
            handle_push(&args, &config)
        }

        CliCommand::Reset {
            files,
            interactive,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            handle_reset(&files, interactive, &config)
        }

        CliCommand::Restore {
            files,
            interactive,
            yes,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            handle_restore(&files, interactive, yes, &config)
        }

        CliCommand::Set { editor, dry_run } => {
            config.set_dry_run(dry_run);
            handle_set(&editor, &config)
        }

        CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            handle_sync(
                &source_branch,
                rebase,
                new_branch.as_deref(),
                no_stash,
                &config,
            )
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::Parser;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    // === ADD COMMAND TESTS ===

    #[test]
    fn test_add_basic() -> TestResult {
        let args = vec!["rona", "-a"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::AddWithExclude {
            to_exclude: exclude,
            interactive,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(exclude.is_empty());
        assert!(!interactive);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_add_single_pattern() -> TestResult {
        let args = vec!["rona", "-a", "*.txt"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::AddWithExclude {
            to_exclude: exclude,
            interactive,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(exclude, vec!["*.txt"]);
        assert!(!interactive);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_add_multiple_patterns() -> TestResult {
        let args = vec!["rona", "-a", "*.txt", "*.log", "target/*"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::AddWithExclude {
            to_exclude: exclude,
            interactive,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(exclude, vec!["*.txt", "*.log", "target/*"]);
        assert!(!interactive);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_add_with_long_name() -> TestResult {
        let args = vec!["rona", "add-with-exclude", "*.txt"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::AddWithExclude {
            to_exclude: exclude,
            interactive,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(exclude, vec!["*.txt"]);
        assert!(!interactive);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_add_interactive() -> TestResult {
        let args = vec!["rona", "-a", "-i"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::AddWithExclude {
            to_exclude: exclude,
            interactive,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(exclude.is_empty());
        assert!(interactive);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_add_interactive_long_flag() -> TestResult {
        let args = vec!["rona", "-a", "--interactive"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::AddWithExclude { interactive, .. } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(interactive);
        Ok(())
    }

    // === RESET COMMAND TESTS ===

    #[test]
    fn test_reset_basic() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "reset"])?;

        let CliCommand::Reset {
            files,
            interactive,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(files.is_empty());
        assert!(!interactive);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_reset_with_files() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "reset", "src/main.rs", "README.md"])?;

        let CliCommand::Reset { files, .. } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(files, vec!["src/main.rs", "README.md"]);
        Ok(())
    }

    #[test]
    fn test_reset_interactive() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "reset", "-i"])?;

        let CliCommand::Reset { interactive, .. } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(interactive);
        Ok(())
    }

    #[test]
    fn test_reset_dry_run() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "reset", "--dry-run"])?;

        let CliCommand::Reset { dry_run, .. } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(dry_run);
        Ok(())
    }

    // === RESTORE COMMAND TESTS ===

    #[test]
    fn test_restore_basic() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "restore"])?;

        let CliCommand::Restore {
            files,
            interactive,
            yes,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(files.is_empty());
        assert!(!interactive);
        assert!(!yes);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_restore_with_files() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "restore", "src/main.rs"])?;

        let CliCommand::Restore { files, .. } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(files, vec!["src/main.rs"]);
        Ok(())
    }

    #[test]
    fn test_restore_interactive_and_yes() -> TestResult {
        let cli = Cli::try_parse_from(["rona", "restore", "-i", "-y"])?;

        let CliCommand::Restore {
            interactive, yes, ..
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(interactive);
        assert!(yes);
        Ok(())
    }

    // === COMMIT COMMAND TESTS ===

    #[test]
    fn test_commit_basic() -> TestResult {
        let args = vec!["rona", "-c"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_push_flag() -> TestResult {
        let args = vec!["rona", "-c", "--push"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(push);
        assert!(args.is_empty());
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_message() -> TestResult {
        let args = vec!["rona", "-c", "Regular commit message"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert_eq!(args, vec!["Regular commit message"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_git_flag() -> TestResult {
        let args = vec!["rona", "-c", "--amend"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert_eq!(args, vec!["--amend"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_multiple_git_flags() -> TestResult {
        let args = vec!["rona", "-c", "--amend", "--no-edit"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert_eq!(args, vec!["--amend", "--no-edit"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_push_and_git_flags() -> TestResult {
        let args = vec!["rona", "-c", "--push", "--amend", "--no-edit"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(push);
        assert_eq!(args, vec!["--amend", "--no-edit"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_message_and_push() -> TestResult {
        let args = vec!["rona", "-c", "--push", "Commit message"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(push);
        assert_eq!(args, vec!["Commit message"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    // === PUSH COMMAND TESTS ===

    #[test]
    fn test_push_basic() -> TestResult {
        let args = vec!["rona", "-p"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Push { args, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(args.is_empty());
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_push_with_force() -> TestResult {
        let args = vec!["rona", "-p", "--force"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Push { args, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(args, vec!["--force"]);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_push_with_multiple_args() -> TestResult {
        let args = vec!["rona", "-p", "--force", "--set-upstream", "origin", "main"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Push { args, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(args, vec!["--force", "--set-upstream", "origin", "main"]);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_push_with_remote_and_branch() -> TestResult {
        let args = vec!["rona", "-p", "origin", "feature/branch"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Push { args, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(args, vec!["origin", "feature/branch"]);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_push_with_upstream_tracking() -> TestResult {
        let args = vec!["rona", "-p", "-u", "origin", "main"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Push { args, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(args, vec!["-u", "origin", "main"]);
        assert!(!dry_run);
        Ok(())
    }

    // === GENERATE COMMAND TESTS ===

    #[test]
    fn test_generate_command() -> TestResult {
        let args = vec!["rona", "-g"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!dry_run);
        assert!(!interactive);
        assert!(!no_commit_number);
        Ok(())
    }

    #[test]
    fn test_generate_interactive_command() -> TestResult {
        let args = vec!["rona", "-g", "-i"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!dry_run);
        assert!(interactive);
        assert!(!no_commit_number);
        Ok(())
    }

    #[test]
    fn test_generate_interactive_long_form() -> TestResult {
        let args = vec!["rona", "-g", "--interactive"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!dry_run);
        assert!(interactive);
        assert!(!no_commit_number);
        Ok(())
    }

    #[test]
    fn test_generate_no_commit_number() -> TestResult {
        let args = vec!["rona", "-g", "-n"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!dry_run);
        assert!(!interactive);
        assert!(no_commit_number);
        Ok(())
    }

    #[test]
    fn test_generate_no_commit_number_long_form() -> TestResult {
        let args = vec!["rona", "-g", "--no-commit-number"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!dry_run);
        assert!(!interactive);
        assert!(no_commit_number);
        Ok(())
    }

    #[test]
    fn test_generate_interactive_no_commit_number() -> TestResult {
        let args = vec!["rona", "-g", "-i", "-n"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!dry_run);
        assert!(interactive);
        assert!(no_commit_number);
        Ok(())
    }

    // === LIST STATUS COMMAND TESTS ===

    #[test]
    fn test_list_status_command() -> TestResult {
        let args = vec!["rona", "-l"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::ListStatus = cli.command else {
            return Err("Wrong command parsed".into());
        };
        Ok(())
    }

    // === INITIALIZE COMMAND TESTS ===

    #[test]
    fn test_init_default_editor() -> TestResult {
        let args = vec!["rona", "-i"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Initialize { editor, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(editor, "nano");
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_init_custom_editor() -> TestResult {
        let args = vec!["rona", "-i", "zed"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Initialize { editor, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(editor, "zed");
        assert!(!dry_run);
        Ok(())
    }

    // === SET EDITOR COMMAND TESTS ===

    #[test]
    fn test_set_editor() -> TestResult {
        let args = vec!["rona", "-s", "vim"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Set { editor, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(editor, "vim");
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_set_editor_with_spaces() -> TestResult {
        let args = vec!["rona", "-s", "\"Visual Studio Code\""];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Set { editor, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(editor, "\"Visual Studio Code\"");
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_set_editor_with_path() -> TestResult {
        let args = vec!["rona", "-s", "/usr/bin/vim"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Set { editor, dry_run } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(editor, "/usr/bin/vim");
        assert!(!dry_run);
        Ok(())
    }

    // === VERBOSE FLAG TESTS ===

    #[test]
    fn test_verbose_with_commit() -> TestResult {
        let args = vec!["rona", "-v", "-c"];
        let cli = Cli::try_parse_from(args)?;
        assert!(cli.verbose);
        Ok(())
    }

    #[test]
    fn test_verbose_with_push() -> TestResult {
        let args = vec!["rona", "-v", "-p"];
        let cli = Cli::try_parse_from(args)?;
        assert!(cli.verbose);
        Ok(())
    }

    #[test]
    fn test_verbose_long_form() -> TestResult {
        let args = vec!["rona", "--verbose", "-c"];
        let cli = Cli::try_parse_from(args)?;
        assert!(cli.verbose);
        Ok(())
    }

    // === EDGE CASES AND ERROR TESTS ===

    #[test]
    fn test_commit_flag_order_sensitivity() -> TestResult {
        let args = vec!["rona", "-c", "--amend", "--push"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push); // --push should be treated as git arg
        assert_eq!(args, vec!["--amend", "--push"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_with_similar_looking_args() -> TestResult {
        let args = vec!["rona", "-c", "--push-to-upstream"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert_eq!(args, vec!["--push-to-upstream"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_invalid_command() {
        let args = vec!["rona", "--invalid"];
        assert!(Cli::try_parse_from(args).is_err());
    }

    #[test]
    fn test_missing_required_value() {
        let args = vec!["rona", "-s"]; // missing editor value
        assert!(Cli::try_parse_from(args).is_err());
    }

    #[test]
    fn test_complex_command_combination() -> TestResult {
        let args = vec!["rona", "-v", "-c", "--push", "--amend", "--no-edit"];
        let cli = Cli::try_parse_from(args)?;

        assert!(cli.verbose);
        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(push);
        assert_eq!(args, vec!["--amend", "--no-edit"]);
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_unsigned_short_flag() -> TestResult {
        let args = vec!["rona", "-c", "-u"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(!dry_run);
        assert!(unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_unsigned_long_flag() -> TestResult {
        let args = vec!["rona", "-c", "--unsigned"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(!dry_run);
        assert!(unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_unsigned_with_push_and_args() -> TestResult {
        let args = vec!["rona", "-c", "-u", "--push", "--amend"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(push);
        assert_eq!(args, vec!["--amend"]);
        assert!(!dry_run);
        assert!(unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_dry_run_short_flag() -> TestResult {
        let args = vec!["rona", "-c", "-d"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_dry_run_long_flag() -> TestResult {
        let args = vec!["rona", "-c", "--dry-run"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    #[test]
    fn test_commit_dry_run_with_push() -> TestResult {
        let args = vec!["rona", "-c", "-d", "--push"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(push);
        assert!(args.is_empty());
        assert!(dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(!copy);
        Ok(())
    }

    // === COPY FLAG TESTS ===

    #[test]
    fn test_commit_with_copy_flag() -> TestResult {
        let args = vec!["rona", "-c", "--copy"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(!dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(copy);
        Ok(())
    }

    #[test]
    fn test_commit_copy_flag_with_other_flags() -> TestResult {
        let args = vec!["rona", "-c", "--copy", "--dry-run"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
            yes,
            copy,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert!(!push);
        assert!(args.is_empty());
        assert!(dry_run);
        assert!(!unsigned);
        assert!(!yes);
        assert!(copy);
        Ok(())
    }

    // === CONFIG COMMAND TESTS ===

    fn unwrap_config_create(
        cli: Cli,
    ) -> std::result::Result<(ConfigScope, bool, bool), Box<dyn std::error::Error>> {
        let CliCommand::Config { subcommand } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        let ConfigSubcommand::Create {
            scope,
            exclude,
            dry_run,
        } = subcommand
        else {
            return Err("Wrong subcommand parsed".into());
        };
        Ok((scope, exclude, dry_run))
    }

    #[test]
    fn test_config_local() -> TestResult {
        let args = vec!["rona", "config", "create", "local"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Local));
        assert!(!exclude);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_config_local_short() -> TestResult {
        let args = vec!["rona", "config", "-c", "local"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Local));
        assert!(!exclude);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_config_global() -> TestResult {
        let args = vec!["rona", "config", "create", "global"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Global));
        assert!(!exclude);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_config_local_dry_run() -> TestResult {
        let args = vec!["rona", "config", "create", "local", "--dry-run"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Local));
        assert!(!exclude);
        assert!(dry_run);
        Ok(())
    }

    #[test]
    fn test_config_global_dry_run() -> TestResult {
        let args = vec!["rona", "config", "create", "global", "--dry-run"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Global));
        assert!(!exclude);
        assert!(dry_run);
        Ok(())
    }

    #[test]
    fn test_config_local_exclude() -> TestResult {
        let args = vec!["rona", "config", "create", "local", "--exclude"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Local));
        assert!(exclude);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_config_local_exclude_short() -> TestResult {
        let args = vec!["rona", "config", "-c", "local", "-e"];
        let cli = Cli::try_parse_from(args)?;
        let (scope, exclude, dry_run) = unwrap_config_create(cli)?;
        assert!(matches!(scope, ConfigScope::Local));
        assert!(exclude);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_config_which() -> TestResult {
        let args = vec!["rona", "config", "which"];
        let cli = Cli::try_parse_from(args)?;
        let CliCommand::Config { subcommand } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        let ConfigSubcommand::Which {
            path,
            show_effective,
        } = subcommand
        else {
            return Err("Wrong subcommand parsed".into());
        };
        assert!(path.is_none());
        assert!(!show_effective);
        Ok(())
    }

    #[test]
    fn test_config_which_short() -> TestResult {
        let args = vec!["rona", "config", "-w"];
        let cli = Cli::try_parse_from(args)?;
        let CliCommand::Config { subcommand } = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(matches!(subcommand, ConfigSubcommand::Which { .. }));
        Ok(())
    }

    #[test]
    fn test_config_missing_subcommand() {
        let args = vec!["rona", "config"];
        assert!(Cli::try_parse_from(args).is_err());
    }

    #[test]
    fn test_config_invalid_scope() {
        let args = vec!["rona", "config", "create", "invalid"];
        assert!(Cli::try_parse_from(args).is_err());
    }

    // === TEMPLATE SELECTION TESTS (REGRESSION TESTS) ===
    // These tests would have caught the bug where `rona -g -i -n` produced empty brackets []

    /// REGRESSION TEST: Verify template selection logic for interactive mode with `no_commit_number`
    /// This test verifies that the new conditional block syntax properly handles None `commit_number`
    #[test]
    fn test_template_selection_with_no_commit_number() -> TestResult {
        use std::collections::HashMap;

        use crate::template::{TemplateVariables, process_template};

        let default_template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}";

        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(default_template, &variables, &HashMap::new())?;

        assert!(
            !result.contains("[]"),
            "Output should not contain empty brackets: {result}"
        );
        assert_eq!(result, "(docs on main) Update docs");
        Ok(())
    }

    /// REGRESSION TEST: Verify template selection logic for interactive mode WITH `commit_number`
    #[test]
    fn test_template_selection_with_commit_number() -> TestResult {
        use std::collections::HashMap;

        use crate::template::{TemplateVariables, process_template};

        let default_template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}";

        let variables = TemplateVariables {
            commit_number: Some(42),
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(default_template, &variables, &HashMap::new())?;

        assert!(
            result.starts_with("[42]"),
            "Output should start with [42]: {result}"
        );
        assert_eq!(result, "[42] (feat on new-feature) Add feature");
        Ok(())
    }

    /// REGRESSION TEST: Verify that using wrong template produces the bug
    /// This documents the original bug and ensures our fix prevents it
    #[test]
    fn test_bug_using_wrong_template_with_no_commit_number() -> TestResult {
        use std::collections::HashMap;

        use crate::template::{TemplateVariables, process_template};

        let wrong_template = "[{commit_number}] ({commit_type} on {branch_name}) {message}";

        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(wrong_template, &variables, &HashMap::new())?;

        assert_eq!(result, "[] (docs on main) Update docs");
        assert!(result.contains("[]"), "This demonstrates the bug we fixed");
        Ok(())
    }

    /// REGRESSION TEST: Test fallback format in `handle_interactive_mode`
    /// Verify the fallback format also respects `no_commit_number` flag
    #[test]
    fn test_fallback_format_with_no_commit_number() {
        let no_commit_number = true;
        let commit_type = "fix";
        let branch_name = "bugfix";
        let message = "Fix issue";

        let formatted_message = if no_commit_number {
            format!("({commit_type} on {branch_name}) {message}")
        } else {
            format!("[42] ({commit_type} on {branch_name}) {message}")
        };

        assert_eq!(formatted_message, "(fix on bugfix) Fix issue");
        assert!(
            !formatted_message.contains("[]"),
            "Fallback should not produce empty brackets"
        );
    }

    /// REGRESSION TEST: Test fallback format with commit number
    #[test]
    fn test_fallback_format_with_commit_number() {
        let no_commit_number = false;
        let commit_number = 15u32;
        let commit_type = "feat";
        let branch_name = "feature";
        let message = "Add feature";

        let formatted_message = if no_commit_number {
            format!("({commit_type} on {branch_name}) {message}")
        } else {
            format!("[{commit_number}] ({commit_type} on {branch_name}) {message}")
        };

        assert_eq!(formatted_message, "[15] (feat on feature) Add feature");
        assert!(
            !formatted_message.contains("[]"),
            "Should not produce empty brackets"
        );
    }

    // === COMMIT TYPE SELECTOR TESTS ===

    #[test]
    fn test_template_references_plain_variable() {
        assert!(template_references(
            "({commit_type}) {message}",
            "commit_type"
        ));
        assert!(template_references("({commit_type}) {message}", "message"));
    }

    #[test]
    fn test_template_references_conditional_block() {
        let template = "{?commit_number}[{commit_number}] {/commit_number}{message}";
        assert!(template_references(template, "commit_number"));
    }

    #[test]
    fn test_template_references_ignores_unused_variables() {
        assert!(!template_references("{message}", "commit_type"));
        assert!(!template_references("{message}", "ticket"));
    }

    #[test]
    fn test_template_references_requires_exact_name() {
        // A longer name that merely contains the shorter one must not count as a reference.
        assert!(!template_references("{commit_type_extra}", "commit_type"));
    }

    #[test]
    fn test_default_template_references_commit_type() {
        assert!(template_references(DEFAULT_COMMIT_TEMPLATE, "commit_type"));
    }

    // === SYNC COMMAND TESTS ===

    #[test]
    fn test_sync_basic() -> TestResult {
        let args = vec!["rona", "sync"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(!rebase);
        assert!(new_branch.is_none());
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_branch() -> TestResult {
        let args = vec!["rona", "sync", "--branch", "develop"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "develop");
        assert!(!rebase);
        assert!(new_branch.is_none());
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_branch_short_flag() -> TestResult {
        let args = vec!["rona", "sync", "-b", "staging"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "staging");
        assert!(!rebase);
        assert!(new_branch.is_none());
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_rebase() -> TestResult {
        let args = vec!["rona", "sync", "--rebase"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(rebase);
        assert!(new_branch.is_none());
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_rebase_short_flag() -> TestResult {
        let args = vec!["rona", "sync", "-r"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(rebase);
        assert!(new_branch.is_none());
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_new_branch() -> TestResult {
        let args = vec!["rona", "sync", "--new-branch", "feature/new-feature"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(!rebase);
        assert_eq!(new_branch, Some("feature/new-feature".to_string()));
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_new_branch_short_flag() -> TestResult {
        let args = vec!["rona", "sync", "-n", "bugfix/issue-123"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(!rebase);
        assert_eq!(new_branch, Some("bugfix/issue-123".to_string()));
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_no_stash() -> TestResult {
        let args = vec!["rona", "sync", "--no-stash"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(!rebase);
        assert!(new_branch.is_none());
        assert!(no_stash);
        assert!(!dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_with_dry_run() -> TestResult {
        let args = vec!["rona", "sync", "--dry-run"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "main");
        assert!(!rebase);
        assert!(new_branch.is_none());
        assert!(!no_stash);
        assert!(dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_all_options() -> TestResult {
        let args = vec![
            "rona",
            "sync",
            "--branch",
            "develop",
            "--rebase",
            "--new-branch",
            "feature/test",
            "--dry-run",
        ];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "develop");
        assert!(rebase);
        assert_eq!(new_branch, Some("feature/test".to_string()));
        assert!(!no_stash);
        assert!(dry_run);
        Ok(())
    }

    #[test]
    fn test_sync_short_flags_combination() -> TestResult {
        let args = vec![
            "rona",
            "sync",
            "-b",
            "staging",
            "-r",
            "-n",
            "hotfix/critical",
        ];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } = cli.command
        else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(source_branch, "staging");
        assert!(rebase);
        assert_eq!(new_branch, Some("hotfix/critical".to_string()));
        assert!(!no_stash);
        assert!(!dry_run);
        Ok(())
    }

    // === PR COMMAND TESTS ===

    #[test]
    fn test_pr_defaults() -> TestResult {
        let args = vec!["rona", "pr"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(pr.target.is_none());
        assert!(pr.title.is_none());
        assert!(pr.body_file.is_none());
        assert!(pr.backend.is_none());
        assert!(pr.remote.is_none());
        assert!(!pr.draft);
        assert!(!pr.web);
        assert!(!pr.no_edit);
        assert!(!pr.no_push);
        assert!(!pr.yes);
        assert!(!pr.dry_run);
        assert!(pr.labels.is_empty());
        assert!(pr.reviewers.is_empty());
        assert!(pr.assignees.is_empty());
        Ok(())
    }

    #[test]
    fn test_pr_mr_alias() -> TestResult {
        let cli = Cli::try_parse_from(vec!["rona", "mr"])?;

        assert!(matches!(cli.command, CliCommand::Pr(_)));
        Ok(())
    }

    #[test]
    fn test_pr_pull_request_alias() -> TestResult {
        let cli = Cli::try_parse_from(vec!["rona", "pull-request"])?;

        assert!(matches!(cli.command, CliCommand::Pr(_)));
        Ok(())
    }

    #[test]
    fn test_pr_target_and_title() -> TestResult {
        let args = vec![
            "rona",
            "pr",
            "--target",
            "develop",
            "--title",
            "Add the pr command",
        ];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(pr.target, Some("develop".to_string()));
        assert_eq!(pr.title, Some("Add the pr command".to_string()));
        Ok(())
    }

    #[test]
    fn test_pr_short_flags() -> TestResult {
        let args = vec!["rona", "pr", "-t", "main", "-T", "Title", "-d", "-y", "-w"];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(pr.target, Some("main".to_string()));
        assert_eq!(pr.title, Some("Title".to_string()));
        assert!(pr.draft);
        assert!(pr.yes);
        assert!(pr.web);
        Ok(())
    }

    #[test]
    fn test_pr_repeated_labels_and_reviewers() -> TestResult {
        let args = vec![
            "rona", "pr", "-l", "bug", "-l", "urgent", "-r", "alice", "-A", "bob",
        ];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(pr.labels, vec!["bug".to_string(), "urgent".to_string()]);
        assert_eq!(pr.reviewers, vec!["alice".to_string()]);
        assert_eq!(pr.assignees, vec!["bob".to_string()]);
        Ok(())
    }

    #[test]
    fn test_pr_backend_accepts_known_names() -> TestResult {
        for backend in ["auto", "gh", "glab", "push-options", "browser"] {
            let cli = Cli::try_parse_from(vec!["rona", "pr", "--backend", backend])?;
            let CliCommand::Pr(pr) = cli.command else {
                return Err("Wrong command parsed".into());
            };
            assert_eq!(pr.backend, Some(backend.to_string()));
        }
        Ok(())
    }

    #[test]
    fn test_pr_backend_rejects_unknown_names() {
        // A typo must fail at parse time rather than silently opening nothing.
        assert!(Cli::try_parse_from(vec!["rona", "pr", "--backend", "carrier-pigeon"]).is_err());
    }

    #[test]
    fn test_pr_body_file_and_no_edit() -> TestResult {
        let args = vec![
            "rona",
            "pr",
            "--body-file",
            "notes.md",
            "--no-edit",
            "--no-push",
        ];
        let cli = Cli::try_parse_from(args)?;

        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert_eq!(pr.body_file, Some("notes.md".to_string()));
        assert!(pr.no_edit);
        assert!(pr.no_push);
        Ok(())
    }

    #[test]
    fn test_pr_dry_run() -> TestResult {
        let cli = Cli::try_parse_from(vec!["rona", "pr", "--dry-run"])?;

        let CliCommand::Pr(pr) = cli.command else {
            return Err("Wrong command parsed".into());
        };
        assert!(pr.dry_run);
        Ok(())
    }

    // === PR TITLE RESOLUTION TESTS ===

    #[test]
    fn test_choose_pr_title_prefers_the_flag() {
        let title = choose_pr_title(Some("From flag"), Some("From heading"), "From seed");

        assert_eq!(title, "From flag");
    }

    #[test]
    fn test_choose_pr_title_falls_back_to_the_heading() {
        let title = choose_pr_title(None, Some("From heading"), "From seed");

        assert_eq!(title, "From heading");
    }

    #[test]
    fn test_choose_pr_title_falls_back_to_the_seed() {
        let title = choose_pr_title(None, None, "From seed");

        assert_eq!(title, "From seed");
    }

    #[test]
    fn test_choose_pr_title_skips_blank_candidates() {
        // An explicitly empty flag must not leave the request untitled.
        assert_eq!(
            choose_pr_title(Some("   "), Some("Heading"), "Seed"),
            "Heading"
        );
        assert_eq!(choose_pr_title(Some(""), None, "Seed"), "Seed");
    }

    #[test]
    fn test_choose_pr_title_trims_whitespace() {
        assert_eq!(choose_pr_title(Some("  Padded  "), None, "Seed"), "Padded");
    }

    #[test]
    fn test_choose_pr_title_is_empty_when_nothing_supplies_one() {
        assert!(choose_pr_title(None, None, "").is_empty());
    }

    // === PR FIELD ORDERING AND MERGING TESTS ===

    fn field(name: &str) -> ExtraField {
        ExtraField {
            name: name.to_string(),
            prompt: None,
            kind: crate::extra_fields::FieldKind::default(),
            required: false,
            validation: None,
            prefetch: None,
        }
    }

    #[test]
    fn test_ordered_field_names_appends_builtin_last_by_default() {
        let fields = vec![field("ticket")];

        let ordered = ordered_field_names(&fields, &[], "title", true);

        assert_eq!(ordered, vec!["ticket".to_string(), "title".to_string()]);
    }

    #[test]
    fn test_ordered_field_names_honours_a_configured_order() {
        let fields = vec![field("ticket"), field("scope")];
        let order = vec!["title".to_string(), "ticket".to_string()];

        let ordered = ordered_field_names(&fields, &order, "title", true);

        // Listed items keep their order, and the unlisted field is appended.
        assert_eq!(
            ordered,
            vec![
                "title".to_string(),
                "ticket".to_string(),
                "scope".to_string()
            ]
        );
    }

    #[test]
    fn test_ordered_field_names_omits_a_disabled_builtin() {
        let fields = vec![field("ticket")];

        let ordered = ordered_field_names(&fields, &[], "title", false);

        assert_eq!(ordered, vec!["ticket".to_string()]);
    }

    #[test]
    fn test_merged_pr_values_keeps_config_first_and_deduplicates() {
        let from_flags = vec!["urgent".to_string(), "bug".to_string()];
        let from_config = vec!["bug".to_string()];

        let merged = merged_pr_values(&from_flags, &from_config);

        assert_eq!(merged, vec!["bug".to_string(), "urgent".to_string()]);
    }

    #[test]
    fn test_merged_pr_values_without_config_defaults() {
        let merged = merged_pr_values(&["bug".to_string()], &[]);

        assert_eq!(merged, vec!["bug".to_string()]);
    }
}
