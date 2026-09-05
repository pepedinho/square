use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Mode};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    if let Some(active_pane) = app.panes.iter().find(|p| p.id == app.active_pane_id) {
        let screen = active_pane.parser.screen();

        let (rows, cols) = screen.size();
        let lines: Vec<Line> = (0..rows)
            .map(|row_idx| {
                let mut spans = vec![];

                for col_idx in 0..cols {
                    if let Some(cell) = screen.cell(row_idx, col_idx) {
                        let mut style = Style::default();

                        style = style
                            .fg(vt100_color_to_ratatui(cell.fgcolor()))
                            .bg(vt100_color_to_ratatui(cell.bgcolor()));

                        if cell.bold() {
                            style = style.add_modifier(Modifier::BOLD);
                        }

                        let content = cell.contents();
                        let text = if content.is_empty() { " " } else { content };

                        spans.push(Span::styled(text.to_string(), style));
                    }
                }

                Line::from(spans)
            })
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

        let paragraph = Paragraph::new(lines).block(pane_block);
        f.render_widget(paragraph, chunks[0]);

        let (cur_row, cur_col) = screen.cursor_position();

        f.set_cursor_position((chunks[0].x + 1 + cur_col, chunks[0].y + 1 + cur_row));
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

fn vt100_color_to_ratatui(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
