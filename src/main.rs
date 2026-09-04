//! # Rona - Git Workflow Enhancement Tool
//!
//! Rona is a command-line interface tool designed to enhance your Git workflow with powerful features
//! and intuitive commands. It simplifies common Git operations and provides additional functionality
//! for managing commits, files, and repository status.
//!
//! ## Key Features
//!
//! - Intelligent file staging with pattern exclusion
//! - Structured commit message generation
//! - Streamlined push operations
//! - Pull and merge request creation (GitHub, GitLab, Bitbucket)
//! - Interactive commit type selection
//! - Multi-shell completion support
//!
//! ## Usage
//!
//! ```bash
//! # Initialize Rona
//! rona init [editor]
//!
//! # Add files excluding patterns
//! rona -a "*.rs"
//!
//! # Generate commit message
//! rona -g
//!
//! # Commit and push changes
//! rona -c -p
//!
//! # Open a pull request for the current branch
//! rona pr
//! ```
//!
//! For more detailed examples and usage instructions, see the [README.md](../README.md) file.
//!
//! # Architecture
//!
//! The application is organized into several modules:
//! - `cli`: Command-line grammar, shell completion, and dispatch
//! - `commands`: What each command does, one module per family of commands
//! - `config`: Manages application configuration
//! - `errors`: Error handling and custom error types
//! - `extra_fields`: Config-declared prompt fields and their prefetching
//! - `git`: Organized Git-related functionality with focused submodules
//! - `pr`: Pull and merge request creation across forges
//! - `prompt`: Interactive prompts shared by the commands
//! - `template`: Commit, branch, and request title template rendering
//! - `theme`: Custom theme for command-line output
//! - `utils`: Common utility functions
//!
//! # Error Handling
//!
//! The application implements a two-tier error handling approach:
//! 1. Initial Git repository validation
//! 2. Main application logic error handling through `Result` types
//!

pub mod cli;
pub mod commands;
pub mod config;
pub mod errors;
pub mod extra_fields;
pub mod git;
pub mod pr;
pub mod prompt;
pub mod template;
pub mod theme;
pub mod utils;

use cli::run;
use errors::Result;
use std::process::exit;

fn main() {
    if let Err(e) = inner_main() {
        // Handle user cancellation gracefully with a friendly message
        if matches!(e, errors::RonaError::UserCancelled) {
            println!("\nBye from Rona!");
            exit(0);
        }

        eprintln!("{e}");
        exit(1);
    }
}

#[doc(hidden)]
fn inner_main() -> Result<()> {
    run()?;

    Ok(())
}
