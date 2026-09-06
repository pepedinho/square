use ratatui::layout::{Rect, Size};
use tokio::sync::mpsc::UnboundedSender;

use crate::pane::Pane;
use crate::tree::{Direction, Node};

/// Editor interaction modes, inspired by Vim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Switch to the given [`Mode`].
    SwitchMode(Mode),
    /// Split the active pane vertically.
    SplitVertical,
    /// Split the active pane horizontally.
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
/// Holds the pane split tree, the active pane id, the current [`Mode`],
/// and the command-mode input buffer. The only way to mutate this struct
/// from outside the module is through [`handle_action`](Self::handle_action).
pub struct App {
    /// Current interaction mode.
    pub current_mode: Mode,
    /// Root of the pane split tree.
    pub root: Node,
    /// Id of the pane that currently receives input.
    pub active_pane_id: usize,
    /// Text accumulated while in [`Mode::Command`].
    pub command_buffer: String,
    /// When `true` the main event loop exits on the next iteration.
    pub should_quit: bool,
    /// Clone of the sender channel, shared by every pane's reader thread.
    tx: UnboundedSender<(usize, Vec<u8>)>,
    /// Monotonic counter for pane ids (safer than `panes.len()` once panes can close).
    next_id: usize,
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
    pub fn new(tx: UnboundedSender<(usize, Vec<u8>)>, size: Size) -> Self {
        let inner_cols = size.width.saturating_sub(2);
        let inner_rows = size.height.saturating_sub(3);

        let mut root = Node::Leaf(Pane::new(0, tx.clone(), inner_rows, inner_cols));
        root.layout(Rect::new(0, 0, inner_cols, inner_rows));

        Self {
            current_mode: Mode::Normal,
            root,
            active_pane_id: 0,
            command_buffer: String::new(),
            should_quit: false,
            tx,
            next_id: 1,
        }
    }

    /// Update pane size according with terminal window size
    pub fn update(&mut self, size: Size) {
        let inner_cols = size.width.saturating_sub(2);
        let inner_rows = size.height.saturating_sub(3);

        self.root.layout(Rect::new(0, 0, inner_cols, inner_rows));
    }

    /// Split the current active pane by the [`Direction`] provided in methods arguments.
    pub fn split(&mut self, direction: Direction) {
        let new_id = self.next_id();
        self.root
            .split_active(self.active_pane_id, direction, self.tx.clone(), new_id);
    }

    /// Increment the [`next_id`](Self::next_id) counter.
    ///
    /// Returns the id before incrementation, so the first call returns `1`
    /// and leaves the internal counter at `2`.
    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Look up the pane whose `id` matches [`active_pane_id`](Self::active_pane_id).
    ///
    /// Returns `None` if no pane has that id.
    pub fn get_current_pane(&self) -> Option<&Pane> {
        self.root.find_leaf(self.active_pane_id)
    }

    /// Mutable counterpart of [`get_current_pane`](Self::get_current_pane).
    ///
    /// Use this when the active pane need to be mutated (e.g. `write_to_shell`).
    pub fn get_current_pane_mut(&mut self) -> Option<&mut Pane> {
        self.root.find_leaf_mut(self.active_pane_id)
    }

    /// The single entry-point for all state changes.
    ///
    /// Dispatches the given [`Action`] and mutates `self` accordingly.
    /// Close and command-execution actions are currently stubbed.
    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::SwitchMode(new_mode) => {
                self.current_mode = new_mode;
                if new_mode != Mode::Command {
                    self.command_buffer.clear();
                }
            }
            Action::WriteToShell(text) => {
                if let Some(pane) = self.get_current_pane_mut() {
                    pane.write_to_shell(&text);
                }
            }
            Action::ResizeTerminal(w, h) => self.update(Size::new(w, h)),
            Action::SplitVertical => self.split(Direction::Vertical),
            Action::SplitHorizontal => self.split(Direction::Horizontal),
            Action::Quit => self.should_quit = true,
            _ => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::sync::mpsc;

    fn new_app(size: Size) -> App {
        let (tx, _rx) = mpsc::unbounded_channel::<(usize, Vec<u8>)>();
        App::new(tx, size)
    }

    #[test]
    fn new_creates_a_single_sized_leaf_root() {
        let app = new_app(Size::new(80, 24));

        assert_eq!(app.current_mode, Mode::Normal);
        assert_eq!(app.active_pane_id, 0);
        assert!(!app.should_quit);
        assert!(matches!(app.root, Node::Leaf(_)));

        let pane = app.get_current_pane().unwrap();
        assert_eq!(pane.id, 0);
        // inner area: width - 2, height - 3
        assert_eq!(pane.rect, Rect::new(0, 0, 78, 21));
    }

    #[test]
    fn next_id_counts_up_from_one() {
        let mut app = new_app(Size::new(80, 24));

        assert_eq!(app.next_id(), 1);
        assert_eq!(app.next_id(), 2);
        assert_eq!(app.next_id(), 3);
    }

    #[test]
    fn split_creates_a_second_pane_in_the_tree() {
        let mut app = new_app(Size::new(80, 24));

        app.handle_action(Action::SplitVertical);

        assert!(matches!(app.root, Node::Split { .. }));
        assert!(app.root.find_leaf(0).is_some());
        assert!(app.root.find_leaf(1).is_some());

        // Splitting does not change the focused pane.
        assert_eq!(app.active_pane_id, 0);
    }

    #[test]
    fn split_horizontal_pane_ids_are_unique() {
        let mut app = new_app(Size::new(80, 24));

        app.handle_action(Action::SplitHorizontal);
        app.handle_action(Action::SplitHorizontal);

        let first = app.root.find_leaf(0).unwrap().id;
        let second = app.root.find_leaf(1).unwrap().id;
        let third = app.root.find_leaf(2).unwrap().id;
        assert_ne!(first, second);
        assert_ne!(second, third);
    }

    #[test]
    fn resize_terminal_relayouts_the_root() {
        let mut app = new_app(Size::new(80, 24));

        app.handle_action(Action::ResizeTerminal(60, 20));

        let pane = app.get_current_pane().unwrap();
        assert_eq!(pane.rect, Rect::new(0, 0, 58, 17));
    }

    #[test]
    fn switch_mode_exits_command_clear_the_buffer() {
        let mut app = new_app(Size::new(80, 24));
        assert_eq!(app.current_mode, Mode::Normal);

        app.handle_action(Action::SwitchMode(Mode::Command));
        assert_eq!(app.current_mode, Mode::Command);

        app.command_buffer = "ls -la".to_string();
        app.handle_action(Action::SwitchMode(Mode::Normal));
        assert_eq!(app.current_mode, Mode::Normal);
        assert!(app.command_buffer.is_empty());
    }

    #[test]
    fn quit_sets_should_quit() {
        let mut app = new_app(Size::new(80, 24));

        app.handle_action(Action::Quit);

        assert!(app.should_quit);
    }

    #[test]
    fn current_pane_getters_target_the_active_pane() {
        let mut app = new_app(Size::new(80, 24));

        app.get_current_pane_mut().unwrap().title = "hello".to_string();

        assert_eq!(app.get_current_pane().unwrap().title, "hello");
        assert_eq!(app.get_current_pane().unwrap().id, app.active_pane_id);
    }
}
