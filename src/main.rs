use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use rusqlite::{Connection, Error as RusqliteError};
use serde::Deserialize;
use std::{
    fs::{self},
    io::{self, stdout},
    path::Path,
    process::Command,
};

const DB_NAME: &str = "scripts.db";
const CONFIG_DIR_NAME: &str = "sqledger";
const CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_SCRIPTS_DIR: &str = "~/.config/sqledger/scripts";

// Configuration struct
#[derive(Deserialize, Debug)]
struct Config {
    #[serde(default = "default_script_dir")]
    script_directory: String,
}

// Default for serde
fn default_script_dir() -> String {
    DEFAULT_SCRIPTS_DIR.to_string()
}

// Implement Default for Config
impl Default for Config {
    fn default() -> Self {
        Self {
            script_directory: default_script_dir(),
        }
    }
}

// Function to load configuration
fn load_config(config_path: &Path) -> Config {
    if let Ok(content) = fs::read_to_string(config_path) {
        // Try to parse, fall back to default on error
        return toml::from_str(&content).unwrap_or_else(|e| {
            println!(
                "Failed to parse config file at {:?}: {}, using default.",
                config_path, e
            );
            Config::default()
        });
    }
    // No config file found, return default
    Config::default()
}

/// App holds the state of the application
struct App {
    sql_files: Vec<String>,
    list_state: ListState,
    query_result: String,
    script_content_preview: String,
}

impl App {
    /// Creates a new App, scanning the configured script directory for .sql files
    fn new(script_dir_path: &Path, db_path: &Path) -> io::Result<Self> {
        let mut sql_files = Vec::new();

        // Read from the configured script directory
        let script_dir_entries = match fs::read_dir(script_dir_path) {
            Ok(entries) => entries,
            Err(e) => {
                // If dir doesn't exist, return error but with a helpful message
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

                .flatten() // unwraps only Ok(entry), skips Err(entry)
                        if extension == "sql" {
                            // Store the full, absolute path
                            sql_files.push(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        sql_files.sort(); // Sort the files alphabetically

        let mut list_state = ListState::default();
        let mut script_content_preview = "No SQL files found.".to_string();

        if !sql_files.is_empty() {
            list_state.select(Some(0));
            // Load initial preview
            script_content_preview = fs::read_to_string(&sql_files[0])
                .unwrap_or_else(|e| format!("Error reading file: {}", e));
        }

        // Updated welcome message
        let welcome_message = format!(
            "Welcome!\n\nLoading scripts from: {}\nLoading database from: {}\n\nPress 'j'/'k' to navigate.\nPress 'l' or 'Enter' to run.\nPress 'e' to edit.\nPress 'q' to quit.",
            script_dir_path.display(),
            db_path.display()
        );

        Ok(Self {
            sql_files,
            list_state,
            query_result: welcome_message,
            script_content_preview,
        })
    }

    /// Selects the next item in the list
    fn next(&mut self) {
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

    /// Selects the previous item in the list
    fn previous(&mut self) {
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

    /// Updates the script content preview based on the selected file
    fn update_preview(&mut self) {
        if let Some(selected_index) = self.list_state.selected() {
            if let Some(file_path) = self.sql_files.get(selected_index) {
                self.script_content_preview = fs::read_to_string(file_path)
                    .unwrap_or_else(|e| format!("Error reading file {}: {}", file_path, e));
            }
        }
    }
}

/// Executes the currently selected SQL script
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

                // Trim whitespace and check if it's a SELECT query
                let trimmed_sql = sql_content.trim();
                if trimmed_sql.to_uppercase().starts_with("SELECT")
                    || trimmed_sql.to_uppercase().starts_with("PRAGMA")
                {
                    // It's a query, try to get results
                    match (|| -> Result<String, RusqliteError> {
                        let mut stmt = conn.prepare(&sql_content)?;
                        // Clone column names to own the data and end the immutable borrow of `stmt`
                        let column_names: Vec<String> =
                            stmt.column_names().iter().map(|s| s.to_string()).collect();
                        let mut widths: Vec<usize> = column_names.iter().map(|s| s.len()).collect();
                        let mut rows_data: Vec<Vec<String>> = Vec::new();

                        let rows = stmt.query_map([], |row| {
                            let mut values = Vec::new();
                            for i in 0..widths.len() {
                                let val: String = row.get(i).unwrap_or_else(|_| "NULL".to_string());
                                widths[i] = widths[i].max(val.len());
                                values.push(val);
                            }
                            Ok(values)
                        })?;

                        for row_result in rows {
                            rows_data.push(row_result?);
                        }

                        // Format the output
                        let mut output = String::new();
                        // Header
                        for (i, name) in column_names.iter().enumerate() {
                            output.push_str(&format!("{:<width$} | ", name, width = widths[i]));
                        }
                        output.push('\n');
                        // Separator
                        for width in &widths {
                            output.push_str(&"-".repeat(*width));
                            output.push_str("---");
                        }
                        output.push('\n');
                        // Rows
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
                    // It's an execute (INSERT, UPDATE, CREATE, etc.)
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

/// Main function to set up and run the TUI
fn main() -> io::Result<()> {
    // Create config dir and default scripts dir if they don't exist
    let config_dir_path = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?
        .join(CONFIG_DIR_NAME);

    // Get data dir path (e.g., ~/.local/share/sqledger)
    let data_dir_path = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find data directory"))?
        .join(CONFIG_DIR_NAME);

    // Create both directories
    fs::create_dir_all(&config_dir_path)?;
    fs::create_dir_all(&data_dir_path)?;

    let config_path = config_dir_path.join(CONFIG_FILE_NAME);

    // Load config to get script directory
    let config = load_config(&config_path);
    let script_dir_path_str = shellexpand::tilde(&config.script_directory).to_string();
    let script_dir_path = Path::new(&script_dir_path_str).to_path_buf();
    // Create script dir if it doesn't exist
    fs::create_dir_all(&script_dir_path)?;

    let db_path = data_dir_path.join(DB_NAME);

    // Create dummy config if it doesn't exist
    if !config_path.exists() {
        fs::write(
            &config_path,
            "# Configuration for sqledger\n# Directory where .sql scripts are stored.\n# You can use '~' for your home directory.\nscript_directory = \"~/.config/sqledger/scripts\"\n",
        )?;
    }

    // Create dummy DB
    if !db_path.exists() {
        let conn = Connection::open(&db_path).expect("Failed to create dummy DB");
        conn.execute_batch("").expect("Failed to open dummy DB");
    }

    // Set up the terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new(&script_dir_path, &db_path)?;
    let res = run_app(&mut terminal, &mut app, &db_path);

    // Restore terminal
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
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                    KeyCode::Char('l') | KeyCode::Enter => {
                        execute_sql(app, &db_path.to_string_lossy())
                    }
                    KeyCode::Char('e') => {
                        // Logic to open editor
                        if let Some(selected_index) = app.list_state.selected() {
                            if let Some(file_path) = app.sql_files.get(selected_index) {
                                // Suspend TUI
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;
                                terminal.show_cursor()?;

                                // Run nvim
                                let editor =
                                    std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
                                let status = Command::new(editor).arg(file_path).status()?;

                                // Resume TUI
                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?; // Redraw entire TUI

                                if !status.success() {
                                    app.query_result = "Editor exited with an error.".to_string();
                                }
                                // Refresh preview in case file was changed
                                app.update_preview();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Renders the user interface
fn ui(f: &mut Frame, app: &mut App) {
    // Create two panes: 30% left, 70% right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(f.area());

    // --- Left Pane: SQL File List ---
    let items: Vec<ListItem> = app
        .sql_files
        .iter()
        .map(|full_path| {
            // Get just the filename for display
            let filename = Path::new(full_path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("invalid_filename"))
                .to_string_lossy()
                .to_string();
            ListItem::new(filename)
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

    // Use `&mut` to render the stateful widget
    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // --- Right Panes (Vertically Split) ---
    // Split right pane: 40% top (preview), 60% bottom (results)
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
}
