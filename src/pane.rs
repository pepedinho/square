use std::io::Write;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tokio::sync::mpsc::UnboundedSender;
use vt100::Parser;

pub struct Pane {
    pub id: usize,
    pub title: String,
    pub parser: Parser,
    pub pty_writer: Box<dyn Write + Send>,
    pub is_focused: bool,
}

impl Pane {
    pub fn new(id: usize, tx: UnboundedSender<(usize, Vec<u8>)>) -> Self {
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
                let _ = tx.send((id, buf[..n].to_vec()));
            }
        });

        Self {
            id,
            title: String::new(),
            is_focused: false,
            parser: Parser::new(24, 80, 0),
            pty_writer,
        }
    }

    pub fn write_to_shell(&mut self, text: &str) {
        let _ = self.pty_writer.write_all(text.as_bytes());
        let _ = self.pty_writer.flush();
    }

    pub fn process_output(&mut self, data: &[u8]) {
        self.parser.process(data);
    }
}
