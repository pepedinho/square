use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, Mode};

/// Translates a single [`KeyEvent`] into an [`Action`] based on `current_mode`.
///
/// `Ctrl+N` is intercepted globally and toggles between Insert and the other modes.
/// Everything else is delegated to the mode-specific handler.
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

/// Normal-mode key bindings:
///
/// | Key | Action |
/// |-----|--------|
/// | `:` | Enter Command mode |
/// | `i` | Enter Insert mode |
/// | `q` | Quit |
/// | `v` | Split vertically (not yet implemented) |
/// | `h` | Split horizontally (not yet implemented) |
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

/// Insert-mode key handler.
///
/// `Ctrl+<letter>` sends the corresponding control byte (`\x01`–`\x1a`).
/// All other keys are converted to the byte sequence a real terminal would
/// receive (e.g. arrow keys become VT100 escape sequences).
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

/// Command-mode key handler.
///
/// `Esc` cancels and returns to Normal mode.
/// `Enter` currently just returns to Normal mode — the command buffer is
/// **not** dispatched yet (see the `TODO` in the body).
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn ctrl_n_toggles_between_insert_and_the_other_modes() {
        let ctrl_n = ctrl(KeyCode::Char('n'));

        assert_eq!(
            handle_key(ctrl_n, Mode::Insert),
            Some(Action::SwitchMode(Mode::Normal))
        );
        assert_eq!(
            handle_key(ctrl_n, Mode::Normal),
            Some(Action::SwitchMode(Mode::Insert))
        );
        assert_eq!(
            handle_key(ctrl_n, Mode::Command),
            Some(Action::SwitchMode(Mode::Insert))
        );
    }

    #[test]
    fn normal_mode_bindings() {
        let cases = [
            (key(KeyCode::Char(':')), Action::SwitchMode(Mode::Command)),
            (key(KeyCode::Char('i')), Action::SwitchMode(Mode::Insert)),
            (key(KeyCode::Char('q')), Action::Quit),
            (key(KeyCode::Char('v')), Action::SplitVertical),
            (key(KeyCode::Char('h')), Action::SplitHorizontal),
        ];

        for (key_event, expected) in cases {
            assert_eq!(handle_key(key_event, Mode::Normal), Some(expected));
        }

        assert_eq!(handle_key(key(KeyCode::Char('x')), Mode::Normal), None);
    }

    #[test]
    fn insert_mode_maps_printables_and_special_keys() {
        assert_eq!(
            handle_key(key(KeyCode::Char('a')), Mode::Insert),
            Some(Action::WriteToShell("a".to_string()))
        );
        assert_eq!(
            handle_key(key(KeyCode::Enter), Mode::Insert),
            Some(Action::WriteToShell("\r".to_string()))
        );
        assert_eq!(
            handle_key(key(KeyCode::Backspace), Mode::Insert),
            Some(Action::WriteToShell("\u{7f}".to_string()))
        );
        assert_eq!(
            handle_key(key(KeyCode::Tab), Mode::Insert),
            Some(Action::WriteToShell("\t".to_string()))
        );
        assert_eq!(
            handle_key(key(KeyCode::Esc), Mode::Insert),
            Some(Action::WriteToShell("\x1b".to_string()))
        );
    }

    #[test]
    fn insert_mode_maps_arrow_keys_to_vt100_sequences() {
        let cases = [
            (KeyCode::Up, "\x1b[A"),
            (KeyCode::Down, "\x1b[B"),
            (KeyCode::Right, "\x1b[C"),
            (KeyCode::Left, "\x1b[D"),
        ];

        for (code, expected) in cases {
            assert_eq!(
                handle_key(key(code), Mode::Insert),
                Some(Action::WriteToShell(expected.to_string()))
            );
        }
    }

    #[test]
    fn insert_mode_maps_control_letters_to_control_bytes() {
        assert_eq!(
            handle_key(ctrl(KeyCode::Char('a')), Mode::Insert),
            Some(Action::WriteToShell("\x01".to_string()))
        );
        assert_eq!(
            handle_key(ctrl(KeyCode::Char('z')), Mode::Insert),
            Some(Action::WriteToShell("\x1a".to_string()))
        );
        assert_eq!(
            handle_key(ctrl(KeyCode::Char('A')), Mode::Insert),
            Some(Action::WriteToShell("\x01".to_string()))
        );
    }

    #[test]
    fn ctrl_n_takes_precedence_over_control_byte_mapping() {
        let ctrl_n = ctrl(KeyCode::Char('n'));
        assert_eq!(
            handle_key(ctrl_n, Mode::Insert),
            Some(Action::SwitchMode(Mode::Normal))
        );
    }

    #[test]
    fn ctrl_non_letters_fall_through() {
        assert_eq!(
            handle_key(key(KeyCode::Char('n')), Mode::Insert),
            Some(Action::WriteToShell("n".to_string()))
        );
    }

    #[test]
    fn command_mode_bindings() {
        assert_eq!(
            handle_key(key(KeyCode::Esc), Mode::Command),
            Some(Action::SwitchMode(Mode::Normal))
        );
        assert_eq!(
            handle_key(key(KeyCode::Enter), Mode::Command),
            Some(Action::SwitchMode(Mode::Normal))
        );
        assert_eq!(handle_key(key(KeyCode::Char('l')), Mode::Command), None);
    }
}
