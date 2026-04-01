//! Extra Fields Module for Rona
//!
//! Provides configurable extra prompt fields for commit message generation,
//! declared in `.rona.toml` under `[[extra_fields]]`.

use std::collections::{HashMap, HashSet};

use inquire::{Select, Text, validator::Validation};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    errors::{Result, RonaError},
    git::get_current_branch,
};

/// How the field is presented to the user.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    /// Free-form text input.
    #[default]
    Text,
    /// Selection from a list, with an "Other (enter manually)" fallback.
    Select,
}

/// Where prefetch data comes from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PrefetchSource {
    /// Run a shell command and extract values from its output.
    #[default]
    Command,
    /// Extract a value from the current git branch name.
    Branch,
}

/// Configuration for prefetching data to populate a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefetchConfig {
    /// Data source: `"command"` or `"branch"`.
    pub source: PrefetchSource,
    /// Shell command to run when `source = "command"`.
    pub command: Option<String>,
    /// Regex applied per output line (command) or to the branch name (branch).
    /// Priority for extraction: named group `value`, capture group 1, full match.
    pub extract_regex: String,
    /// Deduplicate extracted values (only meaningful for `source = "command"`).
    #[serde(default)]
    pub deduplicate: bool,
}

/// A configurable extra field to prompt for during commit message generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraField {
    /// Variable name used in templates: `{scope}`, `{ticket}`, etc.
    pub name: String,
    /// Text shown to the user. Defaults to `name` when absent.
    pub prompt: Option<String>,
    /// Whether this is a text or select prompt.
    #[serde(default)]
    pub kind: FieldKind,
    /// Whether the user must enter a non-empty value.
    #[serde(default)]
    pub required: bool,
    /// Optional regex the entered value must match.
    pub validation: Option<String>,
    /// Optional configuration for pre-populating the prompt.
    pub prefetch: Option<PrefetchConfig>,
}

/// Run a prefetch config and return the candidate strings.
///
/// Command failures (non-zero exit, spawn error) are treated as soft failures and
/// return an empty `Vec`. Invalid regex patterns are hard errors.
///
/// # Errors
/// Returns an error if the `extract_regex` pattern is invalid.
pub fn run_prefetch(prefetch: &PrefetchConfig) -> Result<Vec<String>> {
    let re = Regex::new(&prefetch.extract_regex).map_err(|e| {
        RonaError::InvalidInput(format!(
            "Invalid prefetch regex '{}': {e}",
            prefetch.extract_regex
        ))
    })?;

    match prefetch.source {
        PrefetchSource::Branch => {
            let branch = get_current_branch().unwrap_or_default();
            Ok(extract_matches(
                &re,
                std::iter::once(branch.as_str()),
                false,
            ))
        }

        PrefetchSource::Command => {
            let Some(ref command) = prefetch.command else {
                return Ok(vec![]);
            };
            let Ok(output) = std::process::Command::new("sh")
                .args(["-c", command.as_str()])
                .output()
            else {
                return Ok(vec![]);
            };
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(extract_matches(&re, stdout.lines(), prefetch.deduplicate))
        }
    }
}

/// Extract regex matches from an iterator of lines.
fn extract_matches<'a>(
    re: &Regex,
    lines: impl Iterator<Item = &'a str>,
    deduplicate: bool,
) -> Vec<String> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for line in lines {
        for cap in re.captures_iter(line) {
            let value = cap
                .name("value")
                .or_else(|| cap.get(1))
                .or_else(|| cap.get(0))
                .map(|m| m.as_str().to_string());

            let Some(v) = value else { continue };
            if v.is_empty() {
                continue;
            }

            if deduplicate {
                if seen.insert(v.clone()) {
                    results.push(v);
                }
            } else {
                results.push(v);
            }
        }
    }

    results
}

const NONE_OPTION: &str = "(none)";
const OTHER_OPTION: &str = "Other (enter manually)";

/// Prompt the user for an extra field value.
///
/// Returns `None` when the field is optional and the user chose to skip it.
///
/// # Errors
/// Returns an error if the user cancels the prompt or the validation regex is invalid.
pub fn prompt_extra_field(field: &ExtraField) -> Result<Option<String>> {
    let prompt_text = field.prompt.as_deref().unwrap_or(field.name.as_str());

    let validator_regex = field
        .validation
        .as_deref()
        .map(Regex::new)
        .transpose()
        .map_err(|e| {
            RonaError::InvalidInput(format!(
                "Invalid validation regex for field '{}': {e}",
                field.name
            ))
        })?;

    let candidates = field
        .prefetch
        .as_ref()
        .map(run_prefetch)
        .transpose()?
        .unwrap_or_default();

    // Show a select prompt when we have candidates and either:
    // - the field kind is "select", or
    // - the kind is "text" but prefetch source is "command" (provides a list of options)
    let use_select = !candidates.is_empty()
        && (field.kind == FieldKind::Select
            || matches!(
                &field.prefetch,
                Some(PrefetchConfig {
                    source: PrefetchSource::Command,
                    ..
                })
            ));

    if use_select {
        prompt_as_select(field, prompt_text, candidates, validator_regex)
    } else {
        // Branch prefetch: the single extracted value becomes the text default
        let default_owned = candidates.into_iter().next();
        prompt_as_text(
            field,
            prompt_text,
            default_owned.as_deref(),
            validator_regex,
        )
    }
}

fn prompt_as_select(
    field: &ExtraField,
    prompt_text: &str,
    candidates: Vec<String>,
    validator_regex: Option<Regex>,
) -> Result<Option<String>> {
    let mut options = candidates;
    if !field.required {
        options.push(NONE_OPTION.to_string());
    }
    options.push(OTHER_OPTION.to_string());

    let selected = Select::new(prompt_text, options)
        .prompt()
        .map_err(|_| RonaError::UserCancelled)?;

    match selected.as_str() {
        s if s == NONE_OPTION => Ok(None),
        s if s == OTHER_OPTION => prompt_as_text(field, prompt_text, None::<&str>, validator_regex),
        value => Ok(Some(value.to_string())),
    }
}

fn prompt_as_text(
    field: &ExtraField,
    prompt_text: &str,
    default: Option<&str>,
    validator_regex: Option<Regex>,
) -> Result<Option<String>> {
    let required = field.required;
    let pattern_str = field.validation.clone();
    let needs_validator = required || validator_regex.is_some();

    let value = if needs_validator {
        let mut text_prompt = Text::new(prompt_text);
        if let Some(d) = default {
            text_prompt = text_prompt.with_default(d);
        }
        text_prompt
            .with_validator(move |input: &str| {
                if required && input.trim().is_empty() {
                    return Ok(Validation::Invalid("This field is required.".into()));
                }
                if let Some(ref re) = validator_regex
                    && !input.is_empty()
                    && !re.is_match(input)
                {
                    return Ok(Validation::Invalid(
                        format!(
                            "Must match pattern: {}",
                            pattern_str.as_deref().unwrap_or("")
                        )
                        .into(),
                    ));
                }
                Ok(Validation::Valid)
            })
            .prompt()
            .map_err(|_| RonaError::UserCancelled)?
    } else {
        let mut text_prompt = Text::new(prompt_text);
        if let Some(d) = default {
            text_prompt = text_prompt.with_default(d);
        }
        text_prompt.prompt().map_err(|_| RonaError::UserCancelled)?
    };

    if value.is_empty() && !required {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Prompt all extra fields in order and return a map of `name → value`.
///
/// Fields the user skips (optional, chose "(none)") are omitted from the returned map.
/// Template conditional blocks (`{?scope}...{/scope}`) handle absent fields naturally.
///
/// # Errors
/// Returns an error if any prompt is cancelled or a regex pattern is invalid.
pub fn prompt_all_extra_fields(fields: &[ExtraField]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::with_capacity(fields.len());
    for field in fields {
        if let Some(value) = prompt_extra_field(field)? {
            map.insert(field.name.clone(), value);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    fn make_command_prefetch(command: &str, regex: &str, dedup: bool) -> PrefetchConfig {
        PrefetchConfig {
            source: PrefetchSource::Command,
            command: Some(command.to_string()),
            extract_regex: regex.to_string(),
            deduplicate: dedup,
        }
    }

    fn make_branch_prefetch(regex: &str) -> PrefetchConfig {
        PrefetchConfig {
            source: PrefetchSource::Branch,
            command: None,
            extract_regex: regex.to_string(),
            deduplicate: false,
        }
    }

    #[test]
    fn test_run_prefetch_invalid_regex_hard_errors() {
        let prefetch = make_command_prefetch("echo test", "[invalid", false);
        assert!(run_prefetch(&prefetch).is_err());
    }

    #[test]
    fn test_run_prefetch_branch_invalid_regex_hard_errors() {
        let prefetch = make_branch_prefetch("[invalid");
        assert!(run_prefetch(&prefetch).is_err());
    }

    #[test]
    fn test_run_prefetch_command_no_command_returns_empty() -> TestResult {
        let prefetch = PrefetchConfig {
            source: PrefetchSource::Command,
            command: None,
            extract_regex: "(.+)".to_string(),
            deduplicate: false,
        };
        let result = run_prefetch(&prefetch)?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn test_run_prefetch_command_extracts_with_named_group() -> TestResult {
        let input = "printf 'feat(auth): msg1\\nfix(api): msg2\\nfeat(auth): msg3\\n'";
        let prefetch = make_command_prefetch(input, r"\w+\((?P<value>[^)]*)\):", true);
        let result = run_prefetch(&prefetch)?;
        assert_eq!(result, vec!["auth", "api"]);
        Ok(())
    }

    #[test]
    fn test_run_prefetch_command_no_dedup() -> TestResult {
        let input = "printf 'feat(auth): msg1\\nfix(api): msg2\\nfeat(auth): msg3\\n'";
        let prefetch = make_command_prefetch(input, r"\w+\((?P<value>[^)]*)\):", false);
        let result = run_prefetch(&prefetch)?;
        assert_eq!(result, vec!["auth", "api", "auth"]);
        Ok(())
    }

    #[test]
    fn test_run_prefetch_command_first_capture_group_fallback() -> TestResult {
        let input = "printf 'feat(auth): msg1\\nfix(api): msg2\\n'";
        // No named group — falls back to capture group 1
        let prefetch = make_command_prefetch(input, r"\w+\(([^)]*)\):", true);
        let result = run_prefetch(&prefetch)?;
        assert_eq!(result, vec!["auth", "api"]);
        Ok(())
    }

    #[test]
    fn test_extract_matches_dedup() -> TestResult {
        let re = Regex::new(r"scope:(\w+)")?;
        let lines = ["scope:api", "scope:auth", "scope:api"];
        let result = extract_matches(&re, lines.iter().copied(), true);
        assert_eq!(result, vec!["api", "auth"]);
        Ok(())
    }

    #[test]
    fn test_extract_matches_no_dedup() -> TestResult {
        let re = Regex::new(r"scope:(\w+)")?;
        let lines = ["scope:api", "scope:auth", "scope:api"];
        let result = extract_matches(&re, lines.iter().copied(), false);
        assert_eq!(result, vec!["api", "auth", "api"]);
        Ok(())
    }

    #[test]
    fn test_extract_matches_skips_empty() -> TestResult {
        let re = Regex::new(r"scope:(\w*)")?;
        let lines = ["scope:", "scope:auth"];
        let result = extract_matches(&re, lines.iter().copied(), false);
        assert_eq!(result, vec!["auth"]);
        Ok(())
    }
}
