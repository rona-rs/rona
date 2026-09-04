//! `rona branch`: creates a branch whose name comes from the configured branch template.

use colored::Colorize;

use crate::{
    config::{Config, DEFAULT_BRANCH_TEMPLATE},
    errors::{Result, RonaError},
    extra_fields::ExtraField,
    git::{git_branch_only, git_create_branch, sanitize_branch_name},
    prompt::{self, BuiltInPrompt},
    template::{
        BranchTemplateVariables, process_branch_template, references, validate_branch_template,
    },
};

/// Creates a new branch, prompting for the parts its template needs.
///
/// Only the variables the template references are prompted for, so a template that drops the
/// type or the description does not ask for them.
///
/// # Arguments
/// * `no_switch` - Whether to create the branch without switching to it
/// * `config` - Global configuration including dry-run settings
///
/// # Errors
/// * If the template is invalid or renders an empty name
/// * If the user cancels a prompt
/// * If branch creation fails
pub(crate) fn create(no_switch: bool, config: &Config) -> Result<()> {
    let template = config
        .project_config
        .branch_template
        .as_deref()
        .unwrap_or(DEFAULT_BRANCH_TEMPLATE);

    let needs_branch_type = references(template, "branch_type");
    let needs_description = references(template, "description");

    let extra_fields = branch_extra_fields(template, config);

    // Validate the template before prompting, so a broken template is reported up front
    // instead of after the user has answered every prompt.
    let extra_names: Vec<&str> = extra_fields.iter().map(|f| f.name.as_str()).collect();
    validate_branch_template(template, &extra_names)
        .map_err(|e| RonaError::InvalidInput(format!("Branch template validation error: {e}")))?;

    let branch_type = if needs_branch_type {
        let types = config.project_config.branch_type_choices();
        let index = prompt::select("Select branch type", &types)?;
        types[index].to_string()
    } else {
        String::new()
    };

    let description_config = config.project_config.branch_description.as_ref();
    let (description, extra_values) = prompt::fields(
        &extra_fields,
        &config.project_config.branch_field_order,
        &BuiltInPrompt {
            key: "description",
            default_prompt: "Branch description",
            config: description_config,
            default_value: None,
            needed: needs_description && prompt::is_enabled(description_config),
        },
    )?;

    if needs_description && description.trim().is_empty() {
        println!(
            "{} Empty description provided. Exiting.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    }

    let variables = BranchTemplateVariables::new(branch_type, description.trim().to_owned())?;
    let raw_name = process_branch_template(template, &variables, &extra_values)?;
    let branch_name = sanitize_branch_name(&raw_name);

    if branch_name.is_empty() {
        return Err(RonaError::InvalidInput(
            "Generated branch name is empty after sanitization.".to_string(),
        ));
    }

    if config.dry_run {
        println!("Would create branch: {branch_name}");
        if no_switch {
            println!("Would not switch to the new branch.");
        } else {
            println!("Would switch to the new branch.");
        }
        return Ok(());
    }

    if no_switch {
        git_branch_only(&branch_name)?;
        println!("Branch created: {branch_name}");
    } else {
        git_create_branch(&branch_name)?;
        println!("Switched to new branch: {branch_name}");
    }

    Ok(())
}

/// Returns the extra fields to prompt for when rendering `template`.
///
/// Branch fields the template does not reference are skipped (with a note), and commit fields
/// are pulled in when the branch template references them by name, so a ticket declared once
/// under `[[commit_extra_fields]]` can be reused in branch names.
fn branch_extra_fields(template: &str, config: &Config) -> Vec<ExtraField> {
    let mut fields = prompt::referenced_fields(
        &config.project_config.branch_extra_fields,
        template,
        "Branch extra field",
    );

    for commit_field in &config.project_config.commit_extra_fields {
        let already_present = fields.iter().any(|f| f.name == commit_field.name);
        if !already_present && references(template, &commit_field.name) {
            fields.push(commit_field.clone());
        }
    }

    fields
}
