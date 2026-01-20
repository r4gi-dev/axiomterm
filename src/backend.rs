use crate::types::{ShellEvent, TerminalColor, Line};
use crossbeam_channel::Sender;
use std::sync::{Arc, Mutex};
use crate::types::ShellState;

pub trait ProcessHandle: Send + Sync {
    fn wait(&mut self) -> std::io::Result<()>;
    fn kill(&mut self) -> std::io::Result<()>;
}

pub trait ProcessBackend: Send + Sync {
    fn spawn(
        &self,
        command: &str,
        args: &[String],
        output_tx: Sender<ShellEvent>,
        thread_state: Arc<Mutex<ShellState>>,
    ) -> std::io::Result<Box<dyn ProcessHandle>>;
}

pub struct StdProcessHandle {
    pub child: std::process::Child,
}

impl ProcessHandle for StdProcessHandle {
    fn wait(&mut self) -> std::io::Result<()> {
        let _ = self.child.wait()?;
        Ok(())
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }
}

pub struct StdBackend;

impl ProcessBackend for StdBackend {
    fn spawn(
        &self,
        command: &str,
        args: &[String],
        output_tx: Sender<ShellEvent>,
        thread_state: Arc<Mutex<ShellState>>,
    ) -> std::io::Result<Box<dyn ProcessHandle>> {
        use std::process::{Command, Stdio};
        use std::io::{BufRead, BufReader};
        use std::thread;

        let mut child = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(stdout) = child.stdout.take() {
            let state_clone = Arc::clone(&thread_state);
            let tx_clone = output_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        let mut s = state_clone.lock().unwrap();
                        let text_color = s.text_color;
                        let op = s.screen.push_line(Line::from_string(&l, text_color));
                        let _ = tx_clone.send(ShellEvent::Operation(op));
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let state_clone = Arc::clone(&thread_state);
            let tx_clone = output_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        let mut s = state_clone.lock().unwrap();
                        let op = s.screen.push_line(Line::from_string(&l, TerminalColor::RED));
                        let _ = tx_clone.send(ShellEvent::Operation(op));
                    }
                }
            });
        }

        Ok(Box::new(StdProcessHandle { child }))
    }
}
