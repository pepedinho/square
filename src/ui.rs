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

#[cfg(test)]
mod tests {
    use super::*;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use tokio::sync::mpsc;

    use crate::app::Action;

    fn app(width: u16, height: u16) -> App {
        let (tx, _rx) = mpsc::unbounded_channel::<(usize, Vec<u8>)>();
        App::new(tx, ratatui::layout::Size::new(width, height))
    }

    fn buffer_row(buffer: &ratatui::buffer::Buffer, row: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    #[test]
    fn renders_a_bordered_pane_with_content_and_mode_bar() {
        let (width, height) = (60, 20);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut app = app(width, height);
        app.get_current_pane_mut().unwrap().process_output(b"hi");

        terminal.draw(|f| render(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();

        // Pane outer rect: 58x17 content + 2 => 60x19, top-left corner at (0,0).
        assert_eq!(buffer[(0, 0)].symbol(), "┌");
        assert_eq!(buffer[(width - 1, 0)].symbol(), "┐");
        assert_eq!(buffer[(0, 18)].symbol(), "└");
        assert_eq!(buffer[(width - 1, 18)].symbol(), "┘");
        assert_eq!(buffer[(0, 1)].symbol(), "│");
        assert_eq!(buffer[(width - 1, 1)].symbol(), "│");

        // The title lives on the top border.
        assert!(buffer_row(buffer, 0).contains("SQUARE - Pane 0 [Normal]"));

        // Content is drawn at outer + 1.
        assert_eq!(buffer[(1, 1)].symbol(), "h");
        assert_eq!(buffer[(2, 1)].symbol(), "i");

        // Mode bar occupies the last row.
        assert!(buffer_row(buffer, height - 1).contains("-- NORMAL --"));

        // Active pane border is bold.
        assert!(buffer[(0, 0)].modifier.contains(Modifier::BOLD));

        // Cursor tracks the pane's terminal cursor at outer + 1 + offset.
        assert_eq!(
            terminal.backend().cursor_position(),
            Position { x: 1 + 2, y: 1 }
        );
    }

    #[test]
    fn mode_bar_reflects_insert_mode() {
        let (width, height) = (60, 20);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut app = app(width, height);
        app.handle_action(Action::SwitchMode(Mode::Insert));

        terminal.draw(|f| render(f, &app)).unwrap();

        assert!(buffer_row(terminal.backend().buffer(), height - 1).contains("-- INSERT --"));
    }

    #[test]
    fn renders_every_pane_of_a_split_tree() {
        let (width, height) = (60, 20);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut app = app(width, height);
        app.handle_action(Action::SplitVertical);
        app.get_current_pane_mut().unwrap().process_output(b"left");
        app.root.find_leaf_mut(1).unwrap().process_output(b"right");

        terminal.draw(|f| render(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let top = buffer_row(buffer, 0);

        // Each pane gets its own title on the shared top border.
        assert!(top.contains("SQUARE - Pane 0"));
        assert!(top.contains("SQUARE - Pane 1"));

        // Both panes' content areas are rendered.
        assert_eq!(buffer[(1, 1)].symbol(), "l");
        assert_eq!(buffer[(29, 1)].symbol(), "r");

        // Cursor stays on the *active* pane (pane 0), one row too low fixed.
        assert_eq!(
            terminal.backend().cursor_position(),
            Position { x: 1 + 4, y: 1 }
        );
    }

    #[test]
    fn empty_cells_are_filled_with_spaces() {
        let (width, height) = (60, 20);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let app = app(width, height);

        terminal.draw(|f| render(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(1, 1)].symbol(), " ");
        assert_eq!(buffer[(10, 10)].symbol(), " ");
    }
}
