use crate::db::{QueryResult, Script, get_all_scripts};
use postgres::Client;
use ratatui::widgets::ListState;
use std::io;

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
    pub scripts: Vec<Script>,
    pub list_state: ListState,
    pub query_result: String,
    pub query_row_count: Option<usize>,
    pub script_content_preview: String,
    pub input_mode: InputMode,
    pub filename_input: String, // Used for "Script Name" input
    pub help_message: String,
    pub result_scroll_x: u16,
    pub result_scroll_y: u16,
}

impl App {

    pub fn new(client: &mut Client, db_url: &str) -> io::Result<Self> {
        let help_message = format!(
            "Welcome to sqledger!\n\nSource: Postgres Table (sqledger_scripts)\nDatabase: {}\n\n--- Keybinds ---\n'j'/'k'         : Navigate scripts\n'Enter'       : Run selected script\n'e'           : Edit selected script\n'a'           : Add a new script\n'd'           : Delete selected script\n'r'           : Rename selected script\n'c'           : Copy results to clipboard\n'h'/'l' or ←/→ : Scroll results horizontally\n↓/↑           : Scroll results vertically\n'?'           : Toggle this help message\n'q'           : Quit",
            db_url
        );

        let mut app = Self {
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


        app.refresh_scripts(client)
            .map_err(|e| io::Error::other(e))?;

        Ok(app)
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


    pub fn refresh_scripts(&mut self, client: &mut Client) -> Result<(), String> {
        let scripts = get_all_scripts(client)?;
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