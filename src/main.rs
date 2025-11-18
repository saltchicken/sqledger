mod app;
mod config;
mod db;
mod editor;
mod event;
mod ui;

use crate::{
    app::App, config::setup_config, db::init_script_table, event::handle_key_event, ui::ui,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use postgres::{Client, NoTls};
use ratatui::{Terminal, backend::Backend};
use std::io::{self, stdout};

fn main() -> io::Result<()> {
    let config = setup_config()?;
    let db_url = &config.database_url;

    let mut client = Client::connect(db_url, NoTls)
        .map_err(|e| io::Error::other(format!("DB connect error: {}", e)))?;

    init_script_table(&mut client)
        .map_err(|e| io::Error::other(format!("Failed to init DB table: {}", e)))?;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&mut client, db_url)?;

    let res = run_app(&mut terminal, &mut app, &mut client);

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
    client: &mut Client,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = read()?
            && key.kind == KeyEventKind::Press
            && !handle_key_event(key, app, client, terminal)?
        {
            return Ok(());
        }
    }
}

