use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

/// A shell process attached to a PTY, plus the parser that turns its raw
/// output into an in-memory screen grid. Mirrors `Document` in spirit: this
/// is the data/process side, while `terminal_view` handles rendering and
/// input.
pub struct Terminal {
    parser: vt100::Parser,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Output bytes read off the PTY on a background thread (`read` on it
    /// blocks, so it can't happen on the UI thread) and drained in `pump`.
    rx: mpsc::Receiver<Vec<u8>>,
    rows: u16,
    cols: u16,
    alive: bool,
    pid: Option<u32>,
}

impl Terminal {
    pub fn spawn(rows: u16, cols: u16, cwd: &Path) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new_default_prog();
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd)?;
        // Dropping the slave in the parent process is required on unix for
        // the child to see EOF/HUP when it's the last holder of that end.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let pid = child.process_id();

        Ok(Self {
            parser: vt100::Parser::new(rows, cols, 5000),
            writer,
            master: pair.master,
            child,
            rx,
            rows,
            cols,
            alive: true,
            pid,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Drains any output the shell has produced since the last frame and
    /// checks whether the child process is still running. Cheap to call
    /// every frame - it never blocks. Returns a message the first time the
    /// child is observed to have exited, so the caller can surface it (a
    /// shell that dies moments after spawning otherwise looks identical to
    /// one that's just quietly waiting at a prompt - a blank, unresponsive
    /// pane either way).
    pub fn pump(&mut self) -> Option<String> {
        while let Ok(bytes) = self.rx.try_recv() {
            self.parser.process(&bytes);
        }
        if self.alive {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.alive = false;
                return Some(format!("terminal: shell exited ({})", status));
            }
        }
        None
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn write_input(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols || rows == 0 || cols == 0 {
            return;
        }
        self.rows = rows;
        self.cols = cols;
        self.parser.screen_mut().set_size(rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn shell_echo_round_trip() {
        let mut term = Terminal::spawn(24, 80, &std::env::temp_dir()).expect("spawn shell");
        term.write_input(b"echo hello_from_pty\n");

        let mut found = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            term.pump();
            if term.screen().contents().contains("hello_from_pty") {
                found = true;
                break;
            }
        }
        assert!(found, "expected pty output to contain the echoed text");
    }
}
