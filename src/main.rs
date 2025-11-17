mod app;
mod config;
mod db;
mod editor;
mod event;
mod ui;

use crate::{
    app::App,
    config::{CONFIG_DIR_NAME, CONFIG_FILE_NAME, load_config},
    db::init_script_table,
    event::handle_key_event,
    ui::ui,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use postgres::{Client, NoTls};
use ratatui::{Terminal, backend::Backend};
use std::{
    fs,
    io::{self, stdout},
};

fn main() -> io::Result<()> {
    // Setup config dir (keep this for config.toml)
    let config_dir_path = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?
        .join(CONFIG_DIR_NAME);

    fs::create_dir_all(&config_dir_path)?;
    let config_path = config_dir_path.join(CONFIG_FILE_NAME);
    let config = load_config(&config_path);



    let db_url = &config.database_url;

    if !config_path.exists() {
        fs::write(
            &config_path,

            "# Configuration for sqledger\n\n# PostgreSQL connection string.\ndatabase_url = \"postgresql://postgres:postgres@localhost/postgres\"\n",
        )?;
    }

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