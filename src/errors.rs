use thiserror::Error;

/// Main error type for the Rona application
#[derive(Error, Debug)]
pub enum RonaError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Git error: {0}")]
    Git(#[from] GitError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Operation cancelled by user")]
    UserCancelled,

    #[error("Command execution failed: {command}")]
    CommandFailed { command: String },
}

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error while accessing config: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Regex compilation error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Configuration file not found at expected location")]
    ConfigNotFound,

    #[error("Configuration file already exists - use 'rona set-editor' to modify")]
    ConfigAlreadyExists,

    #[error("Invalid configuration format - please check your config.toml syntax")]
    InvalidConfig,

    #[error("Could not determine home directory - please set HOME environment variable")]
    HomeDirNotFound,

    #[error("Unsupported editor: {editor}. Supported editors: vim, zed, nano")]
    UnsupportedEditor { editor: String },
}

/// Git-related errors
#[derive(Error, Debug)]
pub enum GitError {
    #[error("IO error during git operation: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Git2 error: {0}")]
    Git2Error(#[from] git2::Error),

    #[error("Not in a git repository - please run this command from within a git repository")]
    RepositoryNotFound,

    #[error("Git command failed: {command}\nOutput: {output}")]
    CommandFailed { command: String, output: String },

    #[error("Invalid git status output format: {output}")]
    InvalidStatus { output: String },

    #[error("Commit message file 'commit_message.md' not found - run 'rona generate' first")]
    CommitMessageNotFound,

    #[error("Failed to process .gitignore file: {reason}")]
    GitignoreError { reason: String },

    #[error("Failed to process .commitignore file: {reason}")]
    CommitignoreError { reason: String },

    #[error("No staged changes to commit - use 'rona add-with-exclude' to stage files")]
    NoStagedChanges,

    #[error("Working directory is not clean - commit or stash your changes first")]
    DirtyWorkingDirectory,

    #[error("Remote repository not configured - add a remote with 'git remote add origin <url>'")]
    NoRemoteConfigured,
}

/// Type alias for Result using `RonaError`
pub type Result<T> = std::result::Result<T, RonaError>;

// Manual From implementation for git2::Error
impl From<git2::Error> for RonaError {
    fn from(err: git2::Error) -> Self {
        RonaError::Git(GitError::Git2Error(err))
    }
}

/// Formats and prints error messages in a clean, readable format.
///
/// This function takes an error message and formats it for display by:
/// - Removing empty lines
/// - Trimming whitespace from each line
/// - Printing each non-empty line
///
/// If the error message contains only empty lines, it prints a default message
/// indicating no additional information is available.
///
/// # Arguments
///
/// * `error_message` - A borrowed string containing the error message to format
/// ```
pub fn pretty_print_error(error_message: &str) {
    println!("-------------------");

    if error_message.lines().all(|line| line.trim().is_empty()) {
        println!("No additional information provided.");
    } else {
        for line in error_message.lines() {
            if !line.trim().is_empty() {
                println!("{}", line.trim());
            }
        }
    }

    println!("-------------------");
}
