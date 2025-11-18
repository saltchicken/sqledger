use crate::app::{App, InputMode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

/// Renders the user interface
pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(f.area());

    // --- Left Pane: Script List ---
    let items: Vec<ListItem> = app
        .scripts
        .iter()
        .map(|script| ListItem::new(script.name.clone()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Stored Scripts"),
        )
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

    let preview_title = format!("Preview (DB: {})", app.current_connection_name);
    let preview_block = Block::default().borders(Borders::ALL).title(preview_title);
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
            let area = centered_rect(60, 15, f.area());
            let popup_block = Block::default().title("Help").borders(Borders::ALL);
            let popup_paragraph = Paragraph::new(app.help_message.as_str())
                .block(popup_block)
                .alignment(Alignment::Left);
            f.render_widget(Clear, area);
            f.render_widget(popup_paragraph, area);
        }

        InputMode::SelectingConnection => {
            let area = centered_rect(40, 40, f.area());

            let items: Vec<ListItem> = app
                .connections
                .keys()
                .map(|name| {
                    let display = if name == &app.current_connection_name {
                        format!("* {}", name)
                    } else {
                        format!("  {}", name)
                    };
                    ListItem::new(display)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title("Select Database")
                        .borders(Borders::ALL)
                        .style(Style::default()),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            f.render_widget(Clear, area);
            f.render_stateful_widget(list, area, &mut app.connection_list_state);
        }
        InputMode::AddingConnectionName => {
            let area = centered_rect(50, 3, f.area());
            let input_text = format!("{}_", app.filename_input); // Reuse filename_input buffer
            let popup_block = Block::default()
                .title("Connection Name (e.g. 'Production')")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Blue).fg(Color::White));
            let input_paragraph = Paragraph::new(input_text.as_str()).block(popup_block);
            f.render_widget(Clear, area);
            f.render_widget(input_paragraph, area);
        }

        InputMode::AddingConnectionUrl => {
            let area = centered_rect(70, 3, f.area());
            let input_text = format!("{}_", app.filename_input); // Reuse filename_input buffer
            let popup_block = Block::default()
                .title(format!("URL for '{}'", app.new_connection_name_buffer))
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Blue).fg(Color::White));
            let input_paragraph = Paragraph::new(input_text.as_str()).block(popup_block);
            f.render_widget(Clear, area);
            f.render_widget(input_paragraph, area);
        }
        InputMode::Normal => {
            // Do nothing
        }
    }
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    // ... (Logic remains the same, adjusting variable names slightly for clarity if needed, but kept as is)
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
