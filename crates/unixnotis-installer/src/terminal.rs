//! Terminal setup and teardown for ratatui sessions.

use std::io;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, ExecutableCommand};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub struct TerminalGuard {
    // Own the ratatui Terminal and restore terminal state on drop.
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        // Enable raw mode so keypresses are delivered directly.
        enable_raw_mode()?;

        let mut stdout = io::stdout();

        // Switch to the alternate screen buffer to preserve shell scrollback.
        stdout.execute(EnterAlternateScreen)?;

        stdout.execute(EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);

        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    pub fn restore(&mut self) -> io::Result<()> {
        // Restore terminal state back to normal.
        disable_raw_mode()?;

        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;

        self.terminal.show_cursor()?;

        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort cleanup, never panic in Drop.
        let _ = self.restore();
    }
}
