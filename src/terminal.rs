use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

/// Answers the terminal-capability queries real programs send before they'll
/// trust a terminal - device attributes, cursor position, the kitty keyboard
/// protocol, XTVERSION, the OSC 11 background-color query. Without these,
/// anything that probes the terminal and blocks waiting for a reply hangs
/// completely: no prompt ever appears and no keystroke does anything, since
/// the shell's own startup never gets past the probe. This is exactly what
/// fish does on startup (kitty keyboard flags, XTVERSION, OSC 11, then DA1),
/// and what Powerlevel10k's first-run capability detection does too.
/// `vt100`'s own parser has no built-in responder for any of these (its CSI
/// dispatch has no case for `c`/`n`/`u`/`q`, and OSC 11 isn't one of the
/// handful of OSCs it recognizes, so they all fall through to
/// `unhandled_csi`/`unhandled_osc`), so we supply the responses and forward
/// them back into the pty ourselves.
#[derive(Default)]
struct QueryResponder {
    pending: Vec<u8>,
}

impl vt100::Callbacks for QueryResponder {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        intermediate: Option<u8>,
        _intermediate2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        match (intermediate, c) {
            // Primary Device Attributes (`ESC [ c`): claim to be a basic
            // VT102-class terminal.
            (None, 'c') => self.pending.extend_from_slice(b"\x1b[?6c"),
            // Secondary Device Attributes (`ESC [ > c`).
            (Some(b'>'), 'c') => self.pending.extend_from_slice(b"\x1b[>0;0;0c"),
            // Device Status Report (`ESC [ n`): 5 = "are you OK", 6 = cursor
            // position report.
            (None, 'n') => {
                let code = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
                if code == 5 {
                    self.pending.extend_from_slice(b"\x1b[0n");
                } else if code == 6 {
                    let (row, col) = screen.cursor_position();
                    self.pending
                        .extend(format!("\x1b[{};{}R", row + 1, col + 1).into_bytes());
                }
            }
            // Kitty keyboard protocol "report current flags" query
            // (`ESC [ ? u`): report no progressive-enhancement flags set.
            (Some(b'?'), 'u') => self.pending.extend_from_slice(b"\x1b[?0u"),
            // XTVERSION (`ESC [ > 0 q`): report a terminal name/version.
            (Some(b'>'), 'q') => self.pending.extend_from_slice(b"\x1bP>|DonutEx(0.1.0)\x1b\\"),
            _ => {}
        }
    }

    fn unhandled_osc(&mut self, _screen: &mut vt100::Screen, params: &[&[u8]]) {
        // OSC 11 `;` `?`: "what's your background color?"
        if params.first() == Some(&b"11".as_slice()) && params.get(1) == Some(&b"?".as_slice()) {
            self.pending
                .extend_from_slice(b"\x1b]11;rgb:1e1e/1e1e/1e1e\x1b\\");
        }
    }
}

/// `vt100` has no hook at all for DCS sequences (its `Callbacks` trait only
/// covers CSI/OSC), so XTGETTCAP capability queries (`ESC P + q <hex-name>
/// ST`) - which fish also sends at startup - can't be answered through it.
/// This scans raw PTY output directly for that one pattern and replies
/// "capability not supported" for whatever was asked, echoing the query
/// verbatim in the response as the protocol expects.
fn xtgettcap_replies(buf: &[u8]) -> Vec<u8> {
    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    let mut replies = Vec::new();
    let mut pos = 0;
    while let Some(rel_start) = find(&buf[pos..], b"\x1bP+q") {
        let payload_start = pos + rel_start + 4;
        let Some(rel_end) = find(&buf[payload_start..], b"\x1b\\") else {
            break;
        };
        let payload_end = payload_start + rel_end;
        replies.extend_from_slice(b"\x1bP0+r");
        replies.extend_from_slice(&buf[payload_start..payload_end]);
        replies.extend_from_slice(b"\x1b\\");
        pos = payload_end + 2;
    }
    replies
}

/// A shell process attached to a PTY, plus the parser that turns its raw
/// output into an in-memory screen grid. Mirrors `Document` in spirit: this
/// is the data/process side, while `terminal_view` handles rendering and
/// input.
pub struct Terminal {
    parser: vt100::Parser<QueryResponder>,
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
            parser: vt100::Parser::new_with_callbacks(rows, cols, 5000, QueryResponder::default()),
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
            let dcs_replies = xtgettcap_replies(&bytes);
            if !dcs_replies.is_empty() {
                let _ = self.writer.write_all(&dcs_replies);
                let _ = self.writer.flush();
            }

            self.parser.process(&bytes);
            if !self.parser.callbacks_mut().pending.is_empty() {
                let reply = std::mem::take(&mut self.parser.callbacks_mut().pending);
                let _ = self.writer.write_all(&reply);
                let _ = self.writer.flush();
            }
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

    /// Regression test for the real bug: without a Device Attributes /
    /// Cursor Position Report responder, anything that queries the terminal
    /// and blocks on a reply - notably Powerlevel10k's terminal-capability
    /// probe - hangs, and the pane never shows a usable prompt. Exercises
    /// `QueryResponder` directly against the escape sequences a real query
    /// would send, rather than through a live shell (whose line-buffering
    /// makes reading a reply with no trailing newline unreliable to script).
    #[test]
    fn answers_device_attribute_and_cursor_position_queries() {
        let mut parser = vt100::Parser::new_with_callbacks(24, 80, 0, QueryResponder::default());

        parser.process(b"\x1b[c");
        assert_eq!(
            std::mem::take(&mut parser.callbacks_mut().pending),
            b"\x1b[?6c",
            "expected a Primary Device Attributes reply"
        );

        parser.process(b"\x1b[6n");
        assert_eq!(
            std::mem::take(&mut parser.callbacks_mut().pending),
            format!("\x1b[{};{}R", 1, 1).into_bytes(),
            "expected a Cursor Position Report reply for the cursor's starting position"
        );

        // Move the cursor and confirm the report reflects the new position.
        parser.process(b"\x1b[5;10H\x1b[6n");
        assert_eq!(
            std::mem::take(&mut parser.callbacks_mut().pending),
            b"\x1b[5;10R".to_vec()
        );
    }
}

/// Not run by `cargo test` (needs a real `fish` install and depends on
/// shared process env, so it's `#[ignore]`d) - a manual live check for the
/// original bug report: fish's startup capability probe (kitty keyboard
/// protocol, XTVERSION, OSC 11, DA1) left it hung forever before this fix,
/// with no prompt and no reaction to typing. Run explicitly with
/// `cargo test --ignored default_shell_reaches_a_prompt -- --nocapture`.
#[cfg(test)]
mod manual_check {
    use super::*;
    use std::time::Duration;

    #[test]
    #[ignore]
    fn default_shell_reaches_a_prompt() {
        let previous_shell = std::env::var("SHELL").ok();
        std::env::set_var("SHELL", "/usr/bin/fish");
        let mut term = Terminal::spawn(24, 80, std::path::Path::new("/tmp")).expect("spawn");
        match previous_shell {
            Some(s) => std::env::set_var("SHELL", s),
            None => std::env::remove_var("SHELL"),
        }

        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(200));
            term.pump();
        }
        term.write_input(b"echo REACHED_PROMPT_2\n");
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(200));
            term.pump();
        }
        println!("=== screen contents ===\n{}\n=== end ===", term.screen().contents());
        assert!(term.screen().contents().contains("REACHED_PROMPT_2"));
    }
}
