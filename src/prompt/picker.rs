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
    console::{Key, Term, style, truncate_str},
    theme::{ColorfulTheme, Theme},
};

use crate::errors::{Result, RonaError};

/// Label of the row that skips an optional field.
const NONE_LABEL: &str = "(none)";

/// Shown under a multi pick, where Enter alone does not say what to press to tick a row.
const MULTI_HINT: &str = "Space to tick, Enter to confirm";

/// Refusal shown when a required multi pick has nothing ticked.
const REQUIRED_MESSAGE: &str = "This field is required.";

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

/// How many values the picker returns, and how it draws its rows.
#[derive(Debug)]
enum Mode {
    /// One row is accepted, and Enter returns it.
    Single,
    /// Rows are ticked, and Enter returns every ticked value in the order it was ticked.
    Multi(Vec<String>),
}

/// Terminal state of one running picker.
struct Picker<'a> {
    /// Terminal the prompt is drawn on; stderr, like every other prompt.
    term: Term,
    /// Shared prompt theme, used for every rendered line.
    theme: ColorfulTheme,
    /// Label shown before the typed text.
    prompt: &'a str,
    /// Values offered by the prefetch, in the order it returned them. A created value joins them
    /// so that a multi-select can show it ticked instead of dropping it out of the list.
    candidates: Vec<String>,
    /// Whether the picker takes one value or several.
    mode: Mode,
    /// Whether the field accepts no value at all: the skip row of a single pick, an empty
    /// selection of a multi pick.
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
    ) -> Result<Vec<String>> {
        self.term.hide_cursor()?;
        let outcome = self.interact(validate);
        let _ = self.term.show_cursor();
        outcome
    }

    /// Draws the list and handles keys until the user accepts the selection or cancels.
    fn interact(
        &mut self,
        validate: &dyn Fn(&str) -> std::result::Result<(), String>,
    ) -> Result<Vec<String>> {
        loop {
            let rows = rows(&self.candidates, &self.query, self.skip_row());
            self.selected = self.selected.min(rows.len().saturating_sub(1));
            self.render(&rows)?;

            match self.term.read_key()? {
                Key::Escape | Key::CtrlC => {
                    self.clear()?;
                    return Err(RonaError::UserCancelled);
                }
                Key::Enter => {
                    if let Some(values) = self.accept(&rows, validate)? {
                        return Ok(values);
                    }
                }
                Key::Char(' ') if self.is_multi() => self.toggle(&rows, validate),
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

    /// Whether the picker takes several values.
    const fn is_multi(&self) -> bool {
        matches!(self.mode, Mode::Multi(_))
    }

    /// Whether the list offers the `(none)` row. Only a single pick has one: in a multi pick an
    /// empty selection already says the same thing.
    const fn skip_row(&self) -> bool {
        self.allow_skip && !self.is_multi()
    }

    /// Handles Enter: the values to return, or `None` to keep the prompt open.
    fn accept(
        &mut self,
        rows: &[Row],
        validate: &dyn Fn(&str) -> std::result::Result<(), String>,
    ) -> Result<Option<Vec<String>>> {
        if let Mode::Multi(chosen) = &self.mode {
            // Typed text that was never ticked would be lost on the way out, so Enter ticks the
            // highlighted row while a filter is up, and the next one closes the prompt.
            if !self.query.is_empty() {
                self.toggle(rows, validate);
                return Ok(None);
            }

            if chosen.is_empty() && !self.allow_skip {
                self.error = Some(REQUIRED_MESSAGE.to_string());
                return Ok(None);
            }

            let chosen = chosen.clone();
            let echo = if chosen.is_empty() {
                NONE_LABEL.to_string()
            } else {
                chosen.join(", ")
            };
            self.report(&echo)?;
            return Ok(Some(chosen));
        }

        let Some(row) = rows.get(self.selected) else {
            return Ok(None);
        };
        let Some(value) = row.value() else {
            self.report(NONE_LABEL)?;
            return Ok(Some(vec![]));
        };

        match validate(value) {
            Ok(()) => {
                let value = value.to_string();
                self.report(&value)?;
                Ok(Some(vec![value]))
            }
            Err(message) => {
                self.error = Some(message);
                Ok(None)
            }
        }
    }

    /// Ticks or unticks the highlighted row, and keeps a created value in the list so that it
    /// shows up ticked instead of vanishing with the filter that produced it.
    fn toggle(&mut self, rows: &[Row], validate: &dyn Fn(&str) -> std::result::Result<(), String>) {
        let Some(row) = rows.get(self.selected) else {
            return;
        };
        let Some(value) = row.value().map(str::to_string) else {
            return;
        };
        let created = matches!(row, Row::Create(_));

        let Mode::Multi(chosen) = &mut self.mode else {
            return;
        };

        if let Some(position) = chosen.iter().position(|ticked| *ticked == value) {
            chosen.remove(position);
        } else {
            if let Err(message) = validate(&value) {
                self.error = Some(message);
                return;
            }
            chosen.push(value.clone());
            if created {
                self.candidates.push(value);
            }
        }

        // Ticking a row answers the filter that found it; the next value starts from a clean list.
        // With no filter up the highlight stays where it is, so a run of Space keys walks the list.
        self.error = None;
        if !self.query.is_empty() {
            self.query.clear();
            self.cursor = 0;
            self.reset_filter();
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
        // Two lines are kept for the prompt and a validation message, a third for the key hint.
        let reserved = 2 + usize::from(self.is_multi());
        let visible = usize::from(height).saturating_sub(reserved).max(1);
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
                &self.header_label(),
                &self.query,
                byte_position(&self.query, self.cursor),
            )
            .map_err(render_error)?;
        lines.push(header);

        for (index, row) in rows.iter().enumerate().skip(self.offset).take(visible) {
            let mut line = String::new();
            let active = index == self.selected;
            match &self.mode {
                Mode::Single => {
                    self.theme
                        .format_select_prompt_item(&mut line, &row.label(), active)
                }
                Mode::Multi(chosen) => {
                    let ticked = row
                        .value()
                        .is_some_and(|value| chosen.iter().any(|chosen| chosen == value));
                    self.theme.format_multi_select_prompt_item(
                        &mut line,
                        &row.label(),
                        ticked,
                        active,
                    )
                }
            }
            .map_err(render_error)?;
            lines.push(line);
        }

        if self.is_multi() {
            lines.push(style(MULTI_HINT).dim().to_string());
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

    /// Prompt label, with the ticked values appended so that they stay in sight when the row that
    /// holds one has scrolled off the list.
    fn header_label(&self) -> String {
        match &self.mode {
            Mode::Multi(chosen) if !chosen.is_empty() => {
                format!("{} [{}]", self.prompt, chosen.join(", "))
            }
            _ => self.prompt.to_string(),
        }
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
    let picked = picker(prompt, candidates, Mode::Single, allow_skip)?.run(validate)?;

    Ok(picked.into_iter().next())
}

/// Asks the user to tick any number of `candidates`, or to type values of their own.
///
/// Typing filters the list exactly as it does for a single pick, and the `Create "..."` row is
/// ticked like any other, which is what lets one prompt hold both a known value and a new one.
/// Space ticks the highlighted row; Enter ticks it too while a filter is up, and confirms the
/// selection once the filter is empty. The values come back in the order they were ticked, and an
/// empty `Vec` means the user picked nothing. `allow_empty` decides whether that is accepted.
/// `validate` guards each value as it is ticked, never the joined result.
///
/// # Errors
/// * If the terminal is not interactive
/// * If the user cancels with Esc or Ctrl-C
pub(crate) fn select_or_create_many(
    prompt: &str,
    candidates: &[String],
    allow_empty: bool,
    validate: &dyn Fn(&str) -> std::result::Result<(), String>,
) -> Result<Vec<String>> {
    picker(prompt, candidates, Mode::Multi(Vec::new()), allow_empty)?.run(validate)
}

/// Builds a picker on the terminal every other prompt writes to.
fn picker<'a>(
    prompt: &'a str,
    candidates: &[String],
    mode: Mode,
    allow_skip: bool,
) -> Result<Picker<'a>> {
    let term = Term::stderr();
    if !term.is_term() {
        return Err(RonaError::UserCancelled);
    }

    Ok(Picker {
        term,
        theme: crate::theme::prompt_theme(),
        prompt,
        candidates: candidates.to_vec(),
        mode,
        allow_skip,
        query: String::new(),
        cursor: 0,
        selected: 0,
        offset: 0,
        error: None,
        drawn: 0,
    })
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

    /// A picker in multi mode over the shared candidate list.
    fn multi() -> Picker<'static> {
        Picker {
            term: Term::stderr(),
            theme: crate::theme::prompt_theme(),
            prompt: "Select scopes",
            candidates: candidates(),
            mode: Mode::Multi(Vec::new()),
            allow_skip: true,
            query: String::new(),
            cursor: 0,
            selected: 0,
            offset: 0,
            error: None,
            drawn: 0,
        }
    }

    /// Values ticked so far.
    fn ticked(picker: &Picker<'_>) -> Vec<String> {
        match &picker.mode {
            Mode::Multi(chosen) => chosen.clone(),
            Mode::Single => vec![],
        }
    }

    /// Ticks the row `query` highlights, at `selected`.
    fn tick(picker: &mut Picker<'_>, query: &str, selected: usize) {
        picker.query = query.to_string();
        picker.cursor = query.chars().count();
        let rows = rows(&picker.candidates, query, picker.skip_row());
        picker.selected = selected;
        picker.toggle(&rows, &|_| Ok(()));
    }

    #[test]
    fn test_a_multi_pick_ticks_and_unticks_a_candidate() {
        let mut picker = multi();

        tick(&mut picker, "", 1);
        assert_eq!(ticked(&picker), vec!["auth".to_string()]);

        tick(&mut picker, "", 1);
        assert!(ticked(&picker).is_empty());
    }

    #[test]
    fn test_a_multi_pick_keeps_the_order_the_values_were_ticked_in() {
        let mut picker = multi();

        tick(&mut picker, "", 2);
        tick(&mut picker, "", 0);

        assert_eq!(ticked(&picker), vec!["cli".to_string(), "api".to_string()]);
    }

    #[test]
    fn test_a_ticked_new_value_joins_the_candidate_list() {
        let mut picker = multi();

        tick(&mut picker, "billing", 0);

        assert_eq!(ticked(&picker), vec!["billing".to_string()]);
        assert!(picker.candidates.contains(&"billing".to_string()));
        assert!(picker.query.is_empty());
    }

    #[test]
    fn test_ticking_clears_the_filter_that_found_the_row() {
        let mut picker = multi();

        tick(&mut picker, "au", 0);

        assert_eq!(ticked(&picker), vec!["auth".to_string()]);
        assert!(picker.query.is_empty());
        assert_eq!(picker.cursor, 0);
    }

    #[test]
    fn test_a_refused_value_is_not_ticked() {
        let mut picker = multi();
        let rows = rows(&picker.candidates, "", false);
        picker.selected = 0;

        picker.toggle(&rows, &|_| Err("Must match pattern: ^x".to_string()));

        assert!(ticked(&picker).is_empty());
        assert_eq!(picker.error.as_deref(), Some("Must match pattern: ^x"));
    }

    #[test]
    fn test_unticking_a_value_never_asks_the_validator() {
        let mut picker = multi();
        let rows = rows(&picker.candidates, "", false);
        picker.selected = 0;
        picker.toggle(&rows, &|_| Ok(()));

        picker.toggle(&rows, &|_| Err("refused".to_string()));

        assert!(ticked(&picker).is_empty());
        assert_eq!(picker.error, None);
    }

    #[test]
    fn test_a_multi_pick_offers_no_skip_row() {
        let picker = multi();

        assert!(!picker.skip_row());
        assert!(!rows(&picker.candidates, "", picker.skip_row()).contains(&Row::Skip));
    }

    #[test]
    fn test_the_header_lists_the_ticked_values() {
        let mut picker = multi();

        assert_eq!(picker.header_label(), "Select scopes");

        tick(&mut picker, "", 0);
        tick(&mut picker, "", 1);

        assert_eq!(picker.header_label(), "Select scopes [api, auth]");
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
