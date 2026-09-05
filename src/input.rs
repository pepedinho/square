use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, Mode};

pub fn handle_key(key: KeyEvent, current_mode: Mode) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
        return match current_mode {
            Mode::Insert => Some(Action::SwitchMode(Mode::Normal)),
            Mode::Normal | Mode::Command => Some(Action::SwitchMode(Mode::Insert)),
        };
    }

    match current_mode {
        Mode::Normal => handle_normal_mode(key),
        Mode::Insert => handle_inser_mode(key),
        Mode::Command => handle_command_mode(key),
    }
}

fn handle_normal_mode(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char(':') => Some(Action::SwitchMode(Mode::Command)),
        KeyCode::Char('i') => Some(Action::SwitchMode(Mode::Insert)),
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('v') => Some(Action::SplitVertical),
        KeyCode::Char('h') => Some(Action::SplitHorizontal),
        _ => None,
    }
}

fn handle_inser_mode(key: KeyEvent) -> Option<Action> {
    #[allow(clippy::collapsible_if)]
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_alphabetic() {
                let byte = c.to_ascii_lowercase() as u8 - b'a' + 1;
                return Some(Action::WriteToShell(String::from_utf8(vec![byte]).unwrap()));
            }
        }
    }
    match key.code {
        KeyCode::Char(c) => Some(Action::WriteToShell(c.to_string())),
        KeyCode::Enter => Some(Action::WriteToShell("\r".to_string())),
        KeyCode::Backspace => Some(Action::WriteToShell("\u{7f}".to_string())),
        KeyCode::Esc => Some(Action::WriteToShell("\x1b".to_string())),
        KeyCode::Tab => Some(Action::WriteToShell("\t".to_string())),

        KeyCode::Up => Some(Action::WriteToShell("\x1b[A".to_string())),
        KeyCode::Down => Some(Action::WriteToShell("\x1b[B".to_string())),
        KeyCode::Right => Some(Action::WriteToShell("\x1b[C".to_string())),
        KeyCode::Left => Some(Action::WriteToShell("\x1b[D".to_string())),
        _ => None,
    }
}

fn handle_command_mode(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::SwitchMode(Mode::Normal)),
        KeyCode::Enter => {
            //TODO: send accumulated buffer to ExecuteCommand action
            Some(Action::SwitchMode(Mode::Normal))
        }
        // Command input will be handled in loop
        _ => None,
    }
}
