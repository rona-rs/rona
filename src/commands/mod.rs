//! What each rona command does.
//!
//! [`crate::cli`] owns the command line grammar and dispatches to exactly one function here,
//! so a command's behaviour can be read (and changed) without going through argument parsing.
//! Each module covers one family of commands and is named after it:
//!
//! - [`branch`] - `rona branch`
//! - [`commit`] - `rona commit` and `rona push`
//! - [`config`] - `rona config create`, `rona config which`, `rona init`, `rona set-editor`
//! - [`generate`] - `rona generate`
//! - [`pr`] - `rona pr`
//! - [`staging`] - `rona add-with-exclude`, `rona reset`, `rona restore`, `rona list-status`
//! - [`sync`] - `rona sync`
//!
//! Commands take the values they need plus the loaded [`crate::config::Config`], which carries
//! the `--verbose` and `--dry-run` settings. Every command honours `dry_run` by printing what
//! it would do and changing nothing.

pub(crate) mod branch;
pub(crate) mod commit;
pub(crate) mod config;
pub(crate) mod generate;
pub(crate) mod pr;
pub(crate) mod staging;
pub(crate) mod sync;
