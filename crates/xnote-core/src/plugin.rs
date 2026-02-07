use crate::command::CommandId;
use crate::plugin_protocol::{PluginWireMessage, PLUGIN_PROTOCOL_VERSION};
use crate::plugin_transport::{PluginTransport, StdioProcessTransport};
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PluginCapability {
    Commands,
    ReadVault,
    WriteVault,
    Network,
}

impl PluginCapability {
    fn as_tag(self) -> &'static str {
        match self {
            Self::Commands => "commands",
            Self::ReadVault => "read_vault",
            Self::WriteVault => "write_vault",
            Self::Network => "network",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PluginActivationEvent {
    OnStartupFinished,
    OnVaultOpened,
    OnCommand(CommandId),
}

impl PluginActivationEvent {
    fn matches_trigger(&self, trigger: &PluginActivationEvent) -> bool {
        match (self, trigger) {
            (Self::OnStartupFinished, Self::OnStartupFinished) => true,
            (Self::OnVaultOpened, Self::OnVaultOpened) => true,
            (Self::OnCommand(expected), Self::OnCommand(actual)) => expected == actual,
            _ => false,
        }
    }

    fn as_runtime_tag(&self) -> String {
        match self {
            Self::OnStartupFinished => "on_startup_finished".to_string(),
            Self::OnVaultOpened => "on_vault_opened".to_string(),
            Self::OnCommand(command) => format!("on_command:{}", command.as_str()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginLifecycleState {
    Registered,
    Activating,
    Active,
    Failed,
    Disabled,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginRuntimeState {
    pub state: PluginLifecycleState,
    pub activation_count: u32,
    pub failed_attempts: u32,
    pub cancelled_attempts: u32,
    pub last_error: Option<String>,
    pub last_trigger: Option<PluginActivationEvent>,
    pub total_activation_ms: u128,
}

impl Default for PluginRuntimeState {
    fn default() -> Self {
        Self {
            state: PluginLifecycleState::Registered,
            activation_count: 0,
            failed_attempts: 0,
            cancelled_attempts: 0,
            last_error: None,
            last_trigger: None,
            total_activation_ms: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginActivationOutcome {
    pub plugin_id: String,
    pub state: PluginLifecycleState,
    pub activated: bool,
    pub error: Option<String>,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub capabilities: Vec<PluginCapability>,
    pub command_allowlist: Vec<CommandId>,
    pub activation_events: Vec<PluginActivationEvent>,
}

pub trait Plugin {
    fn manifest(&self) -> &PluginManifest;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluginPolicy {
    pub allow_network: bool,
    pub max_failed_activations: u32,
    pub activation_timeout_ms: u64,
}

impl Default for PluginPolicy {
    fn default() -> Self {
        Self {
            allow_network: false,
            max_failed_activations: 3,
            activation_timeout_ms: 2000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeActivationSpec {
    pub timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeErrorCode {
    InvalidConfig,
    SpawnFailed,
    TransportIo,
    HandshakeRejected,
    ProtocolMismatch,
    CapabilityViolation,
    ProtocolViolation,
    ActivationRejected,
}

impl RuntimeErrorCode {
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid_config",
            Self::SpawnFailed => "spawn_failed",
            Self::TransportIo => "transport_io",
            Self::HandshakeRejected => "handshake_rejected",
            Self::ProtocolMismatch => "protocol_mismatch",
            Self::CapabilityViolation => "capability_violation",
            Self::ProtocolViolation => "protocol_violation",
            Self::ActivationRejected => "activation_rejected",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub detail: String,
}

impl RuntimeError {
    pub fn new(code: RuntimeErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_tag(), self.detail)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStatus {
    Ready,
    Failed(RuntimeError),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeActivationResult {
    pub status: RuntimeStatus,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSessionSnapshot {
    pub session_key: String,
    pub idle_ms: u128,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RuntimeTelemetrySnapshot {
    pub spawn_count: u64,
    pub handshake_count: u64,
    pub activation_request_count: u64,
    pub reused_session_count: u64,
    pub session_ping_failure_count: u64,
    pub evicted_by_limit_count: u64,
    pub evicted_by_idle_ttl_count: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ActivationCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ActivationCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub trait PluginRuntime {
    fn activate(
        &mut self,
        manifest: &PluginManifest,
        trigger: &PluginActivationEvent,
        spec: RuntimeActivationSpec,
        cancellation: &ActivationCancellation,
    ) -> RuntimeActivationResult;
}

pub struct InProcessRuntime;

impl PluginRuntime for InProcessRuntime {
    fn activate(
        &mut self,
        _manifest: &PluginManifest,
        _trigger: &PluginActivationEvent,
        _spec: RuntimeActivationSpec,
        cancellation: &ActivationCancellation,
    ) -> RuntimeActivationResult {
        if cancellation.is_cancelled() {
            return RuntimeActivationResult {
                status: RuntimeStatus::Cancelled,
                elapsed_ms: 0,
            };
        }

        RuntimeActivationResult {
            status: RuntimeStatus::Ready,
            elapsed_ms: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginRuntimeMode {
    InProcess,
    Process,
}

impl PluginRuntimeMode {
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::Process => "process",
        }
    }

    pub fn from_tag(input: &str) -> Self {
        if input.trim().eq_ignore_ascii_case("process") {
            Self::Process
        } else {
            Self::InProcess
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessRuntimeConfig {
    pub command: String,
    pub args: Vec<String>,
    pub extra_env: HashMap<String, String>,
    pub watchdog_interval_ms: u64,
    pub protocol_version: u32,
    pub supported_protocol_versions: Vec<u32>,
    pub keep_alive_session: bool,
    pub session_ping_timeout_ms: u64,
    pub max_keep_alive_sessions: usize,
    pub session_idle_ttl_ms: u64,
}

impl Default for ProcessRuntimeConfig {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        let worker_binary = "xnote-plugin-worker.exe";
        #[cfg(not(target_os = "windows"))]
        let worker_binary = "xnote-plugin-worker";

        let worker_path = std::env::var("XNOTE_PLUGIN_WORKER_BIN")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| worker_binary.to_string());

        Self {
            command: worker_path,
            args: Vec::new(),
            extra_env: HashMap::new(),
            watchdog_interval_ms: 10,
            protocol_version: PLUGIN_PROTOCOL_VERSION,
            supported_protocol_versions: vec![PLUGIN_PROTOCOL_VERSION],
            keep_alive_session: false,
            session_ping_timeout_ms: 150,
            max_keep_alive_sessions: 8,
            session_idle_ttl_ms: 60_000,
        }
    }
}

impl ProcessRuntimeConfig {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            ..Self::default()
        }
    }

    fn normalized_watchdog_interval_ms(&self) -> u64 {
        self.watchdog_interval_ms.max(1)
    }

    fn normalized_supported_protocol_versions(&self) -> Vec<u32> {
        let mut versions = self
            .supported_protocol_versions
            .iter()
            .copied()
            .filter(|v| *v > 0)
            .collect::<Vec<_>>();
        if !versions.contains(&self.protocol_version) {
            versions.push(self.protocol_version);
        }
        versions.sort_unstable();
        versions.dedup();
        versions.reverse();
        versions
    }

    fn negotiate_protocol(&self, runtime_version: u32) -> Option<u32> {
        if runtime_version == 0 {
            return None;
        }
        let versions = self.normalized_supported_protocol_versions();
        if versions.contains(&runtime_version) {
            return Some(runtime_version);
        }
        None
    }

    fn normalized_session_ping_timeout_ms(&self) -> u64 {
        self.session_ping_timeout_ms.max(10)
    }

    fn normalized_max_keep_alive_sessions(&self) -> usize {
        self.max_keep_alive_sessions.max(1)
    }

    fn normalized_session_idle_ttl_ms(&self) -> u64 {
        self.session_idle_ttl_ms.max(100)
    }
}

fn recv_wire_message_with_watchdog(
    transport: &mut dyn PluginTransport,
    deadline: Instant,
    cancellation: &ActivationCancellation,
    request_id: Option<&str>,
    watchdog_tick: Duration,
) -> Result<PluginWireMessage, RuntimeStatus> {
    loop {
        if cancellation.is_cancelled() {
            return Err(RuntimeStatus::Cancelled);
        }

        if Instant::now() >= deadline {
            return Err(RuntimeStatus::Cancelled);
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait_for = remaining.min(watchdog_tick);
        match transport.receive(wait_for) {
            Ok(Some(msg)) => {
                if let Some(request_id) = request_id {
                    if let PluginWireMessage::ActivateResult {
                        request_id: rid, ..
                    } = &msg
                    {
                        if rid != request_id {
                            continue;
                        }
                    }
                }
                return Ok(msg);
            }
            Ok(None) => continue,
            Err(err) => {
                return Err(RuntimeStatus::Failed(RuntimeError::new(
                    RuntimeErrorCode::TransportIo,
                    err,
                )));
            }
        }
    }
}

fn runtime_failed(code: RuntimeErrorCode, detail: impl Into<String>) -> RuntimeStatus {
    RuntimeStatus::Failed(RuntimeError::new(code, detail))
}

fn cancel_request_if_possible(
    transport: &mut dyn PluginTransport,
    request_id: &str,
    reason: &str,
) -> Result<(), String> {
    transport.send(&PluginWireMessage::Cancel {
        request_id: request_id.to_string(),
        reason: reason.to_string(),
    })
}

fn ping_transport(
    transport: &mut dyn PluginTransport,
    timeout_ms: u64,
    watchdog_tick: Duration,
    request_id: String,
) -> Result<(), RuntimeStatus> {
    let started_at = Instant::now();
    let deadline = started_at + Duration::from_millis(timeout_ms.max(10));

    transport
        .send(&PluginWireMessage::Ping {
            request_id: request_id.clone(),
        })
        .map_err(|err| runtime_failed(RuntimeErrorCode::TransportIo, err))?;

    loop {
        if Instant::now() >= deadline {
            return Err(RuntimeStatus::Cancelled);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait_for = remaining.min(watchdog_tick);
        match transport.receive(wait_for) {
            Ok(Some(PluginWireMessage::Pong { request_id: rid })) => {
                if rid == request_id {
                    return Ok(());
                }
            }
            Ok(Some(_)) => continue,
            Ok(None) => continue,
            Err(err) => return Err(runtime_failed(RuntimeErrorCode::TransportIo, err)),
        }
    }
}

struct RuntimeSession {
    transport: Box<dyn PluginTransport>,
    last_used_at: Instant,
}

pub struct ProcessPluginRuntime {
    config: ProcessRuntimeConfig,
    sessions: HashMap<String, RuntimeSession>,
    session_order: Vec<String>,
    request_sequence: u64,
    telemetry: RuntimeTelemetrySnapshot,
}

impl ProcessPluginRuntime {
    pub fn new(config: ProcessRuntimeConfig) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
            session_order: Vec::new(),
            request_sequence: 0,
            telemetry: RuntimeTelemetrySnapshot::default(),
        }
    }

    pub fn config(&self) -> &ProcessRuntimeConfig {
        &self.config
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn active_sessions_snapshot(&self) -> Vec<RuntimeSessionSnapshot> {
        let now = Instant::now();
        let mut out = self
            .session_order
            .iter()
            .filter_map(|key| {
                self.sessions
                    .get(key)
                    .map(|session| RuntimeSessionSnapshot {
                        session_key: key.clone(),
                        idle_ms: now.duration_since(session.last_used_at).as_millis(),
                    })
            })
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.session_key.cmp(&b.session_key));
        out
    }

    pub fn telemetry_snapshot(&self) -> RuntimeTelemetrySnapshot {
        self.telemetry.clone()
    }

    fn session_key(manifest: &PluginManifest) -> String {
        let mut capabilities = manifest
            .capabilities
            .iter()
            .copied()
            .map(PluginCapability::as_tag)
            .collect::<Vec<_>>();
        capabilities.sort_unstable();
        capabilities.dedup();
        format!(
            "{}:{}:{}",
            manifest.id,
            manifest.version,
            capabilities.join(",")
        )
    }

    fn should_reset_session(status: &RuntimeStatus) -> bool {
        match status {
            RuntimeStatus::Ready => false,
            RuntimeStatus::Cancelled => true,
            RuntimeStatus::Failed(err) => !matches!(err.code, RuntimeErrorCode::ActivationRejected),
        }
    }

    fn next_request_id(&mut self, prefix: &str, plugin_id: &str) -> String {
        self.request_sequence = self.request_sequence.saturating_add(1);
        format!("{prefix}:{plugin_id}:{}", self.request_sequence)
    }

    fn activation_watchdog_tick(&self) -> Duration {
        Duration::from_millis(self.config.normalized_watchdog_interval_ms())
    }

    fn activation_deadline(&self, started_at: Instant, spec: RuntimeActivationSpec) -> Instant {
        started_at + Duration::from_millis(spec.timeout_ms)
    }

    fn touch_session_key(&mut self, session_key: &str) {
        self.session_order
            .retain(|existing| existing != session_key);
        self.session_order.push(session_key.to_string());
    }

    fn remove_session_entry(&mut self, session_key: &str) {
        if let Some(mut session) = self.sessions.remove(session_key) {
            session.transport.terminate();
        }
        self.session_order
            .retain(|existing| existing != session_key);
    }

    fn evict_idle_sessions(&mut self) {
        let ttl = Duration::from_millis(self.config.normalized_session_idle_ttl_ms());
        let now = Instant::now();
        let stale_keys = self
            .session_order
            .iter()
            .filter_map(|key| {
                self.sessions.get(key).and_then(|session| {
                    if now.duration_since(session.last_used_at) >= ttl {
                        Some(key.clone())
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();

        for key in stale_keys {
            self.remove_session_entry(&key);
            self.telemetry.evicted_by_idle_ttl_count =
                self.telemetry.evicted_by_idle_ttl_count.saturating_add(1);
        }
    }

    fn enforce_session_limit(&mut self) {
        let limit = self.config.normalized_max_keep_alive_sessions();
        while self.sessions.len() > limit {
            if let Some(evicted) = self.session_order.first().cloned() {
                self.remove_session_entry(&evicted);
                self.telemetry.evicted_by_limit_count =
                    self.telemetry.evicted_by_limit_count.saturating_add(1);
            } else {
                break;
            }
        }
    }

    fn spawn_transport(
        &mut self,
        manifest: &PluginManifest,
        trigger: &PluginActivationEvent,
        spec: RuntimeActivationSpec,
    ) -> Result<RuntimeSession, RuntimeStatus> {
        let mut command = Command::new(&self.config.command);
        command
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("XNOTE_PLUGIN_ID", manifest.id.as_str())
            .env("XNOTE_PLUGIN_VERSION", manifest.version.as_str())
            .env("XNOTE_PLUGIN_TRIGGER", trigger.as_runtime_tag())
            .env("XNOTE_PLUGIN_TIMEOUT_MS", spec.timeout_ms.to_string());

        for (key, value) in &self.config.extra_env {
            command.env(key, value);
        }

        let child = command.spawn().map_err(|err| {
            runtime_failed(
                RuntimeErrorCode::SpawnFailed,
                format!("spawn process runtime failed: {err}"),
            )
        })?;

        let transport = StdioProcessTransport::from_child(child)
            .map_err(|err| runtime_failed(RuntimeErrorCode::TransportIo, err))?;
        self.telemetry.spawn_count = self.telemetry.spawn_count.saturating_add(1);
        Ok(RuntimeSession {
            transport: Box::new(transport),
            last_used_at: Instant::now(),
        })
    }

    fn perform_handshake(
        &mut self,
        transport: &mut dyn PluginTransport,
        manifest: &PluginManifest,
        deadline: Instant,
        cancellation: &ActivationCancellation,
        watchdog_tick: Duration,
    ) -> Result<(), RuntimeStatus> {
        let handshake = PluginWireMessage::Handshake {
            protocol_version: self.config.protocol_version,
            supported_protocol_versions: self.config.normalized_supported_protocol_versions(),
            plugin_id: manifest.id.clone(),
            plugin_version: manifest.version.clone(),
            capabilities: manifest
                .capabilities
                .iter()
                .copied()
                .map(PluginCapability::as_tag)
                .map(str::to_string)
                .collect(),
        };

        transport
            .send(&handshake)
            .map_err(|err| runtime_failed(RuntimeErrorCode::TransportIo, err))?;

        match recv_wire_message_with_watchdog(
            transport,
            deadline,
            cancellation,
            None,
            watchdog_tick,
        ) {
            Ok(PluginWireMessage::HandshakeAck {
                protocol_version,
                accepted,
                reason,
                reported_capabilities,
            }) => {
                if !accepted {
                    return Err(runtime_failed(
                        RuntimeErrorCode::HandshakeRejected,
                        reason.unwrap_or_else(|| "runtime handshake rejected".to_string()),
                    ));
                }

                if self.config.negotiate_protocol(protocol_version).is_none() {
                    return Err(runtime_failed(
                        RuntimeErrorCode::ProtocolMismatch,
                        format!(
                            "protocol version mismatch: host_supported={:?} runtime={}",
                            self.config.normalized_supported_protocol_versions(),
                            protocol_version
                        ),
                    ));
                }

                let declared_capabilities: HashSet<String> = manifest
                    .capabilities
                    .iter()
                    .copied()
                    .map(PluginCapability::as_tag)
                    .map(str::to_string)
                    .collect();
                let reported_capabilities: HashSet<String> =
                    reported_capabilities.into_iter().collect();

                if !reported_capabilities.is_subset(&declared_capabilities) {
                    let extra = reported_capabilities
                        .difference(&declared_capabilities)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",");
                    return Err(runtime_failed(
                        RuntimeErrorCode::CapabilityViolation,
                        format!("runtime reported undeclared capabilities: {extra}"),
                    ));
                }
                self.telemetry.handshake_count = self.telemetry.handshake_count.saturating_add(1);
                Ok(())
            }
            Ok(_) => Err(runtime_failed(
                RuntimeErrorCode::ProtocolViolation,
                "unexpected runtime message before handshake ack",
            )),
            Err(status) => {
                let _ = cancel_request_if_possible(transport, "handshake", "host_cancelled");
                Err(status)
            }
        }
    }

    fn request_activation(
        &mut self,
        transport: &mut dyn PluginTransport,
        trigger: &PluginActivationEvent,
        spec: RuntimeActivationSpec,
        deadline: Instant,
        cancellation: &ActivationCancellation,
        request_id: String,
    ) -> RuntimeStatus {
        let activate_message = PluginWireMessage::Activate {
            request_id: request_id.clone(),
            event: trigger.as_runtime_tag(),
            timeout_ms: spec.timeout_ms,
        };
        self.telemetry.activation_request_count =
            self.telemetry.activation_request_count.saturating_add(1);

        if let Err(err) = transport.send(&activate_message) {
            return runtime_failed(RuntimeErrorCode::TransportIo, err);
        }

        match recv_wire_message_with_watchdog(
            transport,
            deadline,
            cancellation,
            Some(&request_id),
            self.activation_watchdog_tick(),
        ) {
            Ok(PluginWireMessage::ActivateResult {
                request_id: _,
                ok,
                error,
            }) => {
                if ok {
                    RuntimeStatus::Ready
                } else {
                    runtime_failed(
                        RuntimeErrorCode::ActivationRejected,
                        error.unwrap_or_else(|| "runtime activation failed".to_string()),
                    )
                }
            }
            Ok(_) => runtime_failed(
                RuntimeErrorCode::ProtocolViolation,
                "unexpected runtime message before activation result",
            ),
            Err(status) => {
                let _ = cancel_request_if_possible(transport, &request_id, "host_cancelled");
                status
            }
        }
    }
}

impl Drop for ProcessPluginRuntime {
    fn drop(&mut self) {
        for session in self.sessions.values_mut() {
            session.transport.terminate();
        }
        self.sessions.clear();
        self.session_order.clear();
    }
}

impl PluginRuntime for ProcessPluginRuntime {
    fn activate(
        &mut self,
        manifest: &PluginManifest,
        trigger: &PluginActivationEvent,
        spec: RuntimeActivationSpec,
        cancellation: &ActivationCancellation,
    ) -> RuntimeActivationResult {
        let started_at = Instant::now();
        if cancellation.is_cancelled() {
            return RuntimeActivationResult {
                status: RuntimeStatus::Cancelled,
                elapsed_ms: 0,
            };
        }

        if self.config.command.trim().is_empty() {
            return RuntimeActivationResult {
                status: runtime_failed(
                    RuntimeErrorCode::InvalidConfig,
                    "process runtime command is empty",
                ),
                elapsed_ms: started_at.elapsed().as_millis(),
            };
        }

        if self.config.keep_alive_session {
            self.evict_idle_sessions();
        }

        let watchdog_tick = self.activation_watchdog_tick();
        let deadline = self.activation_deadline(started_at, spec);
        let session_key = Self::session_key(manifest);
        let mut needs_handshake = true;

        let mut session = if self.config.keep_alive_session {
            if let Some(mut existing) = self.sessions.remove(&session_key) {
                self.session_order
                    .retain(|existing_key| existing_key != &session_key);
                let ping_status = ping_transport(
                    existing.transport.as_mut(),
                    self.config.normalized_session_ping_timeout_ms(),
                    watchdog_tick,
                    self.next_request_id("ping", &manifest.id),
                );
                if ping_status.is_ok() {
                    needs_handshake = false;
                    self.telemetry.reused_session_count =
                        self.telemetry.reused_session_count.saturating_add(1);
                    existing
                } else {
                    self.telemetry.session_ping_failure_count =
                        self.telemetry.session_ping_failure_count.saturating_add(1);
                    existing.transport.terminate();
                    match self.spawn_transport(manifest, trigger, spec) {
                        Ok(session) => session,
                        Err(status) => {
                            return RuntimeActivationResult {
                                status,
                                elapsed_ms: started_at.elapsed().as_millis(),
                            }
                        }
                    }
                }
            } else {
                match self.spawn_transport(manifest, trigger, spec) {
                    Ok(session) => session,
                    Err(status) => {
                        return RuntimeActivationResult {
                            status,
                            elapsed_ms: started_at.elapsed().as_millis(),
                        }
                    }
                }
            }
        } else {
            match self.spawn_transport(manifest, trigger, spec) {
                Ok(session) => session,
                Err(status) => {
                    return RuntimeActivationResult {
                        status,
                        elapsed_ms: started_at.elapsed().as_millis(),
                    }
                }
            }
        };

        if needs_handshake {
            if let Err(status) = self.perform_handshake(
                session.transport.as_mut(),
                manifest,
                deadline,
                cancellation,
                watchdog_tick,
            ) {
                session.transport.terminate();
                return RuntimeActivationResult {
                    status,
                    elapsed_ms: started_at.elapsed().as_millis(),
                };
            }
        }

        let activation_request_id = self.next_request_id("act", &manifest.id);
        let status = self.request_activation(
            session.transport.as_mut(),
            trigger,
            spec,
            deadline,
            cancellation,
            activation_request_id,
        );
        session.last_used_at = Instant::now();

        let keep_session = self.config.keep_alive_session && !Self::should_reset_session(&status);
        if keep_session {
            self.sessions.insert(session_key.clone(), session);
            self.touch_session_key(&session_key);
            self.enforce_session_limit();
        } else {
            session.transport.terminate();
            self.session_order
                .retain(|existing_key| existing_key != &session_key);
        }

        RuntimeActivationResult {
            status,
            elapsed_ms: started_at.elapsed().as_millis(),
        }
    }
}

#[derive(Default)]
pub struct PluginRegistry {
    manifests: HashMap<String, PluginManifest>,
    runtime_by_id: HashMap<String, PluginRuntimeState>,
    policy: PluginPolicy,
}

impl PluginRegistry {
    pub fn with_policy(policy: PluginPolicy) -> Self {
        Self {
            manifests: HashMap::new(),
            runtime_by_id: HashMap::new(),
            policy,
        }
    }

    pub fn set_policy(&mut self, policy: PluginPolicy) {
        self.policy = policy;
    }

    pub fn register_manifest(&mut self, manifest: PluginManifest) -> Result<(), String> {
        validate_manifest(&manifest, self.policy)?;
        self.runtime_by_id.entry(manifest.id.clone()).or_default();
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
    }

    pub fn register_plugin(&mut self, plugin: &dyn Plugin) -> Result<(), String> {
        self.register_manifest(plugin.manifest().clone())
    }

    pub fn manifest(&self, id: &str) -> Option<&PluginManifest> {
        self.manifests.get(id)
    }

    pub fn runtime(&self, id: &str) -> Option<&PluginRuntimeState> {
        self.runtime_by_id.get(id)
    }

    pub fn list(&self) -> Vec<&PluginManifest> {
        let mut out: Vec<&PluginManifest> = self.manifests.values().collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn active_count(&self) -> usize {
        self.runtime_by_id
            .values()
            .filter(|runtime| runtime.state == PluginLifecycleState::Active)
            .count()
    }

    pub fn trigger_event(&mut self, event: PluginActivationEvent) -> Vec<PluginActivationOutcome> {
        let mut runtime = InProcessRuntime;
        self.trigger_event_with_runtime(event, &mut runtime)
    }

    pub fn trigger_event_with_mode(
        &mut self,
        event: PluginActivationEvent,
        mode: PluginRuntimeMode,
        process_config: Option<ProcessRuntimeConfig>,
    ) -> Vec<PluginActivationOutcome> {
        match mode {
            PluginRuntimeMode::InProcess => {
                let mut runtime = InProcessRuntime;
                self.trigger_event_with_runtime(event, &mut runtime)
            }
            PluginRuntimeMode::Process => {
                let mut runtime = ProcessPluginRuntime::new(process_config.unwrap_or_default());
                self.trigger_event_with_runtime(event, &mut runtime)
            }
        }
    }

    pub fn trigger_event_with_runtime(
        &mut self,
        event: PluginActivationEvent,
        runtime: &mut dyn PluginRuntime,
    ) -> Vec<PluginActivationOutcome> {
        let cancellation = ActivationCancellation::new();
        self.trigger_event_with_runtime_and_cancel(event, runtime, &cancellation)
    }

    pub fn trigger_event_with_runtime_and_cancel(
        &mut self,
        event: PluginActivationEvent,
        runtime: &mut dyn PluginRuntime,
        cancellation: &ActivationCancellation,
    ) -> Vec<PluginActivationOutcome> {
        let mut candidates: Vec<String> = self
            .manifests
            .iter()
            .filter(|(_, manifest)| {
                manifest
                    .activation_events
                    .iter()
                    .any(|registered| registered.matches_trigger(&event))
            })
            .map(|(id, _)| id.clone())
            .collect();
        candidates.sort();

        let mut outcomes = Vec::new();
        for plugin_id in candidates {
            let Some(manifest) = self.manifests.get(&plugin_id).cloned() else {
                continue;
            };

            let activation_spec = RuntimeActivationSpec {
                timeout_ms: self.policy.activation_timeout_ms.max(10),
            };

            let allowed_capabilities = allowed_capability_tags(self.policy);
            if let Some(disallowed) = manifest
                .capabilities
                .iter()
                .copied()
                .map(PluginCapability::as_tag)
                .find(|cap| !allowed_capabilities.contains(*cap))
            {
                let runtime_state = self.runtime_by_id.entry(plugin_id.clone()).or_default();
                runtime_state.failed_attempts += 1;
                runtime_state.last_error =
                    Some(format!("capability blocked by host policy: {disallowed}"));
                runtime_state.state =
                    if runtime_state.failed_attempts >= self.policy.max_failed_activations.max(1) {
                        PluginLifecycleState::Disabled
                    } else {
                        PluginLifecycleState::Failed
                    };

                outcomes.push(PluginActivationOutcome {
                    plugin_id,
                    state: runtime_state.state,
                    activated: false,
                    error: runtime_state.last_error.clone(),
                    elapsed_ms: 0,
                });
                continue;
            }

            let runtime_state = self.runtime_by_id.entry(plugin_id.clone()).or_default();
            if runtime_state.state == PluginLifecycleState::Active {
                outcomes.push(PluginActivationOutcome {
                    plugin_id,
                    state: PluginLifecycleState::Active,
                    activated: false,
                    error: None,
                    elapsed_ms: 0,
                });
                continue;
            }

            if runtime_state.failed_attempts >= self.policy.max_failed_activations.max(1) {
                runtime_state.state = PluginLifecycleState::Disabled;
                outcomes.push(PluginActivationOutcome {
                    plugin_id,
                    state: PluginLifecycleState::Disabled,
                    activated: false,
                    error: runtime_state.last_error.clone(),
                    elapsed_ms: 0,
                });
                continue;
            }

            runtime_state.state = PluginLifecycleState::Activating;
            runtime_state.last_trigger = Some(event.clone());

            let result =
                run_host_activation(runtime, &manifest, &event, activation_spec, cancellation);

            match result.status {
                RuntimeStatus::Ready => {
                    runtime_state.state = PluginLifecycleState::Active;
                    runtime_state.activation_count += 1;
                    runtime_state.failed_attempts = 0;
                    runtime_state.last_error = None;
                    runtime_state.total_activation_ms += result.elapsed_ms;

                    outcomes.push(PluginActivationOutcome {
                        plugin_id,
                        state: PluginLifecycleState::Active,
                        activated: true,
                        error: None,
                        elapsed_ms: result.elapsed_ms,
                    });
                }
                RuntimeStatus::Failed(err) => {
                    let err_text = err.to_string();
                    runtime_state.failed_attempts += 1;
                    runtime_state.last_error = Some(err_text.clone());
                    runtime_state.total_activation_ms += result.elapsed_ms;
                    if runtime_state.failed_attempts >= self.policy.max_failed_activations.max(1) {
                        runtime_state.state = PluginLifecycleState::Disabled;
                    } else {
                        runtime_state.state = PluginLifecycleState::Failed;
                    }

                    outcomes.push(PluginActivationOutcome {
                        plugin_id,
                        state: runtime_state.state,
                        activated: false,
                        error: Some(err_text),
                        elapsed_ms: result.elapsed_ms,
                    });
                }
                RuntimeStatus::Cancelled => {
                    runtime_state.cancelled_attempts += 1;
                    runtime_state.state = PluginLifecycleState::Cancelled;
                    runtime_state.last_error = Some(format!(
                        "activation cancelled/timeout (>{}ms)",
                        activation_spec.timeout_ms
                    ));
                    runtime_state.total_activation_ms += result.elapsed_ms;

                    outcomes.push(PluginActivationOutcome {
                        plugin_id,
                        state: PluginLifecycleState::Cancelled,
                        activated: false,
                        error: runtime_state.last_error.clone(),
                        elapsed_ms: result.elapsed_ms,
                    });
                }
            }
        }

        outcomes
    }
}

fn validate_manifest(manifest: &PluginManifest, policy: PluginPolicy) -> Result<(), String> {
    if manifest.id.trim().is_empty() {
        return Err("plugin id is required".to_string());
    }
    if manifest.display_name.trim().is_empty() {
        return Err("plugin display name is required".to_string());
    }
    if manifest.version.trim().is_empty() {
        return Err("plugin version is required".to_string());
    }
    if manifest.activation_events.is_empty() {
        return Err("plugin activation events are required".to_string());
    }

    let capability_set: HashSet<PluginCapability> = manifest.capabilities.iter().copied().collect();
    if !policy.allow_network && capability_set.contains(&PluginCapability::Network) {
        return Err("network capability is blocked by policy".to_string());
    }

    if !manifest.command_allowlist.is_empty()
        && !capability_set.contains(&PluginCapability::Commands)
    {
        return Err("command allowlist requires Commands capability".to_string());
    }

    for event in &manifest.activation_events {
        if let PluginActivationEvent::OnCommand(command) = event {
            if !capability_set.contains(&PluginCapability::Commands) {
                return Err("OnCommand activation requires Commands capability".to_string());
            }

            if !manifest.command_allowlist.is_empty()
                && !manifest.command_allowlist.contains(command)
            {
                return Err(
                    "OnCommand activation must be included in command allowlist".to_string()
                );
            }
        }
    }

    Ok(())
}

fn allowed_capability_tags(policy: PluginPolicy) -> HashSet<&'static str> {
    let mut allowed = HashSet::new();
    allowed.insert(PluginCapability::Commands.as_tag());
    allowed.insert(PluginCapability::ReadVault.as_tag());
    allowed.insert(PluginCapability::WriteVault.as_tag());
    if policy.allow_network {
        allowed.insert(PluginCapability::Network.as_tag());
    }
    allowed
}

pub fn run_host_activation(
    runtime: &mut dyn PluginRuntime,
    manifest: &PluginManifest,
    trigger: &PluginActivationEvent,
    spec: RuntimeActivationSpec,
    cancellation: &ActivationCancellation,
) -> RuntimeActivationResult {
    let started_at = Instant::now();

    if cancellation.is_cancelled() {
        return RuntimeActivationResult {
            status: RuntimeStatus::Cancelled,
            elapsed_ms: 0,
        };
    }

    let mut result = runtime.activate(manifest, trigger, spec, cancellation);
    if result.elapsed_ms == 0 {
        result.elapsed_ms = started_at.elapsed().as_millis();
    }
    if cancellation.is_cancelled() || result.elapsed_ms > spec.timeout_ms as u128 {
        result.status = RuntimeStatus::Cancelled;
    }
    result
}

pub fn default_activation_timeout() -> Duration {
    Duration::from_millis(PluginPolicy::default().activation_timeout_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protocol_runtime_config(activate_delay_ms: u64, activate_ok: bool) -> ProcessRuntimeConfig {
        let mut config = ProcessRuntimeConfig::new(
            "cargo",
            vec![
                "run".to_string(),
                "-q".to_string(),
                "-p".to_string(),
                "xnote-plugin-worker".to_string(),
                "--".to_string(),
            ],
        );
        config.watchdog_interval_ms = 5;
        config.extra_env.insert(
            "XNOTE_PLUGIN_WORKER_DELAY_MS".to_string(),
            activate_delay_ms.to_string(),
        );
        config.extra_env.insert(
            "XNOTE_PLUGIN_WORKER_ACTIVATE_OK".to_string(),
            if activate_ok {
                "true".to_string()
            } else {
                "false".to_string()
            },
        );
        config
    }

    struct FakeRuntime {
        status: RuntimeStatus,
        elapsed_ms: u128,
    }

    impl PluginRuntime for FakeRuntime {
        fn activate(
            &mut self,
            _manifest: &PluginManifest,
            _trigger: &PluginActivationEvent,
            _spec: RuntimeActivationSpec,
            _cancellation: &ActivationCancellation,
        ) -> RuntimeActivationResult {
            RuntimeActivationResult {
                status: self.status.clone(),
                elapsed_ms: self.elapsed_ms,
            }
        }
    }

    fn base_manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.to_string(),
            display_name: id.to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::Commands],
            command_allowlist: vec![CommandId::QuickOpen],
            activation_events: vec![PluginActivationEvent::OnCommand(CommandId::QuickOpen)],
        }
    }

    #[test]
    fn process_runtime_config_negotiates_protocol_from_supported_set() {
        let mut config = ProcessRuntimeConfig::new("worker", Vec::new());
        config.protocol_version = 2;
        config.supported_protocol_versions = vec![3, 2, 1];

        assert_eq!(config.negotiate_protocol(3), Some(3));
        assert_eq!(config.negotiate_protocol(2), Some(2));
        assert_eq!(config.negotiate_protocol(1), Some(1));
        assert_eq!(config.negotiate_protocol(4), None);
        assert_eq!(config.negotiate_protocol(0), None);
    }

    #[test]
    fn process_runtime_config_normalizes_session_policy_values() {
        let mut config = ProcessRuntimeConfig::new("worker", Vec::new());
        config.session_ping_timeout_ms = 0;
        config.max_keep_alive_sessions = 0;
        config.session_idle_ttl_ms = 0;

        assert_eq!(config.normalized_session_ping_timeout_ms(), 10);
        assert_eq!(config.normalized_max_keep_alive_sessions(), 1);
        assert_eq!(config.normalized_session_idle_ttl_ms(), 100);
    }

    #[test]
    fn registry_blocks_network_when_policy_disables_it() {
        let mut registry = PluginRegistry::default();
        let result = registry.register_manifest(PluginManifest {
            id: "x.network".to_string(),
            display_name: "Network".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::Network],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        });
        assert!(result.is_err());
    }

    #[test]
    fn registry_accepts_commands_allowlist_when_capability_present() {
        let mut registry = PluginRegistry::default();
        let result = registry.register_manifest(base_manifest("x.quick_open"));
        assert!(result.is_ok());
    }

    #[test]
    fn startup_event_activates_plugin() {
        let mut registry = PluginRegistry::default();
        registry
            .register_manifest(PluginManifest {
                id: "x.startup".to_string(),
                display_name: "Startup".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::ReadVault],
                command_allowlist: vec![],
                activation_events: vec![PluginActivationEvent::OnStartupFinished],
            })
            .expect("register manifest");

        let outcomes = registry.trigger_event(PluginActivationEvent::OnStartupFinished);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].activated);
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn command_event_only_activates_matching_plugin() {
        let mut registry = PluginRegistry::default();
        registry
            .register_manifest(base_manifest("x.quick_open"))
            .expect("register quick open plugin");
        registry
            .register_manifest(PluginManifest {
                id: "x.save_file".to_string(),
                display_name: "Save File".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::Commands],
                command_allowlist: vec![CommandId::SaveFile],
                activation_events: vec![PluginActivationEvent::OnCommand(CommandId::SaveFile)],
            })
            .expect("register save plugin");

        let outcomes =
            registry.trigger_event(PluginActivationEvent::OnCommand(CommandId::QuickOpen));
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].plugin_id, "x.quick_open");
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn plugin_disabled_after_repeated_activation_failures() {
        let mut registry = PluginRegistry::with_policy(PluginPolicy {
            allow_network: false,
            max_failed_activations: 2,
            activation_timeout_ms: 2000,
        });
        registry
            .register_manifest(PluginManifest {
                id: "x.flaky".to_string(),
                display_name: "Flaky".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::ReadVault],
                command_allowlist: vec![],
                activation_events: vec![PluginActivationEvent::OnStartupFinished],
            })
            .expect("register plugin");

        let mut runtime = FakeRuntime {
            status: runtime_failed(RuntimeErrorCode::ActivationRejected, "activation failed"),
            elapsed_ms: 20,
        };

        let _ = registry
            .trigger_event_with_runtime(PluginActivationEvent::OnStartupFinished, &mut runtime);
        let _ = registry
            .trigger_event_with_runtime(PluginActivationEvent::OnStartupFinished, &mut runtime);

        let runtime_state = registry.runtime("x.flaky").expect("runtime state exists");
        assert_eq!(runtime_state.state, PluginLifecycleState::Disabled);
        assert_eq!(runtime_state.failed_attempts, 2);
    }

    #[test]
    fn timeout_cancels_activation() {
        let mut registry = PluginRegistry::with_policy(PluginPolicy {
            allow_network: false,
            max_failed_activations: 3,
            activation_timeout_ms: 50,
        });
        registry
            .register_manifest(PluginManifest {
                id: "x.slow".to_string(),
                display_name: "Slow".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::ReadVault],
                command_allowlist: vec![],
                activation_events: vec![PluginActivationEvent::OnStartupFinished],
            })
            .expect("register plugin");

        let mut runtime = FakeRuntime {
            status: RuntimeStatus::Ready,
            elapsed_ms: 120,
        };

        let outcomes = registry
            .trigger_event_with_runtime(PluginActivationEvent::OnStartupFinished, &mut runtime);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].state, PluginLifecycleState::Cancelled);
        assert!(!outcomes[0].activated);
    }

    #[test]
    fn host_boundary_function_applies_timeout() {
        let manifest = PluginManifest {
            id: "x.boundary".to_string(),
            display_name: "Boundary".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let mut runtime = FakeRuntime {
            status: RuntimeStatus::Ready,
            elapsed_ms: 100,
        };
        let cancellation = ActivationCancellation::new();

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 10 },
            &cancellation,
        );

        assert_eq!(result.status, RuntimeStatus::Cancelled);
    }

    #[test]
    fn cancellation_token_short_circuits_host_activation() {
        let manifest = PluginManifest {
            id: "x.cancel".to_string(),
            display_name: "Cancel".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let mut runtime = FakeRuntime {
            status: RuntimeStatus::Ready,
            elapsed_ms: 1,
        };
        let cancellation = ActivationCancellation::new();
        cancellation.cancel();

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 2000 },
            &cancellation,
        );

        assert_eq!(result.status, RuntimeStatus::Cancelled);
    }

    #[test]
    fn process_runtime_succeeds_with_protocol_handshake() {
        let manifest = PluginManifest {
            id: "x.process.ok".to_string(),
            display_name: "Process OK".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();
        let mut runtime = ProcessPluginRuntime::new(protocol_runtime_config(0, true));

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 20_000 },
            &cancellation,
        );

        assert_eq!(result.status, RuntimeStatus::Ready);
    }

    #[test]
    fn process_runtime_can_report_activation_failure_via_protocol() {
        let manifest = PluginManifest {
            id: "x.process.runtime_error".to_string(),
            display_name: "Process Runtime Error".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();
        let mut runtime = ProcessPluginRuntime::new(protocol_runtime_config(0, false));

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 20_000 },
            &cancellation,
        );

        assert!(matches!(
            result.status,
            RuntimeStatus::Failed(ref e) if e.code == RuntimeErrorCode::ActivationRejected
        ));
    }

    #[test]
    fn process_runtime_detects_protocol_version_mismatch() {
        let manifest = PluginManifest {
            id: "x.process.protocol_mismatch".to_string(),
            display_name: "Process Protocol Mismatch".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();
        let mut config = protocol_runtime_config(0, true);
        config.extra_env.insert(
            "XNOTE_PLUGIN_WORKER_PROTOCOL_VERSION".to_string(),
            "999".to_string(),
        );
        let mut runtime = ProcessPluginRuntime::new(config);

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 20_000 },
            &cancellation,
        );

        assert!(matches!(
            result.status,
            RuntimeStatus::Failed(ref e) if e.code == RuntimeErrorCode::ProtocolMismatch
        ));
    }

    #[test]
    fn process_runtime_accepts_legacy_version_with_supported_set() {
        let manifest = PluginManifest {
            id: "x.process.protocol_legacy".to_string(),
            display_name: "Process Protocol Legacy".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();
        let mut config = protocol_runtime_config(0, true);
        config.protocol_version = 2;
        config.supported_protocol_versions = vec![2, 1];
        config.extra_env.insert(
            "XNOTE_PLUGIN_WORKER_PROTOCOL_VERSION".to_string(),
            "1".to_string(),
        );
        let mut runtime = ProcessPluginRuntime::new(config);

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 20_000 },
            &cancellation,
        );

        assert_eq!(result.status, RuntimeStatus::Ready);
    }

    #[test]
    fn process_runtime_rejects_undeclared_worker_capability() {
        let manifest = PluginManifest {
            id: "x.process.cap_mismatch".to_string(),
            display_name: "Process Capability Mismatch".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();
        let mut config = protocol_runtime_config(0, true);
        config.extra_env.insert(
            "XNOTE_PLUGIN_WORKER_REPORTED_CAPS".to_string(),
            "read_vault,network".to_string(),
        );
        let mut runtime = ProcessPluginRuntime::new(config);

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 20_000 },
            &cancellation,
        );

        assert!(matches!(result.status, RuntimeStatus::Failed(_)));
        assert!(matches!(
            result.status,
            RuntimeStatus::Failed(ref e)
                if e.code == RuntimeErrorCode::CapabilityViolation
                    && e.detail.contains("undeclared capabilities")
        ));
    }

    #[test]
    fn process_runtime_rejects_policy_blocked_worker_capability() {
        let manifest = PluginManifest {
            id: "x.process.policy_blocked".to_string(),
            display_name: "Process Policy Blocked".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault, PluginCapability::Network],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let mut config = protocol_runtime_config(0, true);
        config.extra_env.insert(
            "XNOTE_PLUGIN_WORKER_REPORTED_CAPS".to_string(),
            "read_vault,network".to_string(),
        );
        let mut runtime = ProcessPluginRuntime::new(config);

        let mut registry = PluginRegistry::with_policy(PluginPolicy {
            allow_network: false,
            max_failed_activations: 3,
            activation_timeout_ms: 20_000,
        });
        registry
            .register_manifest(manifest)
            .expect_err("registration should fail when network is blocked");

        let manifest = PluginManifest {
            id: "x.process.policy_blocked.allowed_register".to_string(),
            display_name: "Process Policy Blocked Register".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault, PluginCapability::Network],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let mut registry = PluginRegistry::with_policy(PluginPolicy {
            allow_network: true,
            max_failed_activations: 3,
            activation_timeout_ms: 20_000,
        });
        registry
            .register_manifest(manifest)
            .expect("register when network allowed");
        registry.set_policy(PluginPolicy {
            allow_network: false,
            max_failed_activations: 3,
            activation_timeout_ms: 20_000,
        });

        let outcomes = registry
            .trigger_event_with_runtime(PluginActivationEvent::OnStartupFinished, &mut runtime);
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].state, PluginLifecycleState::Failed));
        assert!(
            outcomes[0]
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("host policy")
                || outcomes[0]
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .contains("policy-blocked")
        );
    }

    #[test]
    fn trigger_event_blocks_disallowed_capability_by_policy() {
        let mut registry = PluginRegistry::with_policy(PluginPolicy {
            allow_network: false,
            max_failed_activations: 3,
            activation_timeout_ms: 2000,
        });
        registry
            .register_manifest(PluginManifest {
                id: "x.policy.net".to_string(),
                display_name: "Policy Net".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::Network],
                command_allowlist: vec![],
                activation_events: vec![PluginActivationEvent::OnStartupFinished],
            })
            .expect_err("registration should fail by manifest validator");

        let mut registry = PluginRegistry::with_policy(PluginPolicy {
            allow_network: true,
            max_failed_activations: 3,
            activation_timeout_ms: 2000,
        });
        registry
            .register_manifest(PluginManifest {
                id: "x.policy.net.allowed".to_string(),
                display_name: "Policy Net Allowed".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::Network],
                command_allowlist: vec![],
                activation_events: vec![PluginActivationEvent::OnStartupFinished],
            })
            .expect("register plugin");

        registry.set_policy(PluginPolicy {
            allow_network: false,
            max_failed_activations: 3,
            activation_timeout_ms: 2000,
        });

        let outcomes = registry.trigger_event(PluginActivationEvent::OnStartupFinished);
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].state, PluginLifecycleState::Failed));
        assert!(outcomes[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("capability blocked by host policy"));
    }

    #[test]
    fn process_runtime_reports_spawn_failure() {
        let manifest = PluginManifest {
            id: "x.process.fail".to_string(),
            display_name: "Process Fail".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();
        let mut runtime = ProcessPluginRuntime::new(ProcessRuntimeConfig::new(
            "xnote-non-existent-runtime-command",
            Vec::new(),
        ));

        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 500 },
            &cancellation,
        );

        assert!(matches!(
            result.status,
            RuntimeStatus::Failed(ref e) if e.code == RuntimeErrorCode::SpawnFailed
        ));
    }

    #[test]
    fn process_runtime_timeout_results_in_cancelled() {
        let manifest = PluginManifest {
            id: "x.process.timeout".to_string(),
            display_name: "Process Timeout".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![PluginCapability::ReadVault],
            command_allowlist: vec![],
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
        };
        let trigger = PluginActivationEvent::OnStartupFinished;
        let cancellation = ActivationCancellation::new();

        let config = protocol_runtime_config(120, true);

        let mut runtime = ProcessPluginRuntime::new(config);
        let result = run_host_activation(
            &mut runtime,
            &manifest,
            &trigger,
            RuntimeActivationSpec { timeout_ms: 30 },
            &cancellation,
        );

        assert_eq!(result.status, RuntimeStatus::Cancelled);
    }
}
