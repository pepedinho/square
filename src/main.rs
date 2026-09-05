//! Binary entry point.
//!
//! Sets up the terminal (raw mode + alternate screen), then runs the async
//! event loop with `tokio::select!`:
//!
//! - PTY output arrives on an unbounded `mpsc` channel `(pane_id, Vec<u8>)`.
//! - Key events arrive via [`crossterm::EventStream`].
//! - A render tick fires every 16 ms (~60 FPS) and redraws only when dirty.

use std::io::{self, stdout};

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, prelude::CrosstermBackend};
use square::{
    app::{Action, App, Mode},
    input, ui,
};
use tokio::{sync::mpsc, time};

/// Application entry point.
///
/// 1. Enable raw mode and switch to the alternate screen.
/// 2. Create the `App` with its initial pane and the PTY output channel.
/// 3. Loop until `App::should_quit`:
///    - ~60 FPS render interval (16 ms), redrawing only when `needs_render`.
///    - Feed PTY output from the channel into the matching pane's parser.
///    - Translate key events into [`square::app::Action`]s via `square::input::handle_key`.
///    - Accumulate characters into `App::command_buffer` while in Command mode.
/// 4. Restore the terminal (leave alternate screen, exit raw mode).
#[tokio::main]
async fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<(usize, Vec<u8>)>();

    let mut app = App::new(tx, size.width, size.height);
    let mut event_stream = EventStream::new();

    let mut render_interval = time::interval(time::Duration::from_millis(16));
    let mut needs_render = false;

    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &app))?;

        tokio::select! {
            maybe_data = rx.recv() => {
                if let Some((pane_id, data)) = maybe_data {
                    if let Some(pane) = app.panes.iter_mut().find(|p| p.id == pane_id) {
                        pane.process_output(&data);
                        needs_render = true;
                    }
                } else {
                    break;
                }
            }
            maybe_event = event_stream.next() => {
                if let Some(Ok(ev)) = maybe_event {
                    if let Event::Resize(w, h) = ev {
                        app.handle_action(Action::ResizeTerminal(w, h));
                        continue;
                    }

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
                    needs_render = true;
                }
            }

            _ = render_interval.tick() => {
            if needs_render {
                terminal.draw(|f| ui::render(f, &app))?;
                needs_render = false;
            }
        }

        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
