//! Extra Fields Module for Rona
//!
//! Provides configurable extra prompt fields for commit message generation,
//! declared in `.rona.toml` under `[[extra_fields]]`.

use std::collections::{HashMap, HashSet};

use dialoguer::Input;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    errors::{Result, RonaError},
    git::{get_all_branches, get_current_branch},
};

/// How the field is presented to the user.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    /// Free-form text input.
    #[default]
    Text,
    /// Fuzzy pick from the candidate list, or a value typed on the spot.
    Select,
    /// Tick any number of candidates, or values typed on the spot, joined by `separator`.
    #[serde(rename = "multi-select", alias = "multiselect", alias = "multi_select")]
    MultiSelect,
}

/// Where prefetch data comes from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PrefetchSource {
    /// Run a shell command and extract values from its output.
    #[default]
    Command,
    /// Extract a value from the current git branch name only.
    Branch,
    /// Extract values from all local branch names.
    Branches,
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
    /// Second-stage regex applied to every value `extract_regex` returned.
    ///
    /// A commit subject holds its scopes as one group, `feat(records,pds,ui): ...`, so the first
    /// stage can only hand back `records,pds,ui`. This pattern splits that group into the entries
    /// a `multi-select` field ticks one by one, e.g. `\w+`. Same extraction priority as
    /// `extract_regex`. Absent, the first stage's values are the candidates.
    pub item_regex: Option<String>,
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
    ///
    /// A `multi-select` field matches it against each ticked value, not against the joined one.
    pub validation: Option<String>,
    /// Optional configuration for pre-populating the prompt.
    pub prefetch: Option<PrefetchConfig>,
    /// What joins the values of a `multi-select` field. Defaults to `","`.
    pub separator: Option<String>,
}

impl ExtraField {
    /// What joins the ticked values of a `multi-select` field.
    fn separator(&self) -> &str {
        self.separator.as_deref().unwrap_or(DEFAULT_SEPARATOR)
    }
}

/// Joins the values of a `multi-select` field when the config names no other separator.
const DEFAULT_SEPARATOR: &str = ",";

/// Run a prefetch config and return the candidate strings.
///
/// Command failures (non-zero exit, spawn error) are treated as soft failures and
/// return an empty `Vec`. Invalid regex patterns are hard errors.
///
/// # Errors
/// Returns an error if the `extract_regex` or `item_regex` pattern is invalid.
pub fn run_prefetch(prefetch: &PrefetchConfig) -> Result<Vec<String>> {
    let re = compile(&prefetch.extract_regex, "prefetch regex")?;
    let item_re = prefetch
        .item_regex
        .as_deref()
        .map(|pattern| compile(pattern, "prefetch item regex"))
        .transpose()?;

    let values = match prefetch.source {
        PrefetchSource::Branch => {
            let branch = get_current_branch().unwrap_or_default();
            extract_matches(&re, std::iter::once(branch.as_str()))
        }

        PrefetchSource::Branches => {
            let branches = get_all_branches().unwrap_or_default();
            extract_matches(&re, branches.iter().map(String::as_str))
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
            extract_matches(&re, stdout.lines())
        }
    };

    Ok(refine(values, item_re.as_ref(), prefetch.deduplicate))
}

/// Compiles a configured pattern, naming it in the error the user reads.
fn compile(pattern: &str, label: &str) -> Result<Regex> {
    Regex::new(pattern)
        .map_err(|e| RonaError::InvalidInput(format!("Invalid {label} '{pattern}': {e}")))
}

/// Splits the extracted values into their entries, then drops the repeats.
///
/// Both steps run after the whole source is read: splitting a group only makes sense once the
/// group is in hand, and deduplicating what comes out of it is what leaves one row per entry.
fn refine(values: Vec<String>, item_re: Option<&Regex>, deduplicate: bool) -> Vec<String> {
    let values = match item_re {
        Some(re) => extract_matches(re, values.iter().map(String::as_str)),
        None => values,
    };

    if !deduplicate {
        return values;
    }

    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|v| seen.insert(v.clone()))
        .collect()
}

/// Extract regex matches from an iterator of lines.
fn extract_matches<'a>(re: &Regex, lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut results = Vec::new();

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

            results.push(v);
        }
    }

    results
}

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

    // A multi-select is a picker whatever the prefetch returned: an empty candidate list still
    // takes typed values, and the field is declared as holding several of them.
    if field.kind == FieldKind::MultiSelect {
        return prompt_as_multi_select(field, prompt_text, &candidates, validator_regex);
    }

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
        prompt_as_select(field, prompt_text, &candidates, validator_regex)
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

/// Offers the prefetched candidates without closing the door on a value they do not hold: the
/// picker filters the list as the user types and turns unmatched text into a new value, so adding
/// a scope costs no more keystrokes than reusing one.
fn prompt_as_select(
    field: &ExtraField,
    prompt_text: &str,
    candidates: &[String],
    validator_regex: Option<Regex>,
) -> Result<Option<String>> {
    let required = field.required;
    let matches_pattern = pattern_validator(field, validator_regex);

    let validate = move |value: &str| -> std::result::Result<(), String> {
        if required && value.trim().is_empty() {
            return Err("This field is required.".to_string());
        }
        matches_pattern(value)
    };

    crate::prompt::select_or_create(prompt_text, candidates, !required, &validate)
}

/// Ticks any number of candidates, and joins them the way the template expects to read them.
///
/// One commit often touches several scopes, and a single-value select forces the second and third
/// to be typed by hand. Here each one is a row, `validation` guards each ticked value on its own,
/// and only the join is a single template variable.
fn prompt_as_multi_select(
    field: &ExtraField,
    prompt_text: &str,
    candidates: &[String],
    validator_regex: Option<Regex>,
) -> Result<Option<String>> {
    let validate = pattern_validator(field, validator_regex);

    let picked =
        crate::prompt::select_or_create_many(prompt_text, candidates, !field.required, &validate)?;

    if picked.is_empty() {
        return Ok(None);
    }

    Ok(Some(picked.join(field.separator())))
}

/// Builds the check of one value against the field's `validation` pattern.
fn pattern_validator(
    field: &ExtraField,
    validator_regex: Option<Regex>,
) -> impl Fn(&str) -> std::result::Result<(), String> + use<> {
    let pattern = field.validation.clone();

    move |value: &str| {
        if let Some(ref re) = validator_regex
            && !value.is_empty()
            && !re.is_match(value)
        {
            return Err(format!(
                "Must match pattern: {}",
                pattern.as_deref().unwrap_or("")
            ));
        }
        Ok(())
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

    let theme = crate::theme::prompt_theme();
    let value = if needs_validator {
        let mut text_prompt = Input::<String>::with_theme(&theme)
            .with_prompt(prompt_text)
            .allow_empty(true);
        if let Some(d) = default {
            text_prompt = text_prompt.default(d.to_string());
        }
        text_prompt
            .validate_with(move |input: &String| -> std::result::Result<(), String> {
                if required && input.trim().is_empty() {
                    return Err("This field is required.".to_string());
                }
                if let Some(ref re) = validator_regex
                    && !input.is_empty()
                    && !re.is_match(input)
                {
                    return Err(format!(
                        "Must match pattern: {}",
                        pattern_str.as_deref().unwrap_or("")
                    ));
                }
                Ok(())
            })
            .interact_text()
            .map_err(|_| RonaError::UserCancelled)?
    } else {
        let mut text_prompt = Input::<String>::with_theme(&theme)
            .with_prompt(prompt_text)
            .allow_empty(true);
        if let Some(d) = default {
            text_prompt = text_prompt.default(d.to_string());
        }
        text_prompt
            .interact_text()
            .map_err(|_| RonaError::UserCancelled)?
    };

    if value.is_empty() && !required {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Configuration overrides for a built-in prompt field (e.g., the branch `description` or
/// commit `message`).
///
/// All fields are optional; omitted fields fall back to the built-in defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuiltInFieldConfig {
    /// Custom prompt text shown to the user.
    pub prompt: Option<String>,
    /// Optional regex the entered value must match.
    pub validation: Option<String>,
    /// When `true`, skip this prompt entirely (the template variable will be empty).
    #[serde(default)]
    pub disabled: bool,
}

/// Prefetch configuration specific to the built-in message prompt.
///
/// Unlike `PrefetchConfig`, this supports a `template` that uses `{extract}` as a
/// placeholder for the first extracted value. When the user submits the message prompt
/// without typing, the rendered template is used as the value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePrefetchConfig {
    /// Data source: `"command"` or `"branch"`.
    pub source: PrefetchSource,
    /// Shell command to run when `source = "command"`.
    pub command: Option<String>,
    /// Regex applied to the source output. Same extraction priority as `PrefetchConfig`.
    pub extract_regex: String,
    /// Optional template rendered into the message default.
    /// Use `{extract}` as a placeholder for the first extracted value.
    /// When absent, the raw extracted value is used as the default.
    pub template: Option<String>,
}

/// Run a `MessagePrefetchConfig` and return the rendered default string, or `None` if
/// nothing was extracted.
///
/// # Errors
/// Returns an error if `extract_regex` is invalid.
pub fn run_message_prefetch(config: &MessagePrefetchConfig) -> Result<Option<String>> {
    let prefetch = PrefetchConfig {
        source: config.source,
        command: config.command.clone(),
        extract_regex: config.extract_regex.clone(),
        item_regex: None,
        deduplicate: false,
    };
    let candidates = run_prefetch(&prefetch)?;
    let Some(extract) = candidates.into_iter().next() else {
        return Ok(None);
    };
    #[allow(clippy::literal_string_with_formatting_args)]
    let default = config
        .template
        .as_ref()
        .map_or_else(|| extract.clone(), |t| t.replace("{extract}", &extract));
    Ok(Some(default))
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
            item_regex: None,
            deduplicate: dedup,
        }
    }

    fn make_branch_prefetch(regex: &str) -> PrefetchConfig {
        PrefetchConfig {
            source: PrefetchSource::Branch,
            command: None,
            extract_regex: regex.to_string(),
            item_regex: None,
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
            item_regex: None,
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
    fn test_run_prefetch_item_regex_splits_a_group_of_scopes() -> TestResult {
        let input = "printf 'feat(records,pds,ui): a\\nfix(ui): b\\n'";
        let mut prefetch = make_command_prefetch(input, r"\w+\((?P<value>[^)]*)\):", true);
        prefetch.item_regex = Some(r"[\w-]+".to_string());

        let result = run_prefetch(&prefetch)?;

        assert_eq!(result, vec!["records", "pds", "ui"]);
        Ok(())
    }

    #[test]
    fn test_run_prefetch_without_item_regex_keeps_the_whole_group() -> TestResult {
        let input = "printf 'feat(records,pds,ui): a\\n'";
        let prefetch = make_command_prefetch(input, r"\w+\((?P<value>[^)]*)\):", true);

        let result = run_prefetch(&prefetch)?;

        assert_eq!(result, vec!["records,pds,ui"]);
        Ok(())
    }

    #[test]
    fn test_run_prefetch_item_regex_invalid_hard_errors() {
        let mut prefetch = make_command_prefetch("echo test", "(.+)", false);
        prefetch.item_regex = Some("[invalid".to_string());

        assert!(run_prefetch(&prefetch).is_err());
    }

    #[test]
    fn test_refine_deduplicates_the_entries_not_the_groups() -> TestResult {
        let item_re = Regex::new(r"\w+")?;
        let values = vec!["ui,ux".to_string(), "ux,api".to_string()];

        let result = refine(values, Some(&item_re), true);

        assert_eq!(result, vec!["ui", "ux", "api"]);
        Ok(())
    }

    #[test]
    fn test_multi_select_kind_and_separator_read_from_toml() -> TestResult {
        let field: ExtraField = toml::from_str(
            r#"
name = "scope"
kind = "multi-select"
separator = ", "
"#,
        )?;

        assert_eq!(field.kind, FieldKind::MultiSelect);
        assert_eq!(field.separator(), ", ");
        Ok(())
    }

    #[test]
    fn test_separator_defaults_to_a_comma() -> TestResult {
        let field: ExtraField = toml::from_str("name = \"scope\"\nkind = \"multiselect\"\n")?;

        assert_eq!(field.kind, FieldKind::MultiSelect);
        assert_eq!(field.separator(), ",");
        Ok(())
    }

    #[test]
    fn test_extract_matches_dedup() -> TestResult {
        let re = Regex::new(r"scope:(\w+)")?;
        let lines = ["scope:api", "scope:auth", "scope:api"];
        let result = refine(extract_matches(&re, lines.iter().copied()), None, true);
        assert_eq!(result, vec!["api", "auth"]);
        Ok(())
    }

    #[test]
    fn test_extract_matches_no_dedup() -> TestResult {
        let re = Regex::new(r"scope:(\w+)")?;
        let lines = ["scope:api", "scope:auth", "scope:api"];
        let result = extract_matches(&re, lines.iter().copied());
        assert_eq!(result, vec!["api", "auth", "api"]);
        Ok(())
    }

    #[test]
    fn test_extract_matches_skips_empty() -> TestResult {
        let re = Regex::new(r"scope:(\w*)")?;
        let lines = ["scope:", "scope:auth"];
        let result = extract_matches(&re, lines.iter().copied());
        assert_eq!(result, vec!["auth"]);
        Ok(())
    }
}
