use tokio::sync::mpsc::UnboundedSender;

use crate::pane::Pane;

/// Editor interaction modes, inspired by Vim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// Normal mode — navigation shortcuts and command dispatch.
    Normal,
    /// Insert mode — keystrokes are forwarded raw to the shell's PTY.
    Insert,
    /// Command mode — text is accumulated in the bottom status bar.
    Command,
}

/// Every possible state mutation is expressed as a variant of this enum.
///
/// All mutations flow through [`App::handle_action`].
pub enum Action {
    /// Switch to the given [`Mode`].
    SwitchMode(Mode),
    /// Split the active pane vertically (not yet implemented).
    SplitVertical,
    /// Split the active pane horizontally (not yet implemented).
    SplitHorizontal,
    /// Close the active pane (not yet implemented).
    ClosePane,
    /// Execute a command string entered in Command mode (not yet implemented).
    ExecuteCommand(String),
    /// Send raw text bytes to the active pane's shell.
    WriteToShell(String),
    /// Resize every pane after the terminal itself has been resized.
    ResizeTerminal(u16, u16),
    /// Quit the application.
    Quit,
}

/// Central application state.
///
/// Holds the list of panes, the active pane id, the current [`Mode`],
/// and the command-mode input buffer. The only way to mutate this struct
/// from outside the module is through [`handle_action`](Self::handle_action).
pub struct App {
    /// Current interaction mode.
    pub current_mode: Mode,
    /// All open panes.
    pub panes: Vec<Pane>,
    /// Id of the pane that currently receives input.
    pub active_pane_id: usize,
    /// Text accumulated while in [`Mode::Command`].
    pub command_buffer: String,
    /// When `true` the main event loop exits on the next iteration.
    pub should_quit: bool,
}

impl App {
    /// Create a new application with a single initial pane sized to fit inside
    /// the given terminal dimensions.
    ///
    /// `tx` is the sending half of the channel that the PTY reader threads
    /// use to push output back into the async event loop.
    ///
    /// The inner area is calculated as:
    /// - columns = `width - 2` (1-col border on each side)
    /// - rows    = `height - 3` (1-row title bar + 1-row status bar + 1-row border)
    pub fn new(tx: UnboundedSender<(usize, Vec<u8>)>, width: u16, height: u16) -> Self {
        let inner_cols = width.saturating_sub(2);
        let inner_rows = height.saturating_sub(3);

        let first_pane = Pane::new(0, tx, inner_rows, inner_cols);

        Self {
            current_mode: Mode::Normal,
            panes: vec![first_pane],
            active_pane_id: 0,
            command_buffer: String::new(),
            should_quit: false,
        }
    }

    /// The single entry-point for all state changes.
    ///
    /// Dispatches the given [`Action`] and mutates `self` accordingly.
    /// Split, close, and command-execution actions are currently stubbed.
    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::SwitchMode(new_mode) => {
                self.current_mode = new_mode;
                if new_mode != Mode::Command {
                    self.command_buffer.clear();
                }
            }
            Action::WriteToShell(text) => {
                if let Some(pane) = self.panes.iter_mut().find(|p| p.id == self.active_pane_id) {
                    pane.write_to_shell(&text);
                }
            }
            Action::ResizeTerminal(w, h) => {
                let inner_cols = w.saturating_sub(2);
                let inner_rows = h.saturating_sub(3);

                for pane in &mut self.panes {
                    pane.resize(inner_rows, inner_cols);
                }
            }
            Action::Quit => self.should_quit = true,
            _ => unimplemented!(),
        }
    }
}
