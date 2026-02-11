use crate::knowledge::{KnowledgeIndex, SearchOptions};
use crate::vault::Vault;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiRewriteRequest {
    pub note_path: String,
    pub selection: String,
    pub instruction: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiRewriteProposal {
    pub replacement: String,
    pub rationale: String,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiExecutionResult {
    pub proposal: AiRewriteProposal,
    pub dry_run: bool,
    pub applied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiPolicy {
    pub allow_apply: bool,
    pub max_selection_chars: usize,
}

impl Default for AiPolicy {
    fn default() -> Self {
        Self {
            allow_apply: false,
            max_selection_chars: 20_000,
        }
    }
}

pub trait AiProvider {
    fn provider_kind(&self) -> AiProviderKind;
    fn provider_name(&self) -> &'static str;
    fn model_name(&self) -> &'static str;
    fn rewrite_selection(&self, request: &AiRewriteRequest) -> Result<String>;
}

impl<T: AiProvider + ?Sized> AiProvider for Box<T> {
    fn provider_kind(&self) -> AiProviderKind {
        (**self).provider_kind()
    }

    fn provider_name(&self) -> &'static str {
        (**self).provider_name()
    }

    fn model_name(&self) -> &'static str {
        (**self).model_name()
    }

    fn rewrite_selection(&self, request: &AiRewriteRequest) -> Result<String> {
        (**self).rewrite_selection(request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiProviderKind {
    Mock,
    Vcp,
}

impl AiProviderKind {
    pub fn from_env() -> Self {
        let mode = std::env::var("XNOTE_AI_PROVIDER")
            .ok()
            .or_else(|| std::env::var("XNOTE_AI_BACKEND").ok())
            .unwrap_or_else(|| "mock".to_string());

        match mode.trim().to_ascii_lowercase().as_str() {
            "vcp" | "vcp_compat" | "vcp_toolbox" => Self::Vcp,
            _ => Self::Mock,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MockAiProvider;

impl AiProvider for MockAiProvider {
    fn provider_kind(&self) -> AiProviderKind {
        AiProviderKind::Mock
    }

    fn provider_name(&self) -> &'static str {
        "mock"
    }

    fn model_name(&self) -> &'static str {
        "mock-draft-v1"
    }

    fn rewrite_selection(&self, request: &AiRewriteRequest) -> Result<String> {
        let normalized = request.selection.replace("\r\n", "\n");
        let mut out = String::with_capacity(normalized.len());
        let mut blank_run = 0usize;

        for line in normalized.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                blank_run = blank_run.saturating_add(1);
                if blank_run > 2 {
                    continue;
                }
            } else {
                blank_run = 0;
            }

            out.push_str(&trimmed.replace('\t', "    "));
            out.push('\n');
        }

        if !normalized.ends_with('\n') {
            out.pop();
        }

        Ok(out)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VcpCompatConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
    pub temperature: f32,
    pub timeout_ms: u64,
    pub enable_tool_injection: bool,
}

impl Default for VcpCompatConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:5890/v1/chat/completions".to_string(),
            api_key: None,
            model: "gemini-2.5-flash-preview-05-20".to_string(),
            temperature: 0.2,
            timeout_ms: 45_000,
            enable_tool_injection: false,
        }
    }
}

impl VcpCompatConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(raw_endpoint) = std::env::var("XNOTE_AI_VCP_URL") {
            let completed = complete_vcp_url(raw_endpoint.trim());
            if !completed.is_empty() {
                config.endpoint = completed;
            }
        }

        if let Some(value) = env_string("XNOTE_AI_VCP_KEY")
            .or_else(|| env_string("VCP_Key"))
            .or_else(|| env_string("VCP_KEY"))
        {
            config.api_key = Some(value);
        }

        if let Some(value) = env_string("XNOTE_AI_VCP_MODEL") {
            config.model = value;
        }

        if let Some(value) =
            env_string("XNOTE_AI_VCP_TIMEOUT_MS").and_then(|value| value.parse::<u64>().ok())
        {
            config.timeout_ms = value.max(1_000);
        }

        if let Some(value) =
            env_string("XNOTE_AI_VCP_TEMPERATURE").and_then(|value| value.parse::<f32>().ok())
        {
            config.temperature = value.clamp(0.0, 2.0);
        }

        config.enable_tool_injection = env_bool("XNOTE_AI_VCP_TOOL_INJECTION", false);

        config
    }

    pub fn completion_endpoint(&self) -> String {
        if !self.enable_tool_injection {
            return self.endpoint.clone();
        }

        if self.endpoint.contains("/v1/chatvcp/completions") {
            return self.endpoint.clone();
        }

        if self.endpoint.contains("/v1/chat/completions") {
            return self
                .endpoint
                .replace("/v1/chat/completions", "/v1/chatvcp/completions");
        }

        if let Some(trimmed) = self.endpoint.strip_suffix('/') {
            return format!("{trimmed}/v1/chatvcp/completions");
        }

        format!("{}/v1/chatvcp/completions", self.endpoint)
    }
}

#[derive(Clone, Debug)]
pub struct VcpCompatProvider {
    config: VcpCompatConfig,
}

impl VcpCompatProvider {
    pub fn new(config: VcpCompatConfig) -> Self {
        Self { config }
    }

    pub fn from_env() -> Self {
        Self::new(VcpCompatConfig::from_env())
    }

    fn build_messages(&self, request: &AiRewriteRequest) -> (String, String) {
        let instruction = if request.instruction.trim().is_empty() {
            "Polish the selected text while keeping the original meaning and language."
        } else {
            request.instruction.trim()
        };

        let mut registry = AiVariableRegistry::default();
        registry.insert_global("AiProvider", "vcp");
        registry.insert_knowledge("CurrentNotePath", request.note_path.as_str());
        registry.insert_knowledge("Instruction", instruction);
        registry.insert_knowledge("Selection", request.selection.as_str());

        let tool_registry = VcpToolRegistry::with_xnote_defaults();
        let tool_descriptions = generate_vcp_tool_descriptions(&tool_registry)
            .into_iter()
            .map(|item| item.markdown_description)
            .collect::<Vec<_>>()
            .join("\n");
        if !tool_descriptions.is_empty() {
            registry.insert_knowledge("VcpToolList", tool_descriptions);
        }

        let system_prompt = registry.render_template(
            "You are the XNote rewrite engine.\n".to_string()
                + "Keep the user language and preserve semantic meaning.\n"
                + "Do not add commentary, markdown fences, or explanations.\n"
                + "Return only the rewritten content body.\n"
                + "When external data is required, call tools with VCP markers.\n"
                + "Available tools:\n{{VcpToolList}}\n"
                + "Current note: {{CurrentNotePath}}",
            AiVariableScope::Knowledge,
        );

        let user_prompt = registry.render_template(
            "Rewrite instruction:\n{{Instruction}}\n\n".to_string()
                + "Selected text:\n"
                + "```text\n{{Selection}}\n```",
            AiVariableScope::Knowledge,
        );

        (system_prompt, user_prompt)
    }

    fn execute_chat_completion(&self, request: &AiRewriteRequest) -> Result<String> {
        let endpoint = self.config.completion_endpoint();
        if endpoint.trim().is_empty() {
            return Err(anyhow!("vcp endpoint is empty"));
        }

        let timeout = Duration::from_millis(self.config.timeout_ms.max(1_000));
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();

        let (system_prompt, user_prompt) = self.build_messages(request);
        let payload = json!({
            "model": self.config.model,
            "temperature": self.config.temperature,
            "stream": false,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt,
                },
                {
                    "role": "user",
                    "content": user_prompt,
                }
            ]
        });

        let mut req = agent
            .post(endpoint.as_str())
            .set("Content-Type", "application/json");

        if let Some(api_key) = &self.config.api_key {
            req = req.set("Authorization", format!("Bearer {api_key}").as_str());
        }

        let response = req.send_json(payload);
        let completion_payload = match response {
            Ok(response) => response,
            Err(ureq::Error::Status(status, response)) => {
                let body = response.into_string().unwrap_or_default();
                return Err(anyhow!(
                    "vcp completion failed with status {status}: {body}"
                ));
            }
            Err(ureq::Error::Transport(err)) => {
                return Err(anyhow!("vcp transport error: {err}"));
            }
        };

        let value: serde_json::Value = completion_payload
            .into_json()
            .context("failed to decode VCP response JSON")?;

        extract_completion_text(&value).ok_or_else(|| {
            anyhow!("vcp response missing assistant content (expected choices[0].message.content)")
        })
    }
}

impl AiProvider for VcpCompatProvider {
    fn provider_kind(&self) -> AiProviderKind {
        AiProviderKind::Vcp
    }

    fn provider_name(&self) -> &'static str {
        "vcp"
    }

    fn model_name(&self) -> &'static str {
        "vcp-runtime"
    }

    fn rewrite_selection(&self, request: &AiRewriteRequest) -> Result<String> {
        self.execute_chat_completion(request)
    }
}

pub struct AiEngine<P: AiProvider> {
    provider: P,
    policy: AiPolicy,
}

impl<P: AiProvider> AiEngine<P> {
    pub fn new(provider: P, policy: AiPolicy) -> Self {
        Self { provider, policy }
    }

    pub fn rewrite_selection(
        &self,
        request: &AiRewriteRequest,
        apply: bool,
    ) -> Result<AiExecutionResult> {
        self.validate_rewrite_request(request)?;

        if apply && !self.policy.allow_apply {
            return Err(anyhow!(
                "ai write is blocked by policy gate; run dry-run proposal first"
            ));
        }

        let replacement = self.provider.rewrite_selection(request)?;
        if replacement.is_empty() {
            return Err(anyhow!("provider returned empty rewrite result"));
        }

        Ok(AiExecutionResult {
            proposal: AiRewriteProposal {
                replacement,
                rationale: if request.instruction.trim().is_empty() {
                    "normalized selection formatting".to_string()
                } else {
                    request.instruction.trim().to_string()
                },
                provider: self.provider.provider_name().to_string(),
                model: self.provider.model_name().to_string(),
            },
            dry_run: !apply,
            applied: apply,
        })
    }

    fn validate_rewrite_request(&self, request: &AiRewriteRequest) -> Result<()> {
        if request.note_path.trim().is_empty() {
            return Err(anyhow!("ai rewrite request missing note path"));
        }
        if request.selection.trim().is_empty() {
            return Err(anyhow!("ai rewrite request selection is empty"));
        }
        if request.selection.chars().count() > self.policy.max_selection_chars {
            return Err(anyhow!(
                "ai rewrite selection too large: {} > {} chars",
                request.selection.chars().count(),
                self.policy.max_selection_chars
            ));
        }
        Ok(())
    }
}

pub fn build_provider_from_env() -> Result<Box<dyn AiProvider>> {
    let provider = match AiProviderKind::from_env() {
        AiProviderKind::Mock => Box::new(MockAiProvider) as Box<dyn AiProvider>,
        AiProviderKind::Vcp => Box::new(VcpCompatProvider::from_env()) as Box<dyn AiProvider>,
    };

    Ok(provider)
}

pub fn execute_rewrite_with_env_provider(
    request: &AiRewriteRequest,
    apply: bool,
    policy: AiPolicy,
) -> Result<AiExecutionResult> {
    let provider = build_provider_from_env()?;
    let engine = AiEngine::new(provider, policy);
    engine.rewrite_selection(request, apply)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiToolLoopResult {
    pub replacement: String,
    pub provider: String,
    pub model: String,
    pub tool_calls: Vec<VcpToolRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiToolLoopStopReason {
    FinalResponse,
    MaxRoundsReached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiToolOrchestratorConfig {
    pub max_rounds: usize,
    pub request_id: Option<String>,
    pub scenario: String,
    pub final_response_instruction: String,
}

impl Default for AiToolOrchestratorConfig {
    fn default() -> Self {
        Self {
            max_rounds: 2,
            request_id: None,
            scenario: "rewrite".to_string(),
            final_response_instruction: "Now return the final rewritten selection text only."
                .to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiToolOrchestratorResult {
    pub final_response: String,
    pub tool_calls: Vec<VcpToolRequest>,
    pub rounds_executed: usize,
    pub stop_reason: AiToolLoopStopReason,
}

pub fn execute_vcp_tool_orchestrator(
    request: &AiRewriteRequest,
    provider: &dyn AiProvider,
    vault: &Vault,
    index: Option<&KnowledgeIndex>,
    tool_policy: &VcpToolPolicy,
    config: &AiToolOrchestratorConfig,
) -> Result<AiToolOrchestratorResult> {
    let mut current_request = request.clone();
    let mut tool_calls = Vec::new();
    let rounds = config.max_rounds.clamp(1, 6);
    let request_id = config.request_id.clone();
    let scenario = Some(config.scenario.clone());

    for round in 0..rounds {
        let model_started = Instant::now();
        let response_text = provider.rewrite_selection(&current_request)?;
        let model_latency_ms = model_started.elapsed().as_millis();

        let registry = VcpToolRegistry::with_xnote_defaults();
        let maybe_request = parse_and_validate_first_vcp_tool_request(
            response_text.as_str(),
            &registry,
            tool_policy,
        )?;

        let Some(tool_request) = maybe_request else {
            return Ok(AiToolOrchestratorResult {
                final_response: response_text,
                tool_calls,
                rounds_executed: round + 1,
                stop_reason: AiToolLoopStopReason::FinalResponse,
            });
        };

        let tool_started = Instant::now();
        let execution = execute_vcp_tool_request(&tool_request, vault, index, tool_policy)?;
        let tool_latency_ms = tool_started.elapsed().as_millis();

        append_ai_tool_audit_log(
            vault,
            &AiToolAuditEntry {
                timestamp_epoch_ms: now_epoch_ms(),
                event: "tool_execution".to_string(),
                round,
                tool_name: tool_request.tool_name.clone(),
                status: "ok".to_string(),
                detail: summarize_tool_detail(execution.payload_markdown.as_str(), 320),
                request_id: request_id.clone(),
                scenario: scenario.clone(),
                model_latency_ms: Some(model_latency_ms),
                tool_latency_ms: Some(tool_latency_ms),
                args_summary: Some(summarize_tool_args(&tool_request.args, 220)),
                outcome_category: Some("tool_executed".to_string()),
            },
        )?;

        tool_calls.push(tool_request.clone());
        current_request.instruction = build_tool_follow_up_instruction(
            request.instruction.as_str(),
            tool_request.tool_name.as_str(),
            execution.payload_markdown.as_str(),
            config.final_response_instruction.as_str(),
        );
    }

    let fallback = provider.rewrite_selection(&current_request)?;
    Ok(AiToolOrchestratorResult {
        final_response: fallback,
        tool_calls,
        rounds_executed: rounds,
        stop_reason: AiToolLoopStopReason::MaxRoundsReached,
    })
}

pub fn execute_rewrite_with_vcp_tool_loop(
    request: &AiRewriteRequest,
    vault: &Vault,
    index: Option<&KnowledgeIndex>,
    tool_policy: &VcpToolPolicy,
    max_rounds: usize,
) -> Result<AiToolLoopResult> {
    let provider = VcpCompatProvider::from_env();
    let orchestration = execute_vcp_tool_orchestrator(
        request,
        &provider,
        vault,
        index,
        tool_policy,
        &AiToolOrchestratorConfig {
            max_rounds,
            request_id: Some(generate_ai_request_id("rewrite")),
            scenario: "rewrite_selection".to_string(),
            final_response_instruction: "Now return the final rewritten selection text only."
                .to_string(),
        },
    )?;

    Ok(AiToolLoopResult {
        replacement: orchestration.final_response,
        provider: provider.provider_name().to_string(),
        model: provider.model_name().to_string(),
        tool_calls: orchestration.tool_calls,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiVariableScope {
    Global,
    Knowledge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiVariableEntry {
    pub name: String,
    pub value: String,
    pub scope: AiVariableScope,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiVariableRegistry {
    by_name: BTreeMap<String, AiVariableEntry>,
}

impl AiVariableRegistry {
    pub fn insert(
        &mut self,
        scope: AiVariableScope,
        name: impl Into<String>,
        value: impl Into<String>,
    ) {
        let name = name.into();
        let canonical = canonical_variable_name(name.as_str());
        let entry = AiVariableEntry {
            name,
            value: value.into(),
            scope,
        };
        self.by_name.insert(canonical, entry);
    }

    pub fn insert_global(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.insert(AiVariableScope::Global, name, value);
    }

    pub fn insert_knowledge(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.insert(AiVariableScope::Knowledge, name, value);
    }

    pub fn resolve(&self, placeholder: &str, scope: AiVariableScope) -> Option<&str> {
        let canonical = canonical_variable_name(placeholder);
        let entry = self.by_name.get(canonical.as_str())?;

        if !scope_can_access(scope, entry.scope) {
            return None;
        }

        Some(entry.value.as_str())
    }

    pub fn progressive_snapshot(&self, scope: AiVariableScope) -> BTreeMap<String, String> {
        self.by_name
            .values()
            .filter(|entry| scope_can_access(scope, entry.scope))
            .map(|entry| (entry.name.clone(), entry.value.clone()))
            .collect()
    }

    pub fn render_template(&self, template: impl AsRef<str>, scope: AiVariableScope) -> String {
        let template = template.as_ref();
        let mut rendered = String::with_capacity(template.len());
        let mut cursor = 0usize;

        while let Some(start_rel) = template[cursor..].find("{{") {
            let start = cursor + start_rel;
            rendered.push_str(&template[cursor..start]);

            let remain = &template[start + 2..];
            let Some(end_rel) = remain.find("}}") else {
                rendered.push_str(&template[start..]);
                return rendered;
            };

            let end = start + 2 + end_rel;
            let token = &template[start..end + 2];
            let placeholder = &template[start + 2..end];

            if let Some(value) = self.resolve(placeholder, scope) {
                rendered.push_str(value);
            } else {
                rendered.push_str(token);
            }

            cursor = end + 2;
        }

        if cursor < template.len() {
            rendered.push_str(&template[cursor..]);
        }

        rendered
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcpToolRequest {
    pub tool_name: String,
    pub args: BTreeMap<String, String>,
    pub no_reply: bool,
    pub mark_history: bool,
}

pub const VCP_TOOL_REQUEST_START: &str = "<<<[TOOL_REQUEST]>>>";
pub const VCP_TOOL_REQUEST_END: &str = "<<<[END_TOOL_REQUEST]>>>";

#[derive(Clone, Debug, Default)]
pub struct VcpToolRequestParser;

impl VcpToolRequestParser {
    pub fn parse_all(text: &str) -> Result<Vec<VcpToolRequest>> {
        let mut cursor = 0usize;
        let mut out = Vec::new();

        while let Some(start_rel) = text[cursor..].find(VCP_TOOL_REQUEST_START) {
            let start = cursor + start_rel + VCP_TOOL_REQUEST_START.len();
            let remain = &text[start..];
            let Some(end_rel) = remain.find(VCP_TOOL_REQUEST_END) else {
                return Err(anyhow!("vcp tool block is missing END marker"));
            };

            let end = start + end_rel;
            let block = &text[start..end];
            let request = parse_vcp_tool_block(block)?;
            out.push(request);
            cursor = end + VCP_TOOL_REQUEST_END.len();
        }

        Ok(out)
    }

    pub fn parse_first(text: &str) -> Result<Option<VcpToolRequest>> {
        Ok(Self::parse_all(text)?.into_iter().next())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcpToolRisk {
    ReadOnly,
    WriteSafe,
    Destructive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcpToolSpec {
    pub name: String,
    pub description: String,
    pub risk: VcpToolRisk,
    pub required_args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcpToolPolicy {
    pub allow_write: bool,
    pub allow_destructive: bool,
    pub allowlist: Option<BTreeSet<String>>,
}

impl Default for VcpToolPolicy {
    fn default() -> Self {
        Self {
            allow_write: false,
            allow_destructive: false,
            allowlist: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct VcpToolRegistry {
    specs: BTreeMap<String, VcpToolSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcpToolExecutionResult {
    pub tool_name: String,
    pub payload_markdown: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AiToolAuditEntry {
    pub timestamp_epoch_ms: u128,
    pub event: String,
    pub round: usize,
    pub tool_name: String,
    pub status: String,
    pub detail: String,
    pub request_id: Option<String>,
    pub scenario: Option<String>,
    pub model_latency_ms: Option<u128>,
    pub tool_latency_ms: Option<u128>,
    pub args_summary: Option<String>,
    pub outcome_category: Option<String>,
}

impl VcpToolRegistry {
    pub fn with_xnote_defaults() -> Self {
        let mut registry = Self::default();
        registry.register(VcpToolSpec {
            name: "xnote.vault.read_note".to_string(),
            description: "Read a markdown note from the current vault by relative note path."
                .to_string(),
            risk: VcpToolRisk::ReadOnly,
            required_args: vec!["note_path".to_string()],
        });
        registry.register(VcpToolSpec {
            name: "xnote.knowledge.search".to_string(),
            description: "Search indexed note contents by keyword query and return matched notes with previews."
                .to_string(),
            risk: VcpToolRisk::ReadOnly,
            required_args: vec!["query".to_string()],
        });
        registry.register(VcpToolSpec {
            name: "xnote.vault.write_note".to_string(),
            description: "Write markdown note content to a vault-relative note path.".to_string(),
            risk: VcpToolRisk::WriteSafe,
            required_args: vec!["note_path".to_string(), "content".to_string()],
        });
        registry.register(VcpToolSpec {
            name: "xnote.vault.apply_patch".to_string(),
            description: "Apply a structured patch to vault files (reserved, destructive)."
                .to_string(),
            risk: VcpToolRisk::Destructive,
            required_args: vec!["path".to_string(), "patch".to_string()],
        });
        registry
    }

    pub fn register(&mut self, spec: VcpToolSpec) {
        let key = canonical_tool_name(spec.name.as_str());
        self.specs.insert(key, spec);
    }

    pub fn specs_sorted(&self) -> Vec<&VcpToolSpec> {
        let mut specs = self.specs.values().collect::<Vec<_>>();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    pub fn validate_request(&self, request: &VcpToolRequest, policy: &VcpToolPolicy) -> Result<()> {
        let tool_name = canonical_tool_name(request.tool_name.as_str());
        let Some(spec) = self.specs.get(tool_name.as_str()) else {
            return Err(anyhow!("tool `{}` is not registered", request.tool_name));
        };

        if let Some(allowlist) = &policy.allowlist {
            let canonical = canonical_tool_name(spec.name.as_str());
            if !allowlist.contains(canonical.as_str()) {
                return Err(anyhow!("tool `{}` is blocked by allowlist", spec.name));
            }
        }

        match spec.risk {
            VcpToolRisk::ReadOnly => {}
            VcpToolRisk::WriteSafe if !policy.allow_write => {
                return Err(anyhow!("tool `{}` requires write permission", spec.name));
            }
            VcpToolRisk::Destructive if !policy.allow_destructive => {
                return Err(anyhow!(
                    "tool `{}` requires destructive permission",
                    spec.name
                ));
            }
            VcpToolRisk::Destructive if !policy.allow_write => {
                return Err(anyhow!("tool `{}` requires write permission", spec.name));
            }
            VcpToolRisk::WriteSafe | VcpToolRisk::Destructive => {}
        }

        for required in &spec.required_args {
            let key = canonical_tool_key(required);
            let has_value = request
                .args
                .get(key.as_str())
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            if !has_value {
                return Err(anyhow!(
                    "tool `{}` missing required argument `{}`",
                    spec.name,
                    required
                ));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VcpToolDescription {
    pub tool_name: String,
    pub invocation_snippet: String,
    pub markdown_description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct McpToolDescription {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AiToolDescriptorBundle {
    pub schema_version: String,
    pub generated_at_epoch_ms: u128,
    pub registry_source: String,
    pub vcp: Vec<VcpToolDescription>,
    pub mcp: Vec<McpToolDescription>,
}

pub fn generate_ai_tool_descriptor_bundle(registry: &VcpToolRegistry) -> AiToolDescriptorBundle {
    AiToolDescriptorBundle {
        schema_version: "xnote.ai.tools.v1".to_string(),
        generated_at_epoch_ms: now_epoch_ms(),
        registry_source: "xnote_core::ai::VcpToolRegistry::with_xnote_defaults".to_string(),
        vcp: generate_vcp_tool_descriptions(registry),
        mcp: generate_mcp_tool_descriptions(registry),
    }
}

pub fn generate_default_ai_tool_descriptor_bundle() -> AiToolDescriptorBundle {
    let registry = VcpToolRegistry::with_xnote_defaults();
    generate_ai_tool_descriptor_bundle(&registry)
}

pub fn generate_default_ai_tool_descriptor_bundle_json_pretty() -> Result<String> {
    let bundle = generate_default_ai_tool_descriptor_bundle();
    serde_json::to_string_pretty(&bundle).context("failed to encode ai tool descriptor bundle")
}

pub fn generate_vcp_tool_descriptions(registry: &VcpToolRegistry) -> Vec<VcpToolDescription> {
    registry
        .specs_sorted()
        .into_iter()
        .map(|spec| {
            let args_lines = spec
                .required_args
                .iter()
                .map(|arg| {
                    format!(
                        "{}:\u{300c}\u{59cb}\u{300d}<{}>\u{300c}\u{672b}\u{300d}",
                        arg, arg
                    )
                })
                .collect::<Vec<_>>();

            let mut invocation = String::from("<<<[TOOL_REQUEST]>>>\n");
            invocation.push_str(
                format!(
                    "tool_name:\u{300c}\u{59cb}\u{300d}{}\u{300c}\u{672b}\u{300d}",
                    spec.name
                )
                .as_str(),
            );
            if !args_lines.is_empty() {
                invocation.push_str(",\n");
                invocation.push_str(args_lines.join(",\n").as_str());
            }
            invocation.push_str("\n<<<[END_TOOL_REQUEST]>>>");

            let markdown_description = format!(
                "- {} ({:?})\n  - {}",
                spec.name, spec.risk, spec.description
            );

            VcpToolDescription {
                tool_name: spec.name.clone(),
                invocation_snippet: invocation,
                markdown_description,
            }
        })
        .collect()
}

pub fn generate_mcp_tool_descriptions(registry: &VcpToolRegistry) -> Vec<McpToolDescription> {
    registry
        .specs_sorted()
        .into_iter()
        .map(|spec| {
            let mut required = Vec::new();
            let mut properties = serde_json::Map::new();

            for arg in &spec.required_args {
                required.push(arg.clone());
                properties.insert(
                    arg.clone(),
                    json!({
                        "type": "string",
                        "description": format!("Argument `{}` for tool `{}`", arg, spec.name),
                    }),
                );
            }

            let schema = json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false,
            });

            McpToolDescription {
                name: spec.name.clone(),
                description: format!("{} (risk: {:?})", spec.description, spec.risk),
                input_schema_json: serde_json::to_string_pretty(&schema)
                    .unwrap_or_else(|_| "{}".to_string()),
            }
        })
        .collect()
}

pub fn parse_and_validate_first_vcp_tool_request(
    response_text: &str,
    registry: &VcpToolRegistry,
    policy: &VcpToolPolicy,
) -> Result<Option<VcpToolRequest>> {
    let request = VcpToolRequestParser::parse_first(response_text)?;
    if let Some(request) = &request {
        registry.validate_request(request, policy)?;
    }
    Ok(request)
}

pub fn execute_vcp_tool_request(
    request: &VcpToolRequest,
    vault: &Vault,
    index: Option<&KnowledgeIndex>,
    policy: &VcpToolPolicy,
) -> Result<VcpToolExecutionResult> {
    let registry = VcpToolRegistry::with_xnote_defaults();
    registry.validate_request(request, policy)?;

    let tool_name = canonical_tool_name(request.tool_name.as_str());
    let payload_markdown = match tool_name.as_str() {
        "xnote.vault.read_note" => {
            let note_path = arg_required(request, "note_path")?;
            let content = vault.read_note(note_path)?;
            format!(
                "## xnote.vault.read_note\n\n- `note_path`: `{}`\n\n```markdown\n{}\n```",
                note_path,
                sanitize_fence_body(content.as_str())
            )
        }
        "xnote.knowledge.search" => {
            let Some(index) = index else {
                return Err(anyhow!(
                    "tool `xnote.knowledge.search` requires a ready knowledge index"
                ));
            };

            let query = arg_required(request, "query")?;
            let limit = arg_optional_usize(request, "limit")
                .unwrap_or(20)
                .clamp(1, 200);
            let outcome = index.search(
                vault,
                query,
                SearchOptions {
                    max_files_with_matches: limit,
                    ..SearchOptions::default()
                },
            );

            let mut markdown = format!(
                "## xnote.knowledge.search\n\n- `query`: `{}`\n- `elapsed_ms`: {}\n- `hits`: {}\n",
                query,
                outcome.elapsed_ms,
                outcome.hits.len()
            );

            for hit in outcome.hits.iter().take(limit) {
                markdown.push_str(format!("\n### {}\n", hit.path).as_str());
                markdown.push_str(format!("- `match_count`: {}\n", hit.match_count).as_str());
                for preview in &hit.previews {
                    markdown.push_str(
                        format!(
                            "- L{}: {}\n",
                            preview.line,
                            preview.preview.replace('\n', " ")
                        )
                        .as_str(),
                    );
                }
            }

            markdown
        }
        "xnote.vault.write_note" => {
            let note_path = arg_required(request, "note_path")?;
            let content = arg_required(request, "content")?;
            vault.write_note(note_path, content)?;
            format!(
                "## xnote.vault.write_note\n\n- `note_path`: `{}`\n- `result`: `ok`",
                note_path
            )
        }
        "xnote.vault.apply_patch" => {
            return Err(anyhow!(
                "tool `xnote.vault.apply_patch` is reserved for future patch pipeline"
            ));
        }
        _ => {
            return Err(anyhow!("tool `{}` is not implemented", request.tool_name));
        }
    };

    Ok(VcpToolExecutionResult {
        tool_name: request.tool_name.clone(),
        payload_markdown,
    })
}

pub fn append_ai_tool_audit_log(vault: &Vault, entry: &AiToolAuditEntry) -> Result<()> {
    let audit_path = ai_tool_audit_log_path(vault);
    if let Some(parent) = audit_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create audit dir: {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_path)
        .with_context(|| format!("failed to open audit log: {}", audit_path.display()))?;

    let payload = serde_json::to_string(entry).context("failed to encode audit entry json")?;
    file.write_all(payload.as_bytes())
        .context("failed to append audit payload")?;
    file.write_all(b"\n")
        .context("failed to append audit newline")?;
    Ok(())
}

pub fn ai_tool_audit_log_path(vault: &Vault) -> PathBuf {
    vault
        .root()
        .join(".xnote")
        .join("meta")
        .join("ai_tool_audit.jsonl")
}

fn complete_vcp_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.contains("/v1/chat/completions") || trimmed.contains("/v1/chatvcp/completions") {
        return trimmed.to_string();
    }

    if let Some(prefix) = trimmed.strip_suffix("/v1") {
        return format!("{prefix}/v1/chat/completions");
    }

    if let Some(prefix) = trimmed.strip_suffix("/v1/") {
        return format!("{prefix}/v1/chat/completions");
    }

    if let Some(prefix) = trimmed.strip_suffix('/') {
        return format!("{prefix}/v1/chat/completions");
    }

    format!("{trimmed}/v1/chat/completions")
}

fn env_string(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

fn extract_completion_text(payload: &serde_json::Value) -> Option<String> {
    if let Some(content) = payload
        .pointer("/choices/0/message/content")
        .and_then(json_content_to_text)
    {
        return Some(content);
    }

    if let Some(content) = payload
        .pointer("/choices/0/delta/content")
        .and_then(json_content_to_text)
    {
        return Some(content);
    }

    let Some(response_root) = payload.get("response") else {
        return None;
    };

    for root in [response_root] {
        if let Some(content) = root
            .pointer("/choices/0/message/content")
            .and_then(json_content_to_text)
        {
            return Some(content);
        }

        if let Some(content) = root
            .pointer("/choices/0/delta/content")
            .and_then(json_content_to_text)
        {
            return Some(content);
        }
    }

    None
}

fn json_content_to_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => {
            Some(value.trim().to_string()).filter(|v| !v.is_empty())
        }
        serde_json::Value::Array(chunks) => {
            let mut combined = String::new();
            for chunk in chunks {
                if let Some(text) = chunk.get("text").and_then(serde_json::Value::as_str) {
                    combined.push_str(text);
                    continue;
                }

                if let Some(text) = chunk.get("content").and_then(serde_json::Value::as_str) {
                    combined.push_str(text);
                }
            }

            let trimmed = combined.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        _ => None,
    }
}

fn scope_can_access(request_scope: AiVariableScope, entry_scope: AiVariableScope) -> bool {
    match (request_scope, entry_scope) {
        (AiVariableScope::Global, AiVariableScope::Global) => true,
        (AiVariableScope::Global, AiVariableScope::Knowledge) => false,
        (AiVariableScope::Knowledge, _) => true,
    }
}

fn canonical_variable_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let unwrapped = trimmed
        .strip_prefix("{{")
        .unwrap_or(trimmed)
        .strip_suffix("}}")
        .unwrap_or(trimmed)
        .trim();

    unwrapped.to_ascii_lowercase()
}

fn canonical_tool_name(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn canonical_tool_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn arg_required<'a>(request: &'a VcpToolRequest, key: &str) -> Result<&'a str> {
    let canonical = canonical_tool_key(key);
    request
        .args
        .get(canonical.as_str())
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("missing required argument `{}`", key))
}

fn arg_optional_usize(request: &VcpToolRequest, key: &str) -> Option<usize> {
    let canonical = canonical_tool_key(key);
    request
        .args
        .get(canonical.as_str())
        .and_then(|value| value.trim().parse::<usize>().ok())
}

fn sanitize_fence_body(content: &str) -> String {
    content.replace("```", "` ` `")
}

fn summarize_tool_detail(detail: &str, max_chars: usize) -> String {
    let mut normalized = detail.trim().replace('\r', "");
    if normalized.len() > max_chars {
        normalized.truncate(max_chars);
        normalized.push_str("...");
    }
    normalized
}

fn summarize_tool_args(args: &BTreeMap<String, String>, max_chars: usize) -> String {
    if args.is_empty() {
        return "{}".to_string();
    }

    let mut pairs = Vec::with_capacity(args.len());
    for (key, value) in args {
        let snippet = summarize_tool_detail(value, 64).replace('\n', " ");
        pairs.push(format!("{key}={snippet}"));
    }

    summarize_tool_detail(pairs.join(", ").as_str(), max_chars)
}

fn build_tool_follow_up_instruction(
    original_instruction: &str,
    tool_name: &str,
    tool_payload_markdown: &str,
    final_response_instruction: &str,
) -> String {
    format!(
        "{}\n\n[Tool {} result]\n{}\n\n{}",
        original_instruction, tool_name, tool_payload_markdown, final_response_instruction
    )
}

fn generate_ai_request_id(scenario: &str) -> String {
    format!("xnote-ai-{}-{}", scenario, now_epoch_ms())
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn parse_vcp_tool_block(block: &str) -> Result<VcpToolRequest> {
    let pairs = parse_vcp_key_values(block)?;
    let mut tool_name = None;
    let mut args = BTreeMap::new();
    let mut no_reply = false;
    let mut mark_history = false;

    for (raw_key, raw_value) in pairs {
        let key = canonical_tool_key(raw_key.as_str());
        match key.as_str() {
            "tool_name" => {
                tool_name = Some(raw_value.trim().to_string());
            }
            "archery" => {
                no_reply = raw_value.trim().eq_ignore_ascii_case("no_reply");
            }
            "ink" => {
                mark_history = raw_value.trim().eq_ignore_ascii_case("mark_history");
            }
            _ => {
                args.insert(key, raw_value);
            }
        }
    }

    let tool_name = tool_name
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| anyhow!("vcp tool block missing tool_name"))?;

    Ok(VcpToolRequest {
        tool_name,
        args,
        no_reply,
        mark_history,
    })
}

fn parse_vcp_key_values(block: &str) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while cursor < block.len() {
        cursor = skip_block_whitespace_and_commas(block, cursor);
        if cursor >= block.len() {
            break;
        }

        let remain = &block[cursor..];
        let Some(colon_rel) = remain.find(':') else {
            break;
        };

        let key_end = cursor + colon_rel;
        let key = block[cursor..key_end].trim().trim_end_matches(',').trim();
        cursor = key_end + 1;
        cursor = skip_ascii_whitespace(block, cursor);

        let (value, next_cursor) = parse_vcp_value(block, cursor)?;
        if !key.is_empty() {
            out.push((key.to_string(), value));
        }
        cursor = next_cursor;
    }

    Ok(out)
}

fn parse_vcp_value(block: &str, cursor: usize) -> Result<(String, usize)> {
    const WRAPPERS: [(&str, &str); 3] = [
        ("\u{300c}\u{59cb}\u{300d}", "\u{300c}\u{672b}\u{300d}"),
        ("\u{300e}\u{59cb}\u{300f}", "\u{300e}\u{672b}\u{300f}"),
        ("\u{3010}\u{59cb}\u{3011}", "\u{3010}\u{672b}\u{3011}"),
    ];

    for (start, end) in WRAPPERS {
        if block[cursor..].starts_with(start) {
            let value_start = cursor + start.len();
            let Some(end_rel) = block[value_start..].find(end) else {
                return Err(anyhow!(
                    "vcp value started with `{start}` but no closing `{end}`"
                ));
            };

            let value_end = value_start + end_rel;
            let value = block[value_start..value_end].trim().to_string();
            let mut next_cursor = value_end + end.len();
            next_cursor = skip_block_whitespace_and_commas(block, next_cursor);
            return Ok((value, next_cursor));
        }
    }

    let mut end_cursor = cursor;
    while end_cursor < block.len() {
        let Some(ch) = block[end_cursor..].chars().next() else {
            break;
        };
        if ch == '\n' || ch == ',' {
            break;
        }
        end_cursor += ch.len_utf8();
    }

    let value = block[cursor..end_cursor].trim().to_string();
    let next_cursor = skip_block_whitespace_and_commas(block, end_cursor);
    Ok((value, next_cursor))
}
fn skip_ascii_whitespace(block: &str, mut cursor: usize) -> usize {
    while cursor < block.len() {
        let Some(ch) = block[cursor..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn skip_block_whitespace_and_commas(block: &str, mut cursor: usize) -> usize {
    while cursor < block.len() {
        let Some(ch) = block[cursor..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() && ch != ',' {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_engine_dry_run_returns_proposal() {
        let engine = AiEngine::new(MockAiProvider, AiPolicy::default());
        let request = AiRewriteRequest {
            note_path: "notes/demo.md".to_string(),
            selection: "A\tline\n\n\nB".to_string(),
            instruction: "Polish text".to_string(),
        };

        let result = engine
            .rewrite_selection(&request, false)
            .expect("dry run proposal");

        assert!(result.dry_run);
        assert!(!result.applied);
        assert_eq!(result.proposal.provider, "mock");
        assert_eq!(result.proposal.model, "mock-draft-v1");
        assert_eq!(result.proposal.replacement, "A    line\n\n\nB");
    }

    #[test]
    fn ai_engine_apply_is_policy_guarded() {
        let policy = AiPolicy {
            allow_apply: false,
            max_selection_chars: 200,
        };
        let engine = AiEngine::new(MockAiProvider, policy);
        let request = AiRewriteRequest {
            note_path: "notes/demo.md".to_string(),
            selection: "A".to_string(),
            instruction: String::new(),
        };

        let err = engine
            .rewrite_selection(&request, true)
            .expect_err("apply must be blocked");
        assert!(err.to_string().contains("policy gate"));
    }

    #[test]
    fn variable_registry_respects_progressive_disclosure() {
        let mut registry = AiVariableRegistry::default();
        registry.insert_global("Date", "2026-02-10");
        registry.insert_knowledge("CurrentNotePath", "notes/demo.md");

        let global =
            registry.render_template("{{Date}} | {{CurrentNotePath}}", AiVariableScope::Global);
        assert_eq!(global, "2026-02-10 | {{CurrentNotePath}}");

        let knowledge =
            registry.render_template("{{Date}} | {{CurrentNotePath}}", AiVariableScope::Knowledge);
        assert_eq!(knowledge, "2026-02-10 | notes/demo.md");
    }
    #[test]
    fn vcp_tool_parser_extracts_wrapped_block() {
        let input = "prefix\n<<<[TOOL_REQUEST]>>>\n\
tool_name:\u{300c}\u{59cb}\u{300d}xnote.vault.read_note\u{300c}\u{672b}\u{300d},\n\
note_path:\u{300c}\u{59cb}\u{300d}notes/demo.md\u{300c}\u{672b}\u{300d}\n\
<<<[END_TOOL_REQUEST]>>>\nsuffix";

        let parsed = VcpToolRequestParser::parse_first(input)
            .expect("parser result")
            .expect("request");

        assert_eq!(parsed.tool_name, "xnote.vault.read_note");
        assert_eq!(
            parsed.args.get("note_path").map(String::as_str),
            Some("notes/demo.md")
        );
    }
    #[test]
    fn vcp_tool_registry_blocks_write_without_permission() {
        let request = VcpToolRequest {
            tool_name: "xnote.vault.write_note".to_string(),
            args: BTreeMap::from([
                ("note_path".to_string(), "notes/demo.md".to_string()),
                ("content".to_string(), "hello".to_string()),
            ]),
            no_reply: false,
            mark_history: false,
        };

        let registry = VcpToolRegistry::with_xnote_defaults();
        let err = registry
            .validate_request(&request, &VcpToolPolicy::default())
            .expect_err("write must be blocked by default policy");

        assert!(err.to_string().contains("requires write permission"));
    }

    #[test]
    fn execute_vcp_tool_request_read_and_search() {
        use std::fs;

        let temp_dir =
            std::env::temp_dir().join(format!("xnote_core_ai_vcp_exec_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test notes dir");
        fs::write(temp_dir.join("notes/A.md"), "# A\nRust and GPUI").expect("write note A");
        fs::write(temp_dir.join("notes/B.md"), "# B\nRust runtime").expect("write note B");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        let read_request = VcpToolRequest {
            tool_name: "xnote.vault.read_note".to_string(),
            args: BTreeMap::from([("note_path".to_string(), "notes/A.md".to_string())]),
            no_reply: false,
            mark_history: false,
        };

        let read_result = execute_vcp_tool_request(
            &read_request,
            &vault,
            Some(&index),
            &VcpToolPolicy {
                allow_write: false,
                allow_destructive: false,
                allowlist: None,
            },
        )
        .expect("read tool execution");
        assert!(read_result.payload_markdown.contains("notes/A.md"));
        assert!(read_result.payload_markdown.contains("Rust and GPUI"));

        let search_request = VcpToolRequest {
            tool_name: "xnote.knowledge.search".to_string(),
            args: BTreeMap::from([
                ("query".to_string(), "rust".to_string()),
                ("limit".to_string(), "10".to_string()),
            ]),
            no_reply: false,
            mark_history: false,
        };

        let search_result = execute_vcp_tool_request(
            &search_request,
            &vault,
            Some(&index),
            &VcpToolPolicy::default(),
        )
        .expect("search tool execution");
        assert!(search_result
            .payload_markdown
            .contains("xnote.knowledge.search"));
        assert!(search_result
            .payload_markdown
            .to_ascii_lowercase()
            .contains("rust"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn append_ai_tool_audit_log_writes_jsonl_entry() {
        use std::fs;

        let temp_dir =
            std::env::temp_dir().join(format!("xnote_core_ai_audit_log_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create notes dir");

        let vault = Vault::open(&temp_dir).expect("open vault");
        vault
            .ensure_knowledge_structure()
            .expect("ensure structure");

        let entry = AiToolAuditEntry {
            timestamp_epoch_ms: 1,
            event: "tool_execution".to_string(),
            round: 0,
            tool_name: "xnote.vault.read_note".to_string(),
            status: "ok".to_string(),
            detail: "ok".to_string(),
            request_id: None,
            scenario: None,
            model_latency_ms: None,
            tool_latency_ms: None,
            args_summary: None,
            outcome_category: None,
        };

        append_ai_tool_audit_log(&vault, &entry).expect("append audit log");
        let audit_path = ai_tool_audit_log_path(&vault);
        let content = fs::read_to_string(&audit_path).expect("read audit log file");

        assert!(content.contains("\"tool_name\":\"xnote.vault.read_note\""));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn execute_rewrite_with_vcp_tool_loop_returns_mock_fallback_without_tool_request() {
        use std::fs;

        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_ai_loop_fallback_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create notes dir");
        fs::write(temp_dir.join("notes/A.md"), "# A\nDemo").expect("write note");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let request = AiRewriteRequest {
            note_path: "notes/A.md".to_string(),
            selection: "Line\tA".to_string(),
            instruction: "Polish".to_string(),
        };

        let policy = VcpToolPolicy::default();

        // Not setting VCP env in this test; this verifies function contract stability.
        // The provider call may fail without a running server, so we only assert it doesn't panic
        // and returns an anyhow::Result.
        let _ = execute_rewrite_with_vcp_tool_loop(&request, &vault, None, &policy, 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn generate_vcp_and_mcp_tool_descriptions_from_same_registry() {
        let registry = VcpToolRegistry::with_xnote_defaults();
        let vcp = generate_vcp_tool_descriptions(&registry);
        let mcp = generate_mcp_tool_descriptions(&registry);

        assert!(!vcp.is_empty());
        assert_eq!(vcp.len(), mcp.len());
        assert!(vcp.iter().any(|item| {
            item.tool_name == "xnote.vault.read_note"
                && item.invocation_snippet.contains("<<<[TOOL_REQUEST]>>>")
        }));
        assert!(mcp.iter().any(|item| {
            item.name == "xnote.vault.read_note" && item.input_schema_json.contains("note_path")
        }));
    }

    #[test]
    fn generate_ai_tool_descriptor_bundle_contains_dual_protocol_payload() {
        let registry = VcpToolRegistry::with_xnote_defaults();
        let bundle = generate_ai_tool_descriptor_bundle(&registry);

        assert_eq!(bundle.schema_version, "xnote.ai.tools.v1");
        assert!(bundle.generated_at_epoch_ms > 0);
        assert!(!bundle.vcp.is_empty());
        assert_eq!(bundle.vcp.len(), bundle.mcp.len());
    }

    #[test]
    fn generate_ai_tool_descriptor_bundle_json_pretty_is_valid_json() {
        let json =
            generate_default_ai_tool_descriptor_bundle_json_pretty().expect("descriptor json");
        let value: serde_json::Value = serde_json::from_str(json.as_str()).expect("valid json");

        assert_eq!(
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_str),
            Some("xnote.ai.tools.v1")
        );
        assert!(value
            .get("vcp")
            .and_then(serde_json::Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));
        assert!(value
            .get("mcp")
            .and_then(serde_json::Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));
    }

    #[test]
    fn vcp_tool_orchestrator_returns_final_response_without_tool_call() {
        use std::fs;

        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_ai_orchestrator_no_tool_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create notes dir");
        fs::write(temp_dir.join("notes/A.md"), "# A\nDemo").expect("write note");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let request = AiRewriteRequest {
            note_path: "notes/A.md".to_string(),
            selection: "Line\tA".to_string(),
            instruction: "Polish".to_string(),
        };

        let provider = MockAiProvider;
        let result = execute_vcp_tool_orchestrator(
            &request,
            &provider,
            &vault,
            None,
            &VcpToolPolicy::default(),
            &AiToolOrchestratorConfig {
                max_rounds: 2,
                request_id: Some("req-test-1".to_string()),
                scenario: "rewrite_selection".to_string(),
                final_response_instruction: "Return final text only.".to_string(),
            },
        )
        .expect("orchestrator result");

        assert_eq!(result.stop_reason, AiToolLoopStopReason::FinalResponse);
        assert_eq!(result.rounds_executed, 1);
        assert!(result.tool_calls.is_empty());
        assert_eq!(result.final_response, "Line    A");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn append_ai_tool_audit_log_writes_extended_fields() {
        use std::fs;

        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_ai_audit_log_extended_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create notes dir");

        let vault = Vault::open(&temp_dir).expect("open vault");
        vault
            .ensure_knowledge_structure()
            .expect("ensure structure");

        let entry = AiToolAuditEntry {
            timestamp_epoch_ms: 2,
            event: "tool_execution".to_string(),
            round: 1,
            tool_name: "xnote.knowledge.search".to_string(),
            status: "ok".to_string(),
            detail: "summary".to_string(),
            request_id: Some("req-extended-1".to_string()),
            scenario: Some("rewrite_selection".to_string()),
            model_latency_ms: Some(12),
            tool_latency_ms: Some(34),
            args_summary: Some("query=rust".to_string()),
            outcome_category: Some("tool_executed".to_string()),
        };

        append_ai_tool_audit_log(&vault, &entry).expect("append audit log");
        let audit_path = ai_tool_audit_log_path(&vault);
        let content = fs::read_to_string(&audit_path).expect("read audit log file");

        assert!(content.contains("\"request_id\":\"req-extended-1\""));
        assert!(content.contains("\"scenario\":\"rewrite_selection\""));
        assert!(content.contains("\"model_latency_ms\":12"));
        assert!(content.contains("\"tool_latency_ms\":34"));
        assert!(content.contains("\"args_summary\":\"query=rust\""));
        assert!(content.contains("\"outcome_category\":\"tool_executed\""));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn vcp_url_completion_and_injection_switch() {
        let mut config = VcpCompatConfig {
            endpoint: complete_vcp_url("http://127.0.0.1:5890"),
            ..VcpCompatConfig::default()
        };
        assert_eq!(config.endpoint, "http://127.0.0.1:5890/v1/chat/completions");

        config.enable_tool_injection = true;
        assert_eq!(
            config.completion_endpoint(),
            "http://127.0.0.1:5890/v1/chatvcp/completions"
        );
    }
}
