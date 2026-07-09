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

use dialoguer::FuzzySelect;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    io::Write,
    path::{Path, PathBuf},
};

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
    /// Extra field names defined in `commit_extra_fields` are also available.
    pub commit_template: Option<String>,

    /// Extra fields to prompt after commit type and before the message.
    /// Each field becomes a template variable with the field's `name`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commit_extra_fields: Vec<crate::extra_fields::ExtraField>,

    /// Controls the order of prompts in interactive mode.
    /// Use the reserved name `"message"` to position the built-in message prompt.
    /// Extra fields not listed are appended after all listed items.
    /// When empty (the default), extra fields are shown first, then `message`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commit_fields_order: Vec<String>,

    /// Template for branch name generation.
    /// Available variables: `{commit_type}`, `{description}`, `{date}`, `{time}`, `{author}`.
    /// Extra field names defined in `branch_extra_fields` are also available.
    pub branch_template: Option<String>,

    /// Extra fields to prompt when generating a branch name.
    /// Each field becomes a template variable with the field's `name`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_extra_fields: Vec<crate::extra_fields::ExtraField>,

    /// Controls the order of prompts for branch name generation.
    /// Use the reserved name `"description"` to position the built-in description prompt.
    /// Extra fields not listed are appended after all listed items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_field_order: Vec<String>,

    /// Dedicated commit types shown in the `rona branch` type selector.
    /// When absent, `commit_types` is used instead.
    pub branch_types: Option<Vec<String>>,

    /// When `true` and `branch_types` is set, the selector for `rona branch` shows
    /// `branch_types` followed by any `commit_types` not already present in it.
    /// When `false` (default), only `branch_types` is shown.
    #[serde(default)]
    pub merge_branch_and_commit_types: bool,

    /// Optional prefetch configuration for the built-in message prompt.
    /// Extracts a value from a source and optionally renders it through a template
    /// using `{extract}` as a placeholder. The result is offered as the default;
    /// pressing Enter without typing accepts it.
    pub message_prefetch: Option<crate::extra_fields::MessagePrefetchConfig>,

    /// Optional overrides for the built-in commit message prompt.
    /// Set `disabled = true` to skip the prompt (the `{message}` variable will be empty).
    pub commit_message: Option<crate::extra_fields::BuiltInFieldConfig>,

    /// Optional overrides for the built-in branch description prompt.
    /// Set `disabled = true` to skip the prompt (the `{description}` variable will be empty).
    pub branch_description: Option<crate::extra_fields::BuiltInFieldConfig>,
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
            commit_template: Some(
                "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}".to_string(),
            ),
            commit_extra_fields: vec![],
            commit_fields_order: vec![],
            branch_template: Some("{branch_type}/{description}".to_string()),
            branch_extra_fields: vec![],
            branch_field_order: vec![],
            branch_types: None,
            merge_branch_and_commit_types: false,
            message_prefetch: None,
            commit_message: None,
            branch_description: None,
        }
    }
}

/// Intermediate deserialization target that accepts both current and legacy field names.
/// All array and bool fields are `Option` so absent keys are distinguishable from explicit
/// empty values, enabling name-keyed merging across config files.
#[derive(serde::Deserialize, Default)]
struct RawProjectConfig {
    editor: Option<String>,
    commit_types: Option<Vec<String>>,
    commit_template: Option<String>,
    template: Option<String>,
    commit_extra_fields: Option<Vec<crate::extra_fields::ExtraField>>,
    extra_fields: Option<Vec<crate::extra_fields::ExtraField>>,
    /// Current name.
    commit_fields_order: Option<Vec<String>>,
    /// Legacy alias for `commit_fields_order`.
    field_order: Option<Vec<String>>,
    branch_template: Option<String>,
    branch_extra_fields: Option<Vec<crate::extra_fields::ExtraField>>,
    branch_field_order: Option<Vec<String>>,
    branch_types: Option<Vec<String>>,
    merge_branch_and_commit_types: Option<bool>,
    message_prefetch: Option<crate::extra_fields::MessagePrefetchConfig>,
    commit_message: Option<crate::extra_fields::BuiltInFieldConfig>,
    branch_description: Option<crate::extra_fields::BuiltInFieldConfig>,
}

impl From<RawProjectConfig> for ProjectConfig {
    fn from(raw: RawProjectConfig) -> Self {
        Self {
            editor: raw.editor,
            commit_types: raw.commit_types,
            commit_template: raw.commit_template,
            commit_extra_fields: raw.commit_extra_fields.unwrap_or_default(),
            commit_fields_order: raw.commit_fields_order.unwrap_or_default(),
            branch_template: raw.branch_template,
            branch_extra_fields: raw.branch_extra_fields.unwrap_or_default(),
            branch_field_order: raw.branch_field_order.unwrap_or_default(),
            branch_types: raw.branch_types,
            merge_branch_and_commit_types: raw.merge_branch_and_commit_types.unwrap_or(false),
            message_prefetch: raw.message_prefetch,
            commit_message: raw.commit_message,
            branch_description: raw.branch_description,
        }
    }
}

/// Resolves backward-compat aliases within a single raw config.
/// `template` → `commit_template`, `extra_fields` → `commit_extra_fields`,
/// `field_order` → `commit_fields_order`.
fn normalize_raw(mut raw: RawProjectConfig) -> RawProjectConfig {
    if raw.commit_template.is_none() {
        raw.commit_template = raw.template.take();
    }
    raw.template = None;
    if raw.commit_extra_fields.is_none() {
        raw.commit_extra_fields = raw.extra_fields.take();
    }
    raw.extra_fields = None;
    if raw.commit_fields_order.is_none() {
        raw.commit_fields_order = raw.field_order.take();
    }
    raw.field_order = None;
    raw
}

/// Merges two `Option<Vec<ExtraField>>` by field name.
/// Child entries override same-named base entries; new child entries are appended.
fn merge_named_fields(
    base: Option<Vec<crate::extra_fields::ExtraField>>,
    child: Option<Vec<crate::extra_fields::ExtraField>>,
) -> Option<Vec<crate::extra_fields::ExtraField>> {
    match (base, child) {
        (None, c) => c,
        (b, None) => b,
        (Some(mut base_fields), Some(child_fields)) => {
            for child_field in child_fields {
                if let Some(existing) = base_fields.iter_mut().find(|f| f.name == child_field.name)
                {
                    *existing = child_field;
                } else {
                    base_fields.push(child_field);
                }
            }
            Some(base_fields)
        }
    }
}

/// Merges two raw configs: scalars use last-wins (child overrides base),
/// array fields (`commit_extra_fields`, `branch_extra_fields`) are merged by name.
fn merge_raw(base: RawProjectConfig, child: RawProjectConfig) -> RawProjectConfig {
    RawProjectConfig {
        editor: child.editor.or(base.editor),
        commit_types: child.commit_types.or(base.commit_types),
        commit_template: child.commit_template.or(base.commit_template),
        template: None,
        commit_extra_fields: merge_named_fields(
            base.commit_extra_fields,
            child.commit_extra_fields,
        ),
        extra_fields: None,
        commit_fields_order: child.commit_fields_order.or(base.commit_fields_order),
        field_order: None,
        branch_template: child.branch_template.or(base.branch_template),
        branch_extra_fields: merge_named_fields(
            base.branch_extra_fields,
            child.branch_extra_fields,
        ),
        branch_field_order: child.branch_field_order.or(base.branch_field_order),
        branch_types: child.branch_types.or(base.branch_types),
        merge_branch_and_commit_types: child
            .merge_branch_and_commit_types
            .or(base.merge_branch_and_commit_types),
        message_prefetch: child.message_prefetch.or(base.message_prefetch),
        commit_message: child.commit_message.or(base.commit_message),
        branch_description: child.branch_description.or(base.branch_description),
    }
}

/// Parses a single TOML config file into a `RawProjectConfig`.
fn load_single_raw_file(path: &Path) -> Result<RawProjectConfig> {
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| {
        RonaError::Config(ConfigError::ParseError {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    })
}

/// Loads an ordered list of config files (base-first) and folds them with `merge_raw`.
/// Files that do not exist are silently skipped.
fn load_and_merge_files(paths: &[PathBuf]) -> Result<RawProjectConfig> {
    let mut result = RawProjectConfig::default();
    for path in paths {
        if path.exists() {
            let raw = normalize_raw(load_single_raw_file(path)?);
            result = merge_raw(result, raw);
        }
    }
    Ok(result)
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

        let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
        let old_global = home.join(".config/rona/config.toml");
        let new_global = home.join(".config/rona.toml");

        let mut paths: Vec<PathBuf> = Vec::new();
        if old_global.exists() {
            paths.push(old_global);
        }
        if new_global.exists() {
            paths.push(new_global);
        }

        let project_config_path = env::current_dir()?.join(".rona.toml");
        if project_config_path.exists() {
            let mut visited = HashSet::new();
            paths.extend(collect_extends_chain(&project_config_path, &mut visited)?);
            paths.push(project_config_path);
        }

        load_and_merge_files(&paths).map(Into::into).map_err(|e| {
            eprintln!("Failed to deserialize config: {e}");
            e
        })
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

        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        let mut visited = HashSet::new();
        let mut paths: Vec<PathBuf> = collect_extends_chain(&abs_path, &mut visited)?;
        paths.push(abs_path);

        load_and_merge_files(&paths).map(Into::into)
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
        let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
        let old_global = home.join(".config/rona/config.toml");
        let new_global = home.join(".config/rona.toml");

        let mut paths: Vec<PathBuf> = Vec::new();
        if old_global.exists() {
            paths.push(old_global);
        }
        if new_global.exists() {
            paths.push(new_global);
        }

        let project_config_path = from_dir.join(".rona.toml");
        if project_config_path.exists() {
            let mut visited = HashSet::new();
            paths.extend(collect_extends_chain(&project_config_path, &mut visited)?);
            paths.push(project_config_path);
        }

        load_and_merge_files(&paths).map(Into::into).map_err(|e| {
            eprintln!("Failed to deserialize config: {e}");
            e
        })
    }
}

/// Peeks at the `extends` key of a TOML config file without full deserialization.
#[derive(Deserialize)]
struct ExtendsOnly {
    extends: Option<String>,
}

/// Resolves an `extends` path relative to the config file that declares it.
/// A leading `~/` is expanded to the user's home directory before any other resolution.
fn resolve_extends_path(extends_value: &str, declaring_config: &Path) -> PathBuf {
    let expanded = extends_value.strip_prefix("~/").map_or_else(
        || PathBuf::from(extends_value),
        |rest| dirs::home_dir().map_or_else(|| PathBuf::from(extends_value), |h| h.join(rest)),
    );

    if expanded.is_absolute() {
        expanded
    } else {
        declaring_config
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(expanded)
    }
}

/// Collects the ordered list of config files implied by `extends` chains.
///
/// Returns files in base-first order (deepest ancestor first), so they can be
/// added to the `config` builder before the file that declared the chain -- meaning
/// each file overrides its ancestors.
///
/// Cycle detection uses canonical paths so that symlinks are handled correctly.
fn collect_extends_chain(
    config_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<Vec<PathBuf>> {
    let canonical = config_path
        .canonicalize()
        .unwrap_or_else(|_| config_path.to_path_buf());

    if !visited.insert(canonical) {
        return Err(ConfigError::CircularExtends {
            path: config_path.display().to_string(),
        }
        .into());
    }

    if !config_path.exists() {
        return Err(ConfigError::ExtendsNotFound {
            path: config_path.display().to_string(),
        }
        .into());
    }

    let content = std::fs::read_to_string(config_path)?;
    let extends_only: ExtendsOnly =
        toml::from_str(&content).unwrap_or(ExtendsOnly { extends: None });

    let Some(extends_str) = extends_only.extends else {
        return Ok(vec![]);
    };

    let extended_path = resolve_extends_path(&extends_str, config_path);

    let mut chain = collect_extends_chain(&extended_path, visited)?;
    chain.push(extended_path);
    Ok(chain)
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

    // Extended configs (priority 3 - between global and project, base-first)
    let project_config = search_dir.join(".rona.toml");
    if project_config.exists() {
        let chain = collect_extends_chain(&project_config, &mut HashSet::new()).unwrap_or_default();
        for (i, extended_path) in chain.iter().enumerate() {
            sources.push(ConfigSource {
                path: extended_path.clone(),
                exists: extended_path.exists(),
                description: format!("Extended config ({})", i + 1),
                priority: 3,
            });
        }
    }

    // Project-local config (priority 4 - highest priority, overrides all)
    sources.push(ConfigSource {
        path: project_config.clone(),
        exists: project_config.exists(),
        description: "Project config".to_string(),
        priority: 4,
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

        let index = FuzzySelect::with_theme(&crate::theme::prompt_theme())
            .with_prompt("Where do you want to set the editor?")
            .items(&options)
            .default(0)
            .interact_opt()
            .map_err(|_| ConfigError::InvalidConfig)?
            .ok_or(ConfigError::InvalidConfig)?;

        let config_path = match options[index] {
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
        let index = FuzzySelect::with_theme(&crate::theme::prompt_theme())
            .with_prompt("Where do you want to initialize the config?")
            .items(&options)
            .default(0)
            .interact_opt()
            .map_err(|_| ConfigError::InvalidConfig)?
            .ok_or(ConfigError::InvalidConfig)?;

        let config_path = match options[index] {
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

    #[test]
    fn test_extends_basic() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base = temp_dir.path().join("base.toml");
        let project = temp_dir.path().join(".rona.toml");

        std::fs::write(&base, r#"editor = "vim""#)?;
        std::fs::write(
            &project,
            format!(r#"extends = "base.toml"{}"#, "\ncommit_types = [\"feat\"]"),
        )?;

        let cfg = ProjectConfig::load_from_file(&project)?;
        assert_eq!(cfg.editor.as_deref(), Some("vim"));
        assert_eq!(
            cfg.commit_types.as_deref(),
            Some(["feat".to_string()].as_slice())
        );

        Ok(())
    }

    #[test]
    fn test_extends_override() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base = temp_dir.path().join("base.toml");
        let project = temp_dir.path().join(".rona.toml");

        std::fs::write(&base, r#"editor = "vim""#)?;
        std::fs::write(
            &project,
            format!(r#"extends = "base.toml"{}"#, "\neditor = \"nano\""),
        )?;

        let cfg = ProjectConfig::load_from_file(&project)?;
        // project file overrides the extended base
        assert_eq!(cfg.editor.as_deref(), Some("nano"));

        Ok(())
    }

    #[test]
    fn test_extends_chain() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let grandparent = temp_dir.path().join("grandparent.toml");
        let parent = temp_dir.path().join("parent.toml");
        let project = temp_dir.path().join(".rona.toml");

        std::fs::write(&grandparent, r#"editor = "vim""#)?;
        std::fs::write(&parent, r#"extends = "grandparent.toml""#)?;
        std::fs::write(
            &project,
            format!(r#"extends = "parent.toml"{}"#, "\ncommit_types = [\"fix\"]"),
        )?;

        let cfg = ProjectConfig::load_from_file(&project)?;
        assert_eq!(cfg.editor.as_deref(), Some("vim"));
        assert_eq!(
            cfg.commit_types.as_deref(),
            Some(["fix".to_string()].as_slice())
        );

        Ok(())
    }

    #[test]
    fn test_extends_missing_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project = temp_dir.path().join(".rona.toml");

        std::fs::write(&project, r#"extends = "nonexistent.toml""#)?;

        let result = ProjectConfig::load_from_file(&project);
        assert!(
            matches!(
                result,
                Err(RonaError::Config(ConfigError::ExtendsNotFound { .. }))
            ),
            "expected ExtendsNotFound, got {result:?}"
        );

        Ok(())
    }

    #[test]
    fn test_extends_circular() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let a = temp_dir.path().join("a.toml");
        let b = temp_dir.path().join("b.toml");

        std::fs::write(&a, r#"extends = "b.toml""#)?;
        std::fs::write(&b, r#"extends = "a.toml""#)?;

        let result = ProjectConfig::load_from_file(&a);
        assert!(
            matches!(
                result,
                Err(RonaError::Config(ConfigError::CircularExtends { .. }))
            ),
            "expected CircularExtends, got {result:?}"
        );

        Ok(())
    }

    #[test]
    fn test_extra_fields_merged_by_name() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base = temp_dir.path().join("base.toml");
        let project = temp_dir.path().join(".rona.toml");

        std::fs::write(
            &base,
            r#"
[[commit_extra_fields]]
name = "scope"
prompt = "Scope (base)"

[[commit_extra_fields]]
name = "ticket"
prompt = "Ticket"
"#,
        )?;

        std::fs::write(
            &project,
            r#"
extends = "base.toml"

[[commit_extra_fields]]
name = "scope"
prompt = "Scope (project)"
"#,
        )?;

        let cfg = ProjectConfig::load_from_file(&project)?;
        assert_eq!(
            cfg.commit_extra_fields.len(),
            2,
            "both fields should be present"
        );

        let scope_prompt = cfg
            .commit_extra_fields
            .iter()
            .find(|f| f.name == "scope")
            .and_then(|f| f.prompt.as_deref());
        let ticket = cfg.commit_extra_fields.iter().find(|f| f.name == "ticket");

        assert!(
            ticket.is_some(),
            "ticket field should be preserved from base"
        );
        assert_eq!(
            scope_prompt,
            Some("Scope (project)"),
            "scope prompt should be overridden by child"
        );

        Ok(())
    }

    #[test]
    fn test_branch_extra_fields_merged_by_name()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base = temp_dir.path().join("base.toml");
        let project = temp_dir.path().join(".rona.toml");

        std::fs::write(
            &base,
            r#"
[[branch_extra_fields]]
name = "service"
prompt = "Service"

[[branch_extra_fields]]
name = "version"
prompt = "Version (base)"
"#,
        )?;

        std::fs::write(
            &project,
            r#"
extends = "base.toml"

[[branch_extra_fields]]
name = "version"
prompt = "Version (project)"
"#,
        )?;

        let cfg = ProjectConfig::load_from_file(&project)?;
        assert_eq!(cfg.branch_extra_fields.len(), 2);

        let version_prompt = cfg
            .branch_extra_fields
            .iter()
            .find(|f| f.name == "version")
            .and_then(|f| f.prompt.as_deref());
        assert_eq!(version_prompt, Some("Version (project)"));

        let service = cfg.branch_extra_fields.iter().find(|f| f.name == "service");
        assert!(service.is_some(), "service should be preserved from base");

        Ok(())
    }
}
