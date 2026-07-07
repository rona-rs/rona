//! Shared interactive-prompt theme for Rona.
//!
//! `dialoguer` has no global render configuration, so every prompt is created with
//! `::with_theme(&prompt_theme())`. This module centralises the colours and icons so all
//! prompts stay visually consistent (light cyan prompts, light magenta answers,
//! light blue highlights).

use dialoguer::{
    console::{Style, style},
    theme::ColorfulTheme,
};

/// Build the shared [`ColorfulTheme`] used by every interactive prompt.
///
/// Starts from the crate default and overrides prefixes and styles to match Rona's
/// look: `$` prompt prefix, `✓`/`✕` success and error markers, and light cyan/magenta
/// accents.
#[must_use]
pub fn prompt_theme() -> ColorfulTheme {
    ColorfulTheme {
        // Input prompt label: light cyan, bold.
        prompt_style: Style::new().for_stderr().cyan().bright().bold(),
        // Prompt / success / error prefixes.
        prompt_prefix: style("$".to_string()).for_stderr().red().bright(),
        success_prefix: style("✓".to_string()).for_stderr().green().bright(),
        error_prefix: style("✕".to_string()).for_stderr().red().bright(),
        // Help / hint text under the input.
        hint_style: Style::new().for_stderr().yellow().italic(),
        // Echoed answer after submit: light magenta, bold.
        values_style: Style::new().for_stderr().magenta().bright().bold(),
        // Highlighted option in select lists.
        active_item_prefix: style("⮕".to_string()).for_stderr().blue().bright(),
        // Multi-select checkboxes.
        checked_item_prefix: style("[x]".to_string()).for_stderr().green().bright(),
        unchecked_item_prefix: style("[ ]".to_string()).for_stderr().black(),
        ..ColorfulTheme::default()
    }
}
