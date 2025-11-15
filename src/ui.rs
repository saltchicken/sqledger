use crate::app::{App, InputMode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::{ffi::OsStr, path::Path};

/// Renders the user interface
pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(f.area());

    // --- Left Pane: SQL File List ---
    let items: Vec<ListItem> = app
        .sql_files
        .iter()
        .map(|full_path| {
            let filename_stem = Path::new(full_path)
                .file_stem()
                .unwrap_or_else(|| OsStr::new("invalid_filename"))
                .to_string_lossy()
                .to_string();
            ListItem::new(filename_stem)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("SQL Scripts"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // --- Right Panes (Vertically Split) ---
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(chunks[1]);

    // Top-Right Pane: Script Preview
    let preview_block = Block::default().borders(Borders::ALL).title("Preview");
    let preview_text = Paragraph::new(app.script_content_preview.as_str()).block(preview_block);
    f.render_widget(preview_text, right_chunks[0]);

    // Bottom-Right Pane: Query Results

    let results_title = match app.query_row_count {
        Some(count) => format!("Results (Rows: {})", count),
        None => "Results".to_string(),
    };


    let results_block = Block::default().borders(Borders::ALL).title(results_title);
    let results_text = Paragraph::new(app.query_result.as_str())
        .block(results_block)
        .scroll((app.result_scroll_y, app.result_scroll_x));
    f.render_widget(results_text, right_chunks[1]);

    // --- Popup Windows ---
    match app.input_mode {
        // ... (rest of file is unchanged)
        InputMode::EditingFilename => {
            let area = centered_rect(50, 3, f.area());
            let input_text = format!("{}_", app.filename_input);
            let popup_block = Block::default()
                .title("New Script Name")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::LightBlue));
            let input_paragraph = Paragraph::new(input_text.as_str()).block(popup_block);
            f.render_widget(Clear, area);
            f.render_widget(input_paragraph, area);
        }
        InputMode::ConfirmingDelete => {
            let area = centered_rect(50, 3, f.area());
            let popup_block = Block::default()
                .title("Confirm Deletion")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Red).fg(Color::White));
            let popup_paragraph = Paragraph::new(app.query_result.as_str())
                .block(popup_block)
                .alignment(Alignment::Center);
            f.render_widget(Clear, area);
            f.render_widget(popup_paragraph, area);
        }
        InputMode::RenamingScript => {
            let area = centered_rect(50, 3, f.area());
            let input_text = format!("{}_", app.filename_input);
            let popup_block = Block::default()
                .title("Rename Script")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::LightYellow).fg(Color::Black));
            let input_paragraph = Paragraph::new(input_text.as_str()).block(popup_block);
            f.render_widget(Clear, area);
            f.render_widget(input_paragraph, area);
        }
        InputMode::ShowHelp => {
            let area = centered_rect(60, 15, f.area()); // 60% width, 15 lines height
            let popup_block = Block::default().title("Help").borders(Borders::ALL);
            let popup_paragraph = Paragraph::new(app.help_message.as_str())
                .block(popup_block)
                .alignment(Alignment::Left);
            f.render_widget(Clear, area);
            f.render_widget(popup_paragraph, area);
        }
        InputMode::Normal => {
            // Do nothing
        }
    }
}

/// Helper function to create a centered rectangle for popups
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let (top_padding, bottom_padding) = {
        let total_padding = r.height.saturating_sub(height);
        (
            total_padding / 2,
            total_padding.saturating_sub(total_padding / 2),
        )
    };

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_padding),
            Constraint::Length(height),
            Constraint::Length(bottom_padding),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
