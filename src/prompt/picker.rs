//! Fuzzy picker that also accepts a value the list does not hold.
//!
//! The candidates of a select field are suggestions, not a closed set: typing a value that is
//! not in the list offers it as a `Create "..."` row, so a brand new scope costs the same
//! keystrokes as an existing one instead of a detour through a manual-entry option.
//!
//! `dialoguer` has no such prompt ([`dialoguer::FuzzySelect`] can only return one of the items it
//! was built with), so this module drives the terminal itself. Every line is still rendered
//! through the shared [`crate::theme::prompt_theme`], so the picker looks like the other prompts.

use dialoguer::{
    console::{Key, Term, truncate_str},
    theme::{ColorfulTheme, Theme},
};

use crate::errors::{Result, RonaError};

/// Label of the row that skips an optional field.
const NONE_LABEL: &str = "(none)";

/// A row of the picker, and what accepting it means.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    /// One of the candidates, accepted as it is.
    Candidate(String),
    /// The typed text, accepted as a new value.
    Create(String),
    /// Skip the field entirely; only offered to optional fields.
    Skip,
}

impl Row {
    /// Text shown for the row.
    fn label(&self) -> String {
        match self {
            Self::Candidate(value) => value.clone(),
            Self::Create(value) => format!("Create \"{value}\""),
            Self::Skip => NONE_LABEL.to_string(),
        }
    }

    /// Value the row yields, or `None` when it skips the field.
    fn value(&self) -> Option<&str> {
        match self {
            Self::Candidate(value) | Self::Create(value) => Some(value),
            Self::Skip => None,
        }
    }
}

/// Builds the rows shown for `query`, best match first.
///
/// An empty query lists every candidate in prefetch order, followed by the skip row when the field
/// is optional. A non-empty query keeps the matching candidates and appends a `Create` row unless a
/// candidate already holds that exact text: the best match keeps the selection, and once the filter
/// has emptied the list the `Create` row is the only one left, so a new value is a plain Enter away.
fn rows(candidates: &[String], query: &str, allow_skip: bool) -> Vec<Row> {
    if query.is_empty() {
        let mut rows: Vec<Row> = candidates.iter().cloned().map(Row::Candidate).collect();
        if allow_skip {
            rows.push(Row::Skip);
        }
        return rows;
    }

    let mut matches: Vec<(usize, i32, &String)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(position, candidate)| {
            score(candidate, query).map(|score| (position, score, candidate))
        })
        .collect();

    // Best score first, ties broken by the order the prefetch returned them in.
    matches.sort_by(|(left_pos, left_score, _), (right_pos, right_score, _)| {
        right_score.cmp(left_score).then(left_pos.cmp(right_pos))
    });

    let mut rows: Vec<Row> = matches
        .into_iter()
        .map(|(_, _, candidate)| Row::Candidate(candidate.clone()))
        .collect();

    if !candidates.iter().any(|candidate| candidate == query) {
        rows.push(Row::Create(query.to_string()));
    }

    rows
}

/// Scores `candidate` against `query`, or `None` when it does not match at all.
///
/// Whole-string equality beats a prefix, a prefix beats a substring, and a substring beats a
/// scattered subsequence; within a rank the shorter or earlier-matching candidate wins.
fn score(candidate: &str, query: &str) -> Option<i32> {
    let candidate = candidate.to_lowercase();
    let query = query.to_lowercase();
    let length = i32::try_from(candidate.chars().count()).unwrap_or(i32::MAX);

    if candidate == query {
        return Some(4000);
    }

    if let Some(position) = candidate.find(&query) {
        let position = i32::try_from(position).unwrap_or(i32::MAX);
        return Some(if position == 0 {
            3000 - length
        } else {
            2000 - position
        });
    }

    is_subsequence(&candidate, &query).then(|| 1000 - length)
}

/// Whether every char of `needle` appears in `haystack`, in order.
fn is_subsequence(haystack: &str, needle: &str) -> bool {
    let mut haystack = haystack.chars();
    needle
        .chars()
        .all(|wanted| haystack.any(|current| current == wanted))
}

/// Byte offset of the `index`-th char, or the end of the string.
fn byte_position(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(offset, _)| offset)
}

/// Turns a rendering failure into an error, since the theme writes into a `String`.
fn render_error(error: std::fmt::Error) -> RonaError {
    RonaError::InvalidInput(format!("Failed to render prompt: {error}"))
}

/// Terminal state of one running picker.
struct Picker<'a> {
    /// Terminal the prompt is drawn on; stderr, like every other prompt.
    term: Term,
    /// Shared prompt theme, used for every rendered line.
    theme: ColorfulTheme,
    /// Label shown before the typed text.
    prompt: &'a str,
    /// Values offered by the prefetch, in the order it returned them.
    candidates: &'a [String],
    /// Whether the skip row is offered.
    allow_skip: bool,
    /// Text typed so far, which both filters the list and feeds the `Create` row.
    query: String,
    /// Caret position in `query`, counted in chars.
    cursor: usize,
    /// Index of the highlighted row.
    selected: usize,
    /// Index of the first rendered row, so a long list can scroll.
    offset: usize,
    /// Validation message shown under the list, cleared as soon as the query changes.
    error: Option<String>,
    /// Number of lines the last frame wrote, so the next one can clear them.
    drawn: usize,
}

impl Picker<'_> {
    /// Runs the prompt, restoring the cursor whatever the outcome.
    fn run(
        &mut self,
        validate: &dyn Fn(&str) -> std::result::Result<(), String>,
    ) -> Result<Option<String>> {
        self.term.hide_cursor()?;
        let outcome = self.interact(validate);
        let _ = self.term.show_cursor();
        outcome
    }

    /// Draws the list and handles keys until the user accepts a row or cancels.
    fn interact(
        &mut self,
        validate: &dyn Fn(&str) -> std::result::Result<(), String>,
    ) -> Result<Option<String>> {
        loop {
            let rows = rows(self.candidates, &self.query, self.allow_skip);
            self.selected = self.selected.min(rows.len().saturating_sub(1));
            self.render(&rows)?;

            match self.term.read_key()? {
                Key::Escape | Key::CtrlC => {
                    self.clear()?;
                    return Err(RonaError::UserCancelled);
                }
                Key::Enter => {
                    let Some(row) = rows.get(self.selected) else {
                        continue;
                    };
                    let Some(value) = row.value() else {
                        self.report(NONE_LABEL)?;
                        return Ok(None);
                    };
                    match validate(value) {
                        Ok(()) => {
                            let value = value.to_string();
                            self.report(&value)?;
                            return Ok(Some(value));
                        }
                        Err(message) => self.error = Some(message),
                    }
                }
                Key::ArrowUp | Key::BackTab => self.move_selection(-1, rows.len()),
                Key::ArrowDown | Key::Tab => self.move_selection(1, rows.len()),
                Key::ArrowLeft => self.cursor = self.cursor.saturating_sub(1),
                Key::ArrowRight => self.cursor = (self.cursor + 1).min(self.query.chars().count()),
                Key::Home => self.cursor = 0,
                Key::End => self.cursor = self.query.chars().count(),
                Key::Backspace if self.cursor > 0 => {
                    self.cursor -= 1;
                    let at = byte_position(&self.query, self.cursor);
                    self.query.remove(at);
                    self.reset_filter();
                }
                Key::Del if self.cursor < self.query.chars().count() => {
                    let at = byte_position(&self.query, self.cursor);
                    self.query.remove(at);
                    self.reset_filter();
                }
                Key::Char(typed) if !typed.is_ascii_control() => {
                    let at = byte_position(&self.query, self.cursor);
                    self.query.insert(at, typed);
                    self.cursor += 1;
                    self.reset_filter();
                }
                _ => {}
            }
        }
    }

    /// Moves the highlight by `delta`, wrapping around the list.
    fn move_selection(&mut self, delta: isize, length: usize) {
        if length == 0 {
            return;
        }
        let length = isize::try_from(length).unwrap_or(isize::MAX);
        let current = isize::try_from(self.selected).unwrap_or(0);
        self.selected = usize::try_from((current + delta).rem_euclid(length)).unwrap_or(0);
    }

    /// Restarts the list from the top; the typed text just changed, so the old state is stale.
    fn reset_filter(&mut self) {
        self.selected = 0;
        self.offset = 0;
        self.error = None;
    }

    /// Redraws the prompt line, the visible rows, and any validation message.
    fn render(&mut self, rows: &[Row]) -> Result<()> {
        let (height, width) = self.term.size();
        // Two lines are kept for the prompt and a validation message.
        let visible = usize::from(height).saturating_sub(2).max(1);
        let width = usize::from(width).max(1);

        if rows.len() <= visible {
            self.offset = 0;
        } else if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }

        let mut lines: Vec<String> = Vec::with_capacity(visible + 2);

        let mut header = String::new();
        self.theme
            .format_fuzzy_select_prompt(
                &mut header,
                self.prompt,
                &self.query,
                byte_position(&self.query, self.cursor),
            )
            .map_err(render_error)?;
        lines.push(header);

        for (index, row) in rows.iter().enumerate().skip(self.offset).take(visible) {
            let mut line = String::new();
            self.theme
                .format_select_prompt_item(&mut line, &row.label(), index == self.selected)
                .map_err(render_error)?;
            lines.push(line);
        }

        if let Some(message) = &self.error {
            let mut line = String::new();
            self.theme
                .format_error(&mut line, message)
                .map_err(render_error)?;
            lines.push(line);
        }

        self.term.clear_last_lines(self.drawn)?;
        for line in &lines {
            // A wrapped line would take two rows and break the clearing above.
            self.term.write_line(&truncate_str(line, width, "..."))?;
        }
        self.term.flush()?;
        self.drawn = lines.len();

        Ok(())
    }

    /// Erases the frame currently on screen.
    fn clear(&mut self) -> Result<()> {
        self.term.clear_last_lines(self.drawn)?;
        self.drawn = 0;
        self.term.flush()?;
        Ok(())
    }

    /// Replaces the frame with the single line echoing the answer, as the other prompts do.
    fn report(&mut self, value: &str) -> Result<()> {
        self.clear()?;
        let mut line = String::new();
        self.theme
            .format_input_prompt_selection(&mut line, self.prompt, value)
            .map_err(render_error)?;
        self.term.write_line(&line)?;
        self.term.flush()?;
        Ok(())
    }
}

/// Asks the user to pick one of `candidates` or to type a value of their own.
///
/// Typing filters the list; Enter takes the highlighted row, which is the `Create "..."` row as
/// soon as the typed text matches no candidate. `allow_skip` adds the `(none)` row that optional
/// fields need, and returns `Ok(None)` when the user takes it. `validate` guards both a picked
/// candidate and a created value: its message is shown under the list and the prompt stays open.
///
/// # Errors
/// * If the terminal is not interactive
/// * If the user cancels with Esc or Ctrl-C
pub(crate) fn select_or_create(
    prompt: &str,
    candidates: &[String],
    allow_skip: bool,
    validate: &dyn Fn(&str) -> std::result::Result<(), String>,
) -> Result<Option<String>> {
    let term = Term::stderr();
    if !term.is_term() {
        return Err(RonaError::UserCancelled);
    }

    Picker {
        term,
        theme: crate::theme::prompt_theme(),
        prompt,
        candidates,
        allow_skip,
        query: String::new(),
        cursor: 0,
        selected: 0,
        offset: 0,
        error: None,
        drawn: 0,
    }
    .run(validate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<String> {
        ["api", "auth", "cli", "parser"]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    }

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::label).collect()
    }

    #[test]
    fn test_empty_query_lists_every_candidate_in_order() {
        let rows = rows(&candidates(), "", false);

        assert_eq!(labels(&rows), vec!["api", "auth", "cli", "parser"]);
    }

    #[test]
    fn test_empty_query_appends_the_skip_row_when_optional() {
        let rows = rows(&candidates(), "", true);

        assert_eq!(rows.last(), Some(&Row::Skip));
    }

    #[test]
    fn test_typed_text_is_offered_as_a_new_value() {
        let rows = rows(&candidates(), "billing", true);

        // Nothing matches, so the create row is the only one and Enter accepts it.
        assert_eq!(rows, vec![Row::Create("billing".to_string())]);
    }

    #[test]
    fn test_matches_come_before_the_create_row() {
        let rows = rows(&candidates(), "a", false);

        assert_eq!(labels(&rows), vec!["api", "auth", "parser", "Create \"a\""]);
    }

    #[test]
    fn test_no_create_row_when_a_candidate_holds_the_text() {
        let rows = rows(&candidates(), "api", false);

        assert_eq!(labels(&rows), vec!["api"]);
    }

    #[test]
    fn test_skip_row_is_hidden_once_the_user_types() {
        let rows = rows(&candidates(), "a", true);

        assert!(!rows.contains(&Row::Skip));
    }

    #[test]
    fn test_exact_match_outranks_prefix_and_substring() {
        let rows = rows(&["parser".to_string(), "par".to_string()], "par", false);

        assert_eq!(labels(&rows), vec!["par", "parser"]);
    }

    #[test]
    fn test_matching_ignores_case() {
        let rows = rows(&["API".to_string()], "api", false);

        // The candidate still matches, but the differently-cased text stays creatable: a scope
        // typed in another case is a value of its own.
        assert_eq!(labels(&rows), vec!["API", "Create \"api\""]);
    }

    #[test]
    fn test_subsequence_matches_rank_last() {
        let rows = rows(&["auth".to_string(), "ah".to_string()], "ah", false);

        assert_eq!(labels(&rows), vec!["ah", "auth"]);
    }

    #[test]
    fn test_score_rejects_a_non_match() {
        assert!(score("api", "zz").is_none());
    }

    #[test]
    fn test_byte_position_handles_multibyte_text() {
        assert_eq!(byte_position("éa", 1), 2);
        assert_eq!(byte_position("éa", 9), 3);
    }
}
