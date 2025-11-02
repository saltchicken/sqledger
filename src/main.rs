// ‼️ Declare the new modules
mod app;
mod config;
mod db;
mod editor;
mod ui;

use crate::{
    app::{App, InputMode},
    config::{load_config, CONFIG_DIR_NAME, CONFIG_FILE_NAME, DB_NAME},
    db::execute_sql,
    editor::open_editor,
    ui::ui,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use rusqlite::Connection;
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

    let db_path = data_dir_path.join(DB_NAME);

    if !config_path.exists() {
        fs::write(
            &config_path,
            "# Configuration for sqledger\n# Directory where .sql scripts are stored.\n# You can use '~' for your home directory.\nscript_directory = \"~/.config/sqledger/scripts\"\n",
        )?;
    }

    if !db_path.exists() {
        let conn = Connection::open(&db_path).expect("Failed to create dummy DB");
        conn.execute_batch("").expect("Failed to open dummy DB");
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&script_dir_path, &db_path)?;
    let res = run_app(&mut terminal, &mut app, &db_path, &script_dir_path);

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
    db_path: &Path,
    script_dir_path: &Path,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous(),
                        KeyCode::Char('l') | KeyCode::Enter => {
                            execute_sql(app, &db_path.to_string_lossy())
                        }
                        KeyCode::Char('e') => {
                            if let Some(selected_index) = app.list_state.selected() {
                                if let Some(file_path_str) = app.sql_files.get(selected_index) {
                                    let file_path = Path::new(file_path_str);
                                    let success = open_editor(terminal, file_path)?;
                                    if !success {
                                        app.query_result =
                                            "Editor exited with an error.".to_string();
                                    }
                                    app.rescan_scripts(script_dir_path)?;
                                }
                            }
                        }
                        KeyCode::Char('a') => {
                            app.input_mode = InputMode::EditingFilename;
                            app.filename_input.clear();
                            app.query_result =
                                "Enter new script name (no extension). Press [Enter] to confirm, [Esc] to cancel."
                                    .to_string();
                        }
                        KeyCode::Char('d') => {
                            if app.list_state.selected().is_some() {
                                app.input_mode = InputMode::ConfirmingDelete;
                                // ‼️ Use helper to get stem (from original code)
                                let filename = app.get_selected_filename_stem().unwrap_or_default();
                                app.query_result = format!("Delete '{}'? (y/n)", filename);
                            } else {
                                app.query_result = "No script selected to delete.".to_string();
                            }
                        }
                        KeyCode::Char('r') => {
                            // ‼️ Use helper to get stem (from original code)
                            if let Some(filename_stem) = app.get_selected_filename_stem() {
                                app.input_mode = InputMode::RenamingScript;
                                app.filename_input = filename_stem;
                                app.query_result =
                                    "Enter new script name (no extension). Press [Enter] to confirm, [Esc] to cancel."
                                        .to_string();
                            } else {
                                app.query_result = "No script selected to rename.".to_string();
                            }
                        }
                        KeyCode::Char('?') => {
                            app.input_mode = InputMode::ShowHelp;
                        }
                        _ => {}
                    },
                    InputMode::EditingFilename => match key.code {
                        KeyCode::Enter => {
                            let filename_stem = app.filename_input.trim();
                            if filename_stem.is_empty() {
                                app.input_mode = InputMode::Normal;
                                app.query_result = "New script cancelled.".to_string();
                            } else {
                                let mut new_file_path = script_dir_path.to_path_buf();
                                // ‼️ Add .sql extension manually (from original code)
                                new_file_path.push(format!("{}.sql", filename_stem));
                                if new_file_path.exists() {
                                    app.query_result = format!(
                                        "Error: File {} already exists.",
                                        new_file_path.display()
                                    );
                                } else {
                                    let new_file_path_str =
                                        new_file_path.to_string_lossy().to_string();
                                    fs::write(&new_file_path, "")?;
                                    let success = open_editor(terminal, &new_file_path)?;
                                    if !success {
                                        app.query_result =
                                            "Editor exited with an error.".to_string();
                                    } else {
                                        app.query_result = format!(
                                            "Script {} created successfully.",
                                            new_file_path.display()
                                        );
                                    }
                                    app.rescan_scripts(script_dir_path)?;
                                    if let Some(new_index) =
                                        app.sql_files.iter().position(|p| p == &new_file_path_str)
                                    {
                                        app.list_state.select(Some(new_index));
                                        app.update_preview();
                                    }
                                }
                                app.input_mode = InputMode::Normal;
                            }
                        }
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            app.input_mode = InputMode::Normal;
                            app.query_result = "New script cancelled.".to_string();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.query_result = "New script cancelled.".to_string();
                        }
                        KeyCode::Backspace => {
                            app.filename_input.pop();
                        }
                        KeyCode::Char(c) => {
                            app.filename_input.push(c);
                        }
                        _ => {}
                    },
                    InputMode::ConfirmingDelete => match key.code {
                        KeyCode::Char('y') => {
                            if let Some(selected_index) = app.list_state.selected() {
                                if let Some(file_path_str) = app.sql_files.get(selected_index) {
                                    match fs::remove_file(file_path_str) {
                                        Ok(_) => {
                                            app.query_result =
                                                format!("File {} deleted.", file_path_str);
                                            app.rescan_scripts(script_dir_path)?;
                                        }
                                        Err(e) => {
                                            app.query_result = format!(
                                                "Error deleting file {}: {}",
                                                file_path_str, e
                                            );
                                        }
                                    }
                                }
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.query_result = "Deletion cancelled.".to_string();
                        }
                        _ => {}
                    },
                    InputMode::RenamingScript => match key.code {
                        KeyCode::Enter => {
                            let new_filename_stem = app.filename_input.trim();
                            if new_filename_stem.is_empty() {
                                app.input_mode = InputMode::Normal;
                                app.query_result = "Rename cancelled.".to_string();
                            } else {
                                if let Some(selected_index) = app.list_state.selected() {
                                    if let Some(old_path_str) = app.sql_files.get(selected_index) {
                                        let old_path = Path::new(old_path_str);
                                        let mut new_path = old_path
                                            .parent()
                                            .unwrap_or(script_dir_path)
                                            .to_path_buf();
                                        // ‼️ Add .sql extension manually (from original code)
                                        new_path.push(format!("{}.sql", new_filename_stem));

                                        if new_path.exists() {
                                            app.query_result = format!(
                                                "Error: File {} already exists.",
                                                new_path.display()
                                            );
                                        } else {
                                            match fs::rename(old_path, &new_path) {
                                                Ok(_) => {
                                                    app.query_result = "File renamed.".to_string();
                                                    let new_path_str =
                                                        new_path.to_string_lossy().to_string();
                                                    app.rescan_scripts(script_dir_path)?;
                                                    if let Some(new_index) = app
                                                        .sql_files
                                                        .iter()
                                                        .position(|p| p == &new_path_str)
                                                    {
                                                        app.list_state.select(Some(new_index));
                                                        app.update_preview();
                                                    }
                                                }
                                                Err(e) => {
                                                    app.query_result =
                                                        format!("Error renaming file: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                                app.input_mode = InputMode::Normal;
                            }
                        }
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            app.input_mode = InputMode::Normal;
                            app.query_result = "Rename cancelled.".to_string();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.query_result = "Rename cancelled.".to_string();
                        }
                        KeyCode::Backspace => {
                            app.filename_input.pop();
                        }
                        KeyCode::Char(c) => {
                            app.filename_input.push(c);
                        }
                        _ => {}
                    },
                    InputMode::ShowHelp => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}
