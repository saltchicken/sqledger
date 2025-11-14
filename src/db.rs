use crate::app::App;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use postgres::{types::Type, Client, Error as PostgresError, NoTls};
use std::fs;
pub fn execute_sql(app: &mut App, db_url: &str) {
    if let Some(selected_index) = app.list_state.selected() {
        let file_path = &app.sql_files[selected_index];
        match fs::read_to_string(file_path) {
            Ok(sql_content) => {
                let mut client = match Client::connect(db_url, NoTls) {
                    Ok(client) => client,
                    Err(e) => {
                        // ‼️ Use new method to set result and reset scroll
                        app.set_query_result(format!("Error connecting to database: {}", e));
                        return;
                    }
                };

                // ‼️ START: New logic to strip leading comments
                // Trim whitespace from the whole string first
                let mut relevant_sql = sql_content.trim();

                // Loop to strip all leading comments (line and block)
                loop {
                    relevant_sql = relevant_sql.trim_start(); // Trim whitespace between comments

                    if relevant_sql.starts_with("--") {
                        // It's a line comment, find the next newline
                        if let Some(newline_idx) = relevant_sql.find('\n') {
                            relevant_sql = &relevant_sql[newline_idx..]; // Keep everything after the newline
                        } else {
                            // The rest of the file is just this comment
                            relevant_sql = "";
                            break;
                        }
                    } else if relevant_sql.starts_with("/*") {
                        // It's a block comment, find the closing */
                        if let Some(end_comment_idx) = relevant_sql.find("*/") {
                            relevant_sql = &relevant_sql[end_comment_idx + 2..];
                        // Keep everything after the */
                        } else {
                            // Unterminated block comment, treat as empty
                            relevant_sql = "";
                            break;
                        }
                    } else {
                        // Not a comment, this is the start of the actual SQL
                        break;
                    }
                }
                // ‼️ END: New logic

                let upper_sql = relevant_sql.to_uppercase();
                if upper_sql.starts_with("SELECT") || upper_sql.starts_with("WITH") {
                    match (|| -> Result<String, PostgresError> {
                        // ‼️ Note: We execute the *original* sql_content, not the stripped version
                        let rows = client.query(&sql_content, &[])?;
                        if rows.is_empty() {
                            return Ok("Query returned 0 rows.".to_string());
                        }

                        let column_names: Vec<String> = rows[0]
                            .columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect();

                        let mut widths: Vec<usize> = column_names.iter().map(|s| s.len()).collect();

                        let mut rows_data: Vec<Vec<String>> = Vec::new();
                        for row in &rows {
                            let mut values = Vec::<String>::new();
                            for (i, col) in row.columns().iter().enumerate() {
                                let val_str: String = match *col.type_() {
                                    Type::BOOL => match row.try_get::<usize, Option<bool>>(i) {
                                        Ok(Some(v)) => v.to_string(),
                                        Ok(None) => "NULL".to_string(),
                                        Err(e) => format!("<Err: {}>", e),
                                    },
                                    Type::INT2 => match row.try_get::<usize, Option<i16>>(i) {
                                        Ok(Some(v)) => v.to_string(),
                                        Ok(None) => "NULL".to_string(),
                                        Err(e) => format!("<Err: {}>", e),
                                    },
                                    Type::INT4 => match row.try_get::<usize, Option<i32>>(i) {
                                        Ok(Some(v)) => v.to_string(),
                                        Ok(None) => "NULL".to_string(),
                                        Err(e) => format!("<Err: {}>", e),
                                    },
                                    Type::INT8 => match row.try_get::<usize, Option<i64>>(i) {
                                        Ok(Some(v)) => v.to_string(),
                                        Ok(None) => "NULL".to_string(),
                                        Err(e) => format!("<Err: {}>", e),
                                    },
                                    Type::FLOAT4 | Type::FLOAT8 => {
                                        match row.try_get::<usize, Option<f64>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    // Add specific arms for date/time types
                                    Type::DATE => {
                                        match row.try_get::<usize, Option<NaiveDate>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    Type::TIME => {
                                        match row.try_get::<usize, Option<NaiveTime>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    Type::TIMESTAMP | Type::TIMESTAMPTZ => {
                                        match row.try_get::<usize, Option<NaiveDateTime>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    Type::TEXT
                                    | Type::VARCHAR
                                    | Type::NAME
                                    | Type::NUMERIC
                                    | Type::UUID
                                    | Type::JSON
                                    | Type::JSONB => {
                                        match row.try_get::<usize, Option<String>>(i) {
                                            Ok(Some(v)) => v,
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    // A fallback for any other unhandled types
                                    _ => match row.try_get::<usize, Option<String>>(i) {
                                        Ok(Some(v)) => v,
                                        Ok(None) => "NULL".to_string(),
                                        Err(e) => format!("<{}: {}>", col.type_().name(), e),
                                    },
                                };
                                widths[i] = widths[i].max(val_str.len());
                                values.push(val_str);
                            }
                            rows_data.push(values);
                        }

                        // This formatting logic remains the same
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
                        Ok(formatted_result) => app.set_query_result(formatted_result),
                        Err(e) => {
                            app.set_query_result(format_db_error(&e, "Error executing query"))
                        }
                    }
                } else {
                    // We run the *original* content here
                    match client.batch_execute(&sql_content) {
                        Ok(_) => {
                            app.set_query_result("Command executed successfully.".to_string());
                        }
                        Err(e) => {
                            app.set_query_result(format_db_error(&e, "Error executing command"))
                        }
                    }
                }
            }
            Err(e) => {
                app.set_query_result(format!("Error reading file {}: {}", file_path, e));
            }
        }
    }
}

/// Formats a PostgresError into a user-friendly, detailed string.
fn format_db_error(e: &PostgresError, context: &str) -> String {
    if let Some(db_error) = e.as_db_error() {
        // Build a detailed, multi-line error message
        let mut error_message = format!(
            "{} ({:?})\n\nMessage: {}\n",
            context,
            db_error.code(), // e.g., 42P01 (undefined_table)
            db_error.message()
        );

        if let Some(detail) = db_error.detail() {
            error_message.push_str(&format!("Detail: {}\n", detail));
        }
        if let Some(hint) = db_error.hint() {
            error_message.push_str(&format!("Hint: {}\n", hint));
        }
        if let Some(position) = db_error.position() {
            error_message.push_str(&format!("Position: at character {:?}\n", position));
        }
        // Remove the last newline for clean output
        error_message.trim_end().to_string()
    } else {
        // It's not a database-level error (e.g., I/O, connection)
        // so the default Display is probably fine.
        format!("{}: {}", context, e)
    }
}

