use std::io::Write;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tokio::sync::mpsc::UnboundedSender;

pub struct Pane {
    pub id: usize,
    pub title: String,
    pub buffer: Vec<String>,
    pub pty_writer: Box<dyn Write + Send>,
    pub is_focused: bool,
}

impl Pane {
    pub fn new(id: usize, tx: UnboundedSender<(usize, String)>) -> Self {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let cmd = CommandBuilder::new(shell);
        let _child = pair.slave.spawn_command(cmd).unwrap();

        let mut pty_reader = pair.master.try_clone_reader().unwrap();
        let pty_writer = pair.master.take_writer().unwrap();

        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];

            while let Ok(n) = pty_reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send((id, text));
            }
        });

        Self {
            id,
            title: String::new(),
            is_focused: false,
            buffer: vec![String::new()],
            pty_writer,
        }
    }

    pub fn write_to_shell(&mut self, text: &str) {
        let _ = self.pty_writer.write_all(text.as_bytes());
        let _ = self.pty_writer.flush();
    }

    pub fn append_text(&mut self, text: &str) {
        let mut in_ansi_sequence = false;

        for c in text.chars() {
            if c == '\x1b' {
                in_ansi_sequence = true;
                continue;
            }
            if in_ansi_sequence {
                if c.is_ascii_alphabetic() {
                    in_ansi_sequence = false;
                }
                continue;
            }

            if c == '\n' {
                self.buffer.push(String::new());
            } else if c == '\r' {
            } else if let Some(line) = self.buffer.last_mut() {
                line.push(c);
            }
        }

        if self.buffer.len() > 500 {
            self.buffer.remove(0);
        }
    }
}
