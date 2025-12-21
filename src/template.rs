//! Template Processing Module for Rona
//!
//! This module handles template parsing and variable substitution for commit messages.
//! It provides a flexible templating system that allows users to customize how their
//! commit messages are formatted using variables.

use chrono::Local;
use regex::Regex;
use std::collections::HashMap;
use std::process::Command;

use crate::errors::{Result, RonaError};

/// Template variables that can be used in commit message templates
#[derive(Debug, Clone)]
pub struct TemplateVariables {
    pub commit_number: Option<u32>,
    pub commit_type: String,
    pub branch_name: String,
    pub message: String,
    pub date: String,
    pub time: String,
    pub author: String,
    pub email: String,
}

impl TemplateVariables {
    /// Creates a new `TemplateVariables` instance with current date/time and git info
    ///
    /// # Errors
    /// * If git author information cannot be retrieved
    pub fn new(
        commit_number: Option<u32>,
        commit_type: String,
        branch_name: String,
        message: String,
    ) -> Result<Self> {
        let now = Local::now();
        let (author, email) = get_git_author_info()?;

        Ok(Self {
            commit_number,
            commit_type,
            branch_name,
            message,
            date: now.format("%Y-%m-%d").to_string(),
            time: now.format("%H:%M:%S").to_string(),
            author,
            email,
        })
    }

    /// Converts the variables to a `HashMap` for template substitution
    #[must_use]
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        map.insert("commit_type".to_string(), self.commit_type.clone());
        map.insert("branch_name".to_string(), self.branch_name.clone());
        map.insert("message".to_string(), self.message.clone());
        map.insert("date".to_string(), self.date.clone());
        map.insert("time".to_string(), self.time.clone());
        map.insert("author".to_string(), self.author.clone());
        map.insert("email".to_string(), self.email.clone());

        if let Some(commit_number) = self.commit_number {
            map.insert("commit_number".to_string(), commit_number.to_string());
        } else {
            map.insert("commit_number".to_string(), String::new());
        }

        map
    }
}

/// Processes conditional blocks in a template string
///
/// Conditional blocks have the syntax: `{?variable_name}content{/variable_name}`
/// The content is only included if the variable has a non-empty value
///
/// # Arguments
/// * `template` - The template string containing conditional blocks
/// * `variables` - The variables to check
///
/// # Returns
/// * `Result<String>` - The template with conditional blocks processed
///
/// # Errors
/// * If the template contains mismatched or invalid conditional blocks
fn process_conditional_blocks(template: &str, variables: &TemplateVariables) -> Result<String> {
    let variable_map = variables.to_map();
    let mut result = template.to_string();

    // Regex to find opening conditional tags: {?variable_name}
    let open_regex = Regex::new(r"\{\?(\w+)\}").map_err(|e| {
        RonaError::Io(std::io::Error::other(format!(
            "Invalid conditional regex: {e}"
        )))
    })?;

    // Process conditional blocks iteratively
    loop {
        // Find the first opening tag
        let open_match = open_regex.find(&result);
        if open_match.is_none() {
            break;
        }

        let open_match = open_match.unwrap();
        let open_start = open_match.start();
        let open_end = open_match.end();

        // Extract variable name from the opening tag
        if let Some(captures) = open_regex.captures(&result[open_start..open_end]) {
            let var_name = captures.get(1).unwrap().as_str();

            // Look for the matching closing tag {/variable_name}
            let close_pattern = format!("{{/{var_name}}}");
            if let Some(close_pos) = result[open_end..].find(&close_pattern) {
                let close_start = open_end + close_pos;
                let close_end = close_start + close_pattern.len();

                // Extract the content between opening and closing tags
                let content = &result[open_end..close_start];

                // Check if variable has a non-empty value
                let has_value = variable_map.get(var_name).is_some_and(|v| !v.is_empty());

                // Replace the entire block
                let replacement = if has_value { content } else { "" };
                let full_block = &result[open_start..close_end];
                result = result.replace(full_block, replacement);
            } else {
                return Err(RonaError::Io(std::io::Error::other(format!(
                    "Unclosed conditional block: {{?{var_name}}}"
                ))));
            }
        }
    }

    Ok(result)
}

/// Processes a template string by substituting variables with their values
///
/// # Arguments
/// * `template` - The template string containing variables in {`variable_name`} format
/// * `variables` - The variables to substitute
///
/// # Returns
/// * `Result<String>` - The processed template with variables substituted
///
/// # Errors
/// * If the template contains invalid variable syntax
/// * If required variables are missing
pub fn process_template(template: &str, variables: &TemplateVariables) -> Result<String> {
    // First, process conditional blocks
    let after_conditionals = process_conditional_blocks(template, variables)?;

    let variable_map = variables.to_map();

    // Find all variables in the template
    let regex = Regex::new(r"\{([^}]+)\}").map_err(|e| {
        RonaError::Io(std::io::Error::other(format!(
            "Invalid template regex: {e}"
        )))
    })?;

    let mut result = after_conditionals.clone();

    // Replace each variable with its value
    for capture in regex.captures_iter(&after_conditionals) {
        if let Some(variable_name) = capture.get(1) {
            let var_name = variable_name.as_str();
            let empty_string = String::new();
            let value = variable_map.get(var_name).unwrap_or(&empty_string);
            result = result.replace(&capture[0], value);
        }
    }

    Ok(result)
}

/// Validates a template string to ensure it contains only valid variables
/// and properly matched conditional blocks
///
/// # Arguments
/// * `template` - The template string to validate
///
/// # Returns
/// * `Result<()>` - Ok if valid, Err if invalid variables found or mismatched blocks
///
/// # Errors
/// * If the template contains unknown variables
/// * If conditional blocks are mismatched or malformed
pub fn validate_template(template: &str) -> Result<()> {
    let valid_variables = [
        "commit_number",
        "commit_type",
        "branch_name",
        "message",
        "date",
        "time",
        "author",
        "email",
    ];

    // First, validate conditional blocks are properly matched
    let conditional_regex = Regex::new(r"\{\?(\w+)\}").map_err(|e| {
        RonaError::Io(std::io::Error::other(format!(
            "Invalid conditional regex: {e}"
        )))
    })?;

    let closing_regex = Regex::new(r"\{/(\w+)\}")
        .map_err(|e| RonaError::Io(std::io::Error::other(format!("Invalid closing regex: {e}"))))?;

    // Collect all opening and closing tags
    let open_tags: Vec<(usize, &str)> = conditional_regex
        .captures_iter(template)
        .filter_map(|cap| {
            let pos = cap.get(0)?.start();
            let name = cap.get(1)?.as_str();
            Some((pos, name))
        })
        .collect();

    let mut close_tags: Vec<(usize, &str)> = closing_regex
        .captures_iter(template)
        .filter_map(|cap| {
            let pos = cap.get(0)?.start();
            let name = cap.get(1)?.as_str();
            Some((pos, name))
        })
        .collect();

    // Check that each opening tag has a matching closing tag
    for (open_pos, open_name) in &open_tags {
        let matching_close = close_tags
            .iter()
            .position(|(close_pos, close_name)| close_pos > open_pos && close_name == open_name);

        let Some(matching_close_idx) = matching_close else {
            return Err(RonaError::Io(std::io::Error::other(format!(
                "Unclosed conditional block: {{?{open_name}}}"
            ))));
        };

        // Validate that the variable in the conditional block is valid
        if !valid_variables.contains(open_name) {
            return Err(RonaError::Io(std::io::Error::other(format!(
                "Unknown variable in conditional block: {{?{open_name}}}. Valid variables are: {}",
                valid_variables.join(", ")
            ))));
        }

        close_tags.remove(matching_close_idx);
    }

    // Check for unmatched closing tags
    if !close_tags.is_empty() {
        let (_, unmatched_name) = close_tags[0];
        return Err(RonaError::Io(std::io::Error::other(format!(
            "Unmatched closing tag: {{/{unmatched_name}}}"
        ))));
    }

    // Now validate regular variables (excluding conditional syntax)
    let regex = Regex::new(r"\{([^}?/]+)\}").map_err(|e| {
        RonaError::Io(std::io::Error::other(format!(
            "Invalid template regex: {e}"
        )))
    })?;

    for capture in regex.captures_iter(template) {
        if let Some(variable_name) = capture.get(1) {
            let var_name = variable_name.as_str();
            // Skip if it's part of a conditional block syntax
            if var_name.starts_with('?') || var_name.starts_with('/') {
                continue;
            }
            if !valid_variables.contains(&var_name) {
                return Err(RonaError::Io(std::io::Error::other(format!(
                    "Unknown template variable: {{{var_name}}}. Valid variables are: {}",
                    valid_variables.join(", ")
                ))));
            }
        }
    }

    Ok(())
}

/// Gets the current git author name and email
fn get_git_author_info() -> Result<(String, String)> {
    let name_output = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .map_err(|e| {
            RonaError::Io(std::io::Error::other(format!(
                "Failed to get git user name: {e}"
            )))
        })?;

    let email_output = Command::new("git")
        .args(["config", "user.email"])
        .output()
        .map_err(|e| {
            RonaError::Io(std::io::Error::other(format!(
                "Failed to get git user email: {e}"
            )))
        })?;

    let name = String::from_utf8_lossy(&name_output.stdout)
        .trim()
        .to_string();
    let email = String::from_utf8_lossy(&email_output.stdout)
        .trim()
        .to_string();

    Ok((name, email))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_processing() {
        let template = "[{commit_number}] ({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: Some(42),
            commit_type: "feat".to_string(),
            branch_name: "feature/new-feature".to_string(),
            message: "Add new functionality".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(
            result,
            "[42] (feat on feature/new-feature) Add new functionality"
        );
    }

    #[test]
    fn test_template_without_commit_number() {
        let template = "({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "fix".to_string(),
            branch_name: "main".to_string(),
            message: "Fix bug".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "(fix on main) Fix bug");
    }

    #[test]
    fn test_template_validation_valid() {
        let template = "[{commit_number}] ({commit_type} on {branch_name}) {message}";
        assert!(validate_template(template).is_ok());
    }

    #[test]
    fn test_template_validation_invalid() {
        let template = "[{commit_number}] ({invalid_var} on {branch_name}) {message}";
        assert!(validate_template(template).is_err());
    }

    #[test]
    fn test_template_variables_to_map() {
        let variables = TemplateVariables {
            commit_number: Some(42),
            commit_type: "feat".to_string(),
            branch_name: "feature/test".to_string(),
            message: "Test message".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test Author".to_string(),
            email: "test@example.com".to_string(),
        };

        let map = variables.to_map();
        assert_eq!(map.get("commit_number").unwrap(), "42");
        assert_eq!(map.get("commit_type").unwrap(), "feat");
        assert_eq!(map.get("branch_name").unwrap(), "feature/test");
        assert_eq!(map.get("message").unwrap(), "Test message");
        assert_eq!(map.get("date").unwrap(), "2024-01-15");
        assert_eq!(map.get("time").unwrap(), "14:30:00");
        assert_eq!(map.get("author").unwrap(), "Test Author");
        assert_eq!(map.get("email").unwrap(), "test@example.com");
    }

    #[test]
    fn test_template_with_all_variables() {
        let template = "{commit_type}: {message} by {author} <{email}> on {branch_name} at {date} {time} (#{commit_number})";
        let variables = TemplateVariables {
            commit_number: Some(123),
            commit_type: "fix".to_string(),
            branch_name: "hotfix/critical-bug".to_string(),
            message: "Fix critical authentication bug".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Jane Doe".to_string(),
            email: "jane@company.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(
            result,
            "fix: Fix critical authentication bug by Jane Doe <jane@company.com> on hotfix/critical-bug at 2024-01-15 14:30:00 (#123)"
        );
    }

    #[test]
    fn test_template_with_emoji() {
        let template = "🚀 {commit_type}: {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "feat".to_string(),
            branch_name: "feature/new-feature".to_string(),
            message: "Add new feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "🚀 feat: Add new feature");
    }

    #[test]
    fn test_template_without_commit_number_variable() {
        let template = "({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update documentation".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "(docs on main) Update documentation");
    }

    #[test]
    fn test_template_validation_with_unknown_variable() {
        let template = "[{commit_number}] ({unknown_var} on {branch_name}) {message}";
        let result = validate_template(template);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown template variable")
        );
    }

    /// REGRESSION TEST: This test would have caught the bug where using the default template
    /// with `no_commit_number` flag would produce empty brackets "[]"
    #[test]
    fn test_default_template_with_none_commit_number_produces_empty_brackets() {
        // This is the BUG - using default template with None commit_number
        let template = "[{commit_number}] ({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();

        // This demonstrates the bug: empty brackets appear
        assert_eq!(result, "[] (docs on main) Update docs");

        // The output should NOT contain empty brackets
        assert!(
            result.contains("[]"),
            "This test documents the bug: empty brackets appear when commit_number is None"
        );
    }

    /// REGRESSION TEST: Verify that using the correct template avoids empty brackets
    #[test]
    fn test_template_without_commit_number_placeholder_avoids_empty_brackets() {
        // This is the FIX - use appropriate template without commit_number placeholder
        let template = "({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "main".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();

        // Correct output without empty brackets
        assert_eq!(result, "(docs on main) Update docs");

        // Verify no empty brackets
        assert!(
            !result.contains("[]"),
            "Output should not contain empty brackets"
        );
        assert!(
            !result.contains("[{"),
            "Output should not contain unprocessed template variables"
        );
    }

    /// REGRESSION TEST: Test multiple scenarios with None `commit_number`
    #[test]
    fn test_various_templates_with_none_commit_number() {
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Jane Doe".to_string(),
            email: "jane@example.com".to_string(),
        };

        // Test template WITH commit_number placeholder (produces empty brackets - the bug)
        let template_with = "[{commit_number}] {commit_type}: {message}";
        let result_with = process_template(template_with, &variables).unwrap();
        assert!(
            result_with.starts_with("[]"),
            "Bug: produces empty brackets"
        );

        // Test template WITHOUT commit_number placeholder (correct)
        let template_without = "{commit_type}: {message}";
        let result_without = process_template(template_without, &variables).unwrap();
        assert_eq!(result_without, "feat: Add feature");
        assert!(
            !result_without.contains("[]"),
            "Should not contain empty brackets"
        );

        // Test template with optional-style syntax (shows limitation of current implementation)
        let template_prefix = "#{commit_number} {commit_type}: {message}";
        let result_prefix = process_template(template_prefix, &variables).unwrap();
        assert_eq!(
            result_prefix, "# feat: Add feature",
            "Empty string for None values"
        );
    }

    /// REGRESSION TEST: Verify `commit_number` `to_map` behavior
    #[test]
    fn test_variables_to_map_with_none_commit_number() {
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "test".to_string(),
            branch_name: "testing".to_string(),
            message: "Test message".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        let map = variables.to_map();

        // When commit_number is None, it should map to empty string
        assert_eq!(map.get("commit_number").unwrap(), "");
        assert_eq!(map.get("commit_type").unwrap(), "test");

        // This empty string is what causes the bug when used in "[{commit_number}]"
    }

    // CONDITIONAL BLOCK TESTS

    #[test]
    fn test_conditional_block_with_value() {
        let template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: Some(42),
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "[42] (feat on new-feature) Add feature");
    }

    #[test]
    fn test_conditional_block_without_value() {
        let template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        // The conditional block should be completely removed, including the space after it
        assert_eq!(result, "(feat on new-feature) Add feature");
        // Verify no empty brackets
        assert!(!result.contains("[]"));
    }

    #[test]
    fn test_multiple_conditional_blocks() {
        let template = "{?commit_number}[{commit_number}]{/commit_number} {?date}on {date}{/date} ({commit_type}) {message}";
        let variables = TemplateVariables {
            commit_number: Some(5),
            commit_type: "fix".to_string(),
            branch_name: "bugfix".to_string(),
            message: "Fix bug".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Jane Doe".to_string(),
            email: "jane@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "[5] on 2024-01-15 (fix) Fix bug");
    }

    #[test]
    fn test_multiple_conditional_blocks_partial() {
        let template = "{?commit_number}[{commit_number}]{/commit_number} {?author}by {author}{/author} - {message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "docs".to_string(),
            branch_name: "docs".to_string(),
            message: "Update docs".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        // commit_number is None, so first block removed; author has value, so second block kept
        assert_eq!(result, " by Alice - Update docs");
    }

    #[test]
    fn test_conditional_block_with_static_text() {
        let template = "{?commit_number}Commit #{commit_number}: {/commit_number}{message}";
        let variables = TemplateVariables {
            commit_number: Some(100),
            commit_type: "chore".to_string(),
            branch_name: "main".to_string(),
            message: "Update dependencies".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Bob".to_string(),
            email: "bob@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "Commit #100: Update dependencies");
    }

    #[test]
    fn test_conditional_block_validation_valid() {
        let template =
            "{?commit_number}[{commit_number}] {/commit_number}({commit_type}) {message}";
        assert!(validate_template(template).is_ok());
    }

    #[test]
    fn test_conditional_block_validation_unclosed() {
        let template = "{?commit_number}[{commit_number}] ({commit_type}) {message}";
        let result = validate_template(template);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unclosed conditional block")
        );
    }

    #[test]
    fn test_conditional_block_validation_unmatched_closing() {
        let template = "[{commit_number}] {/commit_number}({commit_type}) {message}";
        let result = validate_template(template);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unmatched closing tag")
        );
    }

    #[test]
    fn test_conditional_block_validation_invalid_variable() {
        let template = "{?invalid_var}[{invalid_var}]{/invalid_var} {message}";
        let result = validate_template(template);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown variable in conditional block")
        );
    }

    #[test]
    fn test_conditional_block_empty_string_variable() {
        // Test that empty string is treated as "no value"
        let template = "{?commit_number}[{commit_number}] {/commit_number}{message}";
        let variables = TemplateVariables {
            commit_number: None,
            commit_type: "test".to_string(),
            branch_name: "test".to_string(),
            message: "Test".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Tester".to_string(),
            email: "test@example.com".to_string(),
        };

        let result = process_template(template, &variables).unwrap();
        assert_eq!(result, "Test");
        assert!(!result.contains("[]"));
    }

    #[test]
    fn test_original_bug_fix() {
        // This is the original problem: using -n flag should not produce empty brackets
        let template = "{?commit_number}[{commit_number}] {/commit_number}({commit_type} on {branch_name}) {message}";

        // Scenario 1: With commit number (normal flow)
        let with_number = TemplateVariables {
            commit_number: Some(42),
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Dev".to_string(),
            email: "dev@example.com".to_string(),
        };

        let result_with = process_template(template, &with_number).unwrap();
        assert_eq!(result_with, "[42] (feat on new-feature) Add feature");

        // Scenario 2: Without commit number (-n flag)
        let without_number = TemplateVariables {
            commit_number: None,
            commit_type: "feat".to_string(),
            branch_name: "new-feature".to_string(),
            message: "Add feature".to_string(),
            date: "2024-01-15".to_string(),
            time: "14:30:00".to_string(),
            author: "Dev".to_string(),
            email: "dev@example.com".to_string(),
        };

        let result_without = process_template(template, &without_number).unwrap();
        assert_eq!(result_without, "(feat on new-feature) Add feature");
        // CRITICAL: No empty brackets!
        assert!(!result_without.contains("[]"));
        assert!(!result_without.starts_with("[]"));
    }
}
