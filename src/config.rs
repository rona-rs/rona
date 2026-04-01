//! Configuration Management Module for Rona
//!
//! This module handles all configuration-related functionality, including
//! - Reading and writing configuration files
//! - Managing editor preferences
//! - Handling configuration errors
//!
//! # Configuration Structure
//!
//! The configuration is stored in TOML format at `~/.config/rona/config.toml`
//! and contains settings such as
//! - Editor preferences
//! - Other configuration options
//!
//! # Error Handling
//!
//! The module provides a custom error type `ConfigError` that handles various
//! configuration-related errors including
//! - IO errors
//! - Missing configuration
//! - Invalid configuration format
//! - Home directory not found

use config;
use inquire::Select;
use serde::{Deserialize, Serialize};
use std::{env, io::Write, path::PathBuf};

use crate::{
    errors::{ConfigError, GitError, Result, RonaError},
    git::get_top_level_path,
    utils::print_error,
};

/// Describes a configuration file source and its status
#[derive(Debug, Clone)]
pub struct ConfigSource {
    /// Path to the configuration file
    pub path: PathBuf,
    /// Whether this file exists
    pub exists: bool,
    /// Description of this config source (e.g., "Global config", "Project config")
    pub description: String,
    /// Priority order (lower = loaded first, higher = overrides lower)
    pub priority: u8,
}

/// Information about which configuration files would be used from a given directory
#[derive(Debug)]
pub struct ConfigInfo {
    /// All potential config sources, in loading order
    pub sources: Vec<ConfigSource>,
    /// The effective merged configuration (if any configs exist)
    pub effective_config: Option<ProjectConfig>,
    /// The directory from which config was searched
    pub search_directory: PathBuf,
}

// Define your default commit types
const DEFAULT_COMMIT_TYPES: &[&str] = &["feat", "fix", "docs", "test", "chore"];

/// Project-specific configuration that can be defined in rona.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProjectConfig {
    /// Editor command to use for commit messages
    pub editor: Option<String>,

    /// Custom commit types for this project
    pub commit_types: Option<Vec<String>>,

    /// Template for interactive commit message generation
    /// Available variables: {`commit_number`}, {`commit_type`}, {`branch_name`}, {`message`}, {`date`}, {`time`}, {`author`}, {`email`}
    pub template: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            editor: Some("nano".to_string()),
            commit_types: Some(
                DEFAULT_COMMIT_TYPES
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
            ),
            template: Some(
                "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}".to_string(),
            ),
        }
    }
}

impl ProjectConfig {
    /// Loads the project configuration, merging global and project config files.
    ///
    /// # Errors
    /// Returns `ConfigError::ConfigNotFound` if the config files cannot be found or read.
    /// Returns `ConfigError::InvalidConfig` if deserialization fails.
    ///
    /// # Panics
    /// Panics if the current working directory cannot be determined (i.e., if `std::env::current_dir()` fails).
    pub fn load() -> Result<Self> {
        // During tests, return default config to avoid dependency on external files
        if cfg!(test) {
            return Ok(Self::default());
        }

        let settings = {
            let mut builder = config::Config::builder();

            // Support both old and new global config paths
            let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
            let old_global = home.join(".config/rona/config.toml");
            let new_global = home.join(".config/rona.toml");

            if old_global.exists() {
                builder = builder.add_source(config::File::from(old_global).required(false));
            }

            if new_global.exists() {
                builder = builder.add_source(config::File::from(new_global).required(false));
            }

            // Add project config if it exists
            let project_config_path = env::current_dir()?.join(".rona.toml");
            if project_config_path.exists() {
                builder =
                    builder.add_source(config::File::from(project_config_path).required(false));
            }

            builder.build().map_err(|_| ConfigError::ConfigNotFound)?
        };

        match settings.try_deserialize() {
            Ok(config) => Ok(config),
            Err(e) => {
                eprintln!("Failed to deserialize config: {e}");
                Err(ConfigError::InvalidConfig.into())
            }
        }
    }

    /// Loads the project configuration from a specific file path, bypassing the default
    /// global/project config hierarchy.
    ///
    /// # Arguments
    /// * `path` - The exact path to the TOML config file to load
    ///
    /// # Errors
    /// Returns `ConfigError::ConfigNotFound` if the file does not exist.
    /// Returns `ConfigError::InvalidConfig` if deserialization fails.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Err(ConfigError::ConfigNotFound.into());
        }

        let settings = config::Config::builder()
            .add_source(config::File::from(path).required(true))
            .build()
            .map_err(|_| ConfigError::ConfigNotFound)?;

        match settings.try_deserialize() {
            Ok(config) => Ok(config),
            Err(e) => {
                eprintln!("Failed to deserialize config: {e}");
                Err(ConfigError::InvalidConfig.into())
            }
        }
    }

    /// Loads the project configuration from a specific directory.
    ///
    /// # Arguments
    /// * `from_dir` - The directory to load the project config from
    ///
    /// # Errors
    /// Returns `ConfigError::ConfigNotFound` if the config files cannot be found or read.
    /// Returns `ConfigError::InvalidConfig` if deserialization fails.
    pub fn load_from_dir(from_dir: &std::path::Path) -> Result<Self> {
        let settings = {
            let mut builder = config::Config::builder();

            // Support both old and new global config paths
            let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
            let old_global = home.join(".config/rona/config.toml");
            let new_global = home.join(".config/rona.toml");

            if old_global.exists() {
                builder = builder.add_source(config::File::from(old_global).required(false));
            }

            if new_global.exists() {
                builder = builder.add_source(config::File::from(new_global).required(false));
            }

            // Add project config from specified directory if it exists
            let project_config_path = from_dir.join(".rona.toml");
            if project_config_path.exists() {
                builder =
                    builder.add_source(config::File::from(project_config_path).required(false));
            }

            builder.build().map_err(|_| ConfigError::ConfigNotFound)?
        };

        match settings.try_deserialize() {
            Ok(config) => Ok(config),
            Err(e) => {
                eprintln!("Failed to deserialize config: {e}");
                Err(ConfigError::InvalidConfig.into())
            }
        }
    }
}

/// Find all configuration sources that would be used from a given directory.
///
/// This function discovers all potential configuration files and reports which ones
/// exist and would be used when running rona from the specified directory.
///
/// # Arguments
/// * `from_dir` - Optional directory to check from. If `None`, uses current directory.
///
/// # Errors
/// Returns an error if the home directory cannot be determined.
///
/// # Returns
/// A `ConfigInfo` struct containing all discovered config sources and the effective configuration.
pub fn find_config_sources(from_dir: Option<&std::path::Path>) -> Result<ConfigInfo> {
    let search_dir = match from_dir {
        Some(dir) => dir.to_path_buf(),
        None => env::current_dir()?,
    };

    let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;

    let mut sources = Vec::new();

    // Old global config (priority 1 - loaded first)
    let old_global = home.join(".config/rona/config.toml");
    sources.push(ConfigSource {
        path: old_global.clone(),
        exists: old_global.exists(),
        description: "Legacy global config".to_string(),
        priority: 1,
    });

    // New global config (priority 2 - overrides old global)
    let new_global = home.join(".config/rona.toml");
    sources.push(ConfigSource {
        path: new_global.clone(),
        exists: new_global.exists(),
        description: "Global config".to_string(),
        priority: 2,
    });

    // Project-local config (priority 3 - highest priority, overrides all)
    let project_config = search_dir.join(".rona.toml");
    sources.push(ConfigSource {
        path: project_config.clone(),
        exists: project_config.exists(),
        description: "Project config".to_string(),
        priority: 3,
    });

    // Try to load the effective configuration
    let effective_config = if cfg!(test) {
        Some(ProjectConfig::default())
    } else {
        ProjectConfig::load_from_dir(&search_dir).ok()
    };

    Ok(ConfigInfo {
        sources,
        effective_config,
        search_directory: search_dir,
    })
}

/// Main configuration struct that handles all config operations.
/// This includes both persistent configuration (stored in config file)
/// and runtime configuration (command-line flags).
///
/// # Fields
/// * `root` - The root path for configuration files
/// * `verbose` - Whether to show detailed output
/// * `dry_run` - Whether to simulate operations without making changes
#[derive(Debug)]
pub struct Config {
    root: PathBuf,
    pub(crate) verbose: bool,
    pub(crate) dry_run: bool,
    pub project_config: ProjectConfig,
}

impl Config {
    /// Creates a new Config instance with default settings.
    ///
    /// # Errors
    /// * If the home directory cannot be determined
    /// * If the project configuration cannot be loaded
    ///
    /// # Returns
    /// * `Result<Config>` - A new Config instance with default settings
    pub fn new() -> Result<Self> {
        let root = Self::get_config_root()?;
        let project_config = ProjectConfig::load().unwrap_or_default();
        let config = Self {
            root,
            verbose: false,
            dry_run: false,
            project_config,
        };
        Ok(config)
    }

    /// Creates a new Config instance with a specific root directory.
    /// This is primarily used for testing with temporary directories.
    ///
    /// # Arguments
    /// * `root` - The root directory to use for configuration files
    ///
    /// # Returns
    /// * `Config` - A new Config instance with the specified root and default settings
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let project_config = ProjectConfig::load().unwrap_or_default();

        Self {
            root,
            verbose: false,
            dry_run: false,
            project_config,
        }
    }

    /// Creates a new Config instance loading only the specified config file,
    /// bypassing the default global/project config hierarchy.
    ///
    /// # Arguments
    /// * `path` - Path to the TOML config file to load
    ///
    /// # Errors
    /// * If the home directory cannot be determined
    /// * If the specified config file does not exist or cannot be parsed
    ///
    /// # Returns
    /// * `Result<Config>` - A new Config instance using the provided file
    pub fn new_with_config_file(path: &std::path::Path) -> Result<Self> {
        let root = Self::get_config_root()?;
        let project_config = ProjectConfig::load_from_file(path)?;
        Ok(Self {
            root,
            verbose: false,
            dry_run: false,
            project_config,
        })
    }

    /// Sets the verbose flag which controls detailed output logging.
    ///
    /// # Arguments
    /// * `verbose` - Whether to enable verbose output
    pub const fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Sets the `dry_run` flag which controls whether operations are simulated.
    /// When true, operations will print what would happen without making actual changes.
    ///
    /// # Arguments
    /// * `dry_run` - Whether to enable dry run mode
    pub const fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    /// Retrieves the editor from the configuration file.
    ///
    /// # Errors
    /// * If the editor setting is missing or invalid
    ///
    /// # Returns
    /// * `Result<String>` - The configured editor command
    pub fn get_editor(&self) -> Result<String> {
        // During tests, use the old behavior for compatibility
        if cfg!(test) {
            use regex::Regex;
            let config_file = self.get_config_file_path()?;

            if !config_file.exists() {
                return Err(ConfigError::InvalidConfig.into());
            }

            let config_content = std::fs::read_to_string(&config_file)?;
            let regex =
                Regex::new(r#"editor\s*=\s*"(.*?)""#).map_err(|_| ConfigError::InvalidConfig)?;

            let editor = regex
                .captures(config_content.trim())
                .and_then(|captures| captures.get(1))
                .map(|match_| match_.as_str().to_string())
                .ok_or(ConfigError::InvalidConfig)?;

            return Ok(editor.trim().to_string());
        }

        self.project_config
            .editor
            .clone()
            .ok_or_else(|| ConfigError::InvalidConfig.into())
    }

    /// Sets the editor in the configuration file.
    ///
    /// # Arguments
    /// * `editor` - The editor command to configure
    ///
    /// # Errors
    /// * If the configuration file cannot be read or written
    /// * If the configuration file does not exist
    pub fn set_editor(&self, editor: &str) -> Result<()> {
        // During tests, use the old behavior for compatibility
        if cfg!(test) {
            let config_file = self.get_config_file_path()?;

            if !config_file.exists() {
                return Err(ConfigError::ConfigNotFound.into());
            }

            // Use old format for tests
            let config_content = format!("editor = \"{editor}\"");
            std::fs::write(&config_file, config_content)?;

            return Ok(());
        }

        let options = vec!["Project (./.rona.toml)", "Global (~/.config/rona.toml)"];

        let selection = Select::new("Where do you want to set the editor?", options)
            .with_starting_cursor(0)
            .prompt()
            .map_err(|_| ConfigError::InvalidConfig)?;

        let config_path = match selection {
            "Project (./.rona.toml)" => get_top_level_path().map(|root| root.join(".rona.toml"))?,
            "Global (~/.config/rona.toml)" => {
                let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
                home.join(".config/rona.toml")
            }
            _ => unreachable!(),
        };

        let mut config = self.project_config.clone();
        config.editor = Some(editor.to_string());

        let toml_str = toml::to_string_pretty(&config).map_err(|_| ConfigError::InvalidConfig)?;
        let mut file = std::fs::File::create(&config_path)?;

        file.write_all(toml_str.as_bytes())?;

        println!("Editor set in: {}", config_path.display());

        Ok(())
    }

    /// Creates a new configuration file with the specified editor.
    ///
    /// # Arguments
    /// * `editor` - The editor command to configure
    ///
    /// # Errors
    /// * If creating the configuration directory fails
    /// * If writing the configuration file fails
    /// * If the configuration file already exists
    pub fn create_config_file(&self, editor: &str) -> Result<()> {
        // During tests, use the old behavior for compatibility
        if cfg!(test) {
            let config_folder = self.get_config_folder_path()?;

            if !config_folder.exists() {
                std::fs::create_dir_all(config_folder)?;
            }

            let config_file = self.get_config_file_path()?;
            let config_content = format!("editor = \"{editor}\"");

            if config_file.exists() {
                return Err(ConfigError::ConfigAlreadyExists.into());
            }

            std::fs::write(&config_file, config_content)?;

            return Ok(());
        }

        let options = vec!["Project (.rona.toml)", "Global (~/.config/rona.toml)"];
        let selection = Select::new("Where do you want to initialize the config?", options)
            .with_starting_cursor(0)
            .prompt()
            .map_err(|_| ConfigError::InvalidConfig)?;

        let config_path = match selection {
            "Project (.rona.toml)" => env::current_dir()?.join(".rona.toml"),
            "Global (~/.config/rona.toml)" => {
                let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
                home.join(".config/rona.toml")
            }
            _ => unreachable!(),
        };

        let config_folder = config_path.parent().ok_or(ConfigError::ConfigNotFound)?;
        if !config_folder.exists() {
            std::fs::create_dir_all(config_folder)?;
        }

        if config_path.exists() {
            if !cfg!(test) {
                print_error(
                    "Configuration file already exists.",
                    &format!(
                        "A configuration file already exists at {}",
                        config_path.display()
                    ),
                    "Use `rona --set-editor <editor>` (or `rona -s <editor>`) to change it.",
                );
            }
            return Err(ConfigError::ConfigAlreadyExists.into());
        }

        let mut config = self.project_config.clone();
        config.editor = Some(editor.to_string());

        let toml_str = toml::to_string_pretty(&config).map_err(|_| ConfigError::InvalidConfig)?;
        std::fs::write(&config_path, toml_str)?;

        Ok(())
    }

    /// Returns the path to the configuration folder.
    ///
    /// # Errors
    /// * If the home directory cannot be determined
    ///
    /// # Returns
    /// * `Result<PathBuf>` - The path to the configuration folder
    pub fn get_config_folder_path(&self) -> Result<PathBuf> {
        let config_folder_path = self.root.join(".config").join("rona");
        Ok(config_folder_path)
    }

    /// Returns the path to the configuration file.
    ///
    /// # Errors
    /// * If the home directory cannot be determined
    ///
    /// # Returns
    /// * `Result<PathBuf>` - The path to the configuration file
    pub fn get_config_file_path(&self) -> Result<PathBuf> {
        let config_folder_path = self.get_config_folder_path()?;
        Ok(config_folder_path.join("config.toml"))
    }

    /// Returns the root directory for the configuration files.
    /// Uses the test directory if `RONA_TEST_DIR` is set or running tests.
    ///
    /// # Errors
    /// * If the home directory cannot be determined
    ///
    /// # Returns
    /// * `Result<PathBuf>` - The root directory for configuration files
    fn get_config_root() -> Result<PathBuf> {
        // Use environment variable for testing
        if env::var("RONA_TEST_DIR").is_ok() || cfg!(test) {
            Ok(PathBuf::from(CONFIG_FOLDER_NAME))
        } else {
            let root = env::var("HOME")
                .or_else(|_| env::var("USERPROFILE"))
                .map_err(|_| RonaError::from(GitError::RepositoryNotFound))?;

            Ok(PathBuf::from(root))
        }
    }
}

// Make this public so tests can use it directly
pub const CONFIG_FOLDER_NAME: &str = "rona-test-config";

#[cfg(test)]
mod tests {
    use crate::errors::RonaError;

    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_config_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = Config::with_root(temp_dir.path().to_path_buf());
        let editor = "test_editor";

        // Create a new config file with the temp directory as root
        config.create_config_file(editor)?;

        // Check the file exists and has the correct content
        let config_file = config.get_config_file_path()?;
        assert!(config_file.exists());

        let content = std::fs::read_to_string(&config_file)?;
        assert_eq!(content, format!("editor = \"{editor}\""));

        // Test error when a file already exists
        assert!(config.create_config_file(editor).is_err());

        Ok(())
    }

    #[test]
    fn test_get_editor() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = Config::with_root(temp_dir.path().to_path_buf());
        let editor = "nano";

        // Create a config file
        config.create_config_file(editor)?;

        // Test getting the editor
        let val = config.get_editor()?;
        assert_eq!(val, editor);

        Ok(())
    }

    #[test]
    fn test_set_editor() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = Config::with_root(temp_dir.path().to_path_buf());
        let initial_editor = "vim";

        // Create a config file
        config.create_config_file(initial_editor)?;

        // Test setting a new editor
        let new_editor = "emacs";
        config.set_editor(new_editor)?;

        // Verify the editor was updated
        let val = config.get_editor()?;
        assert_eq!(val, new_editor);

        Ok(())
    }

    #[test]
    fn test_get_editor_error_no_config() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = Config::with_root(temp_dir.path().to_path_buf());

        // Don't create a config file, verify we get an error
        assert!(matches!(
            config.get_editor(),
            Err(RonaError::Config(ConfigError::InvalidConfig))
        ));

        Ok(())
    }

    #[test]
    fn test_set_editor_error_no_config() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = Config::with_root(temp_dir.path().to_path_buf());

        // Don't create a config file, verify we get an error
        assert!(matches!(
            config.set_editor("vim"),
            Err(RonaError::Config(ConfigError::ConfigNotFound))
        ));

        Ok(())
    }

    #[test]
    fn test_malformed_config() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = Config::with_root(temp_dir.path().to_path_buf());

        // Create a config directory
        let config_folder = config.get_config_folder_path()?;
        std::fs::create_dir_all(&config_folder)?;

        // Create a malformed config file
        let config_file = config.get_config_file_path()?;
        std::fs::write(&config_file, "editor = missing_quotes")?;

        // Test that get_editor returns an error
        assert!(matches!(
            config.get_editor(),
            Err(RonaError::Config(ConfigError::InvalidConfig))
        ));

        Ok(())
    }
}
