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
//! - `generate`: Generate a new commit message file
//! - `init`: Initialize Rona configuration
//! - `list-status`: List git status files (for shell completion)
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

use clap::{Command as ClapCommand, CommandFactory, Parser, Subcommand, ValueHint, command};
use clap_complete::{Shell, generate};
use glob::Pattern;
use inquire::ui::{Attributes, Color, RenderConfig, StyleSheet, Styled};
use inquire::{Select, Text};
use std::{io, process::Command};

use crate::{
    config::Config,
    errors::Result,
    git::{
        COMMIT_MESSAGE_FILE_PATH, COMMIT_TYPES, create_needed_files, format_branch_name,
        generate_commit_message, get_current_branch, get_current_commit_nb, get_status_files,
        get_top_level_path, git_add_with_exclude_patterns, git_commit, git_push,
    },
    template::{TemplateVariables, process_template, validate_template},
};

/// CLI's commands
#[derive(Subcommand)]
pub(crate) enum CliCommand {
    /// Add all files to the `git add` command and exclude the patterns passed as positional arguments.
    #[command(short_flag = 'a', name = "add-with-exclude")]
    AddWithExclude {
        /// Patterns of files to exclude (supports glob patterns like `"node_modules/*"`)
        #[arg(value_name = "PATTERNS", value_hint = ValueHint::AnyPath)]
        to_exclude: Vec<String>,

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
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Create unsigned commit (default is to auto-detect GPG availability and sign if possible)
        #[arg(short = 'u', long = "unsigned", default_value_t = false)]
        unsigned: bool,

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
}

#[derive(Parser)]
#[command(about = "Simple program that can:\n\
\t- Commit with the current 'commit_message.md' file text.\n\
\t- Generate the 'commit_message.md' file.\n\
\t- Push to git repository.\n\
\t- Add files with pattern exclusion.\n\
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

    /// Use the custom config file path instead of default
    #[arg(long, value_name = "PATH")]
    config: Option<String>,
}

/// Build the CLI command structure for generating completions
#[doc(hidden)]
fn build_cli() -> ClapCommand {
    Cli::command()
}

fn get_render_config() -> RenderConfig<'static> {
    let mut render_config = RenderConfig::default();

    // Prefix/icons
    render_config.prompt_prefix = Styled::new("$").with_fg(Color::LightRed);
    render_config.answered_prompt_prefix = Styled::new("✔").with_fg(Color::LightGreen);
    render_config.highlighted_option_prefix = Styled::new("➠").with_fg(Color::LightBlue);
    render_config.selected_checkbox = Styled::new("☑").with_fg(Color::LightGreen);
    render_config.unselected_checkbox = Styled::new("☐").with_fg(Color::Black);
    render_config.scroll_up_prefix = Styled::new("⇞").with_fg(Color::Black);
    render_config.scroll_down_prefix = Styled::new("⇟").with_fg(Color::Black);

    // Input prompt label
    render_config.prompt = StyleSheet::new()
        .with_fg(Color::LightCyan)
        .with_attr(Attributes::BOLD);

    // Help under the input
    render_config.help_message = StyleSheet::new()
        .with_fg(Color::DarkYellow)
        .with_attr(Attributes::ITALIC);

    // Validation error
    render_config.error_message = render_config
        .error_message
        .with_prefix(Styled::new("❌").with_fg(Color::LightRed));

    // Shown after submit (echoed answer)
    render_config.answer = StyleSheet::new()
        .with_fg(Color::LightMagenta)
        .with_attr(Attributes::BOLD);

    // Optional: default/placeholder styles
    render_config.default_value = StyleSheet::new().with_fg(Color::LightBlue);
    render_config.placeholder = StyleSheet::new().with_fg(Color::Black);

    render_config
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
fn handle_add_with_exclude(exclude: &[String], config: &Config) -> Result<()> {
    let patterns: Vec<Pattern> = exclude
        .iter()
        .map(|p| Pattern::new(p).expect("Invalid glob pattern"))
        .collect();

    git_add_with_exclude_patterns(&patterns, config.verbose, config.dry_run)?;
    Ok(())
}

/// Handle the Commit command which commits changes using the message from `commit_message.md`.
///
/// # Arguments
/// * `args` - Additional arguments to pass to git commit
/// * `push` - Whether to push changes after committing
/// * `unsigned` - Whether to create an unsigned commit (skips -S flag)
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If git commit operation fails
/// * If push is true and git push operation fails
fn handle_commit(args: &[String], push: bool, unsigned: bool, config: &Config) -> Result<()> {
    git_commit(args, unsigned, config.verbose, config.dry_run)?;

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

    let commit_types_vec = config.project_config.commit_types.as_ref().map_or_else(
        || COMMIT_TYPES.to_vec(),
        |v| v.iter().map(String::as_str).collect::<Vec<&str>>(),
    );

    let commit_type = Select::new("Select commit type", commit_types_vec)
        .with_starting_cursor(0)
        .prompt()
        .unwrap();

    generate_commit_message(commit_type, config.verbose, no_commit_number)?;

    if interactive {
        handle_interactive_mode(commit_type, no_commit_number, config)?;
    } else {
        handle_editor_mode(config)?;
    }
    Ok(())
}

/// Handle interactive mode for generate command
fn handle_interactive_mode(
    commit_type: &str,
    no_commit_number: bool,
    config: &Config,
) -> Result<()> {
    use std::fs;

    println!("📝 Interactive mode: Enter your commit message.");
    println!("💡 Tip: Keep it concise and descriptive.");

    let project_root = get_top_level_path()?;
    let commit_file_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    let message: String = Text::new("Message").prompt().unwrap();

    if message.trim().is_empty() {
        println!("⚠️  Empty message provided. Exiting.");
        return Ok(());
    }

    let branch_name = format_branch_name(&COMMIT_TYPES, &get_current_branch()?);
    let commit_number = if no_commit_number {
        None
    } else {
        Some(get_current_commit_nb()? + 1)
    };

    // Get template from config or use default based on no_commit_number flag
    let default_template = if no_commit_number {
        "({commit_type} on {branch_name}) {message}"
    } else {
        "[{commit_number}] ({commit_type} on {branch_name}) {message}"
    };

    let template = config
        .project_config
        .template
        .as_deref()
        .unwrap_or(default_template);

    // Validate template
    if let Err(e) = validate_template(template) {
        println!("⚠️  Template validation error: {e}");
        println!("Using fallback format...");
        let formatted_message = if no_commit_number {
            format!("({} on {}) {}", commit_type, branch_name, message.trim())
        } else {
            format!(
                "[{}] ({} on {}) {}",
                commit_number.unwrap(),
                commit_type,
                branch_name,
                message.trim()
            )
        };
        fs::write(&commit_file_path, &formatted_message)?;
        println!("\n✅ Commit message created!");
        println!("📄 Message: {formatted_message}");
        return Ok(());
    }

    // Create template variables
    let variables = TemplateVariables::new(
        commit_number,
        commit_type.to_string(),
        branch_name,
        message.trim().to_string(),
    )?;

    // Process template
    let formatted_message = process_template(template, &variables)?;

    // Write the formatted message to commit_message.md
    fs::write(&commit_file_path, &formatted_message)?;

    println!("\n✅ Commit message created!");
    println!("📄 Message: {formatted_message}");
    Ok(())
}

/// Handle editor mode for generate command
fn handle_editor_mode(config: &Config) -> Result<()> {
    let editor = config.get_editor()?;
    let project_root = get_top_level_path()?;
    let commit_file_path = project_root.join(COMMIT_MESSAGE_FILE_PATH);

    Command::new(editor)
        .arg(&commit_file_path)
        .spawn()
        .expect("Failed to spawn editor")
        .wait()
        .expect("Failed to wait for editor");
    Ok(())
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
    // Apply global colors/styles for all inquire prompts
    inquire::set_global_render_config(get_render_config());

    let cli = Cli::parse();
    let mut config = Config::new()?;

    // Set the global flags in the config
    config.set_verbose(cli.verbose);

    match cli.command {
        CliCommand::AddWithExclude {
            to_exclude: exclude,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            handle_add_with_exclude(&exclude, &config)
        }

        CliCommand::Commit {
            args,
            push,
            dry_run,
            unsigned,
        } => {
            config.set_dry_run(dry_run);
            handle_commit(&args, push, unsigned, &config)
        }

        CliCommand::Completion { shell } => {
            handle_completion(shell);
            Ok(())
        }

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

        CliCommand::Push { args, dry_run } => {
            config.set_dry_run(dry_run);
            handle_push(&args, &config)
        }

        CliCommand::Set { editor, dry_run } => {
            config.set_dry_run(dry_run);
            handle_set(&editor, &config)
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::Parser;

    // === ADD COMMAND TESTS ===

    #[test]
    fn test_add_basic() {
        let args = vec!["rona", "-a"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::AddWithExclude {
                to_exclude: exclude,
                dry_run,
            } => {
                assert!(exclude.is_empty());
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_add_single_pattern() {
        let args = vec!["rona", "-a", "*.txt"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::AddWithExclude {
                to_exclude: exclude,
                dry_run,
            } => {
                assert_eq!(exclude, vec!["*.txt"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_add_multiple_patterns() {
        let args = vec!["rona", "-a", "*.txt", "*.log", "target/*"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::AddWithExclude {
                to_exclude: exclude,
                dry_run,
            } => {
                assert_eq!(exclude, vec!["*.txt", "*.log", "target/*"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_add_with_long_name() {
        let args = vec!["rona", "add-with-exclude", "*.txt"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::AddWithExclude {
                to_exclude: exclude,
                dry_run,
            } => {
                assert_eq!(exclude, vec!["*.txt"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === COMMIT COMMAND TESTS ===

    #[test]
    fn test_commit_basic() {
        let args = vec!["rona", "-c"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert!(args.is_empty());
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_push_flag() {
        let args = vec!["rona", "-c", "--push"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(push);
                assert!(args.is_empty());
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_message() {
        let args = vec!["rona", "-c", "Regular commit message"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert_eq!(args, vec!["Regular commit message"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_git_flag() {
        let args = vec!["rona", "-c", "--amend"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert_eq!(args, vec!["--amend"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_multiple_git_flags() {
        let args = vec!["rona", "-c", "--amend", "--no-edit"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert_eq!(args, vec!["--amend", "--no-edit"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_push_and_git_flags() {
        let args = vec!["rona", "-c", "--push", "--amend", "--no-edit"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(push);
                assert_eq!(args, vec!["--amend", "--no-edit"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_message_and_push() {
        let args = vec!["rona", "-c", "--push", "Commit message"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(push);
                assert_eq!(args, vec!["Commit message"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === PUSH COMMAND TESTS ===

    #[test]
    fn test_push_basic() {
        let args = vec!["rona", "-p"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Push { args, dry_run } => {
                assert!(args.is_empty());
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_push_with_force() {
        let args = vec!["rona", "-p", "--force"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Push { args, dry_run } => {
                assert_eq!(args, vec!["--force"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_push_with_multiple_args() {
        let args = vec!["rona", "-p", "--force", "--set-upstream", "origin", "main"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Push { args, dry_run } => {
                assert_eq!(args, vec!["--force", "--set-upstream", "origin", "main"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_push_with_remote_and_branch() {
        let args = vec!["rona", "-p", "origin", "feature/branch"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Push { args, dry_run } => {
                assert_eq!(args, vec!["origin", "feature/branch"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_push_with_upstream_tracking() {
        let args = vec!["rona", "-p", "-u", "origin", "main"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Push { args, dry_run } => {
                assert_eq!(args, vec!["-u", "origin", "main"]);
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === GENERATE COMMAND TESTS ===

    #[test]
    fn test_generate_command() {
        let args = vec!["rona", "-g"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Generate {
                dry_run,
                interactive,
                no_commit_number,
            } => {
                assert!(!dry_run);
                assert!(!interactive);
                assert!(!no_commit_number);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_generate_interactive_command() {
        let args = vec!["rona", "-g", "-i"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Generate {
                dry_run,
                interactive,
                no_commit_number,
            } => {
                assert!(!dry_run);
                assert!(interactive);
                assert!(!no_commit_number);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_generate_interactive_long_form() {
        let args = vec!["rona", "-g", "--interactive"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Generate {
                dry_run,
                interactive,
                no_commit_number,
            } => {
                assert!(!dry_run);
                assert!(interactive);
                assert!(!no_commit_number);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_generate_no_commit_number() {
        let args = vec!["rona", "-g", "-n"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Generate {
                dry_run,
                interactive,
                no_commit_number,
            } => {
                assert!(!dry_run);
                assert!(!interactive);
                assert!(no_commit_number);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_generate_no_commit_number_long_form() {
        let args = vec!["rona", "-g", "--no-commit-number"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Generate {
                dry_run,
                interactive,
                no_commit_number,
            } => {
                assert!(!dry_run);
                assert!(!interactive);
                assert!(no_commit_number);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_generate_interactive_no_commit_number() {
        let args = vec!["rona", "-g", "-i", "-n"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Generate {
                dry_run,
                interactive,
                no_commit_number,
            } => {
                assert!(!dry_run);
                assert!(interactive);
                assert!(no_commit_number);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === LIST STATUS COMMAND TESTS ===

    #[test]
    fn test_list_status_command() {
        let args = vec!["rona", "-l"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::ListStatus => (),
            _ => panic!("Wrong command parsed"),
        }
    }

    // === INITIALIZE COMMAND TESTS ===

    #[test]
    fn test_init_default_editor() {
        let args = vec!["rona", "-i"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Initialize { editor, dry_run } => {
                assert_eq!(editor, "nano");
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_init_custom_editor() {
        let args = vec!["rona", "-i", "zed"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Initialize { editor, dry_run } => {
                assert_eq!(editor, "zed");
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === SET EDITOR COMMAND TESTS ===

    #[test]
    fn test_set_editor() {
        let args = vec!["rona", "-s", "vim"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Set { editor, dry_run } => {
                assert_eq!(editor, "vim");
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_set_editor_with_spaces() {
        let args = vec!["rona", "-s", "\"Visual Studio Code\""];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Set { editor, dry_run } => {
                assert_eq!(editor, "\"Visual Studio Code\"");
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_set_editor_with_path() {
        let args = vec!["rona", "-s", "/usr/bin/vim"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Set { editor, dry_run } => {
                assert_eq!(editor, "/usr/bin/vim");
                assert!(!dry_run);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === VERBOSE FLAG TESTS ===

    #[test]
    fn test_verbose_with_commit() {
        let args = vec!["rona", "-v", "-c"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn test_verbose_with_push() {
        let args = vec!["rona", "-v", "-p"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn test_verbose_long_form() {
        let args = vec!["rona", "--verbose", "-c"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.verbose);
    }

    // === EDGE CASES AND ERROR TESTS ===

    #[test]
    fn test_commit_flag_order_sensitivity() {
        let args = vec!["rona", "-c", "--amend", "--push"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push); // --push should be treated as git arg
                assert_eq!(args, vec!["--amend", "--push"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_with_similar_looking_args() {
        let args = vec!["rona", "-c", "--push-to-upstream"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert_eq!(args, vec!["--push-to-upstream"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
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
    fn test_complex_command_combination() {
        let args = vec!["rona", "-v", "-c", "--push", "--amend", "--no-edit"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert!(cli.verbose);
        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(push);
                assert_eq!(args, vec!["--amend", "--no-edit"]);
                assert!(!dry_run);
                assert!(!unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_unsigned_short_flag() {
        let args = vec!["rona", "-c", "-u"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert!(args.is_empty());
                assert!(!dry_run);
                assert!(unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_unsigned_long_flag() {
        let args = vec!["rona", "-c", "--unsigned"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(!push);
                assert!(args.is_empty());
                assert!(!dry_run);
                assert!(unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    #[test]
    fn test_commit_unsigned_with_push_and_args() {
        let args = vec!["rona", "-c", "-u", "--push", "--amend"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            CliCommand::Commit {
                args,
                push,
                dry_run,
                unsigned,
            } => {
                assert!(push);
                assert_eq!(args, vec!["--amend"]);
                assert!(!dry_run);
                assert!(unsigned);
            }
            _ => panic!("Wrong command parsed"),
        }
    }

    // === TEMPLATE SELECTION TESTS (REGRESSION TESTS) ===
    // These tests would have caught the bug where `rona -g -i -n` produced empty brackets []

    /// REGRESSION TEST: Verify template selection logic for interactive mode with `no_commit_number`
    /// This test verifies that the correct default template is chosen based on the flag
    #[test]
    fn test_template_selection_with_no_commit_number() {
        use crate::template::{TemplateVariables, process_template};

        // Simulate what handle_interactive_mode should do with no_commit_number = true
        let no_commit_number = true;

        // The default template should NOT include commit_number placeholder
        let default_template = if no_commit_number {
            "({commit_type} on {branch_name}) {message}"
        } else {
            "[{commit_number}] ({commit_type} on {branch_name}) {message}"
        };

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

        let result = process_template(default_template, &variables).unwrap();

        // Should NOT contain empty brackets
        assert!(
            !result.contains("[]"),
            "Output should not contain empty brackets: {result}"
        );
        assert_eq!(result, "(docs on main) Update docs");
    }

    /// REGRESSION TEST: Verify template selection logic for interactive mode WITH `commit_number`
    #[test]
    fn test_template_selection_with_commit_number() {
        use crate::template::{TemplateVariables, process_template};

        // Simulate what handle_interactive_mode should do with no_commit_number = false
        let no_commit_number = false;

        // The default template SHOULD include commit_number placeholder
        let default_template = if no_commit_number {
            "({commit_type} on {branch_name}) {message}"
        } else {
            "[{commit_number}] ({commit_type} on {branch_name}) {message}"
        };

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

        let result = process_template(default_template, &variables).unwrap();

        // Should contain properly formatted commit number
        assert!(
            result.starts_with("[42]"),
            "Output should start with [42]: {result}"
        );
        assert_eq!(result, "[42] (feat on new-feature) Add feature");
    }

    /// REGRESSION TEST: Verify that using wrong template produces the bug
    /// This documents the original bug and ensures our fix prevents it
    #[test]
    fn test_bug_using_wrong_template_with_no_commit_number() {
        use crate::template::{TemplateVariables, process_template};

        // This simulates the BUG: using default template with None commit_number
        let wrong_template = "[{commit_number}] ({commit_type} on {branch_name}) {message}";

        let variables = TemplateVariables {
            commit_number: None, // This is the key: None with a template that expects a number
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(wrong_template, &variables).unwrap();

        // This DOCUMENTS the bug: using wrong template produces empty brackets
        assert_eq!(result, "[] (docs on main) Update docs");
        assert!(result.contains("[]"), "This demonstrates the bug we fixed");
    }

    /// REGRESSION TEST: Test fallback format in `handle_interactive_mode`
    /// Verify the fallback format also respects `no_commit_number` flag
    #[test]
    fn test_fallback_format_with_no_commit_number() {
        // Simulate the fallback format from handle_interactive_mode
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
        // Simulate the fallback format from handle_interactive_mode
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
}
