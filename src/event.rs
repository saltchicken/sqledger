use crate::{
    app::{App, InputMode},
    db::{QueryResult, execute_sql},
    editor::open_editor,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use postgres::Client;
use ratatui::{Terminal, backend::Backend};
use std::{
    fs, io,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

/// Copies the given text to the clipboard by spawning `wl-copy`.
fn copy_to_clipboard(app: &mut App, text: String) {
    // ... (function is unchanged)
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
    script_dir_path: &Path,
    terminal: &mut Terminal<B>,
) -> io::Result<bool> {
    match app.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('j') => app.next(),
            KeyCode::Char('k') => app.previous(),
            KeyCode::Enter => {
                if let Some(selected_index) = app.list_state.selected() {
                    let file_path = &app.sql_files[selected_index];
                    match fs::read_to_string(file_path) {
                        Ok(sql_content) => {
                            match execute_sql(client, &sql_content) {

                                Ok(result) => app.set_db_result(result),
                                Err(e) => app.set_query_result(e),
                            }
                        }
                        Err(e) => {
                            app.set_query_result(format!(
                                "Error reading file {}: {}",
                                file_path, e
                            ));
                        }
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => app.scroll_results_left(),
            // ... (rest of file is unchanged)
            KeyCode::Char('l') | KeyCode::Right => app.scroll_results_right(),
            KeyCode::Down => app.scroll_results_down(),
            KeyCode::Up => app.scroll_results_up(),
            KeyCode::Char('c') => {
                copy_to_clipboard(app, app.query_result.clone());
            }
            KeyCode::Char('e') => {
                if let Some(selected_index) = app.list_state.selected()
                    && let Some(file_path_str) = app.sql_files.get(selected_index)
                {
                    let file_path = Path::new(file_path_str);
                    let success = open_editor(terminal, file_path)?;
                    if !success {
                        app.set_query_result("Editor exited with an error.".to_string());
                    }
                    app.rescan_scripts(script_dir_path)?;
                }
            }
            KeyCode::Char('a') => {
                app.input_mode = InputMode::EditingFilename;
                app.filename_input.clear();
                app.set_query_result(
                    "Enter new script name (no extension). Press [Enter] to confirm, [Esc] to cancel."
                        .to_string(),
                );
            }
            KeyCode::Char('d') => {
                if app.list_state.selected().is_some() {
                    app.input_mode = InputMode::ConfirmingDelete;
                    let filename = app.get_selected_filename_stem().unwrap_or_default();
                    app.set_query_result(format!("Delete '{}'? (y/n)", filename));
                } else {
                    app.set_query_result("No script selected to delete.".to_string());
                }
            }
            KeyCode::Char('r') => {
                if let Some(filename_stem) = app.get_selected_filename_stem() {
                    app.input_mode = InputMode::RenamingScript;
                    app.filename_input = filename_stem;
                    app.set_query_result(
                        "Enter new script name (no extension). Press [Enter] to confirm, [Esc] to cancel."
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
                let filename_stem = app.filename_input.trim();
                if filename_stem.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("New script cancelled.".to_string());
                } else {
                    let mut new_file_path = script_dir_path.to_path_buf();
                    new_file_path.push(format!("{}.sql", filename_stem));
                    if new_file_path.exists() {
                        app.set_query_result(format!(
                            "Error: File {} already exists.",
                            new_file_path.display()
                        ));
                    } else {
                        let new_file_path_str = new_file_path.to_string_lossy().to_string();
                        fs::write(&new_file_path, "")?;
                        let success = open_editor(terminal, &new_file_path)?;
                        if !success {
                            app.set_query_result("Editor exited with an error.".to_string());
                        } else {
                            app.set_query_result(format!(
                                "Script {} created successfully.",
                                new_file_path.display()
                            ));
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
                if let Some(selected_index) = app.list_state.selected()
                    && let Some(file_path_str) = app.sql_files.get(selected_index)
                {
                    match fs::remove_file(file_path_str) {
                        Ok(_) => {
                            app.set_query_result(format!("File {} deleted.", file_path_str));
                            app.rescan_scripts(script_dir_path)?;
                        }
                        Err(e) => {
                            app.set_query_result(format!(
                                "Error deleting file {}: {}",
                                file_path_str, e
                            ));
                        }
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
                let new_filename_stem = app.filename_input.trim();
                if new_filename_stem.is_empty() {
                    app.input_mode = InputMode::Normal;
                    app.set_query_result("Rename cancelled.".to_string());
                } else {
                    if let Some(selected_index) = app.list_state.selected()
                        && let Some(old_path_str) = app.sql_files.get(selected_index)
                    {
                        let old_path = Path::new(old_path_str);
                        let mut new_path =
                            old_path.parent().unwrap_or(script_dir_path).to_path_buf();
                        new_path.push(format!("{}.sql", new_filename_stem));
                        if new_path.exists() {
                            app.set_query_result(format!(
                                "Error: File {} already exists.",
                                new_path.display()
                            ));
                        } else {
                            match fs::rename(old_path, &new_path) {
                                Ok(_) => {
                                    app.set_query_result("File renamed.".to_string());
                                    let new_path_str = new_path.to_string_lossy().to_string();
                                    app.rescan_scripts(script_dir_path)?;
                                    if let Some(new_index) =
                                        app.sql_files.iter().position(|p| p == &new_path_str)
                                    {
                                        app.list_state.select(Some(new_index));
                                        app.update_preview();
                                    }
                                }
                                Err(e) => {
                                    app.set_query_result(format!("Error renaming file: {}", e));
                                }
                            }
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
