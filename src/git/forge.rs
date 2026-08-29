//! Forge Detection Module
//!
//! Parses git remote URLs into a host, an owner path, and a repository name, and infers
//! which hosting platform (a "forge") is behind them. This is what lets `rona pr` pick a
//! backend without the user configuring one.
//!
//! Like the rest of [`crate::git`], every operation shells out to the `git` binary so that
//! credentials, hooks, and `insteadOf` rewrites all behave exactly as they do on the
//! command line.

use std::process::Command;

use crate::errors::{GitError, Result, RonaError};

/// A code hosting platform inferred from a git remote URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    /// github.com or a GitHub Enterprise host.
    GitHub,
    /// gitlab.com or a self-hosted GitLab instance.
    GitLab,
    /// bitbucket.org or a Bitbucket Server host.
    Bitbucket,
    /// A host rona could not classify.
    Unknown,
}

impl Forge {
    /// The lowercase name used in config files and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
            Self::Bitbucket => "bitbucket",
            Self::Unknown => "unknown",
        }
    }

    /// Parses a forge name from a config value or a command line flag.
    ///
    /// Returns `None` when the name is not recognised.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "github" | "gh" => Some(Self::GitHub),
            "gitlab" | "glab" => Some(Self::GitLab),
            "bitbucket" => Some(Self::Bitbucket),
            _ => None,
        }
    }

    /// Infers the forge from a hostname.
    ///
    /// Self-hosted instances rarely carry the product name in their domain, so an
    /// unrecognised host yields [`Forge::Unknown`] and the user is expected to set
    /// `pr_forge` in their config.
    #[must_use]
    pub fn from_host(host: &str) -> Self {
        let host = host.to_lowercase();

        if host.contains("github") {
            Self::GitHub
        } else if host.contains("gitlab") {
            Self::GitLab
        } else if host.contains("bitbucket") {
            Self::Bitbucket
        } else {
            Self::Unknown
        }
    }

    /// The term this forge uses for a change proposal, for use in user-facing messages.
    #[must_use]
    pub const fn change_request_name(self) -> &'static str {
        match self {
            Self::GitLab => "merge request",
            _ => "pull request",
        }
    }
}

/// A git remote URL broken into its addressable parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    /// The hosting platform behind `host`.
    pub forge: Forge,
    /// Hostname with any port and user info stripped, e.g. `github.com`.
    pub host: String,
    /// Everything between the host and the repository name. May contain `/` for
    /// GitLab subgroups, e.g. `group/subgroup`.
    pub owner: String,
    /// Repository name with any `.git` suffix removed.
    pub repo: String,
}

impl RemoteInfo {
    /// The full project path, e.g. `rona-rs/rona` or `group/subgroup/project`.
    #[must_use]
    pub fn project_path(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// The browsable HTTPS URL of the repository.
    #[must_use]
    pub fn web_url(&self) -> String {
        format!("https://{}/{}", self.host, self.project_path())
    }
}

/// Parses a git remote URL into its parts.
///
/// Handles the three forms git accepts: scp-like (`git@host:owner/repo.git`), a URL with a
/// scheme (`https://host/owner/repo.git`, `ssh://git@host:22/owner/repo.git`), and a bare
/// `host:owner/repo`. Returns `None` when no repository name can be extracted.
#[must_use]
pub fn parse_remote_url(url: &str) -> Option<RemoteInfo> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    // Strip the scheme, remembering whether there was one: a scp-like URL separates the host
    // from the path with `:`, whereas in a real URL a `:` only ever introduces a port.
    let (rest, had_scheme) = url
        .split_once("://")
        .map_or((url, false), |(_, rest)| (rest, true));

    // Drop any `user@` or `user:password@` prefix.
    let rest = rest.rsplit_once('@').map_or(rest, |(_, after)| after);

    let (host_part, path) = if had_scheme {
        rest.split_once('/')?
    } else {
        // scp-like: the first `:` separates host from path. Fall back to `/` so that a
        // path-only remote does not silently parse as a host.
        rest.split_once(':').or_else(|| rest.split_once('/'))?
    };

    // A scheme URL may carry a port, and a scp-like URL may start its path with a digit
    // group (`ssh://git@host:22/owner/repo`), which the split above already removed.
    let host = host_part.split(':').next()?.trim();
    if host.is_empty() {
        return None;
    }

    // `ssh://git@host:22/owner/repo` leaves `22/owner/repo` when the port sits in the path
    // half of a scp-like parse. Drop a leading all-digit segment in that case.
    let path = path.trim_start_matches('/');
    let path = path
        .split_once('/')
        .filter(|(first, _)| !first.is_empty() && first.chars().all(|c| c.is_ascii_digit()))
        .map_or(path, |(_, tail)| tail);

    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);

    let (owner, repo) = path.rsplit_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some(RemoteInfo {
        forge: Forge::from_host(host),
        host: host.to_string(),
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// Reads the URL configured for a remote.
///
/// # Arguments
/// * `remote` - Remote name, e.g. `origin`
///
/// # Errors
/// * If git cannot be run
/// * If the remote is not configured
pub fn get_remote_url(remote: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", remote])
        .output()?;

    if !output.status.success() {
        return Err(RonaError::Git(GitError::NoRemoteConfigured));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        return Err(RonaError::Git(GitError::NoRemoteConfigured));
    }

    Ok(url)
}

/// Reads a remote's URL and parses it.
///
/// # Arguments
/// * `remote` - Remote name, e.g. `origin`
///
/// # Errors
/// * If the remote is not configured
/// * If the URL cannot be parsed into an owner and a repository name
pub fn detect_remote(remote: &str) -> Result<RemoteInfo> {
    let url = get_remote_url(remote)?;

    parse_remote_url(&url).ok_or_else(|| {
        RonaError::InvalidInput(format!(
            "Could not parse the URL of remote '{remote}': {url}"
        ))
    })
}

/// Returns the default branch of a remote, without contacting the network.
///
/// Reads `refs/remotes/<remote>/HEAD` first, which git writes at clone time, then falls
/// back to the first of `main`, `master`, or `develop` that exists as a remote branch.
/// Returns `None` when the repository has never been fetched.
#[must_use]
pub fn default_branch(remote: &str) -> Option<String> {
    let head_ref = format!("refs/remotes/{remote}/HEAD");
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", &head_ref])
        .output()
        .ok()?;

    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // `symbolic-ref --short` yields `origin/main`; keep only the branch part.
        if let Some(branch) = value.strip_prefix(&format!("{remote}/"))
            && !branch.is_empty()
        {
            return Some(branch.to_string());
        }
    }

    ["main", "master", "develop"]
        .into_iter()
        .find(|candidate| remote_branch_exists(remote, candidate))
        .map(ToString::to_string)
}

/// Reports whether `refs/remotes/<remote>/<branch>` exists locally.
fn remote_branch_exists(remote: &str, branch: &str) -> bool {
    let reference = format!("refs/remotes/{remote}/{branch}");

    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &reference])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Reports whether the current branch tracks an upstream branch.
#[must_use]
pub fn has_upstream() -> bool {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Returns the subject line of the most recent commit, or an empty string when the
/// repository has no commits yet.
#[must_use]
pub fn last_commit_subject() -> String {
    Command::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Reports whether an executable of this name can be run.
///
/// Spawning `<name> --version` is used rather than scanning `PATH` directly so that shell
/// wrappers, shims, and absolute paths given in config all resolve the same way git does.
#[must_use]
pub fn binary_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parses_scp_like_url() -> TestResult {
        let info = parse_remote_url("git@github.com:rona-rs/rona.git").ok_or("should parse")?;

        assert_eq!(info.forge, Forge::GitHub);
        assert_eq!(info.host, "github.com");
        assert_eq!(info.owner, "rona-rs");
        assert_eq!(info.repo, "rona");
        Ok(())
    }

    #[test]
    fn parses_https_url() -> TestResult {
        let info = parse_remote_url("https://github.com/rona-rs/rona.git").ok_or("should parse")?;

        assert_eq!(info.host, "github.com");
        assert_eq!(info.project_path(), "rona-rs/rona");
        Ok(())
    }

    #[test]
    fn parses_https_url_without_git_suffix() -> TestResult {
        let info = parse_remote_url("https://gitlab.com/group/project").ok_or("should parse")?;

        assert_eq!(info.forge, Forge::GitLab);
        assert_eq!(info.project_path(), "group/project");
        Ok(())
    }

    #[test]
    fn parses_url_with_credentials() -> TestResult {
        let info = parse_remote_url("https://user:token@bitbucket.org/team/repo.git")
            .ok_or("should parse")?;

        assert_eq!(info.forge, Forge::Bitbucket);
        assert_eq!(info.host, "bitbucket.org");
        assert_eq!(info.project_path(), "team/repo");
        Ok(())
    }

    #[test]
    fn parses_ssh_url_with_port() -> TestResult {
        let info = parse_remote_url("ssh://git@gitlab.example.com:2222/group/sub/project.git")
            .ok_or("should parse")?;

        assert_eq!(info.host, "gitlab.example.com");
        assert_eq!(info.owner, "group/sub");
        assert_eq!(info.repo, "project");
        Ok(())
    }

    #[test]
    fn keeps_gitlab_subgroups_in_the_owner() -> TestResult {
        let info =
            parse_remote_url("git@gitlab.com:group/subgroup/project.git").ok_or("should parse")?;

        assert_eq!(info.owner, "group/subgroup");
        assert_eq!(info.repo, "project");
        assert_eq!(info.web_url(), "https://gitlab.com/group/subgroup/project");
        Ok(())
    }

    #[test]
    fn unknown_host_is_not_classified() -> TestResult {
        let info = parse_remote_url("git@git.example.com:team/repo.git").ok_or("should parse")?;

        assert_eq!(info.forge, Forge::Unknown);
        Ok(())
    }

    #[test]
    fn rejects_urls_without_a_repository() {
        assert!(parse_remote_url("").is_none());
        assert!(parse_remote_url("git@github.com:").is_none());
        assert!(parse_remote_url("https://github.com/").is_none());
    }

    #[test]
    fn forge_parses_config_names_and_aliases() {
        assert_eq!(Forge::parse("GitHub"), Some(Forge::GitHub));
        assert_eq!(Forge::parse("glab"), Some(Forge::GitLab));
        assert_eq!(Forge::parse(" bitbucket "), Some(Forge::Bitbucket));
        assert_eq!(Forge::parse("gitea"), None);
    }

    #[test]
    fn gitlab_calls_it_a_merge_request() {
        assert_eq!(Forge::GitLab.change_request_name(), "merge request");
        assert_eq!(Forge::GitHub.change_request_name(), "pull request");
    }
}
