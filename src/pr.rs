//! Pull / Merge Request Module
//!
//! Builds a forge-agnostic change request payload and hands it to a backend that knows how
//! to submit it. Four backends are supported:
//!
//! - [`PrBackend::Gh`] - the `gh` CLI, for GitHub
//! - [`PrBackend::Glab`] - the `glab` CLI, for GitLab
//! - [`PrBackend::PushOptions`] - `git push -o merge_request.*`, GitLab only, no extra binary
//! - [`PrBackend::Browser`] - open a pre-filled web form, works anywhere, needs no auth
//!
//! Every backend is driven through the `git`, `gh`, or `glab` binaries, matching the rest of
//! rona: authentication, hooks, and enterprise host configuration are inherited from the tools
//! the user has already set up rather than reimplemented here.
//!
//! ## One file for the whole request
//!
//! A request lives in a single Markdown file. Its leading `# Heading` is the title and
//! everything below is the description, so the whole thing can be written, reviewed, and
//! committed as one document:
//!
//! ```markdown
//! # Add the pr command
//!
//! Opens a pull request for the current branch through gh, glab, or the web form.
//! ```
//!
//! [`split_title_and_body`] does the parsing and [`compose_description`] does the reverse,
//! which is how a new description file is seeded.

use std::{
    collections::HashSet,
    fmt::Write as _,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    errors::{Result, RonaError},
    git::{
        Forge, RemoteInfo, binary_exists, find_git_root,
        forge::{has_upstream, last_commit_subject},
        git_push_capture,
    },
};

/// Name of the file holding the change request, written at the repository root.
pub const PR_DESCRIPTION_FILE_PATH: &str = "pr_description.md";

/// Name of the file holding the description alone, written inside the `.git` directory.
///
/// The `gh` backend takes the description as a file. That file cannot be the request document
/// itself, whose first line is the title, so the parsed description is written here instead.
/// It lives in `.git` because it is a handoff detail, not something to edit or commit.
const PR_BODY_FILE_NAME: &str = "rona-pr-body.md";

/// Maximum length of a browser URL before the description is dropped from the query string.
///
/// Browsers and servers disagree on the real limit; 8000 is the smallest value in common use.
const MAX_BROWSER_URL_LEN: usize = 8000;

/// The mechanism used to submit a change request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrBackend {
    /// Pick a backend from the detected forge and the binaries available.
    Auto,
    /// The `gh` CLI.
    Gh,
    /// The `glab` CLI.
    Glab,
    /// GitLab push options carried by `git push`.
    PushOptions,
    /// A pre-filled web form opened in the default browser.
    Browser,
}

impl PrBackend {
    /// The name used in config files and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Gh => "gh",
            Self::Glab => "glab",
            Self::PushOptions => "push-options",
            Self::Browser => "browser",
        }
    }

    /// Parses a backend name from a config value.
    ///
    /// Returns `None` when the name is not recognised.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "gh" | "github" => Some(Self::Gh),
            "glab" | "gitlab" => Some(Self::Glab),
            "push-options" | "push_options" | "push" => Some(Self::PushOptions),
            "browser" | "web" => Some(Self::Browser),
            _ => None,
        }
    }

    /// Reports whether this backend pushes the source branch as part of submitting.
    #[must_use]
    pub const fn pushes_branch(self) -> bool {
        matches!(self, Self::PushOptions)
    }
}

/// A change request, in the shape every backend can render.
#[derive(Debug, Clone)]
pub struct PrRequest {
    /// One line summary.
    pub title: String,
    /// Markdown body. May be empty.
    pub body: String,
    /// Branch holding the changes.
    pub source_branch: String,
    /// Branch the changes are proposed against.
    pub target_branch: String,
    /// Remote the change request is opened on.
    pub remote: String,
    /// Whether to open the request as a draft.
    pub draft: bool,
    /// Labels to apply.
    pub labels: Vec<String>,
    /// Users to request a review from.
    pub reviewers: Vec<String>,
    /// Users to assign.
    pub assignees: Vec<String>,
}

/// Chooses a backend for a remote.
///
/// An explicit `configured` backend other than [`PrBackend::Auto`] is always honoured, even
/// when the matching binary is missing, so that a misconfiguration fails loudly instead of
/// silently falling back to something else.
///
/// # Errors
/// * If the forge is unknown and no backend was configured
/// * If push options are asked for against a forge that ignores them
pub fn resolve_backend(configured: PrBackend, info: &RemoteInfo) -> Result<PrBackend> {
    if configured != PrBackend::Auto {
        // Fail before any prompting: nobody wants to write a description and only then be
        // told the backend cannot post it.
        if configured == PrBackend::PushOptions && info.forge != Forge::GitLab {
            return Err(push_options_forge_error(info));
        }

        return Ok(configured);
    }

    match info.forge {
        Forge::GitHub if binary_exists("gh") => Ok(PrBackend::Gh),
        Forge::GitLab if binary_exists("glab") => Ok(PrBackend::Glab),
        // Push options need no extra binary and no separate login, so they beat the browser.
        Forge::GitLab => Ok(PrBackend::PushOptions),
        Forge::GitHub | Forge::Bitbucket => Ok(PrBackend::Browser),
        Forge::Unknown => Err(RonaError::InvalidInput(format!(
            "Could not tell which forge '{}' is. Set `pr_forge` (github, gitlab, bitbucket) \
             or `pr_backend` in your rona config, or pass --backend.",
            info.host
        ))),
    }
}

/// The error raised when push options are aimed at a forge that ignores them.
fn push_options_forge_error(info: &RemoteInfo) -> RonaError {
    RonaError::InvalidInput(format!(
        "Push options only create merge requests on GitLab, and '{}' is {}. \
         Other forges ignore them, so nothing would be created.",
        info.host,
        info.forge.as_str()
    ))
}

/// Splits a request document into its title and its description.
///
/// The first non-empty line is read as the title when it is a level one heading (`# Title`).
/// Anything else means the document carries no title, and the whole text is the description.
/// The heading and the blank lines after it are removed from the description so the title is
/// never posted twice.
#[must_use]
pub fn split_title_and_body(content: &str) -> (Option<String>, String) {
    // Walk whole lines, keeping the byte offset, so the rest of the document can be sliced out
    // exactly as written: re-joining parsed lines would drop the trailing newline.
    let mut offset = 0;
    let mut heading = None;

    for line in content.split_inclusive('\n') {
        offset += line.len();

        if !line.trim().is_empty() {
            heading = Some(line.trim_end());
            break;
        }
    }

    let Some(heading) = heading else {
        return (None, String::new());
    };

    // Exactly one `#` marks the title. A deeper heading is an ordinary section of the body.
    let Some(title) = heading
        .strip_prefix("# ")
        .map(str::trim)
        .filter(|title| !title.is_empty())
    else {
        return (None, content.to_string());
    };

    // Drop only the blank lines that separate the heading from the body, so indentation and
    // fenced code blocks at the top of the description survive.
    let body = content[offset..].trim_start_matches(['\n', '\r']);

    (Some(title.to_string()), body.to_string())
}

/// Renders a title and a description back into one request document.
#[must_use]
pub fn compose_description(title: &str, body: &str) -> String {
    let body = body.trim_start();

    if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}")
    }
}

/// Writes the description alone to a file the `gh` backend can read.
///
/// # Errors
/// * If the `.git` directory cannot be found or written to
pub fn write_body_file(body: &str) -> Result<PathBuf> {
    let path = find_git_root()?.join(PR_BODY_FILE_NAME);
    std::fs::write(&path, body)?;

    Ok(path)
}

/// The path [`write_body_file`] would write to, for use in a dry run.
///
/// # Errors
/// * If the `.git` directory cannot be found
pub fn body_file_path() -> Result<PathBuf> {
    Ok(find_git_root()?.join(PR_BODY_FILE_NAME))
}

/// Builds the `gh pr create` argument list.
///
/// `body_file` is passed through rather than the body text so that multi-line Markdown
/// survives untouched.
#[must_use]
pub fn gh_args(request: &PrRequest, body_file: &Path, info: &RemoteInfo) -> Vec<String> {
    let mut args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--base".to_string(),
        request.target_branch.clone(),
        "--head".to_string(),
        request.source_branch.clone(),
        "--title".to_string(),
        request.title.clone(),
        "--body-file".to_string(),
        body_file.display().to_string(),
    ];

    // `gh` infers the repository from the remotes it finds. Naming it explicitly is only
    // needed when the user asked for a remote other than the one gh would pick.
    if request.remote != "origin" {
        args.push("--repo".to_string());
        args.push(format!("{}/{}", info.host, info.project_path()));
    }

    if request.draft {
        args.push("--draft".to_string());
    }

    push_repeated(&mut args, "--label", &request.labels);
    push_repeated(&mut args, "--reviewer", &request.reviewers);
    push_repeated(&mut args, "--assignee", &request.assignees);

    args
}

/// Builds the `glab mr create` argument list.
///
/// `glab` takes the description as a string rather than a file, so the body is inlined. It is
/// passed as a single argv entry and never through a shell, so newlines are safe.
#[must_use]
pub fn glab_args(request: &PrRequest, skip_confirmation: bool) -> Vec<String> {
    let mut args = vec![
        "mr".to_string(),
        "create".to_string(),
        "--source-branch".to_string(),
        request.source_branch.clone(),
        "--target-branch".to_string(),
        request.target_branch.clone(),
        "--title".to_string(),
        request.title.clone(),
        "--description".to_string(),
        request.body.clone(),
    ];

    if request.draft {
        args.push("--draft".to_string());
    }

    if skip_confirmation {
        args.push("--yes".to_string());
    }

    push_repeated(&mut args, "--label", &request.labels);
    push_repeated(&mut args, "--reviewer", &request.reviewers);
    push_repeated(&mut args, "--assignee", &request.assignees);

    args
}

/// Builds the `git push` argument list that asks GitLab to open a merge request.
///
/// Reviewers have no push option counterpart and are dropped by the caller with a warning.
#[must_use]
pub fn push_option_args(request: &PrRequest) -> Vec<String> {
    let mut args = vec![
        "--set-upstream".to_string(),
        request.remote.clone(),
        format!("HEAD:{}", request.source_branch),
        "-o".to_string(),
        "merge_request.create".to_string(),
        "-o".to_string(),
        format!("merge_request.target={}", request.target_branch),
        "-o".to_string(),
        format!("merge_request.title={}", request.title),
    ];

    if !request.body.is_empty() {
        args.push("-o".to_string());
        args.push(format!("merge_request.description={}", request.body));
    }

    if request.draft {
        args.push("-o".to_string());
        args.push("merge_request.draft".to_string());
    }

    for label in &request.labels {
        args.push("-o".to_string());
        args.push(format!("merge_request.label={label}"));
    }

    for assignee in &request.assignees {
        args.push("-o".to_string());
        args.push(format!("merge_request.assign={assignee}"));
    }

    args
}

/// Appends `flag value` once per value.
fn push_repeated(args: &mut Vec<String>, flag: &str, values: &[String]) {
    for value in values {
        args.push(flag.to_string());
        args.push(value.clone());
    }
}

/// Percent-encodes a string for use in a URL query value.
///
/// Everything outside the RFC 3986 unreserved set is escaped, so the result is safe in both a
/// path and a query string.
#[must_use]
pub fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                // Writing to a String cannot fail, so the error is not actionable.
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }

    encoded
}

/// Builds a pre-filled web form URL for the request.
///
/// The description is included only while the URL stays under [`MAX_BROWSER_URL_LEN`]; the
/// caller is told through the second return value so it can offer the body another way.
///
/// # Errors
/// * If the forge has no known web form
pub fn browser_url(request: &PrRequest, info: &RemoteInfo) -> Result<(String, bool)> {
    let base = info.web_url();
    let with_body = build_browser_url(request, &base, info.forge, true)?;

    if with_body.len() <= MAX_BROWSER_URL_LEN {
        return Ok((with_body, true));
    }

    Ok((build_browser_url(request, &base, info.forge, false)?, false))
}

/// Renders the web form URL, optionally including the description.
fn build_browser_url(
    request: &PrRequest,
    base: &str,
    forge: Forge,
    include_body: bool,
) -> Result<String> {
    let title = percent_encode(&request.title);
    let source = percent_encode(&request.source_branch);
    let target = percent_encode(&request.target_branch);
    let body = percent_encode(&request.body);

    match forge {
        Forge::GitHub => {
            let mut url = format!(
                "{base}/compare/{}...{}?expand=1&title={title}",
                request.target_branch, request.source_branch
            );
            if include_body && !request.body.is_empty() {
                let _ = write!(url, "&body={body}");
            }
            append_csv_param(&mut url, "labels", &request.labels);
            append_csv_param(&mut url, "reviewers", &request.reviewers);
            append_csv_param(&mut url, "assignees", &request.assignees);
            Ok(url)
        }
        Forge::GitLab => {
            let mut url = format!(
                "{base}/-/merge_requests/new\
                 ?merge_request%5Bsource_branch%5D={source}\
                 &merge_request%5Btarget_branch%5D={target}\
                 &merge_request%5Btitle%5D={title}"
            );
            if include_body && !request.body.is_empty() {
                let _ = write!(url, "&merge_request%5Bdescription%5D={body}");
            }
            Ok(url)
        }
        // Bitbucket's form reads only the branches from the query string.
        Forge::Bitbucket => Ok(format!(
            "{base}/pull-requests/new?source={source}&dest={target}"
        )),
        Forge::Unknown => Err(RonaError::InvalidInput(
            "The browser backend needs a known forge. Set `pr_forge` in your rona config."
                .to_string(),
        )),
    }
}

/// Appends `&name=a,b,c` when `values` is not empty.
fn append_csv_param(url: &mut String, name: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }

    let joined = values
        .iter()
        .map(|value| percent_encode(value))
        .collect::<Vec<String>>()
        .join(",");
    let _ = write!(url, "&{name}={joined}");
}

/// Opens a URL in the user's default browser.
///
/// # Errors
/// * If the platform opener cannot be run
pub fn open_in_browser(url: &str) -> Result<()> {
    let (program, args) = if cfg!(target_os = "macos") {
        ("open", vec![url.to_string()])
    } else if cfg!(target_os = "windows") {
        (
            "cmd",
            vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                url.to_string(),
            ],
        )
    } else {
        ("xdg-open", vec![url.to_string()])
    };

    Command::new(program)
        .args(&args)
        .status()
        .map_err(|e| RonaError::CommandFailed {
            command: format!("Failed to open the browser with '{program}': {e}"),
        })?;

    Ok(())
}

/// Returns the description template files this forge looks for, in discovery order.
///
/// Only paths that exist are returned. A directory of templates yields every Markdown file
/// inside it, so the caller can let the user pick one.
#[must_use]
pub fn find_description_templates(root: &Path, forge: Forge) -> Vec<PathBuf> {
    let single_files: &[&str] = match forge {
        Forge::GitLab => &[
            ".gitlab/merge_request_templates/Default.md",
            ".gitlab/Merge_request_templates/Default.md",
        ],
        _ => &[
            ".github/PULL_REQUEST_TEMPLATE.md",
            ".github/pull_request_template.md",
            "PULL_REQUEST_TEMPLATE.md",
            "pull_request_template.md",
            "docs/PULL_REQUEST_TEMPLATE.md",
        ],
    };

    let directories: &[&str] = match forge {
        Forge::GitLab => &[".gitlab/merge_request_templates"],
        _ => &[".github/PULL_REQUEST_TEMPLATE"],
    };

    let mut candidates: Vec<PathBuf> = single_files
        .iter()
        .map(|candidate| root.join(candidate))
        .filter(|path| path.is_file())
        .collect();

    for directory in directories {
        candidates.extend(markdown_files_in(&root.join(directory)));
    }

    // The same file can be reached by more than one candidate path: the spellings differ only
    // in case, which is a distinct path on Linux but the same file on macOS and Windows, and a
    // directory listing can repeat a file already named above. Compare resolved paths so a
    // template is offered once on every platform.
    let mut seen: HashSet<PathBuf> = HashSet::new();
    candidates.retain(|path| {
        let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        seen.insert(resolved)
    });

    candidates
}

/// Lists the Markdown files directly inside a directory, sorted by name.
fn markdown_files_in(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .collect();

    files.sort();
    files
}

/// Extracts the first URL found in command output.
///
/// Both `gh` and `glab` print the new request's URL on success, and GitLab echoes it in the
/// `remote:` banner of a push. All three are covered by scanning for the first `http` token.
#[must_use]
pub fn extract_url(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|token| token.starts_with("http://") || token.starts_with("https://"))
        .map(|token| {
            token
                .trim_end_matches(['.', ',', ')', '"', '\''])
                .to_string()
        })
}

/// Renders an argument list the way a user would type it, for `--dry-run` output.
#[must_use]
pub fn display_command(program: &str, args: &[String]) -> String {
    let mut rendered = program.to_string();

    for arg in args {
        rendered.push(' ');
        if arg.is_empty() || arg.contains(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
            rendered.push('\'');
            rendered.push_str(&arg.replace('\'', r"'\''"));
            rendered.push('\'');
        } else {
            rendered.push_str(arg);
        }
    }

    rendered
}

/// Pushes the source branch so the forge can see it, setting upstream on first push.
///
/// Backends that push as part of submitting ([`PrBackend::pushes_branch`]) skip this.
///
/// # Errors
/// * If the push is rejected
pub fn push_source_branch(request: &PrRequest, verbose: bool, dry_run: bool) -> Result<()> {
    let args = if has_upstream() {
        Vec::new()
    } else {
        vec![
            "--set-upstream".to_string(),
            request.remote.clone(),
            format!("HEAD:{}", request.source_branch),
        ]
    };

    crate::git::git_push(&args, verbose, dry_run)
}

/// Runs a backend binary and returns its combined output.
///
/// Output is streamed to the terminal as well, because `gh` and `glab` ask their own
/// questions when something is missing.
fn run_backend(program: &str, args: &[String]) -> Result<String> {
    let output =
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| RonaError::CommandFailed {
                command: format!("Failed to run '{program}': {e}. Is it installed?"),
            })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        crate::errors::pretty_print_error(&stderr);
        return Err(RonaError::CommandFailed {
            command: format!("{program} {}", args.first().cloned().unwrap_or_default()),
        });
    }

    if !stdout.trim().is_empty() {
        println!("{}", stdout.trim());
    }

    Ok(format!("{stdout}\n{stderr}"))
}

/// Submits the change request through `backend`.
///
/// Returns the URL of the created request when the backend reports one.
///
/// # Arguments
/// * `request` - The payload to submit
/// * `backend` - The already resolved backend (never [`PrBackend::Auto`])
/// * `info` - The parsed remote the request is opened against
/// * `body_file` - File holding the description, used by the `gh` backend
/// * `skip_confirmation` - Whether the user passed `--yes`
/// * `verbose` - Whether to let git write straight to the terminal
/// * `dry_run` - Print the command instead of running it
///
/// # Errors
/// * If the backend binary is missing or exits non-zero
/// * If push options are requested against a non-GitLab remote
pub fn submit(
    request: &PrRequest,
    backend: PrBackend,
    info: &RemoteInfo,
    body_file: &Path,
    skip_confirmation: bool,
    verbose: bool,
    dry_run: bool,
) -> Result<Option<String>> {
    match backend {
        PrBackend::Auto => Err(RonaError::InvalidInput(
            "Backend must be resolved before submitting.".to_string(),
        )),
        PrBackend::Gh => {
            let args = gh_args(request, body_file, info);
            if dry_run {
                println!("Would run: {}", display_command("gh", &args));
                return Ok(None);
            }
            Ok(extract_url(&run_backend("gh", &args)?))
        }
        PrBackend::Glab => {
            let args = glab_args(request, skip_confirmation);
            if dry_run {
                println!("Would run: {}", display_command("glab", &args));
                return Ok(None);
            }
            Ok(extract_url(&run_backend("glab", &args)?))
        }
        PrBackend::PushOptions => submit_with_push_options(request, info, verbose, dry_run),
        PrBackend::Browser => {
            let (url, body_included) = browser_url(request, info)?;
            if dry_run {
                println!("Would open: {url}");
                return Ok(Some(url));
            }
            if !body_included && !request.body.is_empty() {
                println!(
                    "[NOTE] The description is too long for a URL. The form opens without it; \
                     it is still in {PR_DESCRIPTION_FILE_PATH}."
                );
            }
            open_in_browser(&url)?;
            Ok(Some(url))
        }
    }
}

/// Submits through GitLab push options.
fn submit_with_push_options(
    request: &PrRequest,
    info: &RemoteInfo,
    verbose: bool,
    dry_run: bool,
) -> Result<Option<String>> {
    if info.forge != Forge::GitLab {
        return Err(push_options_forge_error(info));
    }

    let args = push_option_args(request);

    if dry_run {
        println!("Would run: {}", display_command("git push", &args));
        return Ok(None);
    }

    let output = git_push_capture(&args, verbose)?;

    // GitLab prints the merge request URL in the `remote:` banner of the push.
    Ok(extract_url(&output))
}

/// Returns the branch prefix used as `{branch_type}`, e.g. `feat` for `feat/login`.
#[must_use]
pub fn branch_type_of(branch: &str) -> String {
    branch
        .split_once('/')
        .map_or_else(String::new, |(prefix, _)| prefix.to_string())
}

/// Returns the subject of the most recent commit, for use as the default title.
#[must_use]
pub fn default_title_source() -> String {
    last_commit_subject()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> PrRequest {
        PrRequest {
            title: "Add the pr command".to_string(),
            body: "Body line one.\nBody line two.".to_string(),
            source_branch: "feat/pr".to_string(),
            target_branch: "main".to_string(),
            remote: "origin".to_string(),
            draft: false,
            labels: vec![],
            reviewers: vec![],
            assignees: vec![],
        }
    }

    fn github_remote() -> RemoteInfo {
        RemoteInfo {
            forge: Forge::GitHub,
            host: "github.com".to_string(),
            owner: "rona-rs".to_string(),
            repo: "rona".to_string(),
        }
    }

    fn gitlab_remote() -> RemoteInfo {
        RemoteInfo {
            forge: Forge::GitLab,
            host: "gitlab.com".to_string(),
            owner: "group".to_string(),
            repo: "project".to_string(),
        }
    }

    #[test]
    fn backend_parses_names_and_aliases() {
        assert_eq!(PrBackend::parse("auto"), Some(PrBackend::Auto));
        assert_eq!(PrBackend::parse("GitHub"), Some(PrBackend::Gh));
        assert_eq!(
            PrBackend::parse("push_options"),
            Some(PrBackend::PushOptions)
        );
        assert_eq!(PrBackend::parse("web"), Some(PrBackend::Browser));
        assert_eq!(PrBackend::parse("carrier-pigeon"), None);
    }

    #[test]
    fn explicit_backend_survives_resolution() -> Result<()> {
        let resolved = resolve_backend(PrBackend::Browser, &github_remote())?;

        assert_eq!(resolved, PrBackend::Browser);
        Ok(())
    }

    #[test]
    fn push_options_are_refused_before_any_prompting() {
        // Resolution must fail on a GitHub remote, not later at submit time.
        assert!(resolve_backend(PrBackend::PushOptions, &github_remote()).is_err());
        assert!(resolve_backend(PrBackend::PushOptions, &gitlab_remote()).is_ok());
    }

    #[test]
    fn unknown_forge_cannot_be_resolved_automatically() {
        let info = RemoteInfo {
            forge: Forge::Unknown,
            host: "git.example.com".to_string(),
            owner: "team".to_string(),
            repo: "repo".to_string(),
        };

        assert!(resolve_backend(PrBackend::Auto, &info).is_err());
    }

    #[test]
    fn gh_args_carry_the_base_head_and_body_file() {
        let args = gh_args(
            &sample_request(),
            Path::new("pr_description.md"),
            &github_remote(),
        );

        assert!(args.starts_with(&["pr".to_string(), "create".to_string()]));
        assert!(args.contains(&"--base".to_string()));
        assert!(args.contains(&"main".to_string()));
        assert!(args.contains(&"feat/pr".to_string()));
        assert!(args.contains(&"pr_description.md".to_string()));
        // The default remote is left to gh so that fork setups keep working.
        assert!(!args.contains(&"--repo".to_string()));
    }

    #[test]
    fn gh_args_name_the_repo_for_a_non_default_remote() {
        let mut request = sample_request();
        request.remote = "upstream".to_string();

        let args = gh_args(&request, Path::new("pr_description.md"), &github_remote());

        assert!(args.contains(&"--repo".to_string()));
        assert!(args.contains(&"github.com/rona-rs/rona".to_string()));
    }

    #[test]
    fn gh_args_repeat_the_flag_for_each_reviewer() {
        let mut request = sample_request();
        request.reviewers = vec!["alice".to_string(), "bob".to_string()];
        request.draft = true;

        let args = gh_args(&request, Path::new("body.md"), &github_remote());

        assert_eq!(
            args.iter().filter(|arg| *arg == "--reviewer").count(),
            2,
            "each reviewer needs its own flag"
        );
        assert!(args.contains(&"--draft".to_string()));
    }

    #[test]
    fn glab_args_inline_the_description() {
        let args = glab_args(&sample_request(), false);

        assert!(args.starts_with(&["mr".to_string(), "create".to_string()]));
        assert!(args.contains(&"Body line one.\nBody line two.".to_string()));
        assert!(!args.contains(&"--yes".to_string()));
    }

    #[test]
    fn glab_args_skip_confirmation_on_demand() {
        assert!(glab_args(&sample_request(), true).contains(&"--yes".to_string()));
    }

    #[test]
    fn push_options_request_a_merge_request() {
        let args = push_option_args(&sample_request());

        assert!(args.contains(&"merge_request.create".to_string()));
        assert!(args.contains(&"merge_request.target=main".to_string()));
        assert!(args.contains(&"merge_request.title=Add the pr command".to_string()));
        assert!(args.contains(&"HEAD:feat/pr".to_string()));
    }

    #[test]
    fn push_options_omit_an_empty_description() {
        let mut request = sample_request();
        request.body = String::new();

        let args = push_option_args(&request);

        assert!(
            !args
                .iter()
                .any(|arg| arg.starts_with("merge_request.description")),
            "an empty description must not be sent"
        );
    }

    #[test]
    fn push_options_repeat_labels_and_assignees() {
        let mut request = sample_request();
        request.labels = vec!["bug".to_string(), "urgent".to_string()];
        request.assignees = vec!["alice".to_string()];
        request.draft = true;

        let args = push_option_args(&request);

        assert!(args.contains(&"merge_request.label=bug".to_string()));
        assert!(args.contains(&"merge_request.label=urgent".to_string()));
        assert!(args.contains(&"merge_request.assign=alice".to_string()));
        assert!(args.contains(&"merge_request.draft".to_string()));
    }

    #[test]
    fn push_options_are_refused_outside_gitlab() {
        let error = submit_with_push_options(&sample_request(), &github_remote(), false, true);

        assert!(error.is_err(), "GitHub ignores push options silently");
    }

    #[test]
    fn a_leading_heading_is_the_title() {
        let (title, body) = split_title_and_body("# Add the pr command\n\nDoes the thing.\n");

        assert_eq!(title, Some("Add the pr command".to_string()));
        assert_eq!(body, "Does the thing.\n");
    }

    #[test]
    fn blank_lines_before_the_heading_are_tolerated() {
        let (title, body) = split_title_and_body("\n\n# Title\n\nBody.");

        assert_eq!(title, Some("Title".to_string()));
        assert_eq!(body, "Body.");
    }

    #[test]
    fn a_document_can_be_a_title_alone() {
        let (title, body) = split_title_and_body("# Just a title\n");

        assert_eq!(title, Some("Just a title".to_string()));
        assert!(body.is_empty());
    }

    #[test]
    fn a_deeper_heading_is_body_not_title() {
        // `## Summary` is an ordinary section, which is how most forge templates open.
        let document = "## Summary\n\nWhat changed.";
        let (title, body) = split_title_and_body(document);

        assert_eq!(title, None);
        assert_eq!(body, document, "the body must survive untouched");
    }

    #[test]
    fn a_document_without_a_heading_is_all_body() {
        let document = "Just prose, no heading.\n";
        let (title, body) = split_title_and_body(document);

        assert_eq!(title, None);
        assert_eq!(body, document);
    }

    #[test]
    fn an_html_comment_before_the_heading_blocks_extraction() {
        // The first non-empty line decides, so a template opening with a comment keeps its
        // whole text as the body rather than losing a line to a silent misparse.
        let document = "<!-- notes -->\n# Not a title\n";
        let (title, body) = split_title_and_body(document);

        assert_eq!(title, None);
        assert_eq!(body, document);
    }

    #[test]
    fn an_empty_heading_is_not_a_title() {
        let (title, _) = split_title_and_body("#\n\nBody.");
        assert_eq!(title, None);

        let (title, _) = split_title_and_body("#   \n\nBody.");
        assert_eq!(title, None);
    }

    #[test]
    fn only_the_first_heading_is_taken() {
        let (title, body) = split_title_and_body("# First\n\nText.\n\n# Second\n");

        assert_eq!(title, Some("First".to_string()));
        assert!(
            body.contains("# Second"),
            "a later heading belongs to the body"
        );
    }

    #[test]
    fn an_empty_document_has_neither_title_nor_body() {
        let (title, body) = split_title_and_body("");

        assert_eq!(title, None);
        assert!(body.is_empty());
    }

    #[test]
    fn composing_and_splitting_round_trip() {
        let document = compose_description("Add the pr command", "Does the thing.\n");
        let (title, body) = split_title_and_body(&document);

        assert_eq!(title, Some("Add the pr command".to_string()));
        assert_eq!(body, "Does the thing.\n");
    }

    #[test]
    fn composing_without_a_body_leaves_only_the_heading() {
        assert_eq!(compose_description("Title", ""), "# Title\n");
        assert_eq!(compose_description("Title", "\n\n"), "# Title\n");
    }

    #[test]
    fn percent_encoding_escapes_everything_reserved() {
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("feat/pr"), "feat%2Fpr");
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(percent_encode("100%"), "100%25");
    }

    #[test]
    fn github_browser_url_prefills_the_compare_form() -> Result<()> {
        let (url, body_included) = browser_url(&sample_request(), &github_remote())?;

        assert!(url.starts_with("https://github.com/rona-rs/rona/compare/main...feat/pr"));
        assert!(url.contains("expand=1"));
        assert!(url.contains("title=Add%20the%20pr%20command"));
        assert!(body_included);
        Ok(())
    }

    #[test]
    fn gitlab_browser_url_uses_the_new_merge_request_form() -> Result<()> {
        let (url, _) = browser_url(&sample_request(), &gitlab_remote())?;

        assert!(url.starts_with("https://gitlab.com/group/project/-/merge_requests/new"));
        assert!(url.contains("merge_request%5Bsource_branch%5D=feat%2Fpr"));
        assert!(url.contains("merge_request%5Btarget_branch%5D=main"));
        Ok(())
    }

    #[test]
    fn an_oversized_body_is_dropped_from_the_url() -> Result<()> {
        let mut request = sample_request();
        request.body = "x".repeat(MAX_BROWSER_URL_LEN + 1);

        let (url, body_included) = browser_url(&request, &github_remote())?;

        assert!(!body_included);
        assert!(!url.contains("&body="));
        Ok(())
    }

    #[test]
    fn browser_backend_needs_a_known_forge() {
        let info = RemoteInfo {
            forge: Forge::Unknown,
            host: "git.example.com".to_string(),
            owner: "team".to_string(),
            repo: "repo".to_string(),
        };

        assert!(browser_url(&sample_request(), &info).is_err());
    }

    #[test]
    fn urls_are_recovered_from_command_output() {
        assert_eq!(
            extract_url("https://github.com/rona-rs/rona/pull/42\n"),
            Some("https://github.com/rona-rs/rona/pull/42".to_string())
        );
        assert_eq!(
            extract_url(
                "remote: View merge request for feat/pr:\nremote:   https://gitlab.com/g/p/-/merge_requests/7"
            ),
            Some("https://gitlab.com/g/p/-/merge_requests/7".to_string())
        );
        assert_eq!(extract_url("nothing to see here"), None);
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_the_url() {
        assert_eq!(
            extract_url("Created at https://example.com/pr/1."),
            Some("https://example.com/pr/1".to_string())
        );
    }

    #[test]
    fn dry_run_commands_quote_arguments_with_spaces() {
        let rendered = display_command(
            "gh",
            &["--title".to_string(), "Add the pr command".to_string()],
        );

        assert_eq!(rendered, "gh --title 'Add the pr command'");
    }

    #[test]
    fn branch_type_is_the_prefix_before_the_first_slash() {
        assert_eq!(branch_type_of("feat/pr"), "feat");
        assert_eq!(branch_type_of("feat/scope/pr"), "feat");
        assert_eq!(branch_type_of("main"), "");
    }

    #[test]
    fn only_push_options_push_the_branch_themselves() {
        assert!(PrBackend::PushOptions.pushes_branch());
        assert!(!PrBackend::Gh.pushes_branch());
        assert!(!PrBackend::Browser.pushes_branch());
    }

    #[test]
    fn description_templates_are_discovered_per_forge() -> std::result::Result<(), std::io::Error> {
        let temp = tempfile::tempdir()?;
        let github_dir = temp.path().join(".github");
        std::fs::create_dir_all(&github_dir)?;
        std::fs::write(github_dir.join("PULL_REQUEST_TEMPLATE.md"), "# Template")?;

        let found = find_description_templates(temp.path(), Forge::GitHub);
        assert_eq!(
            found.len(),
            1,
            "one file must be offered once, however the filesystem cases paths"
        );

        // A GitHub template must not be offered for a GitLab remote.
        assert!(find_description_templates(temp.path(), Forge::GitLab).is_empty());
        Ok(())
    }

    #[test]
    fn a_directory_of_templates_yields_every_markdown_file()
    -> std::result::Result<(), std::io::Error> {
        let temp = tempfile::tempdir()?;
        let dir = temp.path().join(".gitlab/merge_request_templates");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("Bugfix.md"), "bug")?;
        std::fs::write(dir.join("Feature.md"), "feature")?;
        std::fs::write(dir.join("notes.txt"), "ignored")?;

        let found = find_description_templates(temp.path(), Forge::GitLab);

        assert_eq!(found.len(), 2, "only Markdown files count");
        Ok(())
    }
}
