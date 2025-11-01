use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout}, // ‼️ Removed unused `Rect`
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
    Terminal,
};
use rusqlite::{Connection, Error as RusqliteError};
use std::{
    fs::{self},         // ‼️ Removed unused `File`
    io::{self, stdout}, // ‼️ Removed unused `Stdout` and `Write`
    path::Path,
};

const DB_NAME: &str = "scripts.db";

/// App holds the state of the application
struct App {
    /// List of `.sql` file paths in the current directory
    sql_files: Vec<String>,
    /// State for the file list (tracks selection)
    list_state: ListState,
    /// The string result of the last executed query
    query_result: String,
}

impl App {
    /// Creates a new App, scanning the current directory for .sql files
    fn new() -> io::Result<Self> {
        let mut sql_files = Vec::new();
        let current_dir = fs::read_dir(".")?;

        for entry in current_dir {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "sql" {
                        sql_files.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
        sql_files.sort();

        let mut list_state = ListState::default();
        if !sql_files.is_empty() {
            list_state.select(Some(0));
        }

        Ok(Self {
            sql_files,
            list_state,
            query_result: "Welcome!\n\nPress 'j'/'k' to navigate.\nPress 'l' or 'Enter' to run the selected SQL script.\nPress 'q' to quit.".to_string(),
        })
    }

    /// Moves the list selection down
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
    }

    /// Moves the list selection up
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
    }

    /// Executes the currently selected SQL script
    fn execute_selected_script(&mut self) {
        if let Some(selected_index) = self.list_state.selected() {
            let file_path = &self.sql_files[selected_index];

            // Read the SQL script content
            let script_content = match fs::read_to_string(file_path) {
                Ok(content) => content,
                Err(e) => {
                    self.query_result = format!("Error reading file {}:\n{}", file_path, e);
                    return;
                }
            };

            // Connect to the database and execute
            match Connection::open(DB_NAME) {
                Ok(conn) => {
                    self.query_result =
                        execute_sql(&conn, &script_content).unwrap_or_else(|e| e.to_string());
                }
                Err(e) => {
                    self.query_result = format!("Error opening database {}:\n{}", DB_NAME, e);
                }
            }
        }
    }
}

/// Helper function to execute SQL and format the result
fn execute_sql(conn: &Connection, sql: &str) -> Result<String, RusqliteError> {
    // Check if it's a SELECT query (simple check)
    let is_select = sql.trim_start().to_uppercase().starts_with("SELECT");

    if is_select {
        let mut stmt = conn.prepare(sql)?;

        // Get column names for the header
        let column_names: Vec<&str> = stmt.column_names();
        let header = column_names.join(" | ");
        let mut result_lines = vec![header.clone(), "-".repeat(header.len())];

        let column_count = stmt.column_count();
        let rows = stmt.query_map([], |row| {
            // ‼️ Removed unnecessary `mut`
            let mut row_values = Vec::new();
            for i in 0..column_count {
                // Try to get value as Option<String>, default to "NULL" if empty
                let value_str = row
                    .get::<_, Option<String>>(i)
                    .unwrap_or(Some("NULL".to_string()))
                    .unwrap_or("NULL".to_string());
                row_values.push(value_str);
            }
            Ok(row_values.join(" | "))
        })?;

        for row_result in rows {
            result_lines.push(row_result?);
        }

        if result_lines.len() == 2 {
            // Only header and separator
            Ok(format!(
                "Query returned 0 rows.\n\n{}",
                result_lines.join("\n")
            ))
        } else {
            Ok(result_lines.join("\n"))
        }
    } else {
        // For non-SELECT queries (INSERT, UPDATE, CREATE, etc.)
        match conn.execute_batch(sql) {
            Ok(_) => Ok(format!("Successfully executed script:\n\n{}", sql)),
            Err(e) => Err(e),
        }
    }
}

/// Main function to set up and run the TUI
fn main() -> io::Result<()> {
    // --- Setup for Demo ---
    // Create dummy DB and SQL files if they don't exist
    if !Path::new(DB_NAME).exists() {
        let conn = Connection::open(DB_NAME).expect("Failed to create dummy DB");
        conn.execute_batch("").expect("Failed to open dummy DB");
    }
    if !Path::new("01_create.sql").exists() {
        fs::write(
            "01_create.sql",
            "CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT
);",
        )?;
    }
    if !Path::new("02_insert.sql").exists() {
        fs::write(
            "02_insert.sql",
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
INSERT INTO users (name, email) VALUES ('Bob', 'bob@example.com');",
        )?;
    }
    if !Path::new("03_select.sql").exists() {
        fs::write("03_select.sql", "SELECT id, name, email FROM users;")?;
    }
    // --- End of Setup ---

    // Create app state
    let mut app = App::new()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the TUI loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

/// The main TUI loop
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => app.next(),
                KeyCode::Char('k') | KeyCode::Up => app.previous(),
                KeyCode::Char('l') | KeyCode::Enter => app.execute_selected_script(),
                _ => {}
            }
        }
    }
}

/// Renders the user interface
fn ui(f: &mut Frame, app: &mut App) {
    // ‼️ Changed to `&mut App` to allow modifying list_state
    // Create two panes: 30% left, 70% right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(f.area()); // ‼️ Changed deprecated `f.size()` to `f.area()`

    // --- Left Pane: SQL File List ---
    let items: Vec<ListItem> = app
        .sql_files
        .iter()
        .map(|file_name| ListItem::new(file_name.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("SQL Scripts"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    // Render the list
    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // --- Right Pane: Query Results ---
    let results_paragraph = Paragraph::new(app.query_result.as_str())
        .block(Block::default().borders(Borders::ALL).title("Results"))
        .wrap(Wrap { trim: true });

    f.render_widget(results_paragraph, chunks[1]);
}
