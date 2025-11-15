mod app;
mod config;
mod db;
mod editor;
mod event;
mod ui;
use crate::{
    app::App,
    config::{CONFIG_DIR_NAME, CONFIG_FILE_NAME, load_config},
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
    path::Path,
};
fn main() -> io::Result<()> {
    let config_dir_path = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?
        .join(CONFIG_DIR_NAME);
    let data_dir_path = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find data directory"))?
        .join(CONFIG_DIR_NAME);
    fs::create_dir_all(&config_dir_path)?;
    fs::create_dir_all(&data_dir_path)?;
    let config_path = config_dir_path.join(CONFIG_FILE_NAME);
    let config = load_config(&config_path);
    let script_dir_path_str = shellexpand::tilde(&config.script_directory).to_string();
    let script_dir_path = Path::new(&script_dir_path_str).to_path_buf();
    fs::create_dir_all(&script_dir_path)?;
    let db_url = &config.database_url;
    if !config_path.exists() {
        fs::write(
            &config_path,
            "# Configuration for sqledger\n# Directory where .sql scripts are stored.\n# You can use '~' for your home directory.\nscript_directory = \"~/.config/sqledger/scripts\"\n\n# PostgreSQL connection string.\ndatabase_url = \"postgresql://user:password@host:port/database\"\n",
        )?;
    }
    let mut client = Client::connect(db_url, NoTls)
        .map_err(|e| io::Error::other(format!("DB connect error: {}", e)))?;
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(&script_dir_path, db_url)?;
    let res = run_app(&mut terminal, &mut app, &mut client, &script_dir_path);
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
/// The main TUI loop
fn run_app<B: Backend + io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    client: &mut Client,
    script_dir_path: &Path,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        if let Event::Key(key) = read()?
            && key.kind == KeyEventKind::Press
            && !handle_key_event(key, app, client, script_dir_path, terminal)?
        {
            return Ok(());
        }
    }
}
