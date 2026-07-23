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

/// A path-conditional config layer, declared as `[[overrides]]` in a config file.
///
/// When rona runs from a directory matching `path`, the config file referenced by
/// `config` is layered in above the global config and below the project `.rona.toml`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConfigOverride {
    /// Glob pattern matched against the directory rona runs from.
    /// A leading `~/` is expanded to the user's home directory.
    /// A pattern with no wildcard matches that directory and every descendant.
    pub path: String,

    /// Path to the config file to layer in when `path` matches.
    /// Relative paths resolve against the config file declaring the override,
    /// and a leading `~/` is expanded to the user's home directory.
    pub config: String,
}

/// Expands a leading `~/` to the user's home directory.
fn expand_tilde(value: &str) -> String {
    value.strip_prefix("~/").map_or_else(
        || value.to_string(),
        |rest| {
            dirs::home_dir().map_or_else(
                || value.to_string(),
                |home| home.join(rest).display().to_string(),
            )
        },
    )
}

/// Glob options used for override `path` matching.
///
/// Windows paths are case-insensitive, so patterns are matched that way there.
/// The `glob` crate already treats `/` and `\` as interchangeable separators on
/// Windows, so a pattern may be written with either.
const OVERRIDE_MATCH_OPTIONS: glob::MatchOptions = glob::MatchOptions {
    case_sensitive: !cfg!(windows),
    require_literal_separator: false,
    require_literal_leading_dot: false,
};

/// Matches a single glob pattern against a directory. An invalid pattern never matches.
fn glob_matches_dir(pattern: &str, dir: &Path) -> bool {
    glob::Pattern::new(pattern).is_ok_and(|p| p.matches_path_with(dir, OVERRIDE_MATCH_OPTIONS))
}

/// Returns `true` when `dir` is covered by an override's `path` pattern.
///
/// Matching is intentionally forgiving:
/// 1. The pattern is matched as a glob against the full directory path.
/// 2. A trailing `/**` or `/*` also matches the base directory itself.
/// 3. A wildcard-free pattern matches that directory and all of its descendants.
fn override_pattern_matches(pattern: &str, dir: &Path) -> bool {
    let expanded = expand_tilde(pattern);

    if glob_matches_dir(&expanded, dir) {
        return true;
    }

    // Either separator, so a Windows-style `C:\work\**` behaves like `C:/work/**`.
    if let Some(base) = ["/**", r"\**", "/*", r"\*"]
        .iter()
        .find_map(|suffix| expanded.strip_suffix(suffix))
        && glob_matches_dir(base, dir)
    {
        return true;
    }

    // A wildcard-free pattern also covers every descendant. Going through the glob
    // engine rather than `Path::starts_with` keeps separator and case handling
    // identical to the branches above, which matters on Windows.
    !expanded.contains(['*', '?', '[']) && {
        let trimmed = expanded.trim_end_matches(['/', '\\']);
        glob_matches_dir(&format!("{trimmed}/**"), dir)
    }
}

/// Peeks at the `overrides` key of a TOML config file without full deserialization.
#[derive(Deserialize)]
struct OverridesOnly {
    #[serde(default)]
    overrides: Vec<ConfigOverride>,
}

/// A config file pulled in by a matching `[[overrides]]` entry, paired with the
/// `path` pattern that selected it (so it can be reported to the user).
#[derive(Debug, Clone)]
struct OverrideSource {
    path: PathBuf,
    /// The override's `path` glob that matched the current directory.
    pattern: String,
}

/// Collects the config files pulled in by `[[overrides]]` entries that match `dir`.
///
/// `declaring_paths` are the config files to read overrides from (the global configs),
/// in loading order. Returned paths are base-first and include each override target's
/// own `extends` chain, each tagged with the pattern that pulled it in. Override
/// targets that do not exist are skipped.
///
/// # Errors
/// Returns `ConfigError::ParseError` if a declaring file is not valid TOML or its
/// `overrides` entries are malformed. Silently ignoring that would drop every
/// override with no explanation. Files that cannot be read are skipped.
fn collect_override_sources(
    declaring_paths: &[PathBuf],
    dir: &Path,
) -> Result<Vec<OverrideSource>> {
    let mut collected: Vec<OverrideSource> = Vec::new();

    for declaring in declaring_paths {
        let Ok(content) = std::fs::read_to_string(declaring) else {
            continue;
        };
        let parsed = toml::from_str::<OverridesOnly>(&content).map_err(|e| {
            RonaError::Config(ConfigError::ParseError {
                file: declaring.display().to_string(),
                reason: e.to_string(),
            })
        })?;

        for entry in parsed.overrides {
            if !override_pattern_matches(&entry.path, dir) {
                continue;
            }

            let target = resolve_extends_path(&entry.config, declaring);
            if !target.exists() {
                continue;
            }

            let mut visited = HashSet::new();
            let chain = collect_extends_chain(&target, &mut visited)?;
            collected.extend(
                chain
                    .into_iter()
                    .chain(std::iter::once(target))
                    .map(|path| OverrideSource {
                        path,
                        pattern: entry.path.clone(),
                    }),
            );
        }
    }

    Ok(collected)
}

/// Builds the ordered list of config files to merge for `dir`, base-first.
/// Global configs come first, then any matching `[[overrides]]` targets,
/// then the project `.rona.toml` with its `extends` chain.
fn config_paths_for_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let home = dirs::home_dir().ok_or(ConfigError::ConfigNotFound)?;
    let old_global = home.join(".config/rona/config.toml");
    let new_global = home.join(".config/rona.toml");

    let globals: Vec<PathBuf> = [old_global, new_global]
        .into_iter()
        .filter(|p| p.exists())
        .collect();

    let mut paths = globals.clone();
    paths.extend(
        collect_override_sources(&globals, dir)?
            .into_iter()
            .map(|source| source.path),
    );

    let project_config_path = dir.join(".rona.toml");
    if project_config_path.exists() {
        let mut visited = HashSet::new();
        paths.extend(collect_extends_chain(&project_config_path, &mut visited)?);
        paths.push(project_config_path);
    }

    Ok(paths)
}

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

    /// Path-conditional config layers. Declared as `[[overrides]]`, typically in the
    /// global config, so that running rona under a given directory tree layers in
    /// another config file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<ConfigOverride>,
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
            overrides: vec![],
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
    overrides: Option<Vec<ConfigOverride>>,
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
            overrides: raw.overrides.unwrap_or_default(),
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
        overrides: child.overrides.or(base.overrides),
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

        let paths = config_paths_for_dir(&env::current_dir()?)?;

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
        let paths = config_paths_for_dir(from_dir)?;

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

    // Path-conditional overrides (priority 3 - above global, below project)
    let declaring_globals: Vec<PathBuf> = [old_global, new_global]
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    for source in collect_override_sources(&declaring_globals, &search_dir).unwrap_or_default() {
        sources.push(ConfigSource {
            exists: source.path.exists(),
            description: format!("Override config (path = \"{}\")", source.pattern),
            path: source.path,
            priority: 3,
        });
    }

    // Extended configs (priority 4 - between overrides and project, base-first)
    let project_config = search_dir.join(".rona.toml");
    if project_config.exists() {
        let chain = collect_extends_chain(&project_config, &mut HashSet::new()).unwrap_or_default();
        for (i, extended_path) in chain.iter().enumerate() {
            sources.push(ConfigSource {
                path: extended_path.clone(),
                exists: extended_path.exists(),
                description: format!("Extended config ({})", i + 1),
                priority: 4,
            });
        }
    }

    // Project-local config (priority 5 - highest priority, overrides all)
    sources.push(ConfigSource {
        path: project_config.clone(),
        exists: project_config.exists(),
        description: "Project config".to_string(),
        priority: 5,
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

    /// Extracts just the file paths from collected override sources.
    fn override_paths(sources: &[OverrideSource]) -> Vec<PathBuf> {
        sources.iter().map(|s| s.path.clone()).collect()
    }

    /// Renders `value` as a TOML *literal* string (single-quoted), where backslashes
    /// are not escape characters. Required for Windows paths: in a basic (double-quoted)
    /// string, `C:\Users` contains the invalid escape `\U` and fails to parse.
    fn toml_literal(value: &str) -> String {
        assert!(
            !value.contains('\''),
            "path contains a single quote, which a TOML literal string cannot express: {value}"
        );
        format!("'{value}'")
    }

    #[test]
    fn test_override_pattern_matches_glob_subdirectories() {
        assert!(override_pattern_matches(
            "/Affluences/**",
            Path::new("/Affluences/afl-notes")
        ));
        assert!(override_pattern_matches(
            "/Affluences/**",
            Path::new("/Affluences/afl-notes/deep/nested")
        ));
        assert!(!override_pattern_matches(
            "/Affluences/**",
            Path::new("/Other/project")
        ));
    }

    #[test]
    fn test_override_pattern_matches_base_directory_itself() {
        assert!(override_pattern_matches(
            "/Affluences/**",
            Path::new("/Affluences")
        ));
        assert!(override_pattern_matches(
            "/Affluences/*",
            Path::new("/Affluences")
        ));
    }

    #[test]
    fn test_override_pattern_without_wildcard_matches_descendants() {
        assert!(override_pattern_matches(
            "/Affluences",
            Path::new("/Affluences")
        ));
        assert!(override_pattern_matches(
            "/Affluences",
            Path::new("/Affluences/a/b")
        ));
        assert!(!override_pattern_matches(
            "/Affluences",
            Path::new("/Affluences-other")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn test_override_pattern_matches_windows_separators_and_case() {
        // The glob crate treats `/` and `\` as interchangeable on Windows, so a
        // pattern may be written either way regardless of how the path is spelled.
        assert!(override_pattern_matches(
            r"C:\Users\me\work\**",
            Path::new(r"C:\Users\me\work\repo")
        ));
        assert!(override_pattern_matches(
            "C:/Users/me/work/**",
            Path::new(r"C:\Users\me\work\repo")
        ));
        assert!(override_pattern_matches(
            r"C:\Users\me\work",
            Path::new(r"C:\Users\me\work\repo\src")
        ));

        // A trailing `\**` covers the base directory itself, as `/**` does.
        assert!(override_pattern_matches(
            r"C:\Users\me\work\**",
            Path::new(r"C:\Users\me\work")
        ));

        // Windows paths are case-insensitive.
        assert!(override_pattern_matches(
            r"c:\users\me\work\**",
            Path::new(r"C:\Users\Me\Work\repo")
        ));

        assert!(!override_pattern_matches(
            r"C:\Users\me\work\**",
            Path::new(r"C:\Users\me\other\repo")
        ));
    }

    #[test]
    fn test_collect_override_sources_reports_malformed_declaring_file()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;

        // A Windows path written into a basic string is the realistic way to get
        // here: `\U` is an invalid TOML escape. Dropping every override silently
        // would leave the user with no idea why their config is being ignored.
        let global = root.join("rona.toml");
        std::fs::write(
            &global,
            "[[overrides]]\npath = \"C:\\Users\\me\\work\\**\"\nconfig = 'x.toml'\n",
        )?;

        let result = collect_override_sources(&[global], &root);
        assert!(
            matches!(
                result,
                Err(RonaError::Config(ConfigError::ParseError { .. }))
            ),
            "expected a parse error, got: {result:?}"
        );

        Ok(())
    }

    #[test]
    fn test_collect_override_paths_layers_matching_config()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;

        let work_dir = root.join("Affluences/afl-notes");
        std::fs::create_dir_all(&work_dir)?;

        let override_config = root.join("Affluences/rona.config");
        std::fs::write(&override_config, "editor = \"helix\"\n")?;

        let pattern = format!("{}/Affluences/**", root.display());
        let global = root.join("rona.toml");
        std::fs::write(
            &global,
            format!(
                "editor = \"nano\"\n\n[[overrides]]\npath = {}\nconfig = {}\n",
                toml_literal(&pattern),
                toml_literal(&override_config.display().to_string())
            ),
        )?;

        let collected = collect_override_sources(std::slice::from_ref(&global), &work_dir)?;
        assert_eq!(override_paths(&collected), vec![override_config.clone()]);

        // Each collected file remembers the pattern that pulled it in, so
        // `rona config -w` can report it.
        assert_eq!(collected[0].pattern, pattern);

        // A directory outside the pattern picks up nothing.
        assert!(collect_override_sources(std::slice::from_ref(&global), &root)?.is_empty());

        // The layered config wins over the global one it is layered above.
        let merged: ProjectConfig = load_and_merge_files(&[global, override_config])?.into();
        assert_eq!(merged.editor.as_deref(), Some("helix"));

        Ok(())
    }

    #[test]
    fn test_collect_override_paths_skips_missing_target()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;

        let global = root.join("rona.toml");
        std::fs::write(
            &global,
            format!(
                "[[overrides]]\npath = {}\nconfig = 'does-not-exist.toml'\n",
                toml_literal(&format!("{}/**", root.display()))
            ),
        )?;

        assert!(collect_override_sources(&[global], &root.join("sub"))?.is_empty());

        Ok(())
    }

    #[test]
    fn test_collect_override_paths_resolves_relative_config()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;

        let override_config = root.join("shared.toml");
        std::fs::write(&override_config, "editor = \"vim\"\n")?;

        let global = root.join("rona.toml");
        std::fs::write(
            &global,
            format!(
                "[[overrides]]\npath = {}\nconfig = 'shared.toml'\n",
                toml_literal(&format!("{}/**", root.display()))
            ),
        )?;

        let collected = collect_override_sources(&[global], &root.join("work"))?;
        assert_eq!(override_paths(&collected), vec![override_config]);

        Ok(())
    }

    #[test]
    fn test_collect_override_sources_tags_whole_extends_chain()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().canonicalize()?;

        let base = root.join("base.toml");
        std::fs::write(&base, "editor = \"nano\"\n")?;

        let target = root.join("target.toml");
        std::fs::write(&target, "extends = \"base.toml\"\neditor = \"helix\"\n")?;

        let pattern = format!("{}/**", root.display());
        let global = root.join("rona.toml");
        std::fs::write(
            &global,
            format!(
                "[[overrides]]\npath = {}\nconfig = 'target.toml'\n",
                toml_literal(&pattern)
            ),
        )?;

        let collected = collect_override_sources(&[global], &root.join("work"))?;

        // The target's extends chain is layered in base-first, ahead of the target.
        assert_eq!(override_paths(&collected), vec![base, target]);

        // Every file in the chain is attributed to the pattern that pulled it in.
        assert!(collected.iter().all(|s| s.pattern == pattern));

        Ok(())
    }

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
