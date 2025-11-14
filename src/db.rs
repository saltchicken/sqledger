use crate::app::App;
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
                        app.query_result = format!("Error connecting to database: {}", e);
                        return;
                    }
                };

                let trimmed_sql = sql_content.trim();

                if trimmed_sql.to_uppercase().starts_with("SELECT") {
                    match (|| -> Result<String, PostgresError> {
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
                            // ‼️ Corrected this line's type from Vec<Vec<String>> to Vec<String>
                            // and added :: to satisfy the compiler.
                            let mut values = Vec::<String>::new();
                            for (i, col) in row.columns().iter().enumerate() {
                                let val_str: String = match *col.type_() {
                                    Type::BOOL => match row.try_get::<usize, Option<bool>>(i) {
                                        Ok(Some(v)) => v.to_string(),
                                        Ok(None) => "NULL".to_string(),
                                        Err(e) => format!("<Err: {}>", e),
                                    },

                                    Type::INT2 => {
                                        // This is i16
                                        match row.try_get::<usize, Option<i16>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    Type::INT4 => {
                                        // This is i32 (integer)
                                        match row.try_get::<usize, Option<i32>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }
                                    Type::INT8 => {
                                        // This is i64 (bigint)
                                        match row.try_get::<usize, Option<i64>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }

                                    Type::FLOAT4 | Type::FLOAT8 => {
                                        match row.try_get::<usize, Option<f64>>(i) {
                                            Ok(Some(v)) => v.to_string(),
                                            Ok(None) => "NULL".to_string(),
                                            Err(e) => format!("<Err: {}>", e),
                                        }
                                    }

                                    Type::TEXT
                                    | Type::VARCHAR
                                    | Type::NAME
                                    | Type::NUMERIC
                                    | Type::TIMESTAMP
                                    | Type::TIMESTAMPTZ
                                    | Type::DATE
                                    | Type::TIME
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
                        Ok(formatted_result) => app.query_result = formatted_result,
                        Err(e) => app.query_result = format!("Error executing query: {}", e),
                    }
                } else {
                    match client.batch_execute(&sql_content) {
                        Ok(_) => {
                            app.query_result = "Command executed successfully.".to_string();
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
