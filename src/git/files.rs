//! File and Exclusion Handling
//!
//! Git file operations including exclusion patterns, ignore file processing,
//! and file management utilities.

use std::{
    collections::HashSet,
    fs::{File, OpenOptions, read_to_string},
    io::Write,
    path::Path,
};

use crate::{
    errors::Result,
    git::{COMMIT_MESSAGE_FILE_PATH, find_git_root, get_top_level_path},
};

const COMMITIGNORE_FILE_PATH: &str = ".commitignore";
const GITIGNORE_FILE_PATH: &str = ".gitignore";

/// Add paths to the `.git/info/exclude` file.
///
/// # Arguments
/// * `paths` - List of paths to add to the exclude file.
///
/// # Errors
/// * If the file cannot be read/opened/written to.
///
/// # Returns
/// * `Result<(), std::io::Error>` - Result of the operation.
pub fn add_to_git_exclude(paths: &[&str]) -> Result<()> {
    let git_root_path = find_git_root()?;
    let info_dir = git_root_path.join("info");
    let exclude_file = info_dir.join("exclude");

    // Ensure the info directory exists
    if !info_dir.exists() {
        std::fs::create_dir_all(&info_dir)?;
    }

    // Read existing content to avoid duplicates
    let content = if exclude_file.exists() {
        read_to_string(&exclude_file)?
    } else {
        String::new()
    };

    // Parse existing paths in the file
    let existing_paths: HashSet<&str> = content
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();

    // Filter paths that are not already in the file
    let paths_to_add: Vec<&str> = paths
        .iter()
        .filter(|path| !existing_paths.contains(*path))
        .copied()
        .collect();

    if paths_to_add.is_empty() {
        return Ok(());
    }

    // Open a file in `append` and `create` mode
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(exclude_file)?;

    // Add a marker if it's not already there
    if !content.contains("# Added by git-commit-rust") {
        if !content.is_empty() {
            writeln!(file)?;
        }
        writeln!(file, "# Added by git-commit-rust")?;
    }

    // Add each new path
    for path in paths_to_add {
        writeln!(file, "{path}")?;
    }

    Ok(())
}

/// Creates the necessary files in the git repository root.
///
/// # Errors
/// * If the files cannot be created.
/// * If the git add command fails.
pub fn create_needed_files() -> Result<()> {
    let project_root = get_top_level_path()?;

    let commit_file_path = Path::new(&project_root).join(COMMIT_MESSAGE_FILE_PATH);
    let commitignore_file_path = Path::new(&project_root).join(COMMITIGNORE_FILE_PATH);

    if !commit_file_path.exists() {
        File::create(commit_file_path)?;
    }

    if !commitignore_file_path.exists() {
        File::create(commitignore_file_path)?;
    }

    add_to_git_exclude(&[COMMIT_MESSAGE_FILE_PATH, COMMITIGNORE_FILE_PATH])?;

    Ok(())
}

/// Gets all patterns from commitignore and gitignore files.
///
/// # Errors
/// * If reading the ignored files fails
///
/// # Returns
/// * A vector of patterns to ignore
pub fn get_ignore_patterns() -> Result<Vec<String>> {
    let commitignore_path = Path::new(COMMITIGNORE_FILE_PATH);

    if !commitignore_path.exists() {
        return Ok(Vec::new());
    }

    let mut patterns = process_gitignore_file()?;
    patterns.append(&mut process_gitignore_file()?);

    Ok(patterns)
}

/// Processes the gitignore file.
///
/// # Errors
/// * If the gitignore file is not found
/// * If the gitignore file cannot be read
/// * If the gitignore file contains invalid patterns
///
/// # Returns
/// * `Result<Vec<String>, Error>` - The files and folders to ignore or an error message
pub fn process_gitignore_file() -> Result<Vec<String>> {
    // look for the gitignore file
    let gitignore_file_path = Path::new(GITIGNORE_FILE_PATH);
    //
    if !gitignore_file_path.exists() {
        return Ok(Vec::new());
    }

    let git_ignore_file_contents = read_to_string(gitignore_file_path)?;

    extract_filenames(&git_ignore_file_contents, r"^([^#]\S*)$")
}

// Use the shared extract_filenames function from the parent module
use super::extract_filenames;
