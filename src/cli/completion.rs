//! `rona completion <shell>`: prints a completion script for the command line grammar.
//!
//! The script is generated from the clap command in [`super::args`], so it stays in step with
//! the flags automatically. Fish gets a few extra rules on top, because the commands that take
//! file arguments complete best from `rona -l` (the files git status reports).

use std::io;

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use super::args::Cli;

/// Prints the completion script for `shell` on stdout.
pub(crate) fn print(shell: Shell) {
    let mut command = Cli::command();
    generate(shell, &mut command, "rona", &mut io::stdout());

    if matches!(shell, Shell::Fish) {
        print_fish_extras();
    }
}

/// Prints the fish rules that complete file arguments from git status.
fn print_fish_extras() {
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
