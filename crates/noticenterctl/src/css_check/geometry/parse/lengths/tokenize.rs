//! Shared escape-aware scanner for geometry CSS token boundaries

use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CssScanState {
    quote: Option<char>,
    escaped: bool,
    paren_depth: u32,
    bracket_depth: u32,
}

impl CssScanState {
    const fn is_top_level(self) -> bool {
        self.quote.is_none() && !self.escaped && self.paren_depth == 0 && self.bracket_depth == 0
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(super) enum CssScanError {
    #[error("CSS contains an unmatched closing parenthesis at byte {0}")]
    ClosingParenthesis(usize),
    #[error("CSS contains an unmatched closing bracket at byte {0}")]
    ClosingBracket(usize),
    #[error("CSS contains an unterminated quoted string")]
    UnterminatedQuote,
    #[error("CSS contains an unterminated group")]
    UnterminatedGroup,
    #[error("CSS contains a dangling escape")]
    DanglingEscape,
}

pub(super) fn scan_css(
    input: &str,
    mut visitor: impl FnMut(usize, char, &CssScanState),
) -> Result<(), CssScanError> {
    // Public scans always consume the full input and validate the final state
    scan_css_until(input, |index, character, state| {
        visitor(index, character, state);
        false
    })
}

fn scan_css_until(
    input: &str,
    mut visitor: impl FnMut(usize, char, &CssScanState) -> bool,
) -> Result<(), CssScanError> {
    let mut state = CssScanState::default();

    // Character indices preserve valid UTF-8 slice boundaries for every callback
    for (index, character) in input.char_indices() {
        if state.escaped {
            // Escaped characters are data even when they look like delimiters
            if visitor(index, character, &state) {
                return Ok(());
            }
            state.escaped = false;
            continue;
        }

        if let Some(quote) = state.quote {
            // Quoted delimiters cannot change group depth or split top-level values
            match character {
                '\\' => state.escaped = true,
                current if current == quote => state.quote = None,
                _ => {}
            }
            if visitor(index, character, &state) {
                return Ok(());
            }
            continue;
        }

        // Group depth is updated before visitors inspect the current character
        match character {
            '\\' => state.escaped = true,
            '"' | '\'' => state.quote = Some(character),
            '(' => state.paren_depth += 1,
            ')' => {
                state.paren_depth = state
                    .paren_depth
                    .checked_sub(1)
                    .ok_or(CssScanError::ClosingParenthesis(index))?;
            }
            '[' => state.bracket_depth += 1,
            ']' => {
                state.bracket_depth = state
                    .bracket_depth
                    .checked_sub(1)
                    .ok_or(CssScanError::ClosingBracket(index))?;
            }
            _ => {}
        }
        if visitor(index, character, &state) {
            return Ok(());
        }
    }

    // Final-state checks turn malformed CSS into one consistent parse failure
    if state.escaped {
        return Err(CssScanError::DanglingEscape);
    }
    if state.quote.is_some() {
        return Err(CssScanError::UnterminatedQuote);
    }
    if state.paren_depth != 0 || state.bracket_depth != 0 {
        return Err(CssScanError::UnterminatedGroup);
    }
    Ok(())
}

pub(super) fn consume_balanced_group(input: &str, start: usize) -> Option<usize> {
    // A subslice lets the shared scanner report offsets relative to the opening group
    let remaining = input.get(start..)?;
    let mut end = None;
    scan_css_until(remaining, |index, character, state| {
        if end.is_none() && character == ')' && state.is_top_level() {
            end = Some(start + index + character.len_utf8());
            return true;
        }
        false
    })
    .ok()?;
    end
}

pub(super) fn split_css_value_tokens(value: &str) -> Result<Vec<&str>, CssScanError> {
    let mut tokens = Vec::new();
    let mut start = None;
    // Only unquoted top-level whitespace ends one shorthand token
    scan_css(value, |index, character, state| {
        if character.is_whitespace() && state.is_top_level() {
            if let Some(token_start) = start.take() {
                let token = value[token_start..index].trim();
                if !token.is_empty() {
                    tokens.push(token);
                }
            }
        } else if start.is_none() {
            start = Some(index);
        }
    })?;
    if let Some(token_start) = start {
        let token = value[token_start..].trim();
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    Ok(tokens)
}

pub(super) fn split_top_level_once(
    input: &str,
    separator: char,
) -> Result<(&str, Option<&str>), CssScanError> {
    let mut split = None;
    // The first top-level separator owns the entire remaining fallback value
    scan_css(input, |index, character, state| {
        if split.is_none() && character == separator && state.is_top_level() {
            split = Some(index);
        }
    })?;
    Ok(split.map_or((input, None), |index| {
        let right = index + separator.len_utf8();
        (&input[..index], Some(&input[right..]))
    }))
}

pub(super) fn split_top_level_list(
    input: &str,
    separator: char,
) -> Result<Vec<&str>, CssScanError> {
    let mut split_points = Vec::new();
    // Nested functions and attribute selectors keep their internal separators
    scan_css(input, |index, character, state| {
        if character == separator && state.is_top_level() {
            split_points.push(index);
        }
    })?;

    let mut parts = Vec::new();
    let mut start = 0;
    for index in split_points {
        let part = input[start..index].trim();
        if !part.is_empty() {
            parts.push(part);
        }
        start = index + separator.len_utf8();
    }
    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    Ok(parts)
}

#[cfg(test)]
#[path = "tests/tokenize.rs"]
mod tests;
