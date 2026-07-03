use std::io::{self, stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use square::{
    app::{Action, App, Mode},
    input, ui,
};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<(usize, Vec<u8>)>();

    let size = terminal.size()?;
    let mut app = App::new(tx, size.width, size.height);

    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &app))?;

        tokio::select! {
            Some((pane_id, data)) = rx.recv() => {
                if let Some(pane) = app.panes.iter_mut().find(|p| p.id == pane_id) {
                    pane.process_output(&data);
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(16)) => {
                if event::poll(std::time::Duration::from_secs(0))? {
                    let ev = event::read()?;

                    // 1. On gère le redimensionnement
                    if let Event::Resize(w, h) = ev {
                        app.handle_action(Action::ResizeTerminal(w, h));
                        continue;
                    }

                    // 2. On gère le clavier
                    if let Event::Key(key) = ev {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }

                        if app.current_mode == Mode::Command {
                            match key.code {
                                KeyCode::Char(c) => { app.command_buffer.push(c); continue; }
                                KeyCode::Backspace => { app.command_buffer.pop(); continue; }
                                _ => {}
                            }
                        }

                        if let Some(action) = input::handle_key(key, app.current_mode) {
                            app.handle_action(action);
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
