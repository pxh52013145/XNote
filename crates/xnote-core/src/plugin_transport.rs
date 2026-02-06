use crate::plugin_protocol::PluginWireMessage;
use std::io::{BufRead, BufReader, Write};
use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

pub trait PluginTransport {
    fn send(&mut self, message: &PluginWireMessage) -> Result<(), String>;
    fn receive(&mut self, timeout: Duration) -> Result<Option<PluginWireMessage>, String>;
    fn terminate(&mut self);
}

pub struct StdioProcessTransport {
    child: Child,
    rx: Receiver<Result<PluginWireMessage, String>>,
}

impl StdioProcessTransport {
    pub fn from_child(mut child: Child) -> Result<Self, String> {
        if child.stdin.is_none() {
            kill_and_wait(&mut child);
            return Err("runtime stdin is not piped".to_string());
        }

        let Some(stdout) = child.stdout.take() else {
            kill_and_wait(&mut child);
            return Err("runtime stdout is not piped".to_string());
        };

        let rx = spawn_reader_thread(stdout);
        Ok(Self { child, rx })
    }
}

impl PluginTransport for StdioProcessTransport {
    fn send(&mut self, message: &PluginWireMessage) -> Result<(), String> {
        let payload = serde_json::to_string(message)
            .map_err(|err| format!("serialize runtime message failed: {err}"))?;

        let Some(stdin) = self.child.stdin.as_mut() else {
            return Err("runtime stdin is unavailable".to_string());
        };

        stdin
            .write_all(payload.as_bytes())
            .map_err(|err| format!("write runtime stdin failed: {err}"))?;
        stdin
            .write_all(b"\n")
            .map_err(|err| format!("write runtime newline failed: {err}"))?;
        stdin
            .flush()
            .map_err(|err| format!("flush runtime stdin failed: {err}"))
    }

    fn receive(&mut self, timeout: Duration) -> Result<Option<PluginWireMessage>, String> {
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(message)) => Ok(Some(message)),
            Ok(Err(err)) => Err(err),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err("runtime stdout channel disconnected".to_string())
            }
        }
    }

    fn terminate(&mut self) {
        kill_and_wait(&mut self.child);
    }
}

fn spawn_reader_thread(
    stdout: impl std::io::Read + Send + 'static,
) -> Receiver<Result<PluginWireMessage, String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    let _ = tx.send(Err(format!("read runtime stdout failed: {err}")));
                    return;
                }
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parsed = serde_json::from_str::<PluginWireMessage>(trimmed)
                .map_err(|err| format!("invalid runtime message: {err}"));
            let _ = tx.send(parsed);
        }
    });
    rx
}

fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct MemoryTransport {
        sent: Vec<PluginWireMessage>,
        recv: VecDeque<PluginWireMessage>,
        terminated: bool,
    }

    impl MemoryTransport {
        fn new(messages: Vec<PluginWireMessage>) -> Self {
            Self {
                sent: Vec::new(),
                recv: VecDeque::from(messages),
                terminated: false,
            }
        }
    }

    impl PluginTransport for MemoryTransport {
        fn send(&mut self, message: &PluginWireMessage) -> Result<(), String> {
            self.sent.push(message.clone());
            Ok(())
        }

        fn receive(&mut self, _timeout: Duration) -> Result<Option<PluginWireMessage>, String> {
            Ok(self.recv.pop_front())
        }

        fn terminate(&mut self) {
            self.terminated = true;
        }
    }

    #[test]
    fn memory_transport_send_receive_and_terminate() {
        let mut transport = MemoryTransport::new(vec![PluginWireMessage::ActivateResult {
            request_id: "req-1".to_string(),
            ok: true,
            error: None,
        }]);

        transport
            .send(&PluginWireMessage::Activate {
                request_id: "req-1".to_string(),
                event: "on_startup_finished".to_string(),
                timeout_ms: 10,
            })
            .expect("send message");

        let received = transport
            .receive(Duration::from_millis(1))
            .expect("receive message");
        assert!(matches!(
            received,
            Some(PluginWireMessage::ActivateResult { ok: true, .. })
        ));

        transport.terminate();
        assert!(transport.terminated);
        assert_eq!(transport.sent.len(), 1);
    }
}
