use std::io::{self, IsTerminal, Write};

use anyhow::Result;

pub(super) fn confirm_trial() -> Result<bool> {
    // Non-interactive runs default to false to avoid accidental daemon disruption
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    // Prompt is explicit because trial mode may stop another user service
    print!("Enable UnixNotis trial and stop the active daemon? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    // Any response outside y/yes is treated as no
    Ok(is_trial_confirmation(&input))
}

fn is_trial_confirmation(input: &str) -> bool {
    let input = input.trim();
    input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes")
}

#[cfg(test)]
#[path = "tests/prompt.rs"]
mod tests;
