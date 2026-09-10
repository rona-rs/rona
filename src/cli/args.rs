//! The command line grammar, as clap sees it.
//!
//! Every flag, subcommand, alias, and default value rona accepts is declared here and nowhere
//! else, so the whole surface can be read in one file. What each command then does lives in
//! [`crate::commands`].

use clap::{Args, Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::Shell;

/// Which configuration file `rona config create` writes.
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

        /// Additional arguments forwarded to `git commit` (for example `-s` or `--amend`). They are
        /// not forwarded to the push done by `--push`
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
    pub(crate) verbose: bool,

    /// Config file to use instead of the default global/project hierarchy
    #[arg(short = 'f', long = "config-file", value_name = "PATH", value_hint = ValueHint::FilePath, global = true)]
    pub(crate) config: Option<String>,
}

#[cfg(test)]
mod tests;
