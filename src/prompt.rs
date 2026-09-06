//! Interactive prompts shared by the commands.
//!
//! Every command that asks the user something goes through this module, so all prompts share
//! the same theme ([`crate::theme::prompt_theme`]) and the same cancellation rule: a cancelled
//! or unusable prompt becomes [`RonaError::UserCancelled`] rather than a per-command error.
//!
//! The prompts for config-declared extra fields live in [`crate::extra_fields`], next to the
//! declarations and the prefetch that feed them; this module drives them in the configured
//! order via [`fields`], and lends them the [`select_or_create`] picker for their candidate lists.

mod picker;

use std::{collections::HashMap, fmt::Display};

use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect};
use regex::Regex;

pub(crate) use picker::{select_or_create, select_or_create_many};

use crate::{
    errors::{Result, RonaError},
    extra_fields::{BuiltInFieldConfig, ExtraField, prompt_extra_field},
    template::references,
    theme::prompt_theme,
};

/// Asks for a free-text value.
///
/// `validation` is a regex the answer must match, and `default` is offered as the pre-filled
/// answer so that pressing Enter accepts it. A blank default is ignored.
///
/// # Errors
/// * If the validation regex is invalid
/// * If the user cancels the prompt
pub(crate) fn text(
    prompt: &str,
    validation: Option<&str>,
    default: Option<&str>,
) -> Result<String> {
    let theme = prompt_theme();
    let mut input = Input::<String>::with_theme(&theme)
        .with_prompt(prompt)
        .allow_empty(true);

    if let Some(default) = default.filter(|value| !value.trim().is_empty()) {
        input = input.default(default.to_string());
    }

    if let Some(pattern) = validation {
        let regex = Regex::new(pattern).map_err(|e| {
            RonaError::InvalidInput(format!("Invalid validation regex '{pattern}': {e}"))
        })?;
        let pattern = pattern.to_string();
        input = input.validate_with(move |value: &String| -> std::result::Result<(), String> {
            if regex.is_match(value) {
                Ok(())
            } else {
                Err(format!("Must match pattern: {pattern}"))
            }
        });
    }

    input.interact_text().map_err(|_| RonaError::UserCancelled)
}

/// Asks the user to pick one of `items` and returns its index.
///
/// # Errors
/// * If the user cancels the prompt
pub(crate) fn select<T: Display>(prompt: &str, items: &[T]) -> Result<usize> {
    FuzzySelect::with_theme(&prompt_theme())
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|_| RonaError::UserCancelled)?
        .ok_or(RonaError::UserCancelled)
}

/// Asks the user to tick any number of `items` and returns their indices.
///
/// # Errors
/// * If the user cancels the prompt
pub(crate) fn multi_select<T: Display>(prompt: &str, items: &[T]) -> Result<Vec<usize>> {
    MultiSelect::with_theme(&prompt_theme())
        .with_prompt(prompt)
        .items(items)
        .interact_opt()
        .map_err(|_| RonaError::UserCancelled)?
        .ok_or(RonaError::UserCancelled)
}

/// Asks a yes/no question, falling back to `default` when the prompt cannot be shown.
///
/// Confirmations guard destructive steps, so a prompt that cannot be displayed must not be
/// read as an answer: callers pass `false` as the default for anything irreversible.
pub(crate) fn confirm(prompt: &str, default: bool) -> bool {
    Confirm::with_theme(&prompt_theme())
        .with_prompt(prompt)
        .default(default)
        .interact()
        .unwrap_or(default)
}

/// The built-in prompt of a template family: the commit `message`, the branch `description`,
/// or the request `title`.
///
/// Every family pairs one built-in prompt with the extra fields declared in the config, and
/// [`fields`] prompts them together in the configured order.
#[derive(Debug)]
pub(crate) struct BuiltInPrompt<'a> {
    /// Reserved field name that positions this prompt in the configured order.
    pub(crate) key: &'a str,
    /// Prompt text used when the config does not override it.
    pub(crate) default_prompt: &'a str,
    /// Config overrides for the prompt text and its validation.
    pub(crate) config: Option<&'a BuiltInFieldConfig>,
    /// Answer offered as pre-filled, accepted by pressing Enter.
    pub(crate) default_value: Option<&'a str>,
    /// Whether the prompt is shown at all. Callers combine [`is_enabled`] with whether their
    /// template references the built-in variable.
    pub(crate) needed: bool,
}

/// Whether a built-in prompt is enabled; the config can turn it off entirely.
pub(crate) fn is_enabled(config: Option<&BuiltInFieldConfig>) -> bool {
    !config.is_some_and(|c| c.disabled)
}

/// Prompts the built-in field and the configured extra fields in the configured order.
///
/// The built-in value is empty when its prompt is not needed, which the template renders as an
/// empty variable. Extra fields the user skips are absent from the returned map, so template
/// conditional blocks (`{?scope}...{/scope}`) drop them naturally.
///
/// # Errors
/// * If any prompt is cancelled or a validation regex is invalid
pub(crate) fn fields(
    extra_fields: &[ExtraField],
    field_order: &[String],
    builtin: &BuiltInPrompt<'_>,
) -> Result<(String, HashMap<String, String>)> {
    let mut builtin_value: Option<String> = None;
    let mut extra_values: HashMap<String, String> = HashMap::new();

    for name in field_names(extra_fields, field_order, builtin.key, builtin.needed) {
        if name == builtin.key {
            let prompt = builtin
                .config
                .and_then(|c| c.prompt.as_deref())
                .unwrap_or(builtin.default_prompt);
            let validation = builtin.config.and_then(|c| c.validation.as_deref());

            builtin_value = Some(text(prompt, validation, builtin.default_value)?);
        } else if let Some(field) = extra_fields.iter().find(|f| f.name == name)
            && let Some(value) = prompt_extra_field(field)?
        {
            extra_values.insert(field.name.clone(), value);
        }
    }

    Ok((builtin_value.unwrap_or_default(), extra_values))
}

/// Builds the prompt order for a template family.
///
/// Fields named in `field_order` come first in that order, then any configured field the order
/// does not mention, then the built-in prompt when it is needed and not already positioned.
fn field_names(
    extra_fields: &[ExtraField],
    field_order: &[String],
    builtin_key: &str,
    needs_builtin: bool,
) -> Vec<String> {
    let mut ordered: Vec<String> = if field_order.is_empty() {
        extra_fields.iter().map(|f| f.name.clone()).collect()
    } else {
        let mut listed = field_order.to_vec();
        for field in extra_fields {
            if !listed.iter().any(|name| name == &field.name) {
                listed.push(field.name.clone());
            }
        }
        listed
    };

    if needs_builtin && !ordered.iter().any(|name| name == builtin_key) {
        ordered.push(builtin_key.to_string());
    }

    ordered
}

/// Keeps the extra fields the template actually uses, and notes the ones it skips.
///
/// A field inherited from an extended config (or otherwise configured) but unused by the
/// active template would be prompted for a value that is then discarded, so it is skipped with
/// a `[NOTE]` line instead. `label` names the field family in that line, e.g. `"Extra field"`.
pub(crate) fn referenced_fields(
    extra_fields: &[ExtraField],
    template: &str,
    label: &str,
) -> Vec<ExtraField> {
    extra_fields
        .iter()
        .filter(|field| {
            let referenced = references(template, &field.name);
            if !referenced {
                println!(
                    "[NOTE] {label} '{}' is not referenced in the template; skipping.",
                    field.name
                );
            }
            referenced
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extra_fields::FieldKind;

    fn field(name: &str) -> ExtraField {
        ExtraField {
            name: name.to_string(),
            prompt: None,
            kind: FieldKind::default(),
            required: false,
            validation: None,
            prefetch: None,
            separator: None,
        }
    }

    #[test]
    fn test_field_names_appends_builtin_last_by_default() {
        let fields = vec![field("ticket")];

        let ordered = field_names(&fields, &[], "title", true);

        assert_eq!(ordered, vec!["ticket".to_string(), "title".to_string()]);
    }

    #[test]
    fn test_field_names_honours_a_configured_order() {
        let fields = vec![field("ticket"), field("scope")];
        let order = vec!["title".to_string(), "ticket".to_string()];

        let ordered = field_names(&fields, &order, "title", true);

        // Listed items keep their order, and the unlisted field is appended.
        assert_eq!(
            ordered,
            vec![
                "title".to_string(),
                "ticket".to_string(),
                "scope".to_string()
            ]
        );
    }

    #[test]
    fn test_field_names_omits_a_disabled_builtin() {
        let fields = vec![field("ticket")];

        let ordered = field_names(&fields, &[], "title", false);

        assert_eq!(ordered, vec!["ticket".to_string()]);
    }

    #[test]
    fn test_is_enabled_follows_the_disabled_flag() {
        assert!(is_enabled(None));
        assert!(is_enabled(Some(&BuiltInFieldConfig::default())));
        assert!(!is_enabled(Some(&BuiltInFieldConfig {
            disabled: true,
            ..BuiltInFieldConfig::default()
        })));
    }

    #[test]
    fn test_referenced_fields_keeps_only_what_the_template_uses() {
        let fields = vec![field("scope"), field("ticket"), field("unused")];

        let kept = referenced_fields(
            &fields,
            "({scope}) {?ticket}{ticket}{/ticket}",
            "Extra field",
        );

        let names: Vec<&str> = kept.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["scope", "ticket"]);
    }
}
