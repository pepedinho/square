use std::io::Write;

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use ratatui::layout::{Rect, Size};
use tokio::sync::mpsc::UnboundedSender;
use vt100::Parser;

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
    /// Corresonding ratatui [`Rect`]
    pub rect: Rect,
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
            rect: Rect::new(0, 0, cols, rows),
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

        self.size = Size::new(cols, rows);
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.resize(rect.height, rect.width);
    }

    pub fn split(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    /// A [`Write`] sink that records everything written to it.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Build a real pane (spawns a shell + reader thread) sized `rows`×`cols`.
    fn pane(id: usize, rows: u16, cols: u16) -> Pane {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        Pane::new(id, tx, rows, cols)
    }

    #[test]
    fn new_sets_id_and_size() {
        let pane = pane(7, 10, 20);
        assert_eq!(pane.id, 7);
        assert_eq!(pane.rect, Rect::new(0, 0, 20, 10));
        assert_eq!(pane.size, Size::new(20, 10));
        assert_eq!(pane.parser.screen().size(), (10, 20));
    }

    #[test]
    fn process_output_fills_cells_and_moves_cursor() {
        let mut pane = pane(0, 5, 20);
        pane.process_output(b"hey");

        let screen = pane.parser.screen();
        assert_eq!(screen.size(), (5, 20));
        assert_eq!(screen.cell(0, 0).unwrap().contents(), "h");
        assert_eq!(screen.cell(0, 1).unwrap().contents(), "e");
        assert_eq!(screen.cell(0, 2).unwrap().contents(), "y");
        assert_eq!(screen.cursor_position(), (0, 3));
    }

    #[test]
    fn process_output_scrolls_when_filling_the_screen() {
        let mut pane = pane(0, 3, 10);
        pane.process_output(b"1\n2\n3\n4\n");

        let screen = pane.parser.screen();
        // Line feeds move the cursor down without resetting the column, so
        // the digits end up staggered; once the buffer is full it scrolls.
        assert_eq!(screen.cell(0, 2).unwrap().contents(), "3");
        assert_eq!(screen.cell(1, 3).unwrap().contents(), "4");
        assert_eq!(screen.cell(0, 0).unwrap().contents(), "");
        assert_eq!(screen.cursor_position(), (2, 4));
    }

    #[test]
    fn resize_updates_parser_screen_and_size() {
        let mut pane = pane(0, 24, 80);
        pane.resize(10, 40);

        assert_eq!(pane.parser.screen().size(), (10, 40));
        assert_eq!(pane.size, Size::new(40, 10));
    }

    #[test]
    fn set_rect_syncs_rect_and_parser_size() {
        let mut pane = pane(0, 24, 80);
        pane.set_rect(Rect::new(3, 4, 15, 6));

        assert_eq!(pane.rect, Rect::new(3, 4, 15, 6));
        assert_eq!(pane.parser.screen().size(), (6, 15));
    }

    #[test]
    fn write_to_shell_forwards_bytes_to_pty_writer() {
        let mut pane = pane(0, 5, 10);
        let capture = Capture::default();
        pane.pty_writer = Box::new(capture.clone());

        pane.write_to_shell("abc");
        assert_eq!(*capture.0.lock().unwrap(), b"abc");
    }
}
