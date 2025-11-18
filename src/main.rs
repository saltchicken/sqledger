mod app;
mod config;
mod db;
mod editor;
mod event;
mod ui;

use crate::{

    app::App,
    config::setup_config,
    event::handle_key_event,
    ui::ui,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{Terminal, backend::Backend};
use std::io::{self, stdout};

fn main() -> io::Result<()> {
    let config = setup_config()?;



    let mut app = App::new(config.connections)?;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;


    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend + io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,

) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = read()?
            && key.kind == KeyEventKind::Press

            && !handle_key_event(key, app, terminal)?
        {
            return Ok(());
        }
    }
}
