use crate::app::App; // ‼️ Use crate-relative path
use rusqlite::{Connection, Error as RusqliteError};
use std::fs;

pub fn execute_sql(app: &mut App, db_path: &str) {
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
