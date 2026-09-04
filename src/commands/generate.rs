//! `rona generate`: writes the `commit_message.md` file the commit command later reads.
//!
//! Both modes render the configured commit template. Interactive mode asks for the message (and
//! any extra fields) and writes the finished message; editor mode writes the same header with an
//! empty message and opens the file so the message can be typed in place.

use std::collections::HashMap;

use colored::Colorize;

use crate::{
    config::{Config, DEFAULT_COMMIT_TEMPLATE},
    errors::Result,
    extra_fields::run_message_prefetch,
    git::{
        COMMIT_MESSAGE_FILE_PATH, create_needed_files, format_branch_name, generate_commit_message,
        get_current_branch, get_current_commit_nb, get_top_level_path,
    },
    prompt::{self, BuiltInPrompt},
    template::{TemplateVariables, process_template, references, validate_template},
};

/// Generates the commit message file.
///
/// # Arguments
/// * `interactive` - Whether to prompt for the message instead of opening the editor
/// * `no_commit_number` - Whether to leave the commit number out of the message
/// * `config` - Global configuration including verbose and dry-run settings
///
/// # Errors
/// * If creating the needed files fails
/// * If a prompt is cancelled
/// * If the message cannot be written, or the editor cannot be launched
pub(crate) fn generate(interactive: bool, no_commit_number: bool, config: &Config) -> Result<()> {
    if config.dry_run {
        println!("Would create files: commit_message.md, .commitignore");
        println!("Would add files to .git/info/exclude");
        return Ok(());
    }

    create_needed_files()?;

    let template = commit_template(config);

    // The selector is only worth showing when the chosen type ends up in the message. Both
    // modes render the same template, so the template alone decides.
    let commit_type = if references(template, "commit_type") {
        Some(select_commit_type(config)?)
    } else {
        None
    };

    if interactive {
        write_prompted_message(commit_type, no_commit_number, config)
    } else {
        write_scaffold_and_edit(commit_type, no_commit_number, config)
    }
}

/// Prompts for the message and extra fields, then writes the rendered commit message.
///
/// # Errors
/// * If a prompt is cancelled
/// * If the message cannot be rendered or written
fn write_prompted_message(
    commit_type: Option<&str>,
    no_commit_number: bool,
    config: &Config,
) -> Result<()> {
    let template = commit_template(config);
    let extra_fields = prompt::referenced_fields(
        &config.project_config.commit_extra_fields,
        template,
        "Extra field",
    );

    let message_config = config.project_config.commit_message.as_ref();
    let needed = prompt::is_enabled(message_config);
    // The prefetch may run a command, so only run it when the prompt is actually shown.
    let default_value = if needed {
        config
            .project_config
            .message_prefetch
            .as_ref()
            .map(run_message_prefetch)
            .transpose()?
            .flatten()
    } else {
        None
    };

    let (message, extra_values) = prompt::fields(
        &extra_fields,
        &config.project_config.commit_fields_order,
        &BuiltInPrompt {
            key: "message",
            default_prompt: "Message",
            config: message_config,
            default_value: default_value.as_deref(),
            needed,
        },
    )?;

    if message.trim().is_empty() {
        println!(
            "{} Empty message provided. Exiting.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    }

    let formatted_message = build_commit_message(
        commit_type,
        no_commit_number,
        &message,
        &extra_values,
        config,
    )?;

    let commit_file_path = get_top_level_path()?.join(COMMIT_MESSAGE_FILE_PATH);
    std::fs::write(&commit_file_path, &formatted_message)?;

    println!("\n{} Commit message created!", "✓".green());
    println!("Message: {formatted_message}");

    Ok(())
}

/// Writes the commit message file with an empty message and opens it in the editor.
///
/// The file opens on a header that already matches the configured format, so only the message
/// is missing. Extra fields are never prompted for here, so they render empty as well.
///
/// # Errors
/// * If the message cannot be rendered or written
/// * If the editor cannot be launched
fn write_scaffold_and_edit(
    commit_type: Option<&str>,
    no_commit_number: bool,
    config: &Config,
) -> Result<()> {
    let template = commit_template(config);
    let blank_extra_values: HashMap<String, String> = config
        .project_config
        .commit_extra_fields
        .iter()
        .filter(|f| references(template, &f.name))
        .map(|f| {
            println!(
                "[NOTE] Editor mode leaves the extra field '{}' empty. Complete it in your editor.",
                f.name
            );
            (f.name.clone(), String::new())
        })
        .collect();

    let header = build_commit_message(
        commit_type,
        no_commit_number,
        "",
        &blank_extra_values,
        config,
    )?;

    generate_commit_message(&header)?;

    let commit_file_path = get_top_level_path()?.join(COMMIT_MESSAGE_FILE_PATH);
    config.open_in_editor(&commit_file_path)
}

/// Renders the commit template for `message`.
///
/// `commit_type` is `None` when the template does not use `{commit_type}`, in which case no
/// type was ever selected and the variable renders empty. Editor mode passes an empty
/// `message`, which renders the header the user then completes in their editor.
///
/// When the template fails validation a warning is printed and [`fallback_message`] is used
/// instead, keeping whichever parts are known.
///
/// # Errors
/// * If the current branch, commit count, or git author cannot be read
/// * If the template cannot be processed
fn build_commit_message(
    commit_type: Option<&str>,
    no_commit_number: bool,
    message: &str,
    extra_values: &HashMap<String, String>,
    config: &Config,
) -> Result<String> {
    let branch_name = format_branch_name(
        &config.project_config.commit_type_choices(),
        &get_current_branch()?,
    );
    let commit_number = if no_commit_number {
        None
    } else {
        Some(get_current_commit_nb()? + 1)
    };

    let template = commit_template(config);

    // Validate the template, including any extra field variable names.
    let extra_names: Vec<&str> = extra_values.keys().map(String::as_str).collect();
    if let Err(e) = validate_template(template, &extra_names) {
        println!(
            "{} Template validation error: {e}",
            "WARNING:".yellow().bold()
        );
        println!("Using fallback format...");

        return Ok(fallback_message(
            commit_number,
            commit_type,
            &branch_name,
            message,
        ));
    }

    let variables = TemplateVariables::new(
        commit_number,
        commit_type.unwrap_or_default().to_string(),
        branch_name,
        message.trim().to_string(),
    )?;

    process_template(template, &variables, extra_values)
}

/// The built-in `[number] (type on branch) message` layout, used when the configured template
/// cannot be rendered.
///
/// Parts that are unknown are left out entirely rather than rendered empty, so a missing
/// commit number does not leave `[]` behind.
fn fallback_message(
    commit_number: Option<u32>,
    commit_type: Option<&str>,
    branch_name: &str,
    message: &str,
) -> String {
    let number_prefix = commit_number.map_or_else(String::new, |number| format!("[{number}] "));
    let type_prefix = commit_type.map_or_else(String::new, |commit_type| {
        format!("({commit_type} on {branch_name}) ")
    });

    format!("{number_prefix}{type_prefix}{}", message.trim())
}

/// The commit template in force: the configured one, or [`DEFAULT_COMMIT_TEMPLATE`].
fn commit_template(config: &Config) -> &str {
    config
        .project_config
        .commit_template
        .as_deref()
        .unwrap_or(DEFAULT_COMMIT_TEMPLATE)
}

/// Shows the commit type selector and returns the chosen type.
///
/// # Errors
/// * If the user cancels the prompt
fn select_commit_type(config: &Config) -> Result<&str> {
    let commit_types = config.project_config.commit_type_choices();
    let index = prompt::select("Select commit type", &commit_types)?;

    Ok(commit_types[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    /// The commit type selector is only shown when the template renders the type.
    #[test]
    fn test_default_template_references_commit_type() {
        assert!(references(DEFAULT_COMMIT_TEMPLATE, "commit_type"));
    }

    /// REGRESSION TEST: `rona -g -i -n` used to produce empty brackets `[]`.
    /// The conditional block in the default template drops the number instead.
    #[test]
    fn test_default_template_without_a_commit_number() -> TestResult {
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(DEFAULT_COMMIT_TEMPLATE, &variables, &HashMap::new())?;

        assert_eq!(result, "(docs on main) Update docs");
        Ok(())
    }

    #[test]
    fn test_default_template_with_a_commit_number() -> TestResult {
        let variables = TemplateVariables {
            commit_number: Some(42),
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(DEFAULT_COMMIT_TEMPLATE, &variables, &HashMap::new())?;

        assert_eq!(result, "[42] (feat on new-feature) Add feature");
        Ok(())
    }

    #[test]
    fn test_fallback_message_without_a_commit_number() {
        let message = fallback_message(None, Some("fix"), "bugfix", "Fix issue");

        assert_eq!(message, "(fix on bugfix) Fix issue");
    }

    #[test]
    fn test_fallback_message_with_a_commit_number() {
        let message = fallback_message(Some(15), Some("feat"), "feature", "Add feature");

        assert_eq!(message, "[15] (feat on feature) Add feature");
    }

    #[test]
    fn test_fallback_message_without_a_commit_type() {
        // No type was selected because the template never renders one.
        let message = fallback_message(Some(15), None, "feature", "Add feature");

        assert_eq!(message, "[15] Add feature");
    }
}
