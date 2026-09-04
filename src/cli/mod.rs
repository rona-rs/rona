//! Command Line Interface (CLI) Module for Rona
//!
//! This module is the front door of the binary, and only that:
//!
//! - [`args`] declares the command line grammar (every command, flag, and alias)
//! - [`completion`] turns that grammar into a shell completion script
//! - [`run`] parses the command line, loads the configuration, and calls exactly one
//!   function in [`crate::commands`]
//!
//! What each command actually does lives in [`crate::commands`], one module per family of
//! commands. Keeping the two apart means a command's behaviour can be read without wading
//! through argument parsing, and the whole command line surface can be reviewed in one file.
//!
//! # Global flags
//!
//! `--verbose` and `--config-file` apply to every command. `--dry-run` is declared per
//! command (so it can be discovered in each command's help) and folded into the loaded
//! [`Config`], which is what commands consult.

mod args;
mod completion;

use std::path::Path;

use args::{Cli, CliCommand, ConfigSubcommand};
use clap::Parser;

use crate::{commands, config::Config, errors::Result};

// The parsed values a command implementation reads for itself, rather than through `run`.
pub(crate) use args::{ConfigScope, PrArgs};

/// Runs rona: parses the command line and executes the command it names.
///
/// # Errors
/// * If the configuration cannot be loaded
/// * If the command fails, with the command's own error
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let mut config = load_config(cli.config.as_deref())?;
    config.set_verbose(cli.verbose);

    match cli.command {
        CliCommand::Branch { dry_run, no_switch } => {
            config.set_dry_run(dry_run);
            commands::branch::create(no_switch, &config)
        }

        CliCommand::AddWithExclude {
            to_exclude,
            interactive,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            commands::staging::add(&to_exclude, interactive, &config)
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
            commands::commit::commit(&args, push, unsigned, yes, copy, &config)
        }

        CliCommand::Completion { shell } => {
            completion::print(shell);
            Ok(())
        }

        CliCommand::Config { subcommand } => run_config(subcommand, &mut config),

        CliCommand::Generate {
            dry_run,
            interactive,
            no_commit_number,
        } => {
            config.set_dry_run(dry_run);
            commands::generate::generate(interactive, no_commit_number, &config)
        }

        CliCommand::Initialize { editor, dry_run } => {
            config.set_dry_run(dry_run);
            commands::config::initialize(&editor, &config)
        }

        CliCommand::ListStatus => commands::staging::list_status(),

        CliCommand::Pr(pr_args) => {
            config.set_dry_run(pr_args.dry_run);
            commands::pr::open(&pr_args, &config)
        }

        CliCommand::Push { args, dry_run } => {
            config.set_dry_run(dry_run);
            commands::commit::push(&args, &config)
        }

        CliCommand::Reset {
            files,
            interactive,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            commands::staging::reset(&files, interactive, &config)
        }

        CliCommand::Restore {
            files,
            interactive,
            yes,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            commands::staging::restore(&files, interactive, yes, &config)
        }

        CliCommand::Set { editor, dry_run } => {
            config.set_dry_run(dry_run);
            commands::config::set_editor(&editor, &config)
        }

        CliCommand::Sync {
            source_branch,
            rebase,
            new_branch,
            no_stash,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            commands::sync::sync(
                &source_branch,
                rebase,
                new_branch.as_deref(),
                no_stash,
                &config,
            )
        }
    }
}

/// Dispatches the `config` subcommands.
///
/// # Errors
/// * If the subcommand fails
fn run_config(subcommand: ConfigSubcommand, config: &mut Config) -> Result<()> {
    match subcommand {
        ConfigSubcommand::Create {
            scope,
            exclude,
            dry_run,
        } => {
            config.set_dry_run(dry_run);
            commands::config::create(scope, exclude, config)
        }

        ConfigSubcommand::Which {
            path,
            show_effective,
        } => commands::config::which(path.as_deref(), show_effective),
    }
}

/// Initializes structured logging for the CLI.
///
/// Respects the `RUST_LOG` environment variable; falls back to `debug` when `--verbose` is set
/// and `warn` otherwise. Safe to call once at startup.
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
