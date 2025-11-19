use crate::db::{QueryResult, Script, get_all_scripts, init_script_table};
use postgres::{Client, NoTls};
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::io;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputMode {
    Normal,
    EditingFilename,
    ConfirmingDelete,
    RenamingScript,
    ShowHelp,
    SelectingConnection,
    AddingConnectionName,
    AddingConnectionUrl,
}

/// App holds the state of the application
pub struct App {
    pub client: Client,
    pub connections: HashMap<String, String>,
    pub connection_list_state: ListState,
    pub current_connection_name: String,
    pub new_connection_name_buffer: String,

    pub scripts: Vec<Script>,
    pub list_state: ListState,
    pub query_result: String,
    pub query_row_count: Option<usize>,
    pub script_content_preview: String,
    pub input_mode: InputMode,
    pub filename_input: String,
    pub help_message: String,
    pub result_scroll_x: u16,
    pub result_scroll_y: u16,
}

impl App {
    pub fn new(connections: HashMap<String, String>) -> io::Result<Self> {
        let (initial_name, initial_url) = connections
            .iter()
            .next()
            .map(|(k, v)| (k.clone(), v.clone()))
            .ok_or_else(|| io::Error::other("No connections defined in config"))?;

        let mut client = Client::connect(&initial_url, NoTls)
            .map_err(|e| io::Error::other(format!("DB connect error: {}", e)))?;

        init_script_table(&mut client)
            .map_err(|e| io::Error::other(format!("Failed to init DB table: {}", e)))?;

        let help_message = 
            "Welcome to sqledger!\n\n--- Keybinds ---\n'j'/'k'        : Navigate scripts\n'Enter'        : Run selected script\n'e'            : Edit selected script\n'a'            : Add a new script\n'd'            : Delete selected script\n'r'            : Rename selected script\n'D' (Shift+d)  : Switch Database ‼️\n'c'            : Copy results to clipboard\n'h'/'l'        : Scroll results horizontal\n↓/↑            : Scroll results vertical\n'?'            : Toggle Help\n'q'            : Quit".to_string();

        let mut app = Self {
            client,
            connections,
            connection_list_state: ListState::default(),
            current_connection_name: initial_name,
            new_connection_name_buffer: String::new(),

            scripts: Vec::new(),
            list_state: ListState::default(),
            query_result: "Welcome! Press '?' for help.".to_string(),
            query_row_count: None,
            script_content_preview: "".to_string(),
            input_mode: InputMode::Normal,
            filename_input: String::new(),
            help_message,
            result_scroll_x: 0,
            result_scroll_y: 0,
        };

        app.refresh_scripts().map_err(io::Error::other)?;

        Ok(app)
    }

    pub fn switch_connection(&mut self, name: &str) -> Result<(), String> {
        if let Some(url) = self.connections.get(name) {
            // Try connecting to the new DB
            match Client::connect(url, NoTls) {
                Ok(mut new_client) => {
                    // Ensure table exists on new DB
                    if let Err(e) = init_script_table(&mut new_client) {
                        return Err(format!("Connected, but failed to init table: {}", e));
                    }

                    // Swap the client
                    self.client = new_client;
                    self.current_connection_name = name.to_string();

                    // Reset state
                    self.query_result = format!("Switched to database: {}", name);
                    self.query_row_count = None;

                    // Load scripts from new DB
                    self.refresh_scripts()?;
                    Ok(())
                }
                Err(e) => Err(format!("Failed to connect to '{}': {}", name, e)),
            }
        } else {
            Err("Connection name not found.".to_string())
        }
    }

    pub fn refresh_scripts(&mut self) -> Result<(), String> {
        let scripts = get_all_scripts(&mut self.client)?;
        self.scripts = scripts;

        let mut valid_selection_exists = false;
        if let Some(selected_index) = self.list_state.selected() {
            valid_selection_exists = selected_index < self.scripts.len();
        }
        if !valid_selection_exists {
            if !self.scripts.is_empty() {
                self.list_state.select(Some(0));
            } else {
                self.list_state.select(None);
            }
        }
        self.update_preview();
        Ok(())
    }

    pub fn next_connection(&mut self) {
        if self.connections.is_empty() {
            return;
        }
        let i = match self.connection_list_state.selected() {
            Some(i) => {
                if i >= self.connections.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.connection_list_state.select(Some(i));
    }

    pub fn previous_connection(&mut self) {
        if self.connections.is_empty() {
            return;
        }
        let i = match self.connection_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.connections.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.connection_list_state.select(Some(i));
    }

    pub fn set_db_result(&mut self, result: QueryResult) {
        self.query_result = result.formatted_output;
        self.query_row_count = result.row_count;
        self.result_scroll_x = 0;
        self.result_scroll_y = 0;
    }

    pub fn set_query_result(&mut self, message: String) {
        self.query_result = message;
        self.query_row_count = None;
        self.result_scroll_x = 0;
        self.result_scroll_y = 0;
    }

    pub fn scroll_results_left(&mut self) {
        self.result_scroll_x = self.result_scroll_x.saturating_sub(4);
    }

    pub fn scroll_results_right(&mut self) {
        self.result_scroll_x = self.result_scroll_x.saturating_add(4);
    }

    pub fn scroll_results_up(&mut self) {
        self.result_scroll_y = self.result_scroll_y.saturating_sub(1);
    }

    pub fn scroll_results_down(&mut self) {
        self.result_scroll_y = self.result_scroll_y.saturating_add(1);
    }

    pub fn get_selected_script(&self) -> Option<&Script> {
        self.list_state.selected().and_then(|i| self.scripts.get(i))
    }

    pub fn next(&mut self) {
        if self.scripts.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.scripts.len() - 1 {
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
        if self.scripts.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.scripts.len() - 1
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
        if let Some(script) = self.get_selected_script() {
            self.script_content_preview = script.content.clone();
        } else {
            self.script_content_preview = "No scripts found.".to_string();
        }
    }
}
