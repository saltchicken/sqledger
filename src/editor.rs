use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use std::{io, path::Path, process::Command};

pub fn open_editor<B: Backend + io::Write>(
    terminal: &mut Terminal<B>,
    file_path: &Path,
) -> io::Result<bool> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
    let status = Command::new(editor).arg(file_path).status()?;

    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    terminal.clear()?;

    Ok(status.success())
}
