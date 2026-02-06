use std::io::{self, BufRead, Write};
use std::thread;
use std::time::Duration;
use xnote_core::plugin_protocol::PluginWireMessage;

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut seen_handshake = false;

    let delay_ms = std::env::var("XNOTE_PLUGIN_WORKER_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let activate_ok = std::env::var("XNOTE_PLUGIN_WORKER_ACTIVATE_OK")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(true);
    let protocol_override = std::env::var("XNOTE_PLUGIN_WORKER_PROTOCOL_VERSION")
        .ok()
        .and_then(|v| v.parse::<u32>().ok());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let message = match serde_json::from_str::<PluginWireMessage>(trimmed) {
            Ok(message) => message,
            Err(_) => break,
        };

        match message {
            PluginWireMessage::Handshake {
                protocol_version,
                supported_protocol_versions,
                capabilities,
                ..
            } => {
                seen_handshake = true;
                let selected_protocol_version = if supported_protocol_versions
                    .iter()
                    .any(|candidate| *candidate == protocol_version)
                {
                    protocol_version
                } else {
                    supported_protocol_versions
                        .iter()
                        .copied()
                        .max()
                        .unwrap_or(protocol_version)
                };
                let reported_capabilities = std::env::var("XNOTE_PLUGIN_WORKER_REPORTED_CAPS")
                    .ok()
                    .map(|raw| {
                        raw.split(',')
                            .map(|item| item.trim().to_string())
                            .filter(|item| !item.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or(capabilities);
                let ack = PluginWireMessage::HandshakeAck {
                    protocol_version: protocol_override.unwrap_or(selected_protocol_version),
                    accepted: true,
                    reason: None,
                    reported_capabilities,
                };
                if write_message(&mut stdout, &ack).is_err() {
                    break;
                }
            }
            PluginWireMessage::Activate {
                request_id,
                timeout_ms,
                ..
            } => {
                if !seen_handshake {
                    break;
                }

                if delay_ms > 0 {
                    let sleep_ms = delay_ms.min(timeout_ms.saturating_mul(3).max(1));
                    thread::sleep(Duration::from_millis(sleep_ms));
                }

                let response = if activate_ok {
                    PluginWireMessage::ActivateResult {
                        request_id,
                        ok: true,
                        error: None,
                    }
                } else {
                    PluginWireMessage::ActivateResult {
                        request_id,
                        ok: false,
                        error: Some("worker activation failed".to_string()),
                    }
                };

                if write_message(&mut stdout, &response).is_err() {
                    break;
                }
            }
            PluginWireMessage::Ping { request_id } => {
                if !seen_handshake {
                    break;
                }
                let response = PluginWireMessage::Pong { request_id };
                if write_message(&mut stdout, &response).is_err() {
                    break;
                }
            }
            PluginWireMessage::Cancel { .. } => {
                break;
            }
            PluginWireMessage::HandshakeAck { .. }
            | PluginWireMessage::ActivateResult { .. }
            | PluginWireMessage::Pong { .. } => {}
        }
    }
}

fn write_message(stdout: &mut impl Write, message: &PluginWireMessage) -> io::Result<()> {
    let payload = serde_json::to_string(message)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}
