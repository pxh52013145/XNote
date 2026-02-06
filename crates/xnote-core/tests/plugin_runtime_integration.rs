use xnote_core::plugin::{
    run_host_activation, ActivationCancellation, PluginActivationEvent, PluginCapability,
    PluginManifest, ProcessPluginRuntime, ProcessRuntimeConfig, RuntimeActivationSpec,
    RuntimeErrorCode, RuntimeStatus,
};

fn worker_runtime_config(delay_ms: u64, activate_ok: bool) -> ProcessRuntimeConfig {
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
        delay_ms.to_string(),
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

fn base_manifest() -> PluginManifest {
    PluginManifest {
        id: "x.integration.worker".to_string(),
        display_name: "Integration Worker".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec![PluginCapability::ReadVault],
        command_allowlist: vec![],
        activation_events: vec![PluginActivationEvent::OnStartupFinished],
    }
}

#[test]
fn process_runtime_worker_integration_success() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut runtime = ProcessPluginRuntime::new(worker_runtime_config(0, true));

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
fn process_runtime_worker_integration_cancelled_by_timeout() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut runtime = ProcessPluginRuntime::new(worker_runtime_config(200, true));

    let result = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 30 },
        &cancellation,
    );

    assert_eq!(result.status, RuntimeStatus::Cancelled);
}

#[test]
fn process_runtime_worker_integration_activation_rejected_code() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut runtime = ProcessPluginRuntime::new(worker_runtime_config(0, false));

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
fn process_runtime_worker_integration_keep_alive_session_reuse() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
    config.keep_alive_session = true;
    let mut runtime = ProcessPluginRuntime::new(config);

    let first = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    let second = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );

    assert_eq!(first.status, RuntimeStatus::Ready);
    assert_eq!(second.status, RuntimeStatus::Ready);
}

#[test]
fn process_runtime_worker_integration_protocol_negotiation_accepts_legacy_runtime() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
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
fn process_runtime_worker_integration_session_limit_eviction() {
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
    config.keep_alive_session = true;
    config.max_keep_alive_sessions = 1;
    let mut runtime = ProcessPluginRuntime::new(config);

    let manifest_a = PluginManifest {
        id: "x.integration.worker.a".to_string(),
        ..base_manifest()
    };
    let manifest_b = PluginManifest {
        id: "x.integration.worker.b".to_string(),
        ..base_manifest()
    };

    let result_a = run_host_activation(
        &mut runtime,
        &manifest_a,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    let result_b = run_host_activation(
        &mut runtime,
        &manifest_b,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );

    assert_eq!(result_a.status, RuntimeStatus::Ready);
    assert_eq!(result_b.status, RuntimeStatus::Ready);
    assert_eq!(runtime.active_session_count(), 1);
}

#[test]
fn process_runtime_worker_integration_idle_ttl_eviction() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
    config.keep_alive_session = true;
    config.session_idle_ttl_ms = 100;
    let mut runtime = ProcessPluginRuntime::new(config);

    let first = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    assert_eq!(first.status, RuntimeStatus::Ready);
    assert_eq!(runtime.active_session_count(), 1);

    std::thread::sleep(std::time::Duration::from_millis(150));

    let second = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    assert_eq!(second.status, RuntimeStatus::Ready);
    assert_eq!(runtime.active_session_count(), 1);
}

#[test]
fn process_runtime_worker_integration_session_snapshot_exposed() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
    config.keep_alive_session = true;
    let mut runtime = ProcessPluginRuntime::new(config);

    let result = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    assert_eq!(result.status, RuntimeStatus::Ready);

    let snapshot = runtime.active_sessions_snapshot();
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot[0]
        .session_key
        .contains("x.integration.worker:0.1.0:read_vault"));
}

#[test]
fn process_runtime_worker_integration_telemetry_counts_spawn_reuse_and_activation() {
    let manifest = base_manifest();
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
    config.keep_alive_session = true;
    let mut runtime = ProcessPluginRuntime::new(config);

    let first = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    let second = run_host_activation(
        &mut runtime,
        &manifest,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );

    assert_eq!(first.status, RuntimeStatus::Ready);
    assert_eq!(second.status, RuntimeStatus::Ready);

    let telemetry = runtime.telemetry_snapshot();
    assert_eq!(telemetry.spawn_count, 1);
    assert_eq!(telemetry.handshake_count, 1);
    assert_eq!(telemetry.activation_request_count, 2);
    assert_eq!(telemetry.reused_session_count, 1);
}

#[test]
fn process_runtime_worker_integration_telemetry_counts_limit_eviction() {
    let trigger = PluginActivationEvent::OnStartupFinished;
    let cancellation = ActivationCancellation::new();
    let mut config = worker_runtime_config(0, true);
    config.keep_alive_session = true;
    config.max_keep_alive_sessions = 1;
    let mut runtime = ProcessPluginRuntime::new(config);

    let manifest_a = PluginManifest {
        id: "x.integration.telemetry.a".to_string(),
        ..base_manifest()
    };
    let manifest_b = PluginManifest {
        id: "x.integration.telemetry.b".to_string(),
        ..base_manifest()
    };

    let first = run_host_activation(
        &mut runtime,
        &manifest_a,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );
    let second = run_host_activation(
        &mut runtime,
        &manifest_b,
        &trigger,
        RuntimeActivationSpec { timeout_ms: 20_000 },
        &cancellation,
    );

    assert_eq!(first.status, RuntimeStatus::Ready);
    assert_eq!(second.status, RuntimeStatus::Ready);

    let telemetry = runtime.telemetry_snapshot();
    assert_eq!(telemetry.evicted_by_limit_count, 1);
}
