use ratatui::widgets::ListState;
use std::{fs, io, path::Path};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputMode {
    Normal,
    EditingFilename,
    ConfirmingDelete,
    RenamingScript,
    ShowHelp,
}

/// App holds the state of the application
pub struct App {
    pub sql_files: Vec<String>,
    pub list_state: ListState,
    pub query_result: String,
    pub script_content_preview: String,
    pub input_mode: InputMode,
    pub filename_input: String,
    pub help_message: String,
}

impl App {
    /// Creates a new App, scanning the configured script directory for .sql files
    pub fn new(script_dir_path: &Path, db_path: &Path) -> io::Result<Self> {
        let help_message = format!(
            "Welcome to sqledger!\n\nScripts: {}\nDatabase: {}\n\n--- Keybinds ---\n'j'/'k' or ↓/↑: Navigate scripts\n'l' or 'Enter' : Run selected script\n'e'              : Edit selected script\n'a'              : Add a new script\n'd'              : Delete selected script\n'r'              : Rename selected script\n'?'              : Toggle this help message\n'q'              : Quit",
            script_dir_path.display(),
            db_path.display()
        );
        let mut app = Self {
            sql_files: Vec::new(),
            list_state: ListState::default(),
            query_result: "Welcome! Press '?' for help.".to_string(),
            script_content_preview: "".to_string(),
            input_mode: InputMode::Normal,
            filename_input: String::new(),
            help_message,
        };
        app.rescan_scripts(script_dir_path)?;
        Ok(app)
    }

    pub fn rescan_scripts(&mut self, script_dir_path: &Path) -> io::Result<()> {
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
            valid_selection_exists = selected_index < self.sql_files.len();
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

    pub fn get_selected_filename_stem(&self) -> Option<String> {
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

    pub fn next(&mut self) {
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

    pub fn previous(&mut self) {
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

    pub fn update_preview(&mut self) {
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
