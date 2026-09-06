use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Mode};
use crate::pane::Pane;
use crate::tree::Node;

/// Render the full UI for one frame.
///
/// Every pane is drawn from the split tree, each inside its own bordered
/// rect. The active pane is highlighted with a bold border. A mode bar
/// occupies the last row.
///
/// The border color reflects the current [`Mode`]:
/// - **Normal** → light blue
/// - **Insert** → light green
/// - **Command** → light magenta
pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    draw_node(f, &app.root, app);

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

/// Recursively draw every leaf of the split tree.
fn draw_node(f: &mut Frame, node: &Node, app: &App) {
    match node {
        Node::Placeholder => {}
        Node::Leaf(pane) => {
            let active = pane.id == app.active_pane_id;
            draw_pane(f, pane, active, app.current_mode);
        }
        Node::Split { first, second, .. } => {
            draw_node(f, first, app);
            draw_node(f, second, app);
        }
    }
}

/// Draw a single pane inside its bordered rect.
fn draw_pane(f: &mut Frame, pane: &Pane, active: bool, mode: Mode) {
    let screen = pane.parser.screen();

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

                    spans.push(Span::raw(text).style(style));
                }
            }

            Line::from(spans)
        })
        .collect();

    let rect = pane.rect;
    let outer = Rect {
        x: rect.x.saturating_sub(1),
        y: rect.y.saturating_sub(1),
        width: rect.width.saturating_add(2),
        height: rect.height.saturating_add(2),
    };

    let mut border_style = Style::default().fg(match mode {
        Mode::Normal => Color::LightBlue,
        Mode::Insert => Color::LightGreen,
        Mode::Command => Color::LightMagenta,
    });
    if active {
        border_style = border_style.add_modifier(Modifier::BOLD);
    }

    let pane_block = Block::default()
        .title(format!("SQUARE - Pane {} [{:?}]", pane.id, mode))
        .borders(Borders::ALL)
        .border_style(border_style);

    f.render_widget(Paragraph::new(lines).block(pane_block), outer);

    if active {
        let (cur_row, cur_col) = screen.cursor_position();
        f.set_cursor_position((outer.x + 1 + cur_col, outer.y + 1 + cur_row));
    }
}

/// Convert a [`vt100::Color`] to the closest [`ratatui::style::Color`].
fn vt100_color_to_ratatui(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
