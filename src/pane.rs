use std::io::Write;

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use ratatui::layout::Size;
use tokio::sync::mpsc::UnboundedSender;
use vt100::Parser;

pub enum Direction {
    Vertical,
    Horizontal,
}

/// A single terminal pane.
///
/// Wraps a `portable-pty` master/slave pair and a [`Parser`] that keeps the
/// virtual screen state.  A dedicated OS thread reads PTY output and forwards
/// it to the async event loop through an `mpsc` channel.
pub struct Pane {
    /// Unique identifier, used to route PTY output back to the correct pane.
    pub id: usize,
    /// Display title (currently unused / empty).
    pub title: String,
    /// VT100 parser that maintains the screen buffer for this pane.
    pub parser: Parser,
    /// Write handle to the PTY's master side — keystrokes go here.
    pub pty_writer: Box<dyn Write + Send>,
    /// Master PTY handle, kept alive for resize operations.
    pub master_pty: Box<dyn MasterPty + Send>,
    /// Whether this pane currently has focus (not yet wired up for multi-pane).
    pub is_focused: bool,
    /// Size of pane.
    pub size: Size,
}

impl Pane {
    /// Spawn a new shell inside a fresh PTY and start a background reader thread.
    ///
    /// The reader thread reads up to 1 024 bytes at a time and sends each chunk
    /// as `(id, Vec<u8>)` over `tx` so the async loop can feed it to [`Parser`].
    ///
    /// The shell is determined by `$SHELL`, falling back to `/bin/sh`.
    pub fn new(id: usize, tx: UnboundedSender<(usize, Vec<u8>)>, rows: u16, cols: u16) -> Self {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
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
            parser: Parser::new(rows, cols, 0),
            pty_writer,
            master_pty: pair.master,
            size: Size::new(cols, rows),
        }
    }

    /// Write `text` to the shell's stdin via the PTY writer.
    pub fn write_to_shell(&mut self, text: &str) {
        let _ = self.pty_writer.write_all(text.as_bytes());
        let _ = self.pty_writer.flush();
    }

    /// Feed raw PTY output into the VT100 parser to update the screen buffer.
    pub fn process_output(&mut self, data: &[u8]) {
        self.parser.process(data);
    }

    /// Resize both the VT100 parser's screen and the underlying PTY.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);

        let _ = self.master_pty.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn split(&mut self) {}
}
