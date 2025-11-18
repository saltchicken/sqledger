// src/db.rs
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use postgres::{Client, Error as PostgresError, types::Type};

#[derive(Clone, Debug)]
pub struct Script {
    pub id: i32,
    pub name: String,
    pub content: String,
}

pub fn init_script_table(client: &mut Client) -> Result<(), String> {
    let query = "
        CREATE TABLE IF NOT EXISTS sqledger_scripts (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL DEFAULT '',
            created_at TIMESTAMP DEFAULT NOW(),
            updated_at TIMESTAMP DEFAULT NOW()
        );
    ";
    client.batch_execute(query).map_err(|e| e.to_string())
}

pub fn get_all_scripts(client: &mut Client) -> Result<Vec<Script>, String> {
    let query = "SELECT id, name, content FROM sqledger_scripts ORDER BY name ASC";
    let rows = client.query(query, &[]).map_err(|e| e.to_string())?;

    let scripts = rows
        .iter()
        .map(|row| Script {
            id: row.get(0),
            name: row.get(1),
            content: row.get(2),
        })
        .collect();

    Ok(scripts)
}

pub fn create_script(client: &mut Client, name: &str) -> Result<(), String> {
    client
        .execute(
            "INSERT INTO sqledger_scripts (name, content) VALUES ($1, '')",
            &[&name],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_script(client: &mut Client, id: i32) -> Result<(), String> {
    client
        .execute("DELETE FROM sqledger_scripts WHERE id = $1", &[&id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn rename_script(client: &mut Client, id: i32, new_name: &str) -> Result<(), String> {
    client
        .execute(
            "UPDATE sqledger_scripts SET name = $1 WHERE id = $2",
            &[&new_name, &id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_script_content(client: &mut Client, id: i32, content: &str) -> Result<(), String> {
    client
        .execute(
            "UPDATE sqledger_scripts SET content = $1, updated_at = NOW() WHERE id = $2",
            &[&content, &id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

// Define a new struct to hold the query result and row count
pub struct QueryResult {
    pub formatted_output: String,
    pub row_count: Option<usize>,
}

pub fn execute_sql(client: &mut Client, sql_content: &str) -> Result<QueryResult, String> {
    // ... (Rest of the file remains exactly the same as previous version)
    let mut relevant_sql = sql_content.trim();
    loop {
        relevant_sql = relevant_sql.trim_start();
        if relevant_sql.starts_with("--") {
            if let Some(newline_idx) = relevant_sql.find('\n') {
                relevant_sql = &relevant_sql[newline_idx..];
            } else {
                relevant_sql = "";
                break;
            }
        } else if relevant_sql.starts_with("/*") {
            if let Some(end_comment_idx) = relevant_sql.find("*/") {
                relevant_sql = &relevant_sql[end_comment_idx + 2..];
            } else {
                relevant_sql = "";
                break;
            }
        } else {
            break;
        }
    }

    let upper_sql = relevant_sql.to_uppercase();
    if upper_sql.starts_with("SELECT") || upper_sql.starts_with("WITH") {
        match (|| -> Result<QueryResult, PostgresError> {
            let rows = client.query(sql_content, &[])?;
            if rows.is_empty() {
                return Ok(QueryResult {
                    formatted_output: "Query returned 0 rows.".to_string(),
                    row_count: Some(0),
                });
            }
            let row_count = rows.len();
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
                        Type::FLOAT4 | Type::FLOAT8 => match row.try_get::<usize, Option<f64>>(i) {
                            Ok(Some(v)) => v.to_string(),
                            Ok(None) => "NULL".to_string(),
                            Err(e) => format!("<Err: {}>", e),
                        },
                        Type::DATE => match row.try_get::<usize, Option<NaiveDate>>(i) {
                            Ok(Some(v)) => v.to_string(),
                            Ok(None) => "NULL".to_string(),
                            Err(e) => format!("<Err: {}>", e),
                        },
                        Type::TIME => match row.try_get::<usize, Option<NaiveTime>>(i) {
                            Ok(Some(v)) => v.to_string(),
                            Ok(None) => "NULL".to_string(),
                            Err(e) => format!("<Err: {}>", e),
                        },
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
                        | Type::JSONB => match row.try_get::<usize, Option<String>>(i) {
                            Ok(Some(v)) => v,
                            Ok(None) => "NULL".to_string(),
                            Err(e) => format!("<Err: {}>", e),
                        },
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
                    output.push_str(&format!("{:<width$} | ", value, width = widths[i]));
                }
                output.push('\n');
            }
            Ok(QueryResult {
                formatted_output: output,
                row_count: Some(row_count),
            })
        })() {
            Ok(query_result) => Ok(query_result),
            Err(e) => Err(format_db_error(&e, "Error executing query")),
        }
    } else {
        match client.batch_execute(sql_content) {
            Ok(_) => Ok(QueryResult {
                formatted_output: "Command executed successfully.".to_string(),
                row_count: None,
            }),
            Err(e) => Err(format_db_error(&e, "Error executing command")),
        }
    }
}

fn format_db_error(e: &PostgresError, context: &str) -> String {
    if let Some(db_error) = e.as_db_error() {
        let mut error_message = format!(
            "{} ({:?})\n\nMessage: {}\n",
            context,
            db_error.code(),
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
        error_message.trim_end().to_string()
    } else {
        format!("{}: {}", context, e)
    }
}
