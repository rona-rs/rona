//! `rona pr`: opens a pull (or merge) request for the current branch.
//!
//! The request is assembled the same way whatever the forge is - remote, target, title, and
//! description - and only the last step differs: `gh`, `glab`, GitLab push options, or a
//! pre-filled web form. Command line flags always win over the config, and anything left
//! unset falls back to a value derived from the repository.

use std::{fs::read_to_string, path::Path};

use colored::Colorize;

use crate::{
    cli::PrArgs,
    config::{Config, DEFAULT_PR_TITLE_TEMPLATE},
    errors::{Result, RonaError},
    extra_fields::ExtraField,
    git::{
        Forge, RemoteInfo, add_to_git_exclude, default_branch, detect_remote, get_current_branch,
        get_top_level_path,
    },
    pr::{
        PR_DESCRIPTION_FILE_PATH, PrBackend, PrRequest, body_file_path, branch_type_of,
        compose_description, default_title_source, find_description_templates, push_source_branch,
        resolve_backend, split_title_and_body, submit, write_body_file,
    },
    prompt::{self, BuiltInPrompt},
    template::{PrTemplateVariables, process_pr_template, references, validate_pr_template},
};

/// Opens a pull or merge request for the current branch.
///
/// # Errors
/// * If the remote is missing, unparsable, or of an unknown forge
/// * If the target branch cannot be determined
/// * If the user cancels a prompt
/// * If the backend fails to open the request
pub(crate) fn open(args: &PrArgs, config: &Config) -> Result<()> {
    let (remote, info) = resolve_remote(args, config)?;
    let source_branch = get_current_branch()?;
    let target_branch = resolve_target(args, config, &remote, &source_branch)?;
    let backend = resolve_pr_backend(args, config, &info)?;

    // The title template seeds the document's heading, which the user can then edit in place.
    let seed_title = build_title(args, config, &source_branch, &target_branch)?;
    let document = prepare_document(args, config, &info, &seed_title)?;
    let (heading, body) = split_title_and_body(&document);

    let title = choose_title(args.title.as_deref(), heading.as_deref(), &seed_title);

    if title.is_empty() {
        println!(
            "{} No title found. Give the request a `# Heading` or pass --title.",
            "WARNING:".yellow().bold()
        );
        return Ok(());
    }

    let body_path = if config.dry_run {
        body_file_path()?
    } else {
        write_body_file(body.trim())?
    };

    let request = PrRequest {
        title,
        body: body.trim().to_string(),
        source_branch,
        target_branch,
        remote,
        draft: args.draft || config.project_config.pr_draft,
        labels: merged_values(&args.labels, &config.project_config.pr_labels),
        reviewers: merged_values(&args.reviewers, &config.project_config.pr_reviewers),
        assignees: merged_values(&args.assignees, &config.project_config.pr_assignees),
    };

    warn_unsupported_fields(backend, &request, info.forge);

    if !args.yes && !config.dry_run && !confirm(&request, backend, &info) {
        println!("Request cancelled.");
        return Ok(());
    }

    // Push options carry the branch themselves; every other backend needs it on the remote first.
    if !args.no_push && !backend.pushes_branch() {
        push_source_branch(&request, config.verbose, config.dry_run)?;
    }

    let url = submit(
        &request,
        backend,
        &info,
        &body_path,
        args.yes,
        config.verbose,
        config.dry_run,
    )?;

    // A dry run has already printed the command it would have run, URL included.
    if let Some(url) = url
        && !config.dry_run
    {
        println!("\n{} {url}", "✓".green());
    }

    Ok(())
}

/// Resolves the remote a request is opened against, and the forge behind it.
///
/// A `pr_forge` config value overrides detection, which is what self-hosted instances need
/// when their hostname does not name the product.
///
/// # Errors
/// * If the remote is not configured or its URL cannot be parsed
/// * If `pr_forge` names something rona does not know
fn resolve_remote(args: &PrArgs, config: &Config) -> Result<(String, RemoteInfo)> {
    let remote = args
        .remote
        .clone()
        .or_else(|| config.project_config.pr_remote.clone())
        .unwrap_or_else(|| "origin".to_string());

    let mut info = detect_remote(&remote)?;

    if let Some(name) = &config.project_config.pr_forge {
        info.forge = Forge::parse(name).ok_or_else(|| {
            RonaError::InvalidInput(format!(
                "Unknown pr_forge '{name}'. Use github, gitlab, or bitbucket."
            ))
        })?;
    }

    Ok((remote, info))
}

/// Resolves the branch a request targets.
///
/// # Errors
/// * If no target can be determined
/// * If the target is the branch the request is opened from
fn resolve_target(args: &PrArgs, config: &Config, remote: &str, source: &str) -> Result<String> {
    let target = args
        .target
        .clone()
        .or_else(|| config.project_config.pr_target.clone())
        .or_else(|| default_branch(remote))
        .ok_or_else(|| {
            RonaError::InvalidInput(format!(
                "Could not determine the target branch of '{remote}'. \
                 Pass --target or set `pr_target` in your rona config."
            ))
        })?;

    if target == source {
        return Err(RonaError::InvalidInput(format!(
            "'{source}' cannot target itself. Switch to your feature branch, or pass --target."
        )));
    }

    Ok(target)
}

/// Resolves the backend used to open the request.
///
/// `--web` is a shorthand for `--backend browser` and wins over the config.
///
/// # Errors
/// * If a configured backend name is not recognised
/// * If the forge is unknown and no backend was configured
fn resolve_pr_backend(args: &PrArgs, config: &Config, info: &RemoteInfo) -> Result<PrBackend> {
    if args.web {
        return Ok(PrBackend::Browser);
    }

    let configured = args
        .backend
        .clone()
        .or_else(|| config.project_config.pr_backend.clone());

    let backend = match configured {
        None => PrBackend::Auto,
        Some(name) => PrBackend::parse(&name).ok_or_else(|| {
            RonaError::InvalidInput(format!(
                "Unknown pr backend '{name}'. Use auto, gh, glab, push-options, or browser."
            ))
        })?,
    };

    resolve_backend(backend, info)
}

/// Builds the request title from the flag, or from the template and its prompts.
///
/// # Errors
/// * If the title template is invalid
/// * If the user cancels a prompt
fn build_title(
    args: &PrArgs,
    config: &Config,
    source_branch: &str,
    target_branch: &str,
) -> Result<String> {
    if let Some(title) = &args.title {
        return Ok(title.clone());
    }

    let template = config
        .project_config
        .pr_title_template
        .as_deref()
        .unwrap_or(DEFAULT_PR_TITLE_TEMPLATE);

    let extra_fields: Vec<ExtraField> = prompt::referenced_fields(
        &config.project_config.pr_extra_fields,
        template,
        "PR extra field",
    );

    let extra_names: Vec<&str> = extra_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    validate_pr_template(template, &extra_names)
        .map_err(|e| RonaError::InvalidInput(format!("PR title template validation error: {e}")))?;

    let title_config = config.project_config.pr_title.as_ref();
    let title_seed = default_title_source();
    let (title, extra_values) = prompt::fields(
        &extra_fields,
        &config.project_config.pr_field_order,
        &BuiltInPrompt {
            key: "title",
            default_prompt: "Request title",
            config: title_config,
            default_value: Some(&title_seed),
            needed: references(template, "title") && prompt::is_enabled(title_config),
        },
    )?;

    let variables = PrTemplateVariables::new(
        source_branch.to_string(),
        branch_type_of(source_branch),
        title.trim().to_string(),
        target_branch.to_string(),
        title_seed,
    )?;

    Ok(process_pr_template(template, &variables, &extra_values)?
        .trim()
        .to_string())
}

/// Prepares the request document and returns its content.
///
/// `--body-file` is read as it is. Otherwise `pr_description.md` at the repository root is
/// created, excluded from git, and opened in the configured editor unless `--no-edit` was
/// passed. A new file is seeded with `seed_title` as its heading followed by the repository's
/// own request template, so the whole request is one editable document.
///
/// The returned text still carries the title heading; [`split_title_and_body`] separates them.
///
/// # Errors
/// * If the document cannot be read or written
/// * If the editor cannot be launched
fn prepare_document(
    args: &PrArgs,
    config: &Config,
    info: &RemoteInfo,
    seed_title: &str,
) -> Result<String> {
    if let Some(body_file) = &args.body_file {
        return read_to_string(body_file).map_err(|e| {
            RonaError::Io(std::io::Error::other(format!(
                "Could not read body file '{body_file}': {e}"
            )))
        });
    }

    let root = get_top_level_path()?;
    let path = root.join(PR_DESCRIPTION_FILE_PATH);

    if config.dry_run {
        println!("Would write the request to: {}", path.display());

        return if path.exists() {
            Ok(read_to_string(&path)?)
        } else {
            Ok(compose_description(seed_title, ""))
        };
    }

    if !path.exists() {
        let template = seed_description(&root, info.forge)?;
        std::fs::write(&path, compose_description(seed_title, &template))?;
        add_to_git_exclude(&[PR_DESCRIPTION_FILE_PATH])?;
    }

    if !args.no_edit {
        println!("The first heading is the title; everything below it is the description.");
        config.open_in_editor(&path)?;
    }

    Ok(read_to_string(&path)?)
}

/// Returns the content a new description file starts from.
///
/// Repositories with a single forge template use it. With several, the user picks one. With
/// none, the file starts empty.
///
/// # Errors
/// * If a template file cannot be read
/// * If the user cancels the template picker
fn seed_description(root: &Path, forge: Forge) -> Result<String> {
    let templates = find_description_templates(root, forge);

    let chosen = match templates.len() {
        0 => return Ok(String::new()),
        1 => templates[0].clone(),
        _ => {
            let labels: Vec<String> = templates
                .iter()
                .map(|path| {
                    path.strip_prefix(root)
                        .unwrap_or(path)
                        .display()
                        .to_string()
                })
                .collect();
            let index = prompt::select("Select a description template", &labels)?;
            templates[index].clone()
        }
    };

    println!(
        "Seeded {PR_DESCRIPTION_FILE_PATH} from {}",
        chosen.strip_prefix(root).unwrap_or(&chosen).display()
    );

    Ok(read_to_string(&chosen)?)
}

/// Warns about request fields the chosen backend cannot carry.
///
/// Silence here would be the worst outcome: the request would be created without the labels or
/// reviewers the user asked for, and nothing would say so.
fn warn_unsupported_fields(backend: PrBackend, request: &PrRequest, forge: Forge) {
    let warn = |what: &str, why: &str| {
        println!("{} {what} {why}", "WARNING:".yellow().bold());
    };

    if backend == PrBackend::PushOptions && !request.reviewers.is_empty() {
        warn(
            "Reviewers are dropped:",
            "GitLab push options cannot set them. Use --backend glab instead.",
        );
    }

    if backend == PrBackend::Browser {
        if forge == Forge::GitLab
            && !(request.labels.is_empty()
                && request.reviewers.is_empty()
                && request.assignees.is_empty())
        {
            warn(
                "Labels, reviewers, and assignees are not pre-filled:",
                "the GitLab form takes only the branches, title, and description.",
            );
        }

        if forge == Forge::Bitbucket {
            warn(
                "The title and description are not pre-filled:",
                "the Bitbucket form takes only the branches.",
            );
        }

        if request.draft {
            warn(
                "Draft is not pre-filled:",
                "tick the draft box in the web form.",
            );
        }
    }
}

/// Shows what is about to be opened and asks for confirmation.
fn confirm(request: &PrRequest, backend: PrBackend, info: &RemoteInfo) -> bool {
    let summary = format!(
        "Open a {} on {}\n  {} -> {}\n  Title: {}\n  Backend: {}\nProceed?",
        info.forge.change_request_name(),
        info.project_path(),
        request.source_branch,
        request.target_branch,
        request.title,
        backend.as_str()
    );

    prompt::confirm(&summary, true)
}

/// Chooses the request title from the sources that can supply one.
///
/// The flag wins, then the heading the user wrote in the document, then whatever seeded that
/// heading. Blank candidates are skipped rather than accepted, so `--title ""` falls through to
/// the document instead of leaving the request untitled.
fn choose_title(from_flag: Option<&str>, from_heading: Option<&str>, seed: &str) -> String {
    [from_flag, from_heading, Some(seed)]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|candidate| !candidate.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// Combines command line values with their config defaults, keeping each value once.
fn merged_values(from_flags: &[String], from_config: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = from_config.to_vec();

    for value in from_flags {
        if !merged.contains(value) {
            merged.push(value.clone());
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choose_title_prefers_the_flag() {
        let title = choose_title(Some("From flag"), Some("From heading"), "From seed");

        assert_eq!(title, "From flag");
    }

    #[test]
    fn test_choose_title_falls_back_to_the_heading() {
        let title = choose_title(None, Some("From heading"), "From seed");

        assert_eq!(title, "From heading");
    }

    #[test]
    fn test_choose_title_falls_back_to_the_seed() {
        let title = choose_title(None, None, "From seed");

        assert_eq!(title, "From seed");
    }

    #[test]
    fn test_choose_title_skips_blank_candidates() {
        // An explicitly empty flag must not leave the request untitled.
        assert_eq!(
            choose_title(Some("   "), Some("Heading"), "Seed"),
            "Heading"
        );
        assert_eq!(choose_title(Some(""), None, "Seed"), "Seed");
    }

    #[test]
    fn test_choose_title_trims_whitespace() {
        assert_eq!(choose_title(Some("  Padded  "), None, "Seed"), "Padded");
    }

    #[test]
    fn test_choose_title_is_empty_when_nothing_supplies_one() {
        assert!(choose_title(None, None, "").is_empty());
    }

    #[test]
    fn test_merged_values_keeps_config_first_and_deduplicates() {
        let from_flags = vec!["urgent".to_string(), "bug".to_string()];
        let from_config = vec!["bug".to_string()];

        let merged = merged_values(&from_flags, &from_config);

        assert_eq!(merged, vec!["bug".to_string(), "urgent".to_string()]);
    }

    #[test]
    fn test_merged_values_without_config_defaults() {
        let merged = merged_values(&["bug".to_string()], &[]);

        assert_eq!(merged, vec!["bug".to_string()]);
    }
}
