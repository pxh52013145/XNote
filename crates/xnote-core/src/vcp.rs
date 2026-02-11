use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_VCP_CHAT_ENDPOINT: &str = "http://127.0.0.1:5890/v1/chat/completions";
pub const DEFAULT_VCP_ADMIN_ENDPOINT: &str = "http://127.0.0.1:6005";
pub const DEFAULT_VCP_WS_ENDPOINT: &str = "ws://127.0.0.1:6005";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcpRuntimeConfig {
    pub chat_endpoint: String,
    pub api_key: Option<String>,
    pub admin_endpoint: String,
    pub admin_auth_header: Option<String>,
    pub timeout_ms: u64,
}

impl Default for VcpRuntimeConfig {
    fn default() -> Self {
        Self {
            chat_endpoint: DEFAULT_VCP_CHAT_ENDPOINT.to_string(),
            api_key: None,
            admin_endpoint: DEFAULT_VCP_ADMIN_ENDPOINT.to_string(),
            admin_auth_header: None,
            timeout_ms: 2_000,
        }
    }
}

impl VcpRuntimeConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.max(200))
    }

    pub fn normalized_chat_endpoint(&self) -> String {
        normalize_vcp_chat_endpoint(self.chat_endpoint.as_str())
    }

    pub fn normalized_admin_endpoint(&self) -> String {
        normalize_vcp_admin_endpoint(self.admin_endpoint.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VcpHealthCategory {
    Connected,
    Unauthorized,
    ApiPathNotFound,
    Timeout,
    InvalidEndpoint,
    NetworkError,
    ServerError,
    ClientError,
    UnknownError,
}

impl VcpHealthCategory {
    pub fn is_connected(self) -> bool {
        matches!(self, Self::Connected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcpEndpointProbe {
    pub category: VcpHealthCategory,
    pub status_code: Option<u16>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcpProbeReport {
    pub chat: VcpEndpointProbe,
    pub admin: VcpEndpointProbe,
    pub models: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcpPluginSummary {
    pub name: String,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcpMetricEntry {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcpAdminSnapshot {
    pub generated_at_epoch_ms: u64,
    pub models: Vec<String>,
    pub agents: Vec<String>,
    pub plugins: Vec<VcpPluginSummary>,
    pub rag_tags: Vec<String>,
    pub schedules: Vec<String>,
    pub clusters: Vec<String>,
    pub config_items: Vec<VcpMetricEntry>,
    pub system_metrics: Vec<VcpMetricEntry>,
    pub warnings: Vec<String>,
}

impl Default for VcpAdminSnapshot {
    fn default() -> Self {
        Self {
            generated_at_epoch_ms: current_epoch_ms(),
            models: Vec::new(),
            agents: Vec::new(),
            plugins: Vec::new(),
            rag_tags: Vec::new(),
            schedules: Vec::new(),
            clusters: Vec::new(),
            config_items: Vec::new(),
            system_metrics: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct HttpProbeResponse {
    category: VcpHealthCategory,
    status_code: Option<u16>,
    detail: String,
    body: Option<String>,
}

pub fn normalize_vcp_chat_endpoint(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_VCP_CHAT_ENDPOINT.to_string();
    }

    let mut normalized = trimmed.to_string();
    if !(normalized.starts_with("http://") || normalized.starts_with("https://")) {
        normalized = format!("http://{normalized}");
    }

    if normalized.contains("/v1/chat/completions") || normalized.contains("/v1/chatvcp/completions") {
        return normalized;
    }
    if normalized.contains("/v1/models") {
        return normalized.replace("/v1/models", "/v1/chat/completions");
    }

    let base = normalized.trim_end_matches('/');
    format!("{base}/v1/chat/completions")
}

pub fn normalize_vcp_admin_endpoint(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_VCP_ADMIN_ENDPOINT.to_string();
    }

    let mut normalized = trimmed.to_string();
    if !(normalized.starts_with("http://") || normalized.starts_with("https://")) {
        normalized = format!("http://{normalized}");
    }

    for suffix in [
        "/v1/chat/completions",
        "/v1/chatvcp/completions",
        "/v1/models",
        "/admin_api/check-auth",
    ] {
        if normalized.contains(suffix) {
            normalized = normalized.replace(suffix, "");
        }
    }

    if let Some((head, _tail)) = normalized.split_once("/admin_api") {
        normalized = head.to_string();
    }

    normalized.trim_end_matches('/').to_string()
}

pub fn infer_vcp_ws_endpoint(admin_endpoint: &str) -> String {
    let base = normalize_vcp_admin_endpoint(admin_endpoint);
    if let Some(stripped) = base.strip_prefix("https://") {
        return format!("wss://{stripped}");
    }
    if let Some(stripped) = base.strip_prefix("http://") {
        return format!("ws://{stripped}");
    }
    DEFAULT_VCP_WS_ENDPOINT.to_string()
}

pub fn build_models_endpoint(chat_endpoint: &str) -> String {
    let normalized = normalize_vcp_chat_endpoint(chat_endpoint);
    if normalized.contains("/v1/chat/completions") {
        return normalized.replace("/v1/chat/completions", "/v1/models");
    }
    if normalized.contains("/v1/chatvcp/completions") {
        return normalized.replace("/v1/chatvcp/completions", "/v1/models");
    }
    if normalized.contains("/v1/models") {
        return normalized;
    }
    format!("{}/v1/models", normalized.trim_end_matches('/'))
}

pub fn build_admin_api_endpoint(admin_endpoint: &str, path: &str) -> String {
    let base = normalize_vcp_admin_endpoint(admin_endpoint);
    let normalized_path = path.trim();
    if normalized_path.is_empty() {
        return format!("{base}/admin_api");
    }

    if normalized_path.starts_with("http://") || normalized_path.starts_with("https://") {
        return normalized_path.to_string();
    }

    if normalized_path.starts_with("/admin_api") {
        return format!("{base}{normalized_path}");
    }

    let mut path_part = normalized_path.to_string();
    if !path_part.starts_with('/') {
        path_part.insert(0, '/');
    }

    format!("{base}/admin_api{path_part}")
}

pub fn probe_vcp_runtime(config: &VcpRuntimeConfig) -> VcpProbeReport {
    let timeout = config.timeout();
    let models_endpoint = build_models_endpoint(config.chat_endpoint.as_str());
    let chat_probe = probe_endpoint(
        models_endpoint.as_str(),
        config
            .api_key
            .as_ref()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("Bearer {value}")),
        timeout,
    );

    let admin_check_endpoint = build_admin_api_endpoint(config.admin_endpoint.as_str(), "/check-auth");
    let admin_probe = probe_endpoint(
        admin_check_endpoint.as_str(),
        config
            .admin_auth_header
            .as_ref()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string()),
        timeout,
    );

    VcpProbeReport {
        chat: VcpEndpointProbe {
            category: chat_probe.category,
            status_code: chat_probe.status_code,
            detail: chat_probe.detail,
        },
        admin: VcpEndpointProbe {
            category: admin_probe.category,
            status_code: admin_probe.status_code,
            detail: admin_probe.detail,
        },
        models: parse_models_from_json_body(chat_probe.body),
    }
}

pub fn fetch_vcp_admin_snapshot(config: &VcpRuntimeConfig) -> Result<VcpAdminSnapshot> {
    let timeout = config.timeout();
    let mut snapshot = VcpAdminSnapshot {
        generated_at_epoch_ms: current_epoch_ms(),
        ..VcpAdminSnapshot::default()
    };

    let bearer = config
        .api_key
        .as_ref()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("Bearer {value}"));
    let admin_auth = config
        .admin_auth_header
        .as_ref()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let models_endpoint = build_models_endpoint(config.chat_endpoint.as_str());
    match fetch_json_with_auth(models_endpoint.as_str(), bearer.clone(), timeout) {
        Ok(value) => {
            snapshot.models = extract_models_from_value(&value);
        }
        Err(err) => snapshot
            .warnings
            .push(format!("models endpoint failed: {err}")),
    }

    let plugin_endpoint = build_admin_api_endpoint(config.admin_endpoint.as_str(), "/plugins");
    match fetch_json_with_auth(plugin_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => snapshot.plugins = extract_plugins_from_value(&value),
        Err(err) => snapshot
            .warnings
            .push(format!("plugins endpoint failed: {err}")),
    }

    let agents_endpoint = build_admin_api_endpoint(config.admin_endpoint.as_str(), "/agents");
    match fetch_json_with_auth(agents_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => snapshot.agents = extract_string_list_from_value(&value, &["agents", "files", "data"]),
        Err(err) => snapshot
            .warnings
            .push(format!("agents endpoint failed: {err}")),
    }

    let rag_tags_endpoint = build_admin_api_endpoint(config.admin_endpoint.as_str(), "/rag-tags");
    match fetch_json_with_auth(rag_tags_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => {
            snapshot.rag_tags =
                extract_string_list_from_value(&value, &["tags", "rag_tags", "data"])
        }
        Err(err) => snapshot
            .warnings
            .push(format!("rag-tags endpoint failed: {err}")),
    }

    let schedules_endpoint = build_admin_api_endpoint(config.admin_endpoint.as_str(), "/schedules");
    match fetch_json_with_auth(schedules_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => {
            snapshot.schedules =
                extract_string_list_from_value(&value, &["schedules", "tasks", "data"])
        }
        Err(err) => snapshot
            .warnings
            .push(format!("schedules endpoint failed: {err}")),
    }

    let clusters_endpoint =
        build_admin_api_endpoint(config.admin_endpoint.as_str(), "/available-clusters");
    match fetch_json_with_auth(clusters_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => {
            snapshot.clusters =
                extract_string_list_from_value(&value, &["clusters", "available_clusters", "data"])
        }
        Err(err) => snapshot
            .warnings
            .push(format!("available-clusters endpoint failed: {err}")),
    }

    let config_endpoint = build_admin_api_endpoint(config.admin_endpoint.as_str(), "/config/main");
    match fetch_json_with_auth(config_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => snapshot.config_items = flatten_object_metrics(&value, 20),
        Err(err) => snapshot
            .warnings
            .push(format!("config endpoint failed: {err}")),
    }

    let vectordb_endpoint =
        build_admin_api_endpoint(config.admin_endpoint.as_str(), "/vectordb/status");
    match fetch_json_with_auth(vectordb_endpoint.as_str(), admin_auth.clone(), timeout) {
        Ok(value) => {
            let mut metrics = flatten_object_metrics(&value, 10);
            metrics.retain(|entry| !entry.value.trim().is_empty());
            snapshot.system_metrics.extend(metrics);
        }
        Err(err) => snapshot
            .warnings
            .push(format!("vectordb endpoint failed: {err}")),
    }

    let resources_endpoint =
        build_admin_api_endpoint(config.admin_endpoint.as_str(), "/system-monitor/system/resources");
    match fetch_json_with_auth(resources_endpoint.as_str(), admin_auth, timeout) {
        Ok(value) => {
            let mut metrics = flatten_object_metrics(&value, 20);
            metrics.retain(|entry| !entry.value.trim().is_empty());
            snapshot.system_metrics.extend(metrics);
        }
        Err(err) => snapshot
            .warnings
            .push(format!("system resources endpoint failed: {err}")),
    }

    dedup_string_vec(&mut snapshot.models);
    dedup_string_vec(&mut snapshot.agents);
    dedup_string_vec(&mut snapshot.rag_tags);
    dedup_string_vec(&mut snapshot.schedules);
    dedup_string_vec(&mut snapshot.clusters);

    if snapshot.config_items.is_empty() {
        snapshot.config_items.push(VcpMetricEntry {
            key: "admin_endpoint".to_string(),
            value: normalize_vcp_admin_endpoint(config.admin_endpoint.as_str()),
        });
    }

    Ok(snapshot)
}

fn probe_endpoint(endpoint: &str, authorization: Option<String>, timeout: Duration) -> HttpProbeResponse {
    let endpoint_trimmed = endpoint.trim().to_string();
    let host_port = match parse_host_port_from_endpoint(endpoint_trimmed.as_str()) {
        Ok(value) => value,
        Err(err) => {
            return HttpProbeResponse {
                category: VcpHealthCategory::InvalidEndpoint,
                status_code: None,
                detail: format!("{} ({err})", endpoint_trimmed),
                body: None,
            }
        }
    };

    let mut addrs = match host_port.to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(err) => {
            return HttpProbeResponse {
                category: VcpHealthCategory::InvalidEndpoint,
                status_code: None,
                detail: format!("{} ({err})", endpoint_trimmed),
                body: None,
            }
        }
    };

    let Some(addr) = addrs.next() else {
        return HttpProbeResponse {
            category: VcpHealthCategory::InvalidEndpoint,
            status_code: None,
            detail: format!("{} (endpoint resolved no address)", endpoint_trimmed),
            body: None,
        };
    };

    if let Err(err) = TcpStream::connect_timeout(&addr, timeout) {
        let category = if err.kind() == ErrorKind::TimedOut {
            VcpHealthCategory::Timeout
        } else {
            VcpHealthCategory::NetworkError
        };
        return HttpProbeResponse {
            category,
            status_code: None,
            detail: format!("{} ({err})", endpoint_trimmed),
            body: None,
        };
    }

    let mut req = ureq::get(endpoint_trimmed.as_str())
        .timeout(timeout)
        .set("Accept", "application/json");
    if let Some(value) = authorization.as_deref() {
        req = req.set("Authorization", value);
    }

    match req.call() {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.into_string().ok();
            HttpProbeResponse {
                category: classify_http_status_to_category(status),
                status_code: Some(status),
                detail: format!("{} -> HTTP {}", endpoint_trimmed, status),
                body,
            }
        }
        Err(ureq::Error::Status(code, resp)) => HttpProbeResponse {
            category: classify_http_status_to_category(code),
            status_code: Some(code),
            detail: format!("{} -> HTTP {}", endpoint_trimmed, code),
            body: resp.into_string().ok(),
        },
        Err(ureq::Error::Transport(err)) => {
            let text = err.to_string();
            let category = if text.to_ascii_lowercase().contains("timed out") {
                VcpHealthCategory::Timeout
            } else {
                VcpHealthCategory::NetworkError
            };
            HttpProbeResponse {
                category,
                status_code: None,
                detail: format!("{} ({text})", endpoint_trimmed),
                body: None,
            }
        }
    }
}

fn fetch_json_with_auth(endpoint: &str, authorization: Option<String>, timeout: Duration) -> Result<Value> {
    let mut req = ureq::get(endpoint)
        .timeout(timeout)
        .set("Accept", "application/json");
    if let Some(value) = authorization.as_deref() {
        req = req.set("Authorization", value);
    }

    let response = req.call().with_context(|| format!("request {endpoint}"))?;
    let body = response
        .into_string()
        .with_context(|| format!("read response body {endpoint}"))?;
    let value: Value = serde_json::from_str(body.as_str())
        .with_context(|| format!("parse response json {endpoint}"))?;
    Ok(value)
}

fn extract_models_from_value(value: &Value) -> Vec<String> {
    if let Some(array) = value.get("data").and_then(Value::as_array) {
        let mut models = Vec::new();
        for item in array {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                models.push(id.to_string());
            }
        }
        dedup_string_vec(&mut models);
        return models;
    }

    extract_string_list_from_value(value, &["models", "data"])
}

fn extract_plugins_from_value(value: &Value) -> Vec<VcpPluginSummary> {
    let mut out = Vec::new();
    let candidates = if let Some(array) = value.get("plugins").and_then(Value::as_array) {
        array.clone()
    } else if let Some(array) = value.get("data").and_then(Value::as_array) {
        array.clone()
    } else if let Some(array) = value.as_array() {
        array.clone()
    } else {
        Vec::new()
    };

    for item in candidates {
        if let Some(text) = item.as_str() {
            out.push(VcpPluginSummary {
                name: text.to_string(),
                enabled: None,
            });
            continue;
        }

        let Some(obj) = item.as_object() else {
            continue;
        };
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| obj.get("id").and_then(Value::as_str))
            .or_else(|| obj.get("pluginName").and_then(Value::as_str))
            .or_else(|| obj.get("fileName").and_then(Value::as_str));
        let Some(name) = name else {
            continue;
        };

        let enabled = obj
            .get("enabled")
            .and_then(Value::as_bool)
            .or_else(|| obj.get("isEnabled").and_then(Value::as_bool))
            .or_else(|| obj.get("active").and_then(Value::as_bool));
        out.push(VcpPluginSummary {
            name: name.to_string(),
            enabled,
        });
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name.eq_ignore_ascii_case(b.name.as_str()));
    out
}

fn extract_string_list_from_value(value: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(array) = value.get(*key).and_then(Value::as_array) {
            return normalize_string_array(array.as_slice());
        }
    }

    if let Some(array) = value.as_array() {
        return normalize_string_array(array);
    }

    Vec::new()
}

fn normalize_string_array(items: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        if let Some(text) = item.as_str() {
            let value = text.trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
            continue;
        }

        if let Some(obj) = item.as_object() {
            let candidate = obj
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| obj.get("id").and_then(Value::as_str))
                .or_else(|| obj.get("title").and_then(Value::as_str))
                .or_else(|| obj.get("fileName").and_then(Value::as_str));
            if let Some(value) = candidate.map(str::trim).filter(|value| !value.is_empty()) {
                out.push(value.to_string());
            }
        }
    }

    dedup_string_vec(&mut out);
    out
}

fn flatten_object_metrics(value: &Value, max_entries: usize) -> Vec<VcpMetricEntry> {
    let mut entries = Vec::new();

    if let Some(object) = value.as_object() {
        for (key, item) in object {
            if entries.len() >= max_entries {
                break;
            }
            let text = stringify_json_value(item);
            if text.trim().is_empty() {
                continue;
            }
            entries.push(VcpMetricEntry {
                key: key.to_string(),
                value: text,
            });
        }
    }

    entries
}

fn parse_models_from_json_body(body: Option<String>) -> Vec<String> {
    let Some(raw) = body else {
        return Vec::new();
    };
    let value: Value = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    extract_models_from_value(&value)
}

fn stringify_json_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(v) => {
            if v.is_empty() {
                String::new()
            } else {
                serde_json::to_string(v).unwrap_or_default()
            }
        }
        Value::Object(v) => {
            if v.is_empty() {
                String::new()
            } else {
                serde_json::to_string(v).unwrap_or_default()
            }
        }
    }
}

fn dedup_string_vec(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn parse_host_port_from_endpoint(endpoint: &str) -> std::io::Result<String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty endpoint",
        ));
    }

    let without_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let host_and_path = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .trim();
    if host_and_path.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid endpoint host",
        ));
    }

    if host_and_path.contains(':') {
        return Ok(host_and_path.to_string());
    }

    Ok(format!("{host_and_path}:80"))
}

fn classify_http_status_to_category(code: u16) -> VcpHealthCategory {
    match code {
        200..=299 => VcpHealthCategory::Connected,
        401 | 403 => VcpHealthCategory::Unauthorized,
        404 => VcpHealthCategory::ApiPathNotFound,
        500..=599 => VcpHealthCategory::ServerError,
        400..=499 => VcpHealthCategory::ClientError,
        _ => VcpHealthCategory::UnknownError,
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_vcp_chat_endpoint_accepts_plain_host() {
        assert_eq!(
            normalize_vcp_chat_endpoint("127.0.0.1:6005"),
            "http://127.0.0.1:6005/v1/chat/completions"
        );
    }

    #[test]
    fn normalize_vcp_admin_endpoint_strips_known_paths() {
        assert_eq!(
            normalize_vcp_admin_endpoint("http://127.0.0.1:6005/v1/chat/completions"),
            "http://127.0.0.1:6005"
        );
        assert_eq!(
            normalize_vcp_admin_endpoint("http://127.0.0.1:6005/admin_api/plugins"),
            "http://127.0.0.1:6005"
        );
    }

    #[test]
    fn infer_ws_endpoint_from_admin_http() {
        assert_eq!(
            infer_vcp_ws_endpoint("http://127.0.0.1:6005"),
            "ws://127.0.0.1:6005"
        );
        assert_eq!(
            infer_vcp_ws_endpoint("https://example.com"),
            "wss://example.com"
        );
    }

    #[test]
    fn build_models_endpoint_rewrites_chat_urls() {
        assert_eq!(
            build_models_endpoint("http://127.0.0.1:6005/v1/chat/completions"),
            "http://127.0.0.1:6005/v1/models"
        );
        assert_eq!(
            build_models_endpoint("http://127.0.0.1:6005/v1/chatvcp/completions"),
            "http://127.0.0.1:6005/v1/models"
        );
    }

    #[test]
    fn build_admin_api_endpoint_normalizes_paths() {
        assert_eq!(
            build_admin_api_endpoint("http://127.0.0.1:6005", "/plugins"),
            "http://127.0.0.1:6005/admin_api/plugins"
        );
        assert_eq!(
            build_admin_api_endpoint("http://127.0.0.1:6005", "/admin_api/agents"),
            "http://127.0.0.1:6005/admin_api/agents"
        );
    }

    #[test]
    fn parse_models_from_json_data_array() {
        let body = Some(
            r#"{"data":[{"id":"gemini-2.5"},{"id":"gpt-4.1"}],"object":"list"}"#
                .to_string(),
        );
        let models = parse_models_from_json_body(body);
        assert_eq!(models, vec!["gemini-2.5".to_string(), "gpt-4.1".to_string()]);
    }

    #[test]
    fn extract_plugins_from_mixed_shape() {
        let value: Value = serde_json::from_str(
            r#"{
                "plugins": [
                    "ToolA",
                    {"name":"ToolB","enabled":true},
                    {"pluginName":"ToolC","isEnabled":false}
                ]
            }"#,
        )
        .expect("valid json");

        let plugins = extract_plugins_from_value(&value);
        assert_eq!(plugins.len(), 3);
        assert!(plugins.iter().any(|item| item.name == "ToolA"));
        assert!(plugins
            .iter()
            .any(|item| item.name == "ToolB" && item.enabled == Some(true)));
        assert!(plugins
            .iter()
            .any(|item| item.name == "ToolC" && item.enabled == Some(false)));
    }

    #[test]
    fn extract_string_list_from_object_array() {
        let value: Value = serde_json::from_str(
            r#"{
                "data": [
                    {"name":"Nova"},
                    {"id":"Coder"},
                    {"title":"Planner"}
                ]
            }"#,
        )
        .expect("valid json");

        let list = extract_string_list_from_value(&value, &["data"]);
        assert_eq!(
            list,
            vec![
                "Coder".to_string(),
                "Nova".to_string(),
                "Planner".to_string()
            ]
        );
    }

    #[test]
    fn flatten_object_metrics_keeps_scalar_entries() {
        let value: Value = serde_json::from_str(
            r#"{
                "cpu": 10,
                "memory": {"used": 1024},
                "ok": true,
                "empty": null
            }"#,
        )
        .expect("valid json");

        let metrics = flatten_object_metrics(&value, 10);
        assert!(metrics.iter().any(|entry| entry.key == "cpu"));
        assert!(metrics.iter().any(|entry| entry.key == "memory"));
        assert!(metrics.iter().any(|entry| entry.key == "ok"));
        assert!(!metrics.iter().any(|entry| entry.key == "empty"));
    }
}

