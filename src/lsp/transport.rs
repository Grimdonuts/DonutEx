use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;

/// A single language-server process, framed as JSON-RPC over stdio
/// (`Content-Length: N\r\n\r\n<N bytes of JSON>`). Reading happens on a
/// background thread that forwards whole decoded messages back over a
/// channel; writing happens synchronously from whichever thread calls
/// `send`, matching the mpsc-channel pattern already used for plugins so
/// nothing here needs an async runtime.
pub struct LspTransport {
    child: Child,
    stdin: ChildStdin,
    pub rx: mpsc::Receiver<Value>,
}

impl LspTransport {
    pub fn spawn(cmd: &std::path::Path, args: &[&str]) -> std::io::Result<Self> {
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || read_loop(stdout, tx));
        Ok(Self { child, stdin, rx })
    }

    pub fn send(&mut self, value: &Value) -> std::io::Result<()> {
        let body = serde_json::to_vec(value)?;
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len())?;
        self.stdin.write_all(&body)?;
        self.stdin.flush()
    }
}

impl Drop for LspTransport {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_loop(stdout: impl Read, tx: mpsc::Sender<Value>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return, // EOF: server exited
                Ok(_) => {}
                Err(_) => return,
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break; // blank line ends the header block
            }
            if let Some(v) = trimmed.strip_prefix("Content-Length:") {
                content_length = v.trim().parse::<usize>().ok();
            }
        }
        let Some(len) = content_length else { continue };
        let mut buf = vec![0u8; len];
        if reader.read_exact(&mut buf).is_err() {
            return;
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&buf) {
            if tx.send(value).is_err() {
                return;
            }
        }
    }
}
