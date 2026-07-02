use crate::pane::Pane;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

pub enum Action {
    SwitchMode(Mode),
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    ExecuteCommand(String),
    WriteToShell(String),
    Quit,
}

pub struct App {
    pub current_mode: Mode,
    pub panes: Vec<Pane>,
    pub active_pane_id: usize,
    pub command_buffer: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_mode: Mode::Normal,
            panes: vec![],
            active_pane_id: 0,
            command_buffer: String::new(),
            should_quit: false,
        }
    }

    /// This is the only entrypoint for state modification
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
            Action::Quit => self.should_quit = true,
            _ => unimplemented!(),
        }
    }
}
