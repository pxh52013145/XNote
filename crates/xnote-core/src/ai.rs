use anyhow::{anyhow, Result};

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
    fn provider_name(&self) -> &'static str;
    fn model_name(&self) -> &'static str;
    fn rewrite_selection(&self, request: &AiRewriteRequest) -> Result<String>;
}

#[derive(Clone, Debug, Default)]
pub struct MockAiProvider;

impl AiProvider for MockAiProvider {
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

#[derive(Clone, Debug)]
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
}
