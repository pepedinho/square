use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, Mode};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    if let Some(active_pane) = app.panes.iter().find(|p| p.id == app.active_pane_id) {
        let items: Vec<ListItem> = active_pane
            .buffer
            .iter()
            .map(|line| ListItem::new(line.as_str()))
            .collect();

        let pane_block = Block::default()
            .title(format!(
                "SQUARE - Pane {} [{:?}]",
                active_pane.id, app.current_mode
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(match app.current_mode {
                Mode::Normal => Color::LightBlue,
                Mode::Insert => Color::LightGreen,
                Mode::Command => Color::LightMagenta,
            }));

        let list = List::new(items).block(pane_block);
        f.render_widget(list, chunks[0]);
    }

    let bottom_text = match app.current_mode {
        Mode::Command => Paragraph::new(format!(":{}", app.command_buffer))
            .style(Style::default().fg(Color::LightMagenta)),
        Mode::Normal => Paragraph::new("-- NORMAL --").style(
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Insert => Paragraph::new("-- INSERT -- ").style(
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
    };

    f.render_widget(bottom_text, chunks[1]);
}
