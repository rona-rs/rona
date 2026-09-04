//! The commands that create and inspect rona's configuration: `rona config create`,
//! `rona config which`, `rona init`, and `rona set-editor`.

use std::path::{Path, PathBuf};

use crate::{
    cli::ConfigScope,
    config::{ANNOTATED_CONFIG_TEMPLATE, Config, find_config_sources},
    errors::{ConfigError, Result, RonaError},
    git::{add_to_git_exclude, get_top_level_path},
};

/// The project configuration file, at the repository root.
const LOCAL_CONFIG_FILE: &str = ".rona.toml";

/// Creates a local or global configuration file from the annotated template.
///
/// # Arguments
/// * `scope` - Whether to write the project or the global configuration file
/// * `exclude` - Whether to also hide the project file from git (local scope only)
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If the repository root or the home directory cannot be determined
/// * If the file cannot be written
pub(crate) fn create(scope: ConfigScope, exclude: bool, config: &Config) -> Result<()> {
    let config_path = config_path_for(scope)?;

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
            print_exclude_result(scope, false)?;
        }
        return Ok(());
    }

    if config_path.exists() {
        println!(
            "Configuration file already exists at: {}",
            config_path.display()
        );
        println!("Use 'rona set-editor <editor>' to modify the editor setting.");
    } else {
        // The global config lives under ~/.config, which may not exist yet.
        if let Some(parent) = config_path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&config_path, ANNOTATED_CONFIG_TEMPLATE)?;

        println!("Configuration file created at: {}", config_path.display());
        println!("You can now edit this file to customize your settings.");
    }

    if exclude {
        print_exclude_result(scope, true)?;
    }

    Ok(())
}

/// Shows which configuration files would be used from a directory, in loading order.
///
/// # Arguments
/// * `path` - Directory to search from (defaults to the current directory)
/// * `show_effective` - Whether to also print the merged configuration values
///
/// # Errors
/// * If the directory does not exist
/// * If the home directory cannot be determined
pub(crate) fn which(path: Option<&str>, show_effective: bool) -> Result<()> {
    let search_path = match path {
        Some(path) => {
            let path = Path::new(path);
            if !path.exists() {
                return Err(RonaError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Directory not found: {}", path.display()),
                )));
            }
            Some(path)
        }
        None => None,
    };

    let config_info = find_config_sources(search_path)?;

    println!("Searching from: {}", config_info.search_directory.display());
    println!();

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

    if let Some(highest) = active_sources.iter().max_by_key(|s| s.priority) {
        println!();
        println!("Effective config from: {}", highest.path.display());
    }

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

/// Initializes rona's configuration, asking where the file should live.
///
/// # Arguments
/// * `editor` - The editor command to configure
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If the configuration file cannot be created
pub(crate) fn initialize(editor: &str, config: &Config) -> Result<()> {
    if config.dry_run {
        println!("Would create config file with editor: {editor}");
        return Ok(());
    }

    config.create_config_file(editor)
}

/// Changes the editor used to write commit messages.
///
/// # Arguments
/// * `editor` - The editor command to configure
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If the configuration file cannot be updated
pub(crate) fn set_editor(editor: &str, config: &Config) -> Result<()> {
    if config.dry_run {
        println!("Would set editor to: {editor}");
        return Ok(());
    }

    config.set_editor(editor)
}

/// Returns where the configuration file of `scope` lives.
///
/// # Errors
/// * If the repository root (local) or the home directory (global) cannot be determined
fn config_path_for(scope: ConfigScope) -> Result<PathBuf> {
    match scope {
        ConfigScope::Local => Ok(get_top_level_path()?.join(LOCAL_CONFIG_FILE)),
        ConfigScope::Global => {
            let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
            Ok(home.join(".config/rona.toml"))
        }
    }
}

/// Hides the project config from git, or explains why `--exclude` does not apply.
///
/// `apply` is false in dry-run mode, where the outcome is only described.
///
/// # Errors
/// * If the git exclude file cannot be written
fn print_exclude_result(scope: ConfigScope, apply: bool) -> Result<()> {
    match scope {
        ConfigScope::Local => {
            if apply {
                add_to_git_exclude(&[LOCAL_CONFIG_FILE])?;
                println!("Added {LOCAL_CONFIG_FILE} to .git/info/exclude");
            } else {
                println!("Would add {LOCAL_CONFIG_FILE} to .git/info/exclude");
            }
        }
        ConfigScope::Global => println!("--exclude only applies to local scope, ignoring"),
    }

    Ok(())
}
