use serde::{Deserialize, Serialize};

pub const PLUGIN_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginWireMessage {
    Handshake {
        protocol_version: u32,
        #[serde(default)]
        supported_protocol_versions: Vec<u32>,
        plugin_id: String,
        plugin_version: String,
        capabilities: Vec<String>,
    },
    HandshakeAck {
        protocol_version: u32,
        accepted: bool,
        reason: Option<String>,
        #[serde(default)]
        reported_capabilities: Vec<String>,
    },
    Activate {
        request_id: String,
        event: String,
        timeout_ms: u64,
    },
    Ping {
        request_id: String,
    },
    Pong {
        request_id: String,
    },
    ActivateResult {
        request_id: String,
        ok: bool,
        error: Option<String>,
    },
    Cancel {
        request_id: String,
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_ack_roundtrip_with_reported_capabilities() {
        let message = PluginWireMessage::HandshakeAck {
            protocol_version: PLUGIN_PROTOCOL_VERSION,
            accepted: true,
            reason: None,
            reported_capabilities: vec!["read_vault".to_string(), "commands".to_string()],
        };

        let json = serde_json::to_string(&message).expect("serialize handshake ack");
        let parsed: PluginWireMessage =
            serde_json::from_str(&json).expect("deserialize handshake ack");

        assert_eq!(parsed, message);
    }

    #[test]
    fn handshake_roundtrip_with_supported_protocol_versions() {
        let message = PluginWireMessage::Handshake {
            protocol_version: 2,
            supported_protocol_versions: vec![3, 2, 1],
            plugin_id: "x.plugin.demo".to_string(),
            plugin_version: "0.1.0".to_string(),
            capabilities: vec!["read_vault".to_string()],
        };

        let json = serde_json::to_string(&message).expect("serialize handshake");
        let parsed: PluginWireMessage = serde_json::from_str(&json).expect("deserialize handshake");

        assert_eq!(parsed, message);
    }

    #[test]
    fn handshake_backwards_compatible_without_supported_versions() {
        let json = r#"{"kind":"handshake","protocol_version":1,"plugin_id":"x","plugin_version":"0.1.0","capabilities":["read_vault"]}"#;
        let parsed: PluginWireMessage = serde_json::from_str(json).expect("deserialize handshake");

        match parsed {
            PluginWireMessage::Handshake {
                protocol_version,
                supported_protocol_versions,
                plugin_id,
                plugin_version,
                capabilities,
            } => {
                assert_eq!(protocol_version, 1);
                assert!(supported_protocol_versions.is_empty());
                assert_eq!(plugin_id, "x");
                assert_eq!(plugin_version, "0.1.0");
                assert_eq!(capabilities, vec!["read_vault".to_string()]);
            }
            _ => panic!("unexpected message kind"),
        }
    }

    #[test]
    fn handshake_ack_backwards_compatible_without_reported_capabilities() {
        let json = r#"{"kind":"handshake_ack","protocol_version":1,"accepted":true,"reason":null}"#;
        let parsed: PluginWireMessage =
            serde_json::from_str(json).expect("deserialize legacy handshake ack");

        match parsed {
            PluginWireMessage::HandshakeAck {
                protocol_version,
                accepted,
                reason,
                reported_capabilities,
            } => {
                assert_eq!(protocol_version, 1);
                assert!(accepted);
                assert!(reason.is_none());
                assert!(reported_capabilities.is_empty());
            }
            _ => panic!("unexpected message kind"),
        }
    }

    #[test]
    fn ping_pong_roundtrip() {
        let ping = PluginWireMessage::Ping {
            request_id: "ping-1".to_string(),
        };
        let pong = PluginWireMessage::Pong {
            request_id: "ping-1".to_string(),
        };

        let ping_json = serde_json::to_string(&ping).expect("serialize ping");
        let pong_json = serde_json::to_string(&pong).expect("serialize pong");

        let parsed_ping: PluginWireMessage = serde_json::from_str(&ping_json).expect("parse ping");
        let parsed_pong: PluginWireMessage = serde_json::from_str(&pong_json).expect("parse pong");

        assert_eq!(parsed_ping, ping);
        assert_eq!(parsed_pong, pong);
    }
}
