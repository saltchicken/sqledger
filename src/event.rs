use crate::{
    app::{App, InputMode},
    db::{create_script, delete_script, execute_sql, rename_script, update_script_content},
    editor::open_editor,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use postgres::Client;
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
    client: &mut Client,
    terminal: &mut Terminal<B>,
) -> io::Result<bool> {
    match app.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('j') => app.next(),
            KeyCode::Char('k') => app.previous(),
            KeyCode::Enter => {

                let script_content = app.get_selected_script().map(|s| s.content.clone());
                if let Some(content) = script_content {
                    match execute_sql(client, &content) {
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
                    // Create temp file
                    let mut temp_dir = std::env::temp_dir();
                    temp_dir.push(format!("sqledger_{}.sql", script_name));

                    if let Err(e) = fs::write(&temp_dir, &script_content) {
                        app.set_query_result(format!("Error creating temp file: {}", e));
                    } else {
                        // Open editor
                        let success = open_editor(terminal, &temp_dir)?;

                        if success {
                            // Read back
                            match fs::read_to_string(&temp_dir) {
                                Ok(new_content) => {
                                    // Update DB
                                    match update_script_content(client, script_id, &new_content) {
                                        Ok(_) => {
                                            app.set_query_result(format!(
                                                "Saved changes to '{}'.",
                                                script_name
                                            ));
                                            let _ = app.refresh_scripts(client);
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
                        // Clean up
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
        InputMode::EditingFilename => match key.code {
            KeyCode::Enter => {

                let name = app.filename_input.trim().to_string();
                if name.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("New script cancelled.".to_string());
                } else {
                    match create_script(client, &name) {
                        Ok(_) => {
                            app.set_query_result(format!("Script '{}' created.", name));
                            let _ = app.refresh_scripts(client);
                            // Select the new one
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
                    match delete_script(client, id) {
                        Ok(_) => {
                            app.set_query_result(format!("Script '{}' deleted.", name));
                            let _ = app.refresh_scripts(client);
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
                        match rename_script(client, id, &new_name) {
                            Ok(_) => {
                                app.set_query_result("Script renamed.".to_string());
                                let _ = app.refresh_scripts(client);
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
