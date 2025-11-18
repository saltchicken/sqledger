use crate::{
    app::{App, InputMode},
    config::save_config,
    db::{create_script, delete_script, execute_sql, rename_script, update_script_content},
    editor::open_editor,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use std::{
    fs, io,
    io::Write,
    process::{Command, Stdio},
};

fn copy_to_clipboard(app: &mut App, text: String) {
    let mut child = match Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(e) => {
            app.set_query_result(format!("Error: Failed to spawn wl-copy: {}", e));
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        match stdin.write_all(text.as_bytes()) {
            Ok(_) => {
                app.set_query_result("Results copied to clipboard!".to_string());
            }
            Err(e) => {
                app.set_query_result(format!("Error: Failed to write to wl-copy: {}", e));
            }
        }
    } else {
        app.set_query_result("Error: Failed to get wl-copy stdin.".to_string());
    }
}

pub fn handle_key_event<B: Backend + io::Write>(
    key: KeyEvent,
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> io::Result<bool> {
    match app.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('j') => app.next(),
            KeyCode::Char('k') => app.previous(),
            KeyCode::Char('D') => {
                app.input_mode = InputMode::SelectingConnection;
                app.connection_list_state.select(Some(0));
            }
            KeyCode::Enter => {
                let script_content = app.get_selected_script().map(|s| s.content.clone());
                if let Some(content) = script_content {
                    match execute_sql(&mut app.client, &content) {
                        Ok(result) => app.set_db_result(result),
                        Err(e) => app.set_query_result(e),
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => app.scroll_results_left(),
            KeyCode::Char('l') | KeyCode::Right => app.scroll_results_right(),
            KeyCode::Down => app.scroll_results_down(),
            KeyCode::Up => app.scroll_results_up(),
            KeyCode::Char('c') => {
                copy_to_clipboard(app, app.query_result.clone());
            }
            KeyCode::Char('e') => {
                let script_data = app
                    .get_selected_script()
                    .map(|s| (s.id, s.name.clone(), s.content.clone()));

                if let Some((script_id, script_name, script_content)) = script_data {
                    let mut temp_dir = std::env::temp_dir();
                    temp_dir.push(format!("sqledger_{}.sql", script_name));

                    if let Err(e) = fs::write(&temp_dir, &script_content) {
                        app.set_query_result(format!("Error creating temp file: {}", e));
                    } else {
                        let success = open_editor(terminal, &temp_dir)?;
                        if success {
                            match fs::read_to_string(&temp_dir) {
                                Ok(new_content) => {
                                    match update_script_content(
                                        &mut app.client,
                                        script_id,
                                        &new_content,
                                    ) {
                                        Ok(_) => {
                                            app.set_query_result(format!(
                                                "Saved changes to '{}'.",
                                                script_name
                                            ));
                                            let _ = app.refresh_scripts();
                                        }
                                        Err(e) => {
                                            app.set_query_result(format!("DB Error saving: {}", e))
                                        }
                                    }
                                }
                                Err(e) => {
                                    app.set_query_result(format!("Error reading temp file: {}", e))
                                }
                            }
                        } else {
                            app.set_query_result("Editor exited with error.".to_string());
                        }
                        let _ = fs::remove_file(temp_dir);
                    }
                }
            }
            KeyCode::Char('a') => {
                app.input_mode = InputMode::EditingFilename;
                app.filename_input.clear();
                app.set_query_result(
                    "Enter new script name. Press [Enter] to confirm, [Esc] to cancel.".to_string(),
                );
            }
            KeyCode::Char('d') => {
                let script_name = app.get_selected_script().map(|s| s.name.clone());
                if let Some(name) = script_name {
                    app.input_mode = InputMode::ConfirmingDelete;
                    app.set_query_result(format!("Delete script '{}'? (y/n)", name));
                } else {
                    app.set_query_result("No script selected to delete.".to_string());
                }
            }
            KeyCode::Char('r') => {
                let script_name = app.get_selected_script().map(|s| s.name.clone());
                if let Some(name) = script_name {
                    app.input_mode = InputMode::RenamingScript;
                    app.filename_input = name;
                    app.set_query_result(
                        "Enter new script name. Press [Enter] to confirm, [Esc] to cancel."
                            .to_string(),
                    );
                } else {
                    app.set_query_result("No script selected to rename.".to_string());
                }
            }
            KeyCode::Char('?') => {
                app.input_mode = InputMode::ShowHelp;
            }
            _ => {}
        },


        InputMode::SelectingConnection => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.next_connection(),
            KeyCode::Char('k') | KeyCode::Up => app.previous_connection(),

            KeyCode::Char('a') => {
                app.input_mode = InputMode::AddingConnectionName;
                app.filename_input.clear();
            }
            KeyCode::Esc | KeyCode::Char('q') => app.input_mode = InputMode::Normal,
            KeyCode::Enter => {
                if let Some(idx) = app.connection_list_state.selected() {
                    let keys: Vec<String> = app.connections.keys().cloned().collect();
                    if let Some(name) = keys.get(idx) {
                        let name_clone = name.clone();
                        match app.switch_connection(&name_clone) {
                            Ok(_) => app.input_mode = InputMode::Normal,
                            Err(e) => app.set_query_result(format!("Error: {}", e)),
                        }
                    }
                }
            }
            _ => {}
        },

        InputMode::AddingConnectionName => match key.code {
            KeyCode::Enter => {
                let name = app.filename_input.trim().to_string();
                if name.is_empty() {
                    app.set_query_result("Name cannot be empty.".to_string());
                } else if app.connections.contains_key(&name) {
                    app.set_query_result("Connection name already exists.".to_string());
                } else {
                    app.new_connection_name_buffer = name;
                    app.input_mode = InputMode::AddingConnectionUrl;
                    app.filename_input.clear();
                }
            }
            KeyCode::Esc => {
                app.input_mode = InputMode::SelectingConnection;
            }
            KeyCode::Backspace => {
                app.filename_input.pop();
            }
            KeyCode::Char(c) => {
                app.filename_input.push(c);
            }
            _ => {}
        },

        InputMode::AddingConnectionUrl => match key.code {
            KeyCode::Enter => {
                let url = app.filename_input.trim().to_string();
                if url.is_empty() {
                    app.set_query_result("URL cannot be empty.".to_string());
                } else {
                    let name = app.new_connection_name_buffer.clone();
                    app.connections.insert(name.clone(), url);

                    if let Err(e) = save_config(&app.connections) {
                        app.set_query_result(format!(
                            "Connection added, but failed to save config: {}",
                            e
                        ));
                    } else {
                        app.set_query_result(format!("Added new connection: {}", name));
                    }
                    app.input_mode = InputMode::SelectingConnection;
                }
            }
            KeyCode::Esc => {
                app.input_mode = InputMode::SelectingConnection;
            }
            KeyCode::Backspace => {
                app.filename_input.pop();
            }
            KeyCode::Char(c) => {
                app.filename_input.push(c);
            }
            _ => {}
        },

        InputMode::EditingFilename => match key.code {
            KeyCode::Enter => {
                let name = app.filename_input.trim().to_string();
                if name.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("New script cancelled.".to_string());
                } else {
                    match create_script(&mut app.client, &name) {
                        Ok(_) => {
                            app.set_query_result(format!("Script '{}' created.", name));
                            let _ = app.refresh_scripts();
                            if let Some(idx) = app.scripts.iter().position(|s| s.name == name) {
                                app.list_state.select(Some(idx));
                                app.update_preview();
                            }
                        }
                        Err(e) => app.set_query_result(format!("Error creating script: {}", e)),
                    }
                    app.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.set_query_result("New script cancelled.".to_string());
            }
            KeyCode::Backspace => {
                app.filename_input.pop();
            }
            KeyCode::Char(c) => {
                if c == 'c' && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("New script cancelled.".to_string());
                } else {
                    app.filename_input.push(c);
                }
            }
            _ => {}
        },
        InputMode::ConfirmingDelete => match key.code {
            KeyCode::Char('y') => {
                let script_data = app.get_selected_script().map(|s| (s.id, s.name.clone()));
                if let Some((id, name)) = script_data {
                    match delete_script(&mut app.client, id) {
                        Ok(_) => {
                            app.set_query_result(format!("Script '{}' deleted.", name));
                            let _ = app.refresh_scripts();
                        }
                        Err(e) => app.set_query_result(format!("Error deleting script: {}", e)),
                    }
                }
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.set_query_result("Deletion cancelled.".to_string());
            }
            _ => {}
        },
        InputMode::RenamingScript => match key.code {
            KeyCode::Enter => {
                let new_name = app.filename_input.trim().to_string();
                if new_name.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("Rename cancelled.".to_string());
                } else {
                    let script_id = app.get_selected_script().map(|s| s.id);
                    if let Some(id) = script_id {
                        match rename_script(&mut app.client, id, &new_name) {
                            Ok(_) => {
                                app.set_query_result("Script renamed.".to_string());
                                let _ = app.refresh_scripts();
                                if let Some(idx) =
                                    app.scripts.iter().position(|s| s.name == new_name)
                                {
                                    app.list_state.select(Some(idx));
                                    app.update_preview();
                                }
                            }
                            Err(e) => app.set_query_result(format!("Error renaming: {}", e)),
                        }
                    }
                    app.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.set_query_result("Rename cancelled.".to_string());
            }
            KeyCode::Backspace => {
                app.filename_input.pop();
            }
            KeyCode::Char(c) => {
                if c == 'c' && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("Rename cancelled.".to_string());
                } else {
                    app.filename_input.push(c);
                }
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
    Ok(true)
}