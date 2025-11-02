use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use rusqlite::{Connection, Error as RusqliteError};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs::{self},
    io::{self, stdout},
    path::Path,
    process::Command,
};

const DB_NAME: &str = "scripts.db";
const CONFIG_DIR_NAME: &str = "sqledger";
const CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_SCRIPTS_DIR: &str = "~/.config/sqledger/scripts";

#[derive(Clone, Copy, PartialEq, Debug)]
enum InputMode {
    Normal,
    EditingFilename,
    ConfirmingDelete,
    RenamingScript,
}

#[derive(Deserialize, Debug)]
struct Config {
    #[serde(default = "default_script_dir")]
    script_directory: String,
}
fn default_script_dir() -> String {
    DEFAULT_SCRIPTS_DIR.to_string()
}
impl Default for Config {
    fn default() -> Self {
        Self {
            script_directory: default_script_dir(),
        }
    }
}
fn load_config(config_path: &Path) -> Config {
    if let Ok(content) = fs::read_to_string(config_path) {
        return toml::from_str(&content).unwrap_or_else(|e| {
            println!(
                "Failed to parse config file at {:?}: {}, using default.",
                config_path, e
            );
            Config::default()
        });
    }
    Config::default()
}

/// App holds the state of the application
struct App {
    sql_files: Vec<String>,
    list_state: ListState,
    query_result: String,
    script_content_preview: String,
    input_mode: InputMode,
    filename_input: String,
}

impl App {
    /// Creates a new App, scanning the configured script directory for .sql files
    fn new(script_dir_path: &Path, db_path: &Path) -> io::Result<Self> {
        let welcome_message = format!(
            "Welcome!\n\nLoading scripts from: {}\nLoading database from: {}\n\nPress 'j'/'k' to navigate.\nPress 'l' or 'Enter' to run.\nPress 'e' to edit.\nPress 'a' to add new script.\nPress 'd' to delete.\nPress 'r' to rename.\nPress 'q' to quit.",
            script_dir_path.display(),
            db_path.display()
        );

        let mut app = Self {
            sql_files: Vec::new(),
            list_state: ListState::default(),
            query_result: welcome_message,
            script_content_preview: "".to_string(),
            input_mode: InputMode::Normal,
            filename_input: String::new(),
        };

        app.rescan_scripts(script_dir_path)?;

        Ok(app)
    }

    fn rescan_scripts(&mut self, script_dir_path: &Path) -> io::Result<()> {
        let mut sql_files = Vec::new();
        let script_dir_entries = match fs::read_dir(script_dir_path) {
            Ok(entries) => entries,
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to read script directory at: {}. \nError: {}",
                        script_dir_path.display(),
                        e
                    ),
                ));
            }
        };

        sql_files.extend(
            script_dir_entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
                .map(|path| path.to_string_lossy().to_string()),
        );
        sql_files.sort();

        self.sql_files = sql_files;

        let mut valid_selection_exists = false;
        if let Some(selected_index) = self.list_state.selected() {
            if selected_index >= self.sql_files.len() {
                valid_selection_exists = false;
            } else {
                valid_selection_exists = true;
            }
        }

        if !valid_selection_exists {
            if !self.sql_files.is_empty() {
                self.list_state.select(Some(0));
            } else {
                self.list_state.select(None);
            }
        }

        self.update_preview();
        Ok(())
    }

    fn get_selected_filename_stem(&self) -> Option<String> {
        self.list_state
            .selected()
            .and_then(|i| self.sql_files.get(i))
            .map(|p| {
                Path::new(p)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
    }

    fn next(&mut self) {
        if self.sql_files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.sql_files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_preview();
    }
    fn previous(&mut self) {
        if self.sql_files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.sql_files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_preview();
    }
    fn update_preview(&mut self) {
        if let Some(selected_index) = self.list_state.selected() {
            if let Some(file_path) = self.sql_files.get(selected_index) {
                self.script_content_preview = fs::read_to_string(file_path)
                    .unwrap_or_else(|e| format!("Error reading file {}: {}", file_path, e));
            }
        } else {
            self.script_content_preview = "No SQL files found.".to_string();
        }
    }
}

fn execute_sql(app: &mut App, db_path: &str) {
    if let Some(selected_index) = app.list_state.selected() {
        let file_path = &app.sql_files[selected_index];
        match fs::read_to_string(file_path) {
            Ok(sql_content) => {
                let conn = match Connection::open(db_path) {
                    Ok(conn) => conn,
                    Err(e) => {
                        app.query_result = format!("Error opening database {}: {}", db_path, e);
                        return;
                    }
                };
                let trimmed_sql = sql_content.trim();
                if trimmed_sql.to_uppercase().starts_with("SELECT")
                    || trimmed_sql.to_uppercase().starts_with("PRAGMA")
                {
                    match (|| -> Result<String, RusqliteError> {
                        let mut stmt = conn.prepare(&sql_content)?;
                        let column_names: Vec<String> =
                            stmt.column_names().iter().map(|s| s.to_string()).collect();
                        let mut widths: Vec<usize> = column_names.iter().map(|s| s.len()).collect();
                        let mut rows_data: Vec<Vec<String>> = Vec::new();

                        let rows = stmt.query_map([], |row| {
                            let mut values = Vec::new();
                            for (i, width) in widths.iter_mut().enumerate() {
                                let val: String = row.get(i).unwrap_or_else(|_| "NULL".to_string());
                                *width = (*width).max(val.len());
                                values.push(val);
                            }
                            Ok(values)
                        })?;

                        for row_result in rows {
                            rows_data.push(row_result?);
                        }

                        let mut output = String::new();
                        for (i, name) in column_names.iter().enumerate() {
                            output.push_str(&format!("{:<width$} | ", name, width = widths[i]));
                        }
                        output.push('\n');
                        for width in &widths {
                            output.push_str(&"-".repeat(*width));
                            output.push_str("---");
                        }
                        output.push('\n');
                        for row in rows_data {
                            for (i, value) in row.iter().enumerate() {
                                output.push_str(&format!(
                                    "{:<width$} | ",
                                    value,
                                    width = widths[i]
                                ));
                            }
                            output.push('\n');
                        }
                        Ok(output)
                    })() {
                        Ok(formatted_result) => app.query_result = formatted_result,
                        Err(e) => app.query_result = format!("Error executing query: {}", e),
                    }
                } else {
                    match conn.execute_batch(&sql_content) {
                        Ok(_) => {
                            let changes = conn.total_changes();
                            app.query_result = format!(
                                "Command executed successfully. {} rows affected.",
                                changes
                            );
                        }
                        Err(e) => app.query_result = format!("Error executing command: {}", e),
                    }
                }
            }
            Err(e) => {
                app.query_result = format!("Error reading file {}: {}", file_path, e);
            }
        }
    }
}
fn open_editor<B: Backend + io::Write>(
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
                            // ‼️ Updated prompt text
                            app.query_result =
                                "Enter new script name (no extension). Press [Enter] to confirm, [Esc] to cancel."
                                    .to_string();
                        }
                        KeyCode::Char('d') => {
                            if app.list_state.selected().is_some() {
                                app.input_mode = InputMode::ConfirmingDelete;
                                // ‼️ Use helper to get stem
                                let filename = app.get_selected_filename_stem().unwrap_or_default();
                                app.query_result = format!("Delete '{}'? (y/n)", filename);
                            } else {
                                app.query_result = "No script selected to delete.".to_string();
                            }
                        }
                        KeyCode::Char('r') => {
                            // ‼️ Use helper to get stem
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
                                // ‼️ Add .sql extension manually
                                new_file_path.push(format!("{}.sql", filename_stem));

                                if new_file_path.exists() {
                                    app.query_result = format!(
                                        "Error: File {} already exists.",
                                        new_file_path.display()
                                    );
                                } else {
                                    let new_file_path_str =
                                        new_file_path.to_string_lossy().to_string();

                                    fs::write(&new_file_path, "-- New SQL Script\n")?;
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
                                        // ‼️ Add .sql extension manually
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
                }
            }
        }
    }
}

/// Renders the user interface
fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(f.area());

    // --- Left Pane: SQL File List ---
    let items: Vec<ListItem> = app
        .sql_files
        .iter()
        .map(|full_path| {
            let filename_stem = Path::new(full_path)
                .file_stem()
                .unwrap_or_else(|| OsStr::new("invalid_filename"))
                .to_string_lossy()
                .to_string();
            ListItem::new(filename_stem)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("SQL Scripts"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // --- Right Panes (Vertically Split) ---
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(chunks[1]);

    // Top-Right Pane: Script Preview
    let preview_block = Block::default().borders(Borders::ALL).title("Preview");
    let preview_text = Paragraph::new(app.script_content_preview.as_str()).block(preview_block);
    f.render_widget(preview_text, right_chunks[0]);

    // Bottom-Right Pane: Query Results
    let results_block = Block::default().borders(Borders::ALL).title("Results");
    let results_text = Paragraph::new(app.query_result.as_str()).block(results_block);
    f.render_widget(results_text, right_chunks[1]);

    // --- Popup Windows ---
    if app.input_mode == InputMode::EditingFilename {
        let area = centered_rect(50, 3, f.area());
        let input_text = format!("{}_", app.filename_input);
        let popup_block = Block::default()
            .title("New Script Name (no .sql)")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::LightBlue));

        let input_paragraph = Paragraph::new(input_text.as_str()).block(popup_block);
        f.render_widget(Clear, area);
        f.render_widget(input_paragraph, area);
    } else if app.input_mode == InputMode::ConfirmingDelete {
        let area = centered_rect(50, 3, f.area());
        let popup_block = Block::default()
            .title("Confirm Deletion")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Red).fg(Color::White));

        let popup_paragraph = Paragraph::new(app.query_result.as_str())
            .block(popup_block)
            .alignment(Alignment::Center);

        f.render_widget(Clear, area);
        f.render_widget(popup_paragraph, area);
    } else if app.input_mode == InputMode::RenamingScript {
        let area = centered_rect(50, 3, f.area());
        let input_text = format!("{}_", app.filename_input);
        let popup_block = Block::default()
            .title("Rename Script")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::LightYellow).fg(Color::Black));

        let input_paragraph = Paragraph::new(input_text.as_str()).block(popup_block);
        f.render_widget(Clear, area);
        f.render_widget(input_paragraph, area);
    }
}

/// Helper function to create a centered rectangle for popups
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let (top_padding, bottom_padding) = {
        let total_padding = r.height.saturating_sub(height);
        (
            total_padding / 2,
            total_padding.saturating_sub(total_padding / 2),
        )
    };

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_padding),
            Constraint::Length(height),
            Constraint::Length(bottom_padding),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
